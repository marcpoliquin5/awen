#ifndef AWEN_QPHOTONIC_OPS_TD
#define AWEN_QPHOTONIC_OPS_TD

include "mlir/IR/AttrTypeBase.td"
include "mlir/IR/OpBase.td"

def AwenQPhotonicDialect : Dialect {
  let name = "awen_qphotonic";
  let cppNamespace = "::mlir::awen::qphotonic";
  let summary = "Quantum-photonic state, circuit, and measurement operations";
  let useDefaultTypePrinterParser = 1;
}

class AwenQPhotonicType<string name, string typeMnemonic>
    : TypeDef<AwenQPhotonicDialect, name> {
  let mnemonic = typeMnemonic;
}

def AwenQuantumStateType : AwenQPhotonicType<"QuantumState", "state"> {
  let summary = "Opaque quantum-photonic state handle";
}

class AwenQPhotonicOp<string mnemonic, list<Trait> traits = []>
    : Op<AwenQPhotonicDialect, mnemonic, traits>;

def AwenQPhotonicCircuitOp : AwenQPhotonicOp<"circuit"> {
  let summary = "Target-independent quantum-photonic circuit boundary";
  let arguments = (ins Variadic<AnyType>:$inputs, StrAttr:$target);
  let results = (outs Variadic<AnyType>:$outputs);
}

#endif
