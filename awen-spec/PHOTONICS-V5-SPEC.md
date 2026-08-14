# AWEN Photonics — Version 5 (PHOTONICS-V5)

Status: Superseded as an execution contract by AEP-0020

Migration notice
----------------
The mixed `photonic_ir.v5.json` operation space is now legacy input only. New
programs use `awen.photonic.program.v1`, `awen.qphotonic.program.v1`, and the
narrow `awen.photonic-interop.v1` boundary. Run `awenctl
migrate-photonic-v5`; ambiguous strings are diagnosed and never assigned
semantics automatically. This historical skeleton remains readable so old
documents and decisions are not erased.

Purpose
-------
This document is the historical top-level specification for AWEN Photonics (Version 5).
It defines the required interfaces, schemas, runtime chokepoints, observability contracts,
and conformance requirements for a production-grade, OS-level photonics runtime that
supports both classical photonic computing and quantum photonics.

Absolute rules
--------------
- Follow the AWEN CONSTITUTIONAL DIRECTIVE: include abstractions for all plausible
  frontier requirements (material-agnostic, foundry-agnostic, simulator/lab/production).
- All executable behavior must flow through a non-bypassable runtime chokepoint described
  in this spec.
- All observable behavior must emit traces/metrics/timeline data and be artifact-captured.

Mandatory system dimensions (each section below must be completed with interfaces, reference
implementations, conformance tests, CI rules, and docs):

- Computation model
- Kernel model
- IR & schemas
- Memory & state semantics
- Timing, latency & coherence
- Calibration & drift
- Noise & uncertainty
- Safety & constraints
- Scheduling
- Observability (traces, metrics, timelines)
- Debugging & profiling
- Artifact & reproducibility
- Deterministic replay
- Plugin & ecosystem extensibility
- CLI + API + Studio UX
- CI & verification
- Governance & versioning

Spec sections (skeleton)
-------------------------

1) Overview and Goals
   - Intent, target users, compatibility promises, non-goals (explicitly none-by-default).

2) High-level Architecture
   - Runtime layers
   - Non-bypassable chokepoints (Execution API gateway, Artifact writer, Observability sink)
   - Backend abstraction boundaries (simulator, lab, foundry, cloud, hybrid)

3) IR & Schemas
   - Historical mixed Photonic IR (PHOTONIC-IR) design goals
   - The current canonical schemas are `awen_photonic_program.v1.json`,
     `awen_qphotonic_program.v1.json`, `awen_qphotonic_result.v1.json`, and
     `awen_photonic_interop.v1.json`.
   - Versioning and migration rules are specified by AEP-0020.
   - Executable reference payloads cover calibrated classical operations,
     Fock and Gaussian gates, measurements, feed-forward, and interop.
   - `schemas/photonic_ir.v5.json` is frozen as legacy migration input; missing
     typed fields are supplied only after operator review, never inferred.

4) Runtime Chokepoint: Execution API (MANDATORY)
   - All runtime-executed programs MUST call `execute(program:
     PhotonicProgram, ctx: ExecContext)`, where `PhotonicProgram` is a closed
     union of the classical, quantum, and explicit-interop roots.
   - Responsibilities of chokepoint:
     - Authorize and validate IR against schema
     - Inject calibration and drift compensation
     - Serialize and record artifact metadata
     - Emit structured telemetry (trace spans, metrics, timeline events)
     - Ensure deterministic replay hooks are recorded
     - Route to backend plugins via the Plugin Interface
   - Non-bypassable guarantees: no backend may accept commands without passing through chokepoint
     (enforcement via signing, manifest checks, and CI verification).

5) Kernel Model & Plugin Interface
   - Kernel contract: capability description, resource needs, precision guarantees, probabilistic outputs
   - Plugin API: discovery, registration, capability advertisement, execution, calibration interface,
     health checks, and artifact sink hooks.
   - Must support synchronous, asynchronous, streaming, and measurement-conditioned callbacks.

6) Memory & State Semantics
   - Immutable artifact model vs mutable runtime state
   - Checkpointing semantics and snapshot formats
   - Concurrency model and memory consistency guarantees

7) Timing, Scheduler & Coherence
   - Time model (logical clocks, physical timestamps, coherence windows)
   - Scheduler interface and pluggable policies (latency-first, coherence-first, throughput-first)

8) Calibration & Drift Management
   - Calibration-first workflow: calibration artifacts, baseline traces, continuous drift monitoring
   - APIs for calibration runs, calibration artifacts, and automatic compensation hooks

9) Noise, Uncertainty & Probabilistic Execution
   - Models for noise injection and uncertainty propagation
   - Statistical APIs: confidence intervals, bootstrapping, resampling hooks

10) Observability and Artifacts
    - Trace/span model and required fields (timestamps, op ids, backend ids, calibration ids)
    - Metric families and timeline event types
    - Artifact capture: raw measurement data, compiled backend binaries, calibration logs

11) Debugging and Profiling
    - Timeline visualizer data contract
    - Profiling hooks (latency heatmaps, coherence maps, kernel-level counters)

12) Deterministic Replay & Reproducibility
    - Replay manifests and seed handling
    - Serialization format for deterministic replays

13) Testing, CI & Conformance
    - Conformance test suite structure and example tests
    - CI gating rules and enforcement

14) CLI, API and Studio UX
    - CLI primitives (compile, run, calibrate, profile, artifact-export)
    - API surface (gRPC/HTTP + language bindings)
    - Studio UX integration points (timeline, visualizer, step-debugger)

15) Governance, Versioning & Releases
    - Spec versioning policy, deprecation rules, and compatibility guarantees

16) Security, Safety & Constraints
    - Sandboxing plugin execution, access control, and resource limits

17) Extensions, Roadmap & TODOs
    - Concrete TODOs and AEP placeholders linking to work items and reference implementations.

Appendices
---------
- Appendix A: Example IR snippets (classical + quantum)
- Appendix B: Artifact format definitions
- Appendix C: Conformance test plan matrix

Immediate TODOs (progress)
-------------------------------------------
- `awen-spec/schemas/photonic_ir.v5.json` — present as deprecated migration input.
- AEP-0020 and `specs/photonic-dialect-separation.md` — current typed contracts.
- `awen-spec/SECTIONS.md` — entry added mapping PHOTONICS-V5 to spec artifacts.
- Runtime chokepoint interface — typed fail-closed implementation at `awen-runtime/src/chokepoint.rs`.
- Conformance test harness — separate dialect, interop, replay, and migration coverage at `awen-runtime/tests/photonic_conformance.rs`.

The current implementation validates independent schemas and Rust contracts,
records typed programs in artifact bundles, emits observability, requests
dialect-specific signed-plugin capabilities, and runs conformance in the
required quality gate.

Authors and Contacts
--------------------
Primary: AWEN Photonics Working Group
Maintainers: TBD

License
-------
Same license as repository.

-- End of initial skeleton --
