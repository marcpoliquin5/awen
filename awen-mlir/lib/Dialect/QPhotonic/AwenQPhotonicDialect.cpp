#include "awen/Dialect/QPhotonic/AwenQPhotonicDialect.h"
#include "awen/Support/VersionedDialect.h"

#include "mlir/IR/Builders.h"
#include "mlir/IR/DialectImplementation.h"
#include "llvm/ADT/TypeSwitch.h"

using namespace mlir;
using namespace mlir::awen::qphotonic;

#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsDialect.cpp.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsTypes.cpp.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/QPhotonic/AwenQPhotonicOps.cpp.inc"

void AwenQPhotonicDialect::initialize() {
  addInterfaces<awen::support::AwenBytecodeDialectInterface>();
  addTypes<
#define GET_TYPEDEF_LIST
#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsTypes.cpp.inc"
      >();
  addOperations<
#define GET_OP_LIST
#include "awen/Dialect/QPhotonic/AwenQPhotonicOps.cpp.inc"
      >();
}
