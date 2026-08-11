#ifndef AWEN_DEVICE_OPS_TD
#define AWEN_DEVICE_OPS_TD

include "mlir/IR/AttrTypeBase.td"
include "mlir/IR/OpBase.td"

def AwenDeviceDialect : Dialect {
  let name = "awen_device";
  let cppNamespace = "::mlir::awen::device";
  let summary = "Executable AWEN device command operations";
  let useDefaultTypePrinterParser = 1;
}

class AwenDeviceType<string name, string typeMnemonic>
    : TypeDef<AwenDeviceDialect, name> {
  let mnemonic = typeMnemonic;
}

def AwenCommandBufferType : AwenDeviceType<"CommandBuffer", "command_buffer"> {
  let summary = "Versioned command-buffer ABI handle";
}

class AwenDeviceOp<string mnemonic, list<Trait> traits = []>
    : Op<AwenDeviceDialect, mnemonic, traits>;

def AwenDeviceExecuteGemmOp : AwenDeviceOp<"execute_gemm"> {
  let summary = "Executable tiled GEMM dispatch";
  let arguments = (ins AnyRankedTensor:$lhs,
                       AnyRankedTensor:$rhs,
                       StrAttr:$backend,
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
