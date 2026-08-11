#include "awen/Dialect/Tensor/AwenTensorDialect.h"
#include "awen/Support/VersionedDialect.h"

#include "mlir/IR/Builders.h"
#include "mlir/IR/DialectImplementation.h"
#include "llvm/ADT/TypeSwitch.h"

using namespace mlir;
using namespace mlir::awen::tensor;

#include "awen/Dialect/Tensor/AwenTensorOpsDialect.cpp.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/Tensor/AwenTensorOpsTypes.cpp.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/Tensor/AwenTensorOps.cpp.inc"

void AwenTensorDialect::initialize() {
  addInterfaces<awen::support::AwenBytecodeDialectInterface>();
  addTypes<
#define GET_TYPEDEF_LIST
#include "awen/Dialect/Tensor/AwenTensorOpsTypes.cpp.inc"
      >();
  addOperations<
#define GET_OP_LIST
#include "awen/Dialect/Tensor/AwenTensorOps.cpp.inc"
      >();
}

LogicalResult AwenTensorGemmOp::verify() {
  auto lhsType = dyn_cast<RankedTensorType>(getLhs().getType());
  auto rhsType = dyn_cast<RankedTensorType>(getRhs().getType());
  auto resultType = dyn_cast<RankedTensorType>(getResult().getType());
  if (!lhsType || !rhsType || !resultType || lhsType.getRank() != 2 ||
      rhsType.getRank() != 2 || resultType.getRank() != 2)
    return emitOpError("requires rank-two lhs, rhs, and result tensors");
  if (lhsType.getElementType() != rhsType.getElementType() ||
      lhsType.getElementType() != resultType.getElementType())
    return emitOpError("requires identical lhs, rhs, and result element types");
  if (getMinimumEffectiveBits() <= 0)
    return emitOpError("minimum_effective_bits must be positive");
  if (getLayout() != "row_major" && getLayout() != "column_major")
    return emitOpError("layout must be 'row_major' or 'column_major'");
  return success();
}
