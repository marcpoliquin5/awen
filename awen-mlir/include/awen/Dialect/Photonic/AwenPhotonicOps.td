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

def AwenClassicalSignalType : AwenPhotonicType<"ClassicalSignal", "signal"> {
  let summary = "Classical analog optical signal; never a quantum state";
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
  let hasVerifier = 1;
}

def AwenPhotonicCalibratedTransformOp
    : AwenPhotonicOp<"calibrated_transform", [Pure]> {
  let summary = "Classical analog transform bound to a calibrated transfer function";
  let arguments = (ins AnyRankedTensor:$input,
                       StrAttr:$transfer_model,
                       StrAttr:$calibration_snapshot,
                       StrAttr:$calibration_fingerprint,
                       I64Attr:$optical_effective_bits,
                       F64Attr:$maximum_residual_error);
  let results = (outs AnyRankedTensor:$result);
  let hasVerifier = 1;
}

def AwenPhotonicModulateOp : AwenPhotonicOp<"modulate", [Pure]> {
  let summary = "Classical electrical-to-optical modulation";
  let arguments = (ins AnyRankedTensor:$input,
                       StrAttr:$modulation,
                       F64Attr:$carrier_wavelength_nm,
                       I64Attr:$dac_bits,
                       StrAttr:$calibration_fingerprint);
  let results = (outs AnyRankedTensor:$result);
  let hasVerifier = 1;
}

def AwenPhotonicDetectOp : AwenPhotonicOp<"detect"> {
  let summary = "Classical optical-to-electrical detection";
  let arguments = (ins AnyRankedTensor:$input,
                       StrAttr:$detection,
                       I64Attr:$integration_time_ns,
                       I64Attr:$adc_bits,
                       StrAttr:$calibration_fingerprint);
  let results = (outs AnyRankedTensor:$result);
  let hasVerifier = 1;
}

#endif
