#include "awen/Dialect/Device/AwenDeviceDialect.h"
#include "awen/Support/VersionedDialect.h"

#include "mlir/IR/Builders.h"
#include "mlir/IR/DialectImplementation.h"
#include "llvm/ADT/TypeSwitch.h"

using namespace mlir;
using namespace mlir::awen::device;

#include "awen/Dialect/Device/AwenDeviceOpsDialect.cpp.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/Device/AwenDeviceOpsTypes.cpp.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/Device/AwenDeviceOps.cpp.inc"

void AwenDeviceDialect::initialize() {
  addInterfaces<awen::support::AwenBytecodeDialectInterface>();
  addTypes<
#define GET_TYPEDEF_LIST
#include "awen/Dialect/Device/AwenDeviceOpsTypes.cpp.inc"
      >();
  addOperations<
#define GET_OP_LIST
#include "awen/Dialect/Device/AwenDeviceOps.cpp.inc"
      >();
}
