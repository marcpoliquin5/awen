#include "awen/Dialect/Device/AwenDeviceDialect.h"
#include "awen/Dialect/Photonic/AwenPhotonicDialect.h"
#include "awen/Dialect/QPhotonic/AwenQPhotonicDialect.h"
#include "awen/Dialect/Tensor/AwenTensorDialect.h"
#include "awen/Transforms/Passes.h"

#include "mlir/Dialect/Func/IR/FuncOps.h"
#include "mlir/IR/Dialect.h"
#include "mlir/IR/DialectRegistry.h"
#include "mlir/Tools/mlir-opt/MlirOptMain.h"

namespace {
class StablehloImportDialect final : public mlir::Dialect {
public:
  static llvm::StringRef getDialectNamespace() { return "stablehlo"; }

  explicit StablehloImportDialect(mlir::MLIRContext *context)
      : Dialect(getDialectNamespace(), context,
                mlir::TypeID::get<StablehloImportDialect>()) {
    allowUnknownOperations();
  }
};
} // namespace

int main(int argc, char **argv) {
  mlir::awen::registerAwenPasses();
  mlir::DialectRegistry registry;
  registry.insert<mlir::func::FuncDialect, StablehloImportDialect,
                  mlir::awen::tensor::AwenTensorDialect,
                  mlir::awen::photonic::AwenPhotonicDialect,
                  mlir::awen::qphotonic::AwenQPhotonicDialect,
                  mlir::awen::device::AwenDeviceDialect>();

  return mlir::failed(
      mlir::MlirOptMain(argc, argv, "AWEN MLIR optimizer\n", registry));
}
