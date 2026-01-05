# AWEN Photonics — Version 5 (PHOTONICS-V5)

Status: Draft

Purpose
-------
This document is the authoritative, top-level specification for AWEN Photonics (Version 5).
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
   - Photonic IR (PHOTONIC-IR) design goals
   - Canonical JSON Schema filenames and locations (TODO: add concrete files under `awen-spec/schemas`)
   - Versioning and migration rules
   - Example payloads (classical waveguide ops, quantum gates, measurement primitives)
   - TODO: `schemas/photonic_ir.v5.json` — schema must include kernel metadata, timing, constraints,
     calibration handles, provenance fields.

4) Runtime Chokepoint: Execution API (MANDATORY)
   - All runtime-executed operations MUST call `execute(op: PhotonicOp, ctx: ExecContext)`
     (conceptual signature — exact bindings to be specified per language).
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
- `awen-spec/schemas/photonic_ir.v5.json` — present (canonical IR schema).
- `awen-spec/SECTIONS.md` — entry added mapping PHOTONICS-V5 to spec artifacts.
- Runtime chokepoint interface — stub implemented at `awen-runtime/src/chokepoint.rs` (NonBypassableGateway reference impl).
- Conformance test harness — initial integration test added at `awen-runtime/tests/photonic_conformance.rs`.

Next immediate steps (prioritized):
- Expand `chokepoint` to perform schema validation, calibration injection, artifact emission, and plugin routing.
- Implement plugin capability registry and signer/manifest enforcement for non-bypassability.
- Add observability sinks and trace/span schema under `awen-spec/schemas`.
- Add CI job to run `cargo test` and conformance suite for PR gating.

Authors and Contacts
--------------------
Primary: AWEN Photonics Working Group
Maintainers: TBD

License
-------
Same license as repository.

-- End of initial skeleton --
