#ifndef AWEN_PHOTONIC_OPS_TD
#define AWEN_PHOTONIC_OPS_TD

include "mlir/IR/AttrTypeBase.td"
include "mlir/IR/OpBase.td"
include "mlir/Interfaces/SideEffectInterfaces.td"

def AwenPhotonicDialect : Dialect {
  let name = "awen_photonic";
  let cppNamespace = "::mlir::awen::photonic";
  let summary = "Classical analog-photonic operations";
  let useDefaultTypePrinterParser = 1;
}

class AwenPhotonicType<string name, string typeMnemonic>
    : TypeDef<AwenPhotonicDialect, name> {
  let mnemonic = typeMnemonic;
}

def AwenOpticalTileType : AwenPhotonicType<"OpticalTile", "optical_tile"> {
  let summary = "Capability-specialized classical optical tile";
}

class AwenPhotonicOp<string mnemonic, list<Trait> traits = []>
    : Op<AwenPhotonicDialect, mnemonic, traits>;

def AwenPhotonicGemmTileOp : AwenPhotonicOp<"gemm_tile", [Pure]> {
  let summary = "A classically encoded optical GEMM tile";
  let arguments = (ins AnyRankedTensor:$lhs,
                       AnyRankedTensor:$rhs,
                       I64Attr:$tile_m,
                       I64Attr:$tile_n,
                       I64Attr:$tile_k,
                       I64Attr:$minimum_effective_bits,
                       StrAttr:$calibration,
                       StrAttr:$layout);
  let results = (outs AnyRankedTensor:$result);
  let assemblyFormat = [{
    $lhs `,` $rhs attr-dict `:` type($lhs) `,` type($rhs) `->` type($result)
  }];
}

#endif
