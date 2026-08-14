# Classical and quantum-photonic dialect operator guide

New programs must choose one typed root contract:

- `awen.photonic.program.v1` for calibrated classical analog transforms;
- `awen.qphotonic.program.v1` for Fock/Gaussian state, gates, sampling,
  feed-forward, coherence, and statistical correctness; or
- `awen.photonic-interop.v1` for explicit measurement readout or classical
  control boundaries.

Do not submit `photonic_ir.v5.json` directly to execution. Migrate it first:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  migrate-photonic-v5 legacy-v5.json \
  --output migration-report.json
```

The command writes a report after the input validates against the frozen V5
schema. It exits with failure when the document is malformed or any operation
is missing, ambiguous, or unsupported. A
recognized classification is not executable by itself: fill in the complete
classical precision/calibration contract or the complete quantum
state/sampling/correctness contract, then insert explicit interop operations
where the two programs meet.

Reference fixtures are under `awen-spec/fixtures`:

- `classical_photonic_program.json`;
- `quantum_photonic_program.json` and
  `quantum_photonic_result.json`;
- `quantum_gaussian_feed_forward.json`;
- `photonic_interop_program.json`; and
- `photonic_v5_ambiguous.json`.

The runtime gateway requests `execute:awen.photonic`,
`execute:awen.qphotonic`, or `execute:awen.photonic-interop` from signed
plugins. Plugin implementations receive the entire typed program and execution
context. They must not accept a generic operation string and reinterpret it.

JSON schemas provide structural validation. Rust verifiers additionally check
cross-field rules including ordered signal dataflow, calibration identity,
state-space/gate compatibility, covariance shape/symmetry/positive
semidefiniteness, capability coverage, feed-forward ordering and mode
alignment, destructive measurement liveness, coherence sums, probability
sums, shot totals, distribution distance, means, fidelity, confidence, and
program-bound replay identity.

See AEP-0020 for the normative architecture and migration policy.
