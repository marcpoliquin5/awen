AEP-0001: AWEN Computation Model (v0.1)

Status: Draft
Authors: TBD

Summary:
Define the AWEN computation model: photonic state, time/coherence semantics, measurement and feedback, deterministic vs probabilistic execution.

Motivation:
Provide the foundational model for kernels, IR, runtime scheduling, and reproducibility.

Specification:
- Photonic state: modes, phase, amplitude, entanglement semantics.
- Lifetime semantics: coherence windows, state persistence.
- Time semantics: scheduling, synchronization, delays.
- Measurement: classical vs quantum measurement primitives, conditional branching.
- Determinism: execution modes and reproducibility guarantees.

Test plan:
- Reference simulator validation
- Reproducible artifact checks

TODO: Expand formal definitions and examples.
