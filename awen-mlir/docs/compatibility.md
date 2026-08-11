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
- StableHLO compatibility remains owned by upstream StableHLO. AWEN does not
  copy or redefine its general compatibility guarantee.
