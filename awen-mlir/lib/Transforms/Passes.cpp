#include "awen/Transforms/Passes.h"

#include "awen/Dialect/Device/AwenDeviceDialect.h"
#include "awen/Dialect/Photonic/AwenPhotonicDialect.h"
#include "awen/Dialect/Tensor/AwenTensorDialect.h"

#include "mlir/IR/Builders.h"
#include "mlir/IR/BuiltinAttributes.h"
#include "mlir/IR/BuiltinOps.h"
#include "mlir/IR/BuiltinTypes.h"
#include "mlir/Pass/Pass.h"
#include "mlir/Pass/PassManager.h"
#include "mlir/Pass/PassRegistry.h"
#include "mlir/Transforms/Passes.h"

#include <memory>

namespace mlir::awen {
namespace {

constexpr int64_t kDialectMajor = 1;
constexpr int64_t kDefaultEffectiveBits = 8;
constexpr int64_t kDefaultTile = 128;

bool isEmpty(DenseI64ArrayAttr attr) { return attr && attr.empty(); }

bool equals(DenseI64ArrayAttr attr, ArrayRef<int64_t> expected) {
  return attr && attr.asArrayRef() == expected;
}

bool staticDimensionsMatch(RankedTensorType lhs, int64_t lhsDimension,
                           RankedTensorType rhs, int64_t rhsDimension) {
  return lhs.isDynamicDim(lhsDimension) || rhs.isDynamicDim(rhsDimension) ||
         lhs.getDimSize(lhsDimension) == rhs.getDimSize(rhsDimension);
}

bool isSupportedElementType(Type type) {
  return type.isF32() || type.isF16() || type.isBF16() ||
         isa<ComplexType>(type);
}

void stampVersion(ModuleOp module, StringRef family) {
  OpBuilder builder(module.getContext());
  module->setAttr(("awen." + family + ".version").str(),
                  builder.getI64IntegerAttr(kDialectMajor));
}

Operation *createOperation(OpBuilder &builder, Operation *source,
                           StringRef operationName,
                           ArrayRef<NamedAttribute> attributes) {
  OperationState state(source->getLoc(), operationName);
  state.addOperands(source->getOperands());
  state.addTypes(source->getResultTypes());
  state.addAttributes(attributes);
  return builder.create(state);
}

class ImportStablehloPass final
    : public PassWrapper<ImportStablehloPass, OperationPass<ModuleOp>> {
public:
  MLIR_DEFINE_EXPLICIT_INTERNAL_INLINE_TYPE_ID(ImportStablehloPass)

  StringRef getArgument() const final { return "awen-import-stablehlo"; }
  StringRef getDescription() const final {
    return "Import the supported StableHLO dot_general GEMM subset";
  }

  void getDependentDialects(DialectRegistry &registry) const override {
    registry.insert<tensor::AwenTensorDialect>();
  }

  void runOnOperation() override {
    ModuleOp module = getOperation();
    SmallVector<Operation *> stablehloOps;
    module.walk([&](Operation *op) {
      if (op->getName().getDialectNamespace() == "stablehlo")
        stablehloOps.push_back(op);
    });

    for (Operation *op : stablehloOps) {
      if (op->getName().getStringRef() != "stablehlo.dot_general") {
        op->emitError("unsupported StableHLO operation; AWEN v1 imports only "
                      "rank-two or equal-batch rank-three dot_general GEMM");
        signalPassFailure();
        return;
      }
      if (failed(importDotGeneral(op))) {
        signalPassFailure();
        return;
      }
    }
    stampVersion(module, "tensor");
  }

private:
  LogicalResult importDotGeneral(Operation *op) {
    if (op->getNumOperands() != 2 || op->getNumResults() != 1)
      return op->emitError("dot_general must have two operands and one result");

    auto lhs = dyn_cast<RankedTensorType>(op->getOperand(0).getType());
    auto rhs = dyn_cast<RankedTensorType>(op->getOperand(1).getType());
    auto result = dyn_cast<RankedTensorType>(op->getResult(0).getType());
    if (!lhs || !rhs || !result || (lhs.getRank() != 2 && lhs.getRank() != 3) ||
        rhs.getRank() != lhs.getRank() || result.getRank() != lhs.getRank())
      return op->emitError(
          "AWEN dot_general import requires matching rank-two or rank-three "
          "lhs, rhs, and result tensors");
    const bool batched = lhs.getRank() == 3;

    if (lhs.getElementType() != rhs.getElementType() ||
        lhs.getElementType() != result.getElementType())
      return op->emitError(
          "dot_general lhs, rhs, and result element types must match");
    if (!isSupportedElementType(lhs.getElementType()))
      return op->emitError("unsupported dot_general element type; expected "
                           "f16, bf16, f32, or complex floating point");

    auto lhsBatch =
        op->getAttrOfType<DenseI64ArrayAttr>("lhs_batching_dimensions");
    auto rhsBatch =
        op->getAttrOfType<DenseI64ArrayAttr>("rhs_batching_dimensions");
    auto lhsContract =
        op->getAttrOfType<DenseI64ArrayAttr>("lhs_contracting_dimensions");
    auto rhsContract =
        op->getAttrOfType<DenseI64ArrayAttr>("rhs_contracting_dimensions");
    if (!lhsBatch || !rhsBatch || !lhsContract || !rhsContract)
      return op->emitError(
          "normalized dot_general requires lhs/rhs batching and contracting "
          "dimension attributes as dense i64 arrays");
    if (!batched && (!isEmpty(lhsBatch) || !isEmpty(rhsBatch)))
      return op->emitError(
          "rank-two dot_general requires empty batching dimensions");
    if (batched && (!equals(lhsBatch, {0}) || !equals(rhsBatch, {0})))
      return op->emitError(
          "rank-three dot_general requires lhs and rhs batching dimension [0]");

    const int64_t lhsM = batched ? 1 : 0;
    const int64_t lhsK = batched ? 2 : 1;
    const int64_t rhsK = batched ? 1 : 0;
    const int64_t rhsN = batched ? 2 : 1;
    const int64_t resultM = batched ? 1 : 0;
    const int64_t resultN = batched ? 2 : 1;
    if (!equals(lhsContract, {lhsK}) || !equals(rhsContract, {rhsK}))
      return op->emitError(
          batched
              ? "rank-three dot_general requires lhs contracting dimension "
                "[2] and rhs [1]"
              : "rank-two dot_general requires lhs contracting dimension [1] "
                "and rhs [0]");

    if (batched && (!staticDimensionsMatch(lhs, 0, rhs, 0) ||
                    !staticDimensionsMatch(lhs, 0, result, 0)))
      return op->emitError("static batch dimensions do not match");
    if (!staticDimensionsMatch(lhs, lhsK, rhs, rhsK))
      return op->emitError("static contracting dimensions do not match");
    if (!staticDimensionsMatch(lhs, lhsM, result, resultM))
      return op->emitError("result M dimension does not match lhs");
    if (!staticDimensionsMatch(rhs, rhsN, result, resultN))
      return op->emitError("result N dimension does not match rhs");

    OpBuilder builder(op);
    int64_t effectiveBits = kDefaultEffectiveBits;
    if (auto attr =
            op->getAttrOfType<IntegerAttr>("awen.minimum_effective_bits"))
      effectiveBits = attr.getInt();
    StringRef layout = "row_major";
    if (auto attr = op->getAttrOfType<StringAttr>("awen.layout"))
      layout = attr.getValue();

    SmallVector<NamedAttribute> attrs{
        builder.getNamedAttr("transpose_lhs", builder.getBoolAttr(false)),
        builder.getNamedAttr("transpose_rhs", builder.getBoolAttr(false)),
        builder.getNamedAttr("minimum_effective_bits",
                             builder.getI64IntegerAttr(effectiveBits)),
        builder.getNamedAttr("layout", builder.getStringAttr(layout))};
    Operation *replacement = createOperation(
        builder, op, tensor::AwenTensorGemmOp::getOperationName(), attrs);
    op->replaceAllUsesWith(replacement);
    op->erase();
    return success();
  }
};

class LowerTensorToPhotonicPass final
    : public PassWrapper<LowerTensorToPhotonicPass, OperationPass<ModuleOp>> {
public:
  MLIR_DEFINE_EXPLICIT_INTERNAL_INLINE_TYPE_ID(LowerTensorToPhotonicPass)

  StringRef getArgument() const final {
    return "awen-lower-tensor-to-photonic";
  }
  StringRef getDescription() const final {
    return "Lower typed AWEN tensor GEMM to classical photonic tiles";
  }

  void getDependentDialects(DialectRegistry &registry) const override {
    registry.insert<photonic::AwenPhotonicDialect>();
  }

  void runOnOperation() override {
    ModuleOp module = getOperation();
    SmallVector<tensor::AwenTensorGemmOp> worklist;
    module.walk([&](tensor::AwenTensorGemmOp op) { worklist.push_back(op); });
    for (tensor::AwenTensorGemmOp op : worklist) {
      OpBuilder builder(op);
      SmallVector<NamedAttribute> attrs{
          builder.getNamedAttr("tile_m",
                               builder.getI64IntegerAttr(kDefaultTile)),
          builder.getNamedAttr("tile_n",
                               builder.getI64IntegerAttr(kDefaultTile)),
          builder.getNamedAttr("tile_k",
                               builder.getI64IntegerAttr(kDefaultTile)),
          builder.getNamedAttr("minimum_effective_bits",
                               op.getMinimumEffectiveBitsAttr()),
          builder.getNamedAttr("calibration",
                               builder.getStringAttr("required")),
          builder.getNamedAttr("layout", op.getLayoutAttr())};
      Operation *replacement = createOperation(
          builder, op, photonic::AwenPhotonicGemmTileOp::getOperationName(),
          attrs);
      op->replaceAllUsesWith(replacement);
      op->erase();
    }
    stampVersion(module, "photonic");
  }
};

class LowerPhotonicToDevicePass final
    : public PassWrapper<LowerPhotonicToDevicePass, OperationPass<ModuleOp>> {
public:
  MLIR_DEFINE_EXPLICIT_INTERNAL_INLINE_TYPE_ID(LowerPhotonicToDevicePass)

  StringRef getArgument() const final {
    return "awen-lower-photonic-to-device";
  }
  StringRef getDescription() const final {
    return "Lower classical photonic tiles to executable device dispatches";
  }

  void getDependentDialects(DialectRegistry &registry) const override {
    registry.insert<device::AwenDeviceDialect>();
  }

  void runOnOperation() override {
    ModuleOp module = getOperation();
    SmallVector<photonic::AwenPhotonicGemmTileOp> worklist;
    module.walk(
        [&](photonic::AwenPhotonicGemmTileOp op) { worklist.push_back(op); });
    for (photonic::AwenPhotonicGemmTileOp op : worklist) {
      OpBuilder builder(op);
      SmallVector<NamedAttribute> attrs{
          builder.getNamedAttr("backend",
                               builder.getStringAttr("awen.reference.v1")),
          builder.getNamedAttr("tile_m", op.getTileMAttr()),
          builder.getNamedAttr("tile_n", op.getTileNAttr()),
          builder.getNamedAttr("tile_k", op.getTileKAttr()),
          builder.getNamedAttr("minimum_effective_bits",
                               op.getMinimumEffectiveBitsAttr()),
          builder.getNamedAttr("calibration", op.getCalibrationAttr()),
          builder.getNamedAttr("layout", op.getLayoutAttr())};
      Operation *replacement = createOperation(
          builder, op, device::AwenDeviceExecuteGemmOp::getOperationName(),
          attrs);
      op->replaceAllUsesWith(replacement);
      op->erase();
    }
    stampVersion(module, "device");
    OpBuilder builder(module.getContext());
    module->setAttr("awen.executable.abi_major", builder.getI64IntegerAttr(1));
    module->setAttr("awen.executable.abi_minor", builder.getI64IntegerAttr(0));
  }
};

} // namespace

void registerAwenPasses() {
  PassRegistration<ImportStablehloPass>();
  PassRegistration<LowerTensorToPhotonicPass>();
  PassRegistration<LowerPhotonicToDevicePass>();
  PassPipelineRegistration<>(
      "awen-lower-stablehlo-to-device",
      "Import StableHLO GEMM and lower it through AWEN Device IR",
      [](OpPassManager &manager) { buildAwenLoweringPipeline(manager); });
}

void buildAwenLoweringPipeline(OpPassManager &manager) {
  manager.addPass(std::make_unique<ImportStablehloPass>());
  manager.addPass(createCanonicalizerPass());
  manager.addPass(std::make_unique<LowerTensorToPhotonicPass>());
  manager.addPass(createCanonicalizerPass());
  manager.addPass(std::make_unique<LowerPhotonicToDevicePass>());
  manager.addPass(createCanonicalizerPass());
}

} // namespace mlir::awen
