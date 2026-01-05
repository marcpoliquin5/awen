AWEN Computation Model v0.2

Purpose
-------
Provide a precise, implementation-oriented computation model for photonic and quantum-photonic workloads. This document defines the fundamental primitives, state and memory semantics, time & coherence model, measurement/feedback semantics, determinism modes, and the mapping to AWEN-IR.

Audience
--------
Engineers implementing runtimes and HALs, researchers designing kernels, and integrators writing plugins.

1. Core primitives
------------------
- Mode: an addressable optical degree of freedom. A mode is identified by a tuple (port, frequency, polarization, spatial_label).
- Complex amplitude: classical description uses complex field amplitudes a = A e^{i\theta}. Quantum CV modes use annihilation operator â; DV encodings map to qubits/qudits.
- Phase: relative optical phase between modes; absolute phase is undefined without a reference.
- Interference: linear mixing via unitary transforms (scattering matrices).
- Delay: time-of-flight or engineered delay lines; delays are first-class resources.
- Nonlinearity: devices that implement non-linear maps (Kerr, χ(2), single-photon detectors with saturations).
- Measurement: classical (photocurrent) vs quantum (projective, POVM). Measurements may be destructive and probabilistic.
- Entanglement: quantum correlations between modes (CV or DV); tracked by the runtime when quantum backends are used.
- Noise & Drift: shot noise, thermal noise, phase noise, insertion loss, time-varying drift.

2. Photonic State Model
-----------------------
We define two primary execution domains: Classical-Field (CF) and Quantum (Q). Runtimes may support one or both.

Classical-Field state:
- Represented as a vector of complex amplitudes x(t) ∈ C^N over N labeled modes at time t.

Quantum-CV state:
- Represented abstractly as a density operator ρ over continuous-variable modes, or via finite truncation for simulation.

Quantum-DV state:
- Standard qubit/qudit register semantics when DV encodings are used.

State Metadata:
- Each state carries provenance metadata: timestamp, hardware_revision, noise_model_id, seed (for stochastic runs), calibration_version.

3. Memory model (photonic "RAM")
--------------------------------
Photonics lacks native random-access RAM. AWEN provides a memory abstraction built from physical devices.

Memory primitives:
- DelayBuffer(id, latency, loss, coherence_time): FIFO photonic buffer.
- ResonatorStore(id, lifetime, bandwidth): storage with exponential decay and coupling-in/out semantics.
- HybridRegister(id): an addressable electronic/photonic hybrid register exposing read/write operations; the runtime guarantees coherence crossing.

Semantics:
- Lifetime: each buffer has a declared lifetime τ; reads after τ may return degraded/noisy states.
- Persistence: only HybridRegister guarantees persistent retrievable state across long wall-clock times.
- Addressability: logical addresses map to physical channels via the HAL; address resolution is part of the execution plan.

4. Time & coherence semantics
-----------------------------
Time is explicit. Two orthogonal concepts:
- Scheduling time t (wall-clock / control time)
- Coherence windows: intervals where phase relationships are meaningful (length T_coh)

Execution contexts declare:
- start_time, end_time, required_coherence_window (relative to packet arrival)
- synchronization points (barriers) for measurement-conditioned branching

Runtimes must ensure that operations that require coherence are scheduled inside overlapping coherence windows; otherwise the runtime reports a coherence violation.

5. Measurement and feedback
---------------------------
Measurement primitives expose:
- Measure(mode_set, basis, options) -> outcome (classical) + post-measurement state
- Non-demolition measurement options for partial readout

Measurement semantics:
- Quantum measurements are probabilistic and must attach a PRNG seed when deterministic replay is required.
- Measurement latency (hardware + transport) is declared and must be included in the scheduling plan.

Feedback & conditional execution:
- Conditional kernels: kernels may declare branches on measurement outcomes. The runtime synthesizes an execution plan that includes conditional branches and latencies.
- Feedback loops must specify maximum acceptable latency to preserve the correctness of the control.

6. Determinism, reproducibility, and noise modeling
----------------------------------------------------
ExecutionModes:
- Experimental: uses physical hardware randomness and live noise.
- DeterministicReplay: uses captured noise traces and seeded RNG to recreate prior runs.

To enable reproducibility, every run must capture:
- IR snapshot
- calibration_version
- noise_model_id and seed
- hardware revision and environment snapshot

7. Kernel abstraction (runtime-facing)
------------------------------------
Kernel = { id, ports, params, semantics, timing_contract, calibration_hooks }

Interface summary:
- instantiate(params) -> KernelInstance
- prepare(instance, context) -> compiled resource bindings
- execute(instance, inputs, context) -> outputs + traces
- calibrate(instance, telemetry) -> updated_params

Composition rules:
- Kernels compose by connecting named ports; the runtime flattens the graph and resolves timing/delay constraints.
- Parameter binding supports static, dynamic, and measurement-conditioned bindings.

8. Scheduling & execution model
--------------------------------
The runtime compiles an AWEN-IR graph into an ExecutionPlan:
- Resource allocation (buffers, device ports)
- Timing schedule (start times, delays)
- Calibration plan (pre-run calibration kernels)
- Observability hooks (trace/span insertion)

ExecutionPlan must be deterministic for DeterministicReplay mode; i.e. scheduling decisions must be fully captured.

9. Calibration as first-class computation
----------------------------------------
Calibration units are kernels with special lifecycle semantics:
- calibration kernels declare targets (e.g., MZI extinction ratio), calibration parameters (heater voltages), and cost functions.
- Calibration results produce a calibration artifact linked into the run provenance.

Calibration scheduling:
- Pre-run calibration, in-run recalibration triggers, and asynchronous background calibration are supported with versioned outputs.

10. Observability & artifacts
-----------------------------
Every run emits a run-bundle containing:
- IR snapshot
- ExecutionPlan
- calibration_version
- traces.json (time-resolved spans)
- metrics.json
- measurement_results

Trace schema (brief): spans with fields {span_id, parent_id, name, start_ns, end_ns, metadata}

11. AWEN-IR mapping (high level)
--------------------------------
Graph nodes -> kernels
Edges -> port connections plus explicit delay metadata
Node params -> Kernel.params
Graph metadata -> provenance

Example (MZI chain): IR fragment (pseudocode)
```
nodes:
	- id: mzi_0
		type: MZI
		params: {phase_upper: 0.0, phase_lower: 0.0}
edges:
	- src: laser.out
		dst: mzi_0.in
		delay: 5e-9
```

12. Minimal math examples
------------------------
MZI as a unitary (classical fields):
$$
U_{MZI}(\\phi_1,\\phi_2) = \\begin{pmatrix}
e^{i\\phi_1}\\cos\\theta & -e^{i\\phi_1}\\sin\\theta\\\\
e^{i\\phi_2}\\sin\\theta & e^{i\\phi_2}\\cos\\theta
\\end{pmatrix}
$$
where tunable phases control power splitting. The runtime must map `phase` params to physical voltages using the device's PDK transfer curves (calibration data).

Measurement-conditioned feedback pseudocode
------------------------------------------
```
run(IR):
	plan = compile(IR)
	allocate_resources(plan)
	for step in plan.steps:
		if step.type == 'measure':
			outcome = device.measure(step.modes)
			record(outcome)
			if step.has_conditional:
				branch = step.conditional[outcome]
				apply(branch)
		else:
			device.apply(step.ops)
```

13. Operational guarantees & error modes
---------------------------------------
Runtimes must document:
- When a coherence violation occurs
- When calibration is stale (version mismatch)
- When measurement latency breaks a feedback contract

14. Next steps (spec authors)
-----------------------------
- Formalize state semantics with operator algebra for the Quantum-CV case.
- Provide canonical noise models and a JSON schema for `noise_model`.
- Publish parameter-to-voltage mapping schema for PDKs.
- Provide IR canonical examples and a conformance test-suite.

Appendix: provenance fields (minimal)
- run_id, ir_version, spec_version, runtime_version, hardware_id, calibration_id, timestamp, seed

TODO: expand proofs, add formal quantum operator notation, provide full IR->ExecutionPlan mapping and deterministic replay semantics.
