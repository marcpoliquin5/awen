# AEP-0024: Virtual Photonic ISA (VPISA) v1

Status: Accepted and implemented (classical-domain reference interpreter; quantum domain specified and validated, not executed)

## Decision

AWEN defines a typed, linear, hardware-independent instruction set (VPISA)
between the graph-level programs (`awen.photonic.program.v1`,
`awen.qphotonic.program.v1`, `awen.photonic-interop.v1`) and a backend's
private commands. A backend that consumes VPISA can run any program the
AWEN compiler lowers, without AWEN knowing its internal command format.
`AWENEXE` (AEP-0012) remains the runtime ABI for compiled GEMM dispatches
and is unchanged.

## Rules

1. One canonical JSON form (`awen-spec/schemas/awen_vpisa.v1.json`) and one
   deterministic little-endian binary form (`AWENISA\0`). Both are lossless:
   decode(encode(p)) = p and encode(decode(b)) = b.
2. Three domains. Classical instructions touch buffers, optical fields and
   meshes. Quantum instructions touch only modes. Interop instructions
   (measurements, feed-forward, register reads) are the only bridge, and a
   program must declare `quantum` and `feed_forward` to use them.
3. Every photonic instruction needs a prior `set_calibration` with a
   `sha256:` fingerprint. Programs declare the effective bits and tile size
   they need, so a backend can refuse a program before running it.
4. Electronic fallback is explicit: a non-photonic `gemm` must carry a
   `fallback_reason`.
5. Same major version ⇒ compatible. A backend runs a program whose minor
   version is not newer than its own (`negotiate`).
6. The program digest (`sha256` of the binary form) identifies a program in
   provenance records and replays.

## Implementation

`awen_compiler::isa` provides the types, `codec::{encode, decode}`,
`validate` (27 typed `IsaError` kinds), `negotiate`, `check_target`,
`IsaProgram::lower_gemm`, and `interp::run`, a reference interpreter for the
classical domain. Photonic instructions run on the AEP-0022 compact noise
model (**simulated**), and electronic instructions run exactly in host f64.
The interpreter refuses quantum instructions. Executing them needs a
quantum-photonic simulator or backend, which this AEP does not provide.

Tests (`awen-compiler/tests/isa.rs`):

- 2,000 random programs covering all 24 instruction kinds round-trip byte
  for byte, in binary and in JSON.
- Every truncation of an encoded program is rejected, and each malformed
  binary gets its typed error.
- A negative test for each validator rule.
- Domain separation is enforced.
- The decomposed photonic sequence gives bit-identical results to the fused
  `gemm` for the same seed, and exact results under the ideal contract.
- The schema lists exactly the implemented instructions.

## Not covered

VPISA v1 has no loops or branches beyond measurement feed-forward. It has
no multi-device placement and no explicit memory hierarchy beyond host and
device. These are planned for later minor and major versions, tracked in
`docs/DIRECTIVE_TRACKER.md` (C8, T8, T9).
