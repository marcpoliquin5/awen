# SECTIONS

This document tracks feature sections and their Definition-of-Done (DoD).

## Section: Gradients & Adjoint Provider

- Spec/AEP: [AEP-0008 Differentiable Photonics](../awen-spec/aeps/AEP-0008-differentiable-photonics.md)
- Owner crate: `awen-runtime` (currently `src/gradients.rs`)

DoD checklist:
- [x] Trait + provider implemented (`GradientProvider`, `ReferenceAdjointProvider`, `ReferenceGradientProvider`)
- [x] Conformance tests comparing adjoint vs finite-difference (`src/gradients.rs` test `test_adjoint_vs_fd_conformance`)
- [x] Edge cases: phase wrappoints covered by tolerances in tests
- [x] Implementation documented (docs + README)
- [x] CLI integration: `awenctl` supports `--strategy`/`auto` selection
- [x] CI entry: test added to runtime test suite (CI workflow should run `cargo test`)

Status: DONE

---

## Section: HAL (LabDevice, SafetyPolicy)

- Spec/AEP: [AEP-0005 Observability & Reproducibility] (awen-spec)
- Owner crate: `awen-runtime` (currently `src/hal/mod.rs`)

DoD checklist:
- [x] Interfaces implemented (`Device`, `LabDevice`, `SafetyLimits`, `CalibrationResult`)
- [x] Reference `SimulatedDevice` implements `LabDevice`
- [x] Safety defaults conservative (simulation echoes back applied mapping)
- [ ] Integration: runtime enforces safety policy before applying calibrations
- [ ] Conformance tests for safety enforcement

- [x] Integration: runtime enforces safety policy before applying calibrations (Engine::apply_calibration)
- [x] Conformance tests for safety enforcement (`engine::test_apply_calibration_enforces_safety`)

Status: DONE

---

Notes:
- Recommend moving modules into crates under `awen-runtime/crates/` when project scales. For now, the code lives in `awen-runtime/src/`.
- Completion requires wiring safety enforcement into `control`/`engine` before marking HAL fully done.

Verification Commands (copy-paste):

```bash
cd awen-runtime
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features

# Run integration locally (build then run)
cargo build --workspace --release
./target/release/awenctl run example_ir.json --seed 42
./target/release/awenctl gradient example_ir.json "mzi_0:phase" --strategy auto --seed 42 --samples 1
```

# Additional verification: compile-fail UI tests
```bash
cargo test --test trybuild
```

Owner files:
- awen-runtime/src/gradients.rs
- awen-runtime/src/bin/awenctl.rs
- awen-runtime/src/hal/mod.rs
- awen-runtime/src/engine/mod.rs
- awen-runtime/README.md
- awen/docs/SECTIONS.md
- awen-runtime/.github/workflows/*.yml

---

## Section: Observability & Profiling Model v0.1 (Nsight-like)

- Spec/AEP: `awen-spec/aeps/AEP-0005-observability.md` (placeholder)
- Owner crate: `awen-runtime` (target: `src/observability/*`)

Definition of Done:
- [x] Spec/AEP exists and is linked here
- [x] Core interfaces defined (Tracer, Span, MetricsSink, EventLogger, TimelineBuilder)
- [x] Reference file exporter implemented (timeline.json, traces.jsonl, metrics.json)
- [x] Integration wiring: Engine emits spans for IR load, kernel exec, measurement, calibration, safety events
- [x] Conformance tests + integration validate observability artifacts exist and contain required fields
- [x] CI quality gate runs integration that validates observability artifacts

Status: DONE

Verification Commands (copy-paste):
```bash
cd awen-runtime
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features

# After implementation, run integration commands to validate observability artifacts
cargo build --workspace --release
./target/release/awenctl run example_ir.json --seed 42
# check for traces/metrics/timeline files in awen_run_* artifacts
```

Owner files:
- awen-runtime/src/observability/*
- awen-spec/aeps/AEP-0005-observability.md
- awen-spec/specs/observability.md
- awen-runtime/src/bin/awenctl.rs
- awen-runtime/.github/workflows/*.yml

---

## Section: Quantum Coherence & State Memory Model v0.1

- Spec/AEP: `awen-spec/aeps/AEP-0009-quantum-coherence.md`, `awen-spec/specs/quantum-coherence.md`
- Owner crate: `awen-runtime` (target: `src/state/*`)

Rationale: Quantum photonic systems require explicit coherence window tracking, quantum state representation, and measurement-conditioned control. Omitting this now blocks all quantum workflows and forces retrofit later.

Definition of Done:
- [x] Formal spec: photonic state space (classical modes vs quantum modes), coherence window semantics, decoherence models
- [x] AEP-0009 created with conformance requirements for state preparation, evolution, measurement, feedback
- [x] Runtime interfaces: `QuantumState`, `CoherenceWindow`, `MeasurementOutcome`, `StateEvolver` traits
- [x] Reference implementation: stateless simulator for quantum states (unitary gate evolution, destructive measurement with seeded RNG)
- [x] Integration: Engine tracks coherence windows per IR subgraph and enforces temporal constraints
- [x] Tests: state evolution correctness, coherence window bounds checking, measurement outcome sampling (`test_quantum_state_artifact_created`)
- [x] CI: quality gate + integration validates state artifacts and coherence metadata
- [x] IR Schema: measurement-conditioned feedback support (`ConditionalBranch`, `measure_mode` in Node)
- [x] Artifacts: quantum_states.json, measurements.json created per run with full provenance

Status: DONE (CI validated)

Verification Commands (copy-paste):
```bash
cd awen-runtime
cargo fmt --all -- --check
cargo clippy --all-targets --all-features -- -D warnings
cargo test --all-features

# After implementation, validate quantum state artifacts
cargo build --workspace --release
./target/release/awenctl run example_ir.json --seed 42
# Verify artifacts: quantum_states.json, measurements.json in awen_run_* directory
ls awen_run_*/quantum_states.json
ls awen_run_*/measurements.json
```

Owner files:
- awen-runtime/src/state/*
- awen-runtime/src/ir/mod.rs (ConditionalBranch, IR validation)
- awen-spec/aeps/AEP-0009-quantum-coherence.md
- awen-spec/specs/quantum-coherence.md
- awen-runtime/src/engine/mod.rs (coherence tracking, gate evolution, measurement)
- awen-runtime/.github/workflows/*.yml
