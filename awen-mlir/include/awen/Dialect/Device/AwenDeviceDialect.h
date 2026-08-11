#ifndef AWEN_DIALECT_DEVICE_AWENDEVICEDIALECT_H
#define AWEN_DIALECT_DEVICE_AWENDEVICEDIALECT_H

#include "mlir/Bytecode/BytecodeImplementation.h"
#include "mlir/Bytecode/BytecodeOpInterface.h"
#include "mlir/IR/BuiltinTypes.h"
#include "mlir/IR/Dialect.h"
#include "mlir/IR/OpDefinition.h"
#include "mlir/Interfaces/SideEffectInterfaces.h"

#include "awen/Dialect/Device/AwenDeviceOpsDialect.h.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/Device/AwenDeviceOpsTypes.h.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/Device/AwenDeviceOps.h.inc"

#endif
