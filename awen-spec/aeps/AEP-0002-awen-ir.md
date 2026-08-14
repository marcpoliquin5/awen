# AEP-0002: AWEN IR

Status: Implemented bootstrap contract; superseded in production lowering by AEP-0010

## Decision

The legacy graph IR is defined by `../schemas/awen_ir.proto` and documented in
`../specs/awen-ir.md`. Checked examples include
`../../awen-runtime/example_ir.json` and the typed photonic fixtures under
`../fixtures`.

New compiler infrastructure uses the registered MLIR dialects described by
AEP-0010 and emits the versioned AWENEXE ABI from AEP-0012. The JSON graph is a
compatibility and semantic-reference surface, not a second general compiler.
