# AEP-0009: Quantum Coherence & State Memory Model v0.1

Status: Implemented experimental contract

Purpose
-------
Define the quantum photonic state representation, coherence window semantics, and state evolution model for AWEN. This AEP ensures quantum photonic systems can:
- Track quantum state (superposition, entanglement) across spatial/temporal domains
- Enforce coherence windows (temporal bounds beyond which decoherence invalidates computation)
- Condition control flow on measurement outcomes
- Support hybrid classical-quantum execution

Scope
-----
- Photonic state space: classical modes (deterministic), quantum modes (probabilistic amplitudes), mixed states
- Coherence window model: initialization time, decoherence time, idle time budgets, cross-mode decoherence
- Measurement model: projection, outcome distribution, destructive vs non-destructive
- State evolution: unitary gates, noise channels, mixed-state dynamics
- Measurement-conditioned feedback: branching on measurement outcomes, shot-based control
- Deterministic seeds for quantum state sampling (for reproducibility)

Conformance
-----------
- Runtimes must implement `QuantumState` and `CoherenceWindow` traits
- State preparation, evolution, and measurement must be accessible from IR nodes
- Coherence windows must be validated by the Engine before execution
- All quantum state snapshots must include provenance (seed, parameters, decoherence model)
- Deterministic replay of quantum circuits must be possible via seeded RNG

Implementation evidence
-----------------------
- `awen-spec/specs/quantum-coherence.md` defines state, coherence, measurement, and replay semantics.
- `awen-runtime/src/state`, `awen-runtime/src/quantum.rs`, and the typed quantum-photonic gateway implement reference state, evolution, measurement, and coherence checks.
- Engine and scheduler tests enforce coherence windows and measurement-conditioned feedback.
- Quantum and photonic conformance tests verify seeded replay, distributions, means, fidelity, feed-forward, and artifact lineage.

This experimental reference does not establish physical quantum hardware
fidelity or general-purpose quantum compilation.

