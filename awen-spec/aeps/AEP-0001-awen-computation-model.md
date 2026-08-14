# AEP-0001: AWEN computation model

Status: Implemented experimental contract

Author: [@marcpoliquin5](https://github.com/marcpoliquin5)

## Decision

AWEN programs use versioned typed graphs with explicit classical, classical
photonic, and quantum-photonic semantics. Timing, coherence, measurement,
conditional control, calibration identity, randomness, precision, and
provenance are data in the contract rather than implicit backend behavior.

The normative definitions are in `../specs/computation-model.md`,
`../specs/timing-scheduling.md`, `../specs/calibration.md`, and
`../specs/photonic-dialect-separation.md`. Executable examples are under
`../fixtures`, while compiler/runtime conformance tests enforce separation,
determinism, statistical correctness, and fail-closed compatibility.

This decision does not claim a general quantum compiler, arbitrary StableHLO
coverage, physical hardware correctness, or hardware acceleration.
