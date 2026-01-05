AEP-0007: Calibration as Computation (v0.1)

Status: Draft

Summary:
This AEP formalizes calibration as a first-class computation in AWEN: calibration kernels, artifacts, versioning, scheduling, and integration with the runtime and observability.

Motivation:
Calibration is critical for photonic systems (drift, thermal coupling, device aging). Treating calibration as code ensures reproducibility and enables automation.

Specification:
- Calibration Kernel: a special Kernel subtype with fields: target_nodes, parameters_to_tune, cost_function_spec (classical or differentiable), measurement_sequence, safety_constraints.
- Calibration Artifact: a versioned bundle containing calibration parameters, measurement traces, optimizer state, timestamp, hardware_revision, and provenance.
- Scheduling: calibration kernels can be scheduled pre-run, in-run (blocking) with latency budgets, or asynchronously.
- Recalibration triggers: define thresholds (e.g., extinction_ratio < X, phase_drift > Y) that cause auto-recalibration.
- Interfaces: runtime exposes `calibrate_node(node_id, options)` and `query_calibration_artifact(artifact_id)`.

Backwards compatibility:
- Calibration kernels are optional; runtimes that don't implement calibration must return a clear error code and suggest offline tools.

Test plan:
- Reference simulator supports drift injection and a mock calibration loop.
- Conformance tests verify artifact schema, versioning, and scheduled calibration runs.

TODO:
- Define `cost_function_spec` schema.
- Define optimizer primitives and differentiable hooks.
- Define safety constraint schema for lab hardware.
