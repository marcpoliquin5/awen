#include "awen/Dialect/Photonic/AwenPhotonicDialect.h"
#include "awen/Support/VersionedDialect.h"

#include "mlir/IR/Builders.h"
#include "mlir/IR/DialectImplementation.h"
#include "llvm/ADT/TypeSwitch.h"

using namespace mlir;
using namespace mlir::awen::photonic;

#include "awen/Dialect/Photonic/AwenPhotonicOpsDialect.cpp.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/Photonic/AwenPhotonicOpsTypes.cpp.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/Photonic/AwenPhotonicOps.cpp.inc"

void AwenPhotonicDialect::initialize() {
  addInterfaces<awen::support::AwenBytecodeDialectInterface>();
  addTypes<
#define GET_TYPEDEF_LIST
#include "awen/Dialect/Photonic/AwenPhotonicOpsTypes.cpp.inc"
      >();
  addOperations<
#define GET_OP_LIST
#include "awen/Dialect/Photonic/AwenPhotonicOps.cpp.inc"
      >();
}
