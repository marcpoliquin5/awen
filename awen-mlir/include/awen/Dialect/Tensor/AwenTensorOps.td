#ifndef AWEN_TENSOR_OPS_TD
#define AWEN_TENSOR_OPS_TD

include "mlir/IR/AttrTypeBase.td"
include "mlir/IR/OpBase.td"
include "mlir/Interfaces/SideEffectInterfaces.td"

def AwenTensorDialect : Dialect {
  let name = "awen_tensor";
  let cppNamespace = "::mlir::awen::tensor";
  let summary = "Typed tensor operations accepted by the AWEN compiler";
  let description = [{
    The dialect is the shape, element type, layout, source-location, and
    precision-contract boundary between framework IR and physical lowering.
  }];
  let useDefaultTypePrinterParser = 1;
}

class AwenTensorType<string name, string typeMnemonic>
    : TypeDef<AwenTensorDialect, name> {
  let mnemonic = typeMnemonic;
}

def AwenTensorHandleType : AwenTensorType<"Handle", "handle"> {
  let summary = "Opaque versioned tensor ABI handle";
}

class AwenTensorOp<string mnemonic, list<Trait> traits = []>
    : Op<AwenTensorDialect, mnemonic, traits>;

def AwenTensorGemmOp : AwenTensorOp<"gemm", [Pure]> {
  let summary = "Rank-two matrix multiplication with an accuracy contract";
  let arguments = (ins AnyRankedTensor:$lhs,
                       AnyRankedTensor:$rhs,
                       BoolAttr:$transpose_lhs,
                       BoolAttr:$transpose_rhs,
                       I64Attr:$minimum_effective_bits,
                       StrAttr:$layout);
  let results = (outs AnyRankedTensor:$result);
  let assemblyFormat = [{
    $lhs `,` $rhs attr-dict `:` type($lhs) `,` type($rhs) `->` type($result)
  }];
  let hasVerifier = 1;
}

#endif
