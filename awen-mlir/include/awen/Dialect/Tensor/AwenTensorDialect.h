#ifndef AWEN_DIALECT_TENSOR_AWENTENSORDIALECT_H
#define AWEN_DIALECT_TENSOR_AWENTENSORDIALECT_H

#include "mlir/Bytecode/BytecodeImplementation.h"
#include "mlir/Bytecode/BytecodeOpInterface.h"
#include "mlir/IR/BuiltinTypes.h"
#include "mlir/IR/Dialect.h"
#include "mlir/IR/OpDefinition.h"
#include "mlir/Interfaces/SideEffectInterfaces.h"

#include "awen/Dialect/Tensor/AwenTensorOpsDialect.h.inc"

#define GET_TYPEDEF_CLASSES
#include "awen/Dialect/Tensor/AwenTensorOpsTypes.h.inc"

#define GET_OP_CLASSES
#include "awen/Dialect/Tensor/AwenTensorOps.h.inc"

#endif
