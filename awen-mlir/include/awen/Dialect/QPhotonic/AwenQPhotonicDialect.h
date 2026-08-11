#ifndef AWEN_DIALECT_QPHOTONIC_AWENQPHOTONICDIALECT_H
#define AWEN_DIALECT_QPHOTONIC_AWENQPHOTONICDIALECT_H

#include "mlir/Bytecode/BytecodeImplementation.h"
#include "mlir/Bytecode/BytecodeOpInterface.h"
#include "mlir/IR/Dialect.h"
#include "mlir/IR/OpDefinition.h"
#include "mlir/Interfaces/SideEffectInterfaces.h"

#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsDialect.h.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/QPhotonic/AwenQPhotonicOpsTypes.h.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/QPhotonic/AwenQPhotonicOps.h.inc"

#endif
