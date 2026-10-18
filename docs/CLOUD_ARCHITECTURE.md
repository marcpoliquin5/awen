# AWEN Cloud: architecture (design only)

> **Status: not available. Design only.** There is no hosted AWEN service, no
> API endpoint, no account system, and nothing is deployed. This document
> describes how a hosted service *could* be built on the components that exist
> in this repository, so that future work extends the same contracts instead
> of inventing new ones. No part of it should be read as a product offer,
> roadmap commitment, or availability claim.

## Goals and non-goals

Goals of the design:

* Run the same compiled AWEN programs a developer runs locally
  (`awenctl execute`, `awen_py`, `awen_models`) on shared infrastructure, with
  identical contracts, results, and evidence records.
* Expose photonic devices, when any exist, through the same capability and
  health snapshots the local runtime uses, so a job can be rejected or fall
  back to CPU for the same, inspectable reasons.
* Keep every measured number traceable to a commit, a device snapshot, and a
  calibration record.

Non-goals:

* No new execution semantics. The service would schedule existing runtime
  operations; it would not add hidden optimizations.
* No claim of photonic acceleration. Until measured hardware evidence exists
  (see `docs/BENCHMARKS.md`), every photonic job is a simulator job and is
  labelled that way.

## Component map

Each component lists the repository module or schema it would build on, and
whether that part exists today.

| Component | Responsibility | Builds on | Exists today |
|---|---|---|---|
| Job API | Accept a compiled artifact or a model reference plus inputs; return a job id | `awen_compilation_artifact.v1`, `awen_tensor_ir.v1`, `awen_vpisa.v1` | contracts yes; service no |
| Compiler service | Compile StableHLO/ONNX/tensor IR to AWEN artifacts with a capability snapshot | `awen-compiler`, `awen-mlir`, `awen_models.onnx_import` | library yes; service no |
| Scheduler | Place work on device leases, order it in streams, enforce memory budgets | `awen-runtime/src/devrt/{stream,residency,lease}.rs`, `src/scheduler` | library yes; multi-tenant service no |
| Device pool | Track devices, capabilities, and live health | `awen_device_capability.v1`, `awen_backend_health.v1`, `awen_hardware_description.v1`, `src/hal` | schemas and simulator drivers yes; physical devices no |
| Fallback | Remap work to CPU when a device is unhealthy or a contract fails | `src/devrt/remap.rs` | library yes |
| Calibration service | Record calibration snapshots and refuse stale ones | `awen_calibration_snapshot.v1`, `src/calibration` | library yes; scheduled calibration of real devices no |
| Artifact store | Content-addressed storage of programs, inputs, results, and traces | `src/storage` (deterministic ids, bundles, manifests) | library yes; object storage backend no |
| Evidence store | Immutable benchmark and HIL evidence | `awen_benchmark_evidence.v1`, `awen_hil_artifact.v1`, `awen-bench` | schemas and harness yes; hosted store no |
| Observability | Per-job traces, metrics, timelines | `src/observability`, `src/devrt/profile.rs` (Chrome trace) | library yes; hosted collection no |
| Plugin trust | Only run device drivers signed by pinned keys | `src/plugins/signing.rs` (`TrustedKeys`) | library yes |
| Lab workflows | Simulation-only experiment runs with approval gates | `awen-lab` | library and CLI yes; hosted no |
| Quantum-photonic jobs | Separate queue and result contract | `awen_qphotonic_program.v1`, `awen_qphotonic_result.v1`, `awen-qphotonic` | simulators yes; hardware no |
| Identity, tenancy, billing | Accounts, quotas, metering | none | no |

## Request lifecycle

1. **Submit.** The client sends an artifact (or a model reference resolved by
   the compiler service), inputs, a precision/noise contract
   (`awen_photonic_noise_contract.v1`), and a placement policy (photonic
   simulator, CPU, or "photonic if healthy, else CPU").
2. **Admit.** The service validates every document against its published
   schema (`awen_runtime::schema`) and rejects unknown versions.
3. **Place.** The scheduler reads current device snapshots. A device whose
   health or calibration violates the contract is not eligible; the job is
   either rejected or remapped to CPU according to its policy, and the reason
   is recorded.
4. **Execute.** The runtime executes the artifact on the leased device in a
   stream with an explicit memory budget. Deterministic replay data
   (`src/devrt/record.rs`) is kept with the job.
5. **Record.** Results, traces, the device snapshot, the calibration id, the
   code commit, and the placement decision are written to the artifact store
   under content-addressed ids.
6. **Return.** The client receives results plus a label for every number:
   *measured* (on named hardware), *simulated*, or *estimated*, using the same
   vocabulary as the local tools.

## Isolation and security

* Jobs run in per-tenant sandboxes; artifacts are data, never code. Device
  drivers load only if signed by a pinned key (`TrustedKeys`).
* Model imports follow the local policy: no remote code, safetensors only
  (`scripts/check_safe_model_loading.py`).
* Every service release would carry the SBOM, checksums, and license policy
  described in `SECURITY.md`.
* Device access goes through leases; a lease is revoked on health failure and
  the remaining work is remapped or failed explicitly, never silently dropped.

## Open problems (not solved by this design)

* Physical photonic hardware: none is available to this project, so latency,
  throughput, energy, and calibration-drift figures for hosted devices are
  unknown and must be measured before any are stated.
* Multi-device programs across an optical fabric: only modelled
  (`awen_systems.fabric`), not built.
* Tenant isolation on shared analog hardware (crosstalk, residual state
  between jobs) has no measured basis yet.
* Identity, quotas, and metering have no implementation.

## What would have to be true before any hosted offering

1. A physical device passes the HIL suite with published evidence.
2. The runtime's fallback and replay paths are exercised against that device.
3. Security review of the service boundary, including driver signing and data
   retention.
4. This document is replaced by an operations runbook describing the actual
   deployment.
