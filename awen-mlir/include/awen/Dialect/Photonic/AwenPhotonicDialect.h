#ifndef AWEN_DIALECT_PHOTONIC_AWENPHOTONICDIALECT_H
#define AWEN_DIALECT_PHOTONIC_AWENPHOTONICDIALECT_H

#include "mlir/Bytecode/BytecodeImplementation.h"
#include "mlir/Bytecode/BytecodeOpInterface.h"
#include "mlir/IR/BuiltinTypes.h"
#include "mlir/IR/Dialect.h"
#include "mlir/IR/OpDefinition.h"
#include "mlir/Interfaces/SideEffectInterfaces.h"

#include "awen/Dialect/Photonic/AwenPhotonicOpsDialect.h.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/Photonic/AwenPhotonicOpsTypes.h.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/Photonic/AwenPhotonicOps.h.inc"

#endif
