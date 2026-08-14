# AWEN MLIR compatibility policy

The four registered dialect namespaces are `awen_tensor`, `awen_photonic`,
`awen_qphotonic`, and `awen_device`. The conceptual names in architecture
documents remain `awen.tensor`, `awen.photonic`, `awen.qphotonic`, and
`awen.device`; underscores are used in textual MLIR because the component
before the first period is the registered dialect namespace.

Each dialect implements MLIR's `BytecodeDialectInterface` and writes a
two-component dialect version. Current dialect bytecode is `1.0`.

- A reader accepts any `1.x` dialect bytecode whose changed constructs retain
  their v1 semantics.
- A major-version change is rejected before execution unless an explicit
  `upgradeFromVersion` implementation exists.
- Operations, custom types, required properties, and executable-command
  semantics cannot be removed or reinterpreted within major version 1.
- Additive optional properties require a minor-version increment and a
  canonical default for older bytecode.
- Textual IR is a debugging/interchange representation, not the runtime ABI.
- `AWENEXE` has an independent ABI version because runtime compatibility must
  not depend on a particular MLIR library build.
- AWENEXE 1.0 `ExecuteGemm` result shapes are rank-aware. Rank two represents
  `[M,N]`; equal-batch rank three represents `[B,M,N]`. Both use the existing
  versioned command encoding, so no field or command-kind reinterpretation is
  required. Each dynamic dimension is encoded as signed i64 `-1`; zero and
  values below `-1` are invalid.
- StableHLO compatibility remains owned by upstream StableHLO. AWEN does not
  copy or redefine its general compatibility guarantee.
- Classical `awen_photonic` signal/tensor values and `awen_qphotonic` Fock,
  Gaussian, and sample-stream values are not ABI-compatible. Crossing the
  dialect boundary requires a versioned explicit interop operation outside the
  classical StableHLO GEMM lowering.
