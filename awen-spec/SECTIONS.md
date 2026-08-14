# AWEN Specification Sections

## Photonics

- Architecture: `aeps/AEP-0020-classical-quantum-photonic-separation.md`
- Typed schemas: `schemas/awen_photonic_program.v1.json`,
  `schemas/awen_qphotonic_program.v1.json`,
  `schemas/awen_qphotonic_result.v1.json`, and
  `schemas/awen_photonic_interop.v1.json`
- Legacy migration input: `schemas/photonic_ir.v5.json`
- Runtime boundary: `awen-runtime/src/chokepoint.rs`
- Direct evidence: `awen-runtime/tests/photonic_conformance.rs`

## Runtime subsystems

The normative subsystem specifications are in `specs/`. Their conformance claims
are limited to exported implementation and direct automated evidence.

## Verification

The single required repository quality gate validates schemas and examples, runs
the complete compiler and runtime test suites, builds the C++ and MLIR consumers,
tests the supported Python dependency floors and current versions, audits secrets
and dependencies, and enforces repository policy. See
`docs/IMPLEMENTATION-STATUS.md` for the current evidence boundary.
