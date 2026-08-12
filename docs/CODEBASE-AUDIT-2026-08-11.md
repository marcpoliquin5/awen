# AWEN codebase audit — 2026-08-11

## Scope and evidence

This audit examined `marcpoliquin5/awen` at `main` commit `568bac9` and read-only public repository state for `marcpoliquin5/awen-labs` and `marcpoliquin5/awenphotonics.github.io`. The main repository contained 170 tracked source/specification/document files before this branch. The audit covered repository history, open/closed pull requests and issues, releases, branch protection, GitHub Actions, Rust crates and tests, schemas/specifications, the Python package, Studio/Cloud/Ecosystem scaffolding, tracked artifacts, environment files, and website claims.

The compiler implementation and CI repairs described below were delivered incrementally through the GEMM, MLIR/ABI, capability, cost-model, and partitioner changes merged before this branch. The executable awenBLAS library described below is implemented on `codex/awenblas-kernel-library`. None of those changes are statements about the earlier release tag.

## Repository and product boundary

The repository calls itself a monorepo but has no root Cargo workspace, JavaScript workspace, common build entry point, or complete root documentation on `main`. Before this branch, the root `README.md` contained only a heading and the word `website`. Major directories are independent scaffolds or packages:

- `awen-runtime` is the only substantive compiled product. It is one Rust package with a CLI, legacy graph engine, HALs, schedulers, calibration/control, simulators, observability, artifacts, plugins, gradients, and quantum experiments.
- `awen-spec` contains extensive specifications and phase documents, but several normative files remain draft/skeleton text and many AEPs still contain TODOs.
- `awen-ecosystem` contains a thin Python CLI wrapper, an example PDK, a placeholder kernel directory, marketplace/template scaffolding, and no executable optical kernel library.
- `awen-studio` is a minimal React scaffold. Its README explicitly says that UI and Tauri bindings are not implemented.
- `awen-cloud` is a README-only enterprise scaffold.
- Root phase reports repeatedly describe milestones as complete even though source files and normative documents still label central functionality as stubs, placeholders, skeletons, or TODOs.

The accurate product description is therefore an experimental runtime/specification foundation with a new narrow compiler slice on this branch. It is not yet a CUDA-equivalent platform, shipping accelerator stack, mature Studio/Cloud product, or validated performance product.

## Runtime execution model

`awen-runtime/src/ir/mod.rs` defines the executable legacy IR as a `Graph` of `Node` records. A node has an ID, arbitrary string type, numeric parameter map, optional measurement mode, and optional conditional branches. Edges carry optional ports and delay. Validation only checks that conditional-branch node references exist. It does not validate edge endpoints, shapes, layouts, tensor types, dimensions, precision, topology legality, or backend capability.

`awen-runtime/src/engine/mod.rs` is explicitly labeled an engine skeleton. `Engine::run_graph` combines classical reference simulation, simplified quantum-state initialization/evolution, measurement branching, artifact output, and observability in one function. It assumes one quantum mode per graph node and a fixed 10 ms coherence window. Classical and quantum behavior are selected by string node names and numeric parameters. This is useful conformance/prototyping infrastructure but is not a tensor compiler or production heterogeneous execution engine.

`awen-runtime/src/engine_v2.rs` contains a second engine path with more structured planning and validation. The simultaneous presence of `engine`, `engine_v2`, `hal`, `hal_v0`, `scheduler`, `scheduler_v0`, `control`, and `control_v0` creates version/ownership ambiguity. A migration/deprecation plan is not enforced by the crate API.

The non-bypassable gateway in `awen-runtime/src/chokepoint.rs` embeds the V5 JSON Schema, injects or loads basic calibration, writes telemetry/artifacts, and optionally invokes a signed plugin. It then converts the generic operation back into a one-node legacy graph by retaining only numeric parameters. This destroys typed tensor, layout, precision, and rich operation semantics that a compiler backend would require.

## IR and schema drift

There were three incompatible representations on `main`:

1. The executable Rust `Graph` uses `nodes` and string types such as `MZI`, `RING`, and `DETECTOR`.
2. `awen-spec/schemas/awen_ir.proto` sketches nodes, ports, numeric parameters, and edges but is not generated or consumed by the runtime.
3. `awen-spec/schemas/photonic_ir.v5.json` requires top-level `ops` with string types such as `classical:splitter` or `quantum:beam_splitter`.

The V5 schema does not encode tensor shapes, dynamic dimensions, dtype, layout, optical effective precision, bit slicing, accumulation precision, wavelength allocation, matrix-core dimensions, SNR, ADC/DAC resolution, detector dynamic range, reconfiguration cost, insertion-loss budget, topology, or a typed distinction between classical and quantum semantics.

The Python wrapper mutates legacy `nodes`, while its description implies a bridge to broader framework IR. That confirms API drift rather than compatibility.

The compiler adds independently versioned bootstrap schemas for `awen.tensor.v1`, `awen.device-capability.v1`, `awen.photonic.classical.v1`, and `awen.device.v1`. The implemented `awen-mlir` path registers Tensor, Classical Photonic, Quantum Photonic, and Device dialects, normalizes supported StableHLO rank-two `dot_general`, lowers GEMM into Device IR, emits `AWENEXE`, and is consumed directly by the Rust runtime. AEP-0010 still requires production framework paths to use MLIR/StableHLO rather than growing the JSON bootstrap into a competing general-purpose infrastructure. Complete classical/quantum semantic separation remains #16.

## Simulator and numerical semantics

`awen-runtime/src/plugins/reference_sim.rs` is a sequential complex-amplitude simulator for a small set of component strings. It does not execute tensors or tiled matrix operations. The graph engine initializes amplitude from string metadata and applies simplified component transforms/noise. It is valuable for runtime/state/observability testing but cannot validate a GEMM compiler path.

The new `awen-compiler` reference simulator executes the emitted GEMM tiles, including M/N/K edge tiles, row-major/column-major input indexing, transposes, block quantization at advertised effective precision, digital accumulation, and an explicit measured-transfer/inverse-calibration step. It compares logical row-major outputs against `awenBLAS` reference GEMM using per-element absolute-plus-relative tolerance. It remains a conformance simulator: it does not model per-cell phase/loss, shot/thermal noise, disabled elements, nonlinear transfer curves, or measured hardware timing/energy. Those requirements are #13, #14, and #15.

## Hardware abstraction and capabilities

The existing HAL code has useful device discovery, measurement, calibration, health, resource, safety, and simulator concepts. It is not connected to a compiler capability model that can answer whether a particular shaped/typed GEMM is legal or advantageous.

The new compiler capability contract includes matrix-core M/N/K size, supported dtypes and complex mode, wavelengths, modulation/sample rates, coherence mode, ADC/DAC/effective bits, reconfiguration latency, detector bandwidth, insertion-loss budget, simultaneous channels, accumulation modes, calibration requirements/profile, host bandwidth/boundary latency, laser power, and conversion energy. The included 2×2 and PACE-like 128×128 profiles are reference simulator inputs only. They are not assertions about Lightelligence or any shipping device.

Capability discovery, live health negotiation, runtime/plugin ABI versioning, calibration freshness, resource availability, partial-tile legality, complex-mode consistency, typed fallback reasons, and schema/Rust conformance are implemented under #8. Per-cell fault remapping and drift-triggered recompilation remain part of #14.

## Scheduling, partitioning, and cost

The runtime has extensive scheduler types/tests for dependencies, resources, coherence deadlines, conditional feedback, deterministic replay, and multiple strategy names. Several tests and phase documents explicitly identify greedy, optimal, and hardware-aware strategies as placeholders.

Before this branch, no scheduler/partitioner modeled CPU/GPU/photonics placement, tensor residency, DAC/ADC conversion edges, host transfer, or region fusion.

The compiler now records CPU, GPU, and photonic latency, energy, throughput, and
effective-bit estimates. Its full-system cost model includes provenance,
uncertainty, observations, calibration fitting, batching, queueing, overlap,
residency, and deterministic autotuning. It fails forced photonic compilation
when precision, capability, health, calibration, or cost-model requirements
cannot be met.

The crossing-aware partitioner optimizes complete acyclic tensor graphs rather
than selecting isolated GEMMs. It accounts for deduplicated tensor transfers,
shared-operand fan-out, optical/electrical boundaries, required output
residency, fusion barriers, and peak CPU/GPU/photonic memory. Compilation
artifacts expose ranked assignments, fused regions, transfers, crossings,
memory peaks, profiler events, visualization edges, local-versus-global
rationales, and a deterministic request fingerprint. The implementation and
contracts are specified by AEP-0013 and AEP-0014.

## Calibration and control

Calibration is one of the strongest conceptual parts of AWEN. The runtime models calibration kernels/states, drift detection, safety constraints, optimizers, provenance/versioning, validity windows, measurement/control concepts, and integration tests.

Several implementations remain deliberately simplified. The Nelder-Mead path was a random perturbation loop; cost functions and drift/measurement behavior include mock/reference formulas; `control/mod.rs` is explicitly a stub; and compile-time mapping did not consume measured transfer functions.

The new compiler requires a profile when a backend declares calibration mandatory, records the profile in Photonic/Device IR, and makes its scalar transfer function part of executable simulator semantics. It does not yet validate profile age/environment/device identity or remap per-cell faults. Full calibration-aware compilation and drift-triggered recompilation are #14.

## Kernels and framework integration

`awen-ecosystem/kernels/datacom/README.md` contains a TODO and no kernel implementation. There was no `awenBLAS`, GEMM tiler, convolution/FFT library, or transformer kernel set on `main`.

The original Python package ran `awenctl` as a subprocess, searched the working directory for the newest `awen_run_*` or `awen_grad_*` directory, and read JSON artifacts. Its smoke test referenced an `example_ir.json` that was not present under `awen-ecosystem/python_awen`. The original PyTorch wrapper was not a `torch.autograd.Function` subclass, returned a manually annotated tensor, exposed a separate manual backward helper, and used one finite-difference CLI invocation.

The framework-integration branch replaces that normal path with `awen.framework-runtime.v1`: live NumPy, PyTorch, or JAX tensors; explicit ownership/device/stream records; synchronous and asynchronous execution; typed exceptions; profiler events; numerical contracts; deterministic, fingerprinted plans; and strict serialized replay. A real PyTorch custom compiler backend captures matrix multiplication and linear FX nodes, leaves unsupported operations as diagnosed eager regions, preserves TorchDynamo's dynamic guards, and keeps analytic PyTorch autograd. JAX uses the portable `jax.export` StableHLO API with symbolic shapes, serialization/deserialization, and exported analytic value-and-gradient programs. NumPy supports strided, batched, complex, FFT, `out`, and async calls. Rust now builds static/shared libraries with a checked, panic-contained C ABI and C++20 `std::span` wrapper. The obsolete CLI bridge remains only under explicit `*_cli_debug` names and is not exported from the package root.

The compiler now includes the original tiled-GEMM reference plus a separate executable `awen.blas.v1` registry with 22 kinds: GEMM, batched GEMM, complex GEMM, linear, transformer Q/K/V, attention scores, attention-value multiplication, MLP projection, one-dimensional convolution and correlation, DFT, FFT, Fourier filtering, low-rank multiplication, deterministic random projection, Toeplitz, circulant, block-circulant, beamforming, RF FIR, reservoir step, and propagation. Every kind has validated shape/layout/dtype/accuracy/accumulation/calibration/structure metadata, a CPU reference, deterministic simulator, capability/cost selection, diagnosed CPU fallback, and measured software-conformance execution. Complex and Fourier operations use explicit phase conventions; structured operators are never silently densified during selection. Runtime `kernel`, `kernel-plan`, and `kernel-benchmark` commands expose the versioned request, result, plan, and evidence contracts. Framework-native execution now has the AEP-0016 interfaces described above. Wiring captured framework regions all the way through compiler partitioning to live photonic Device IR execution remains separate from claiming that the current semantic runtime uses hardware.

## Artifacts, observability, and reproducibility

The runtime has real implementation and test coverage for deterministic identifiers, bundles, import/export, checksums, environment capture, citations, lineages, traces, timelines, metrics, and calibration/quantum artifacts. This is useful foundation for compiled executable provenance.

Gaps remain: the legacy engine still contains a TODO to persist the complete artifact bundle, some observability exporters are TODO or `todo!`, parent-span tracking is unfinished, and several phase documents describe intended behavior more completely than the source implements it.

Compiler artifacts record source/artifact versions, backend, compile options, placement and full-system cost decisions, model provenance and uncertainty, boundary crossings, typed Photonic IR, Device IR, calibration identity, diagnostics, and deterministic fingerprints. The MLIR path emits a versioned binary package consumed directly by the runtime. awenBLAS result, selection-plan, and benchmark artifacts separately record kernel descriptors, every backend candidate, fallback rationale, measurement boundary, numerical error, and output checksums. Hardware-in-the-loop raw data and validated physical-device performance remain #15.

## Plugins, PDKs, and physical design

The runtime has plugin manifest discovery, signature verification, capability lookup by string, and subprocess invocation. The reference simulator plugin README still says implementation/registration is TODO. The example PDK is a small YAML scaffold with a TODO. AWEN should not recreate layout, DRC, PDK, or electromagnetic/circuit simulation infrastructure.

The gdsfactory/circulax integration boundary, immutable PDK/model provenance, process-corner invalidation, and safe handling of proprietary PDK data are #17.

## Quantum scope

The runtime and specs contain substantial quantum/coherence types and tests, including Gaussian/CV/DV state experiments, measurement, fidelity/drift, artifacts, and feedback scheduling. The correctness model is still intermixed with classical generic graph/string operations and simplified reference simulation.

Classical analog GEMM and quantum photonic programs require distinct dialect verifiers, state/measurement semantics, precision/error models, schedulers, and conformance. Shared runtime/artifact/device infrastructure can remain common. This separation is #16. Strawberry Fields is useful historical prior art but was archived and must not become a new critical dependency.

## CI, release, and branch protection

On `main`, `.github/workflows/observability-quality-gate.yml` contained the complete workflow twice. GitHub recorded every run of that workflow as `failure` with zero jobs and no logs, including runs on release commit `568bac9`. A second file, `observability-quality-gate-2.yml`, duplicated the intended job again. The repository also has overlapping `ci.yml`, runtime CI, and multiple large conformance workflows.

Issue #4 incorrectly stated that the observability quality gate ran green. The release and tag exist, PRs #1/#2 are merged, and cleanup commit `97385cd` exists, but the green-gate claim was corrected in a closing comment with Actions evidence.

This branch replaces the invalid duplicates with one stable job/context, `AWEN required quality gate`, covering compiler/runtime format, strict Clippy, tests, JSON parsing, and Python syntax. Issue #3 was resolved by applying verified `main` protection with strict branch updates, administrator enforcement, the corrected required context, no force pushes, and no deletions.

Full CI consolidation, release truth/evidence, and policy hardening remain #18.

## Test evidence

The branch was verified on a current Linux Rust toolchain through WSL, matching GitHub Actions more closely than an unconfigured Windows MSVC host.

- `awen-compiler`: format check, strict Clippy, 52 unit/integration tests, and doc tests passed.
- Compiler cases cover 256³-to-eight-tile lowering, calibrated execution, precision rejection, conversion-aware auto placement, invalid output shape, rectangular/partial M/N/K tiles, transposed operands, column-major storage, all 22 awenBLAS kinds, exact complex/Fourier conventions, randomized GEMM/FFT properties, calibration composition, structure preservation, deterministic simulation/planning, capacity/precision/complex rejection, and measured software-conformance execution.
- `awen-runtime`: format and strict Clippy pass, the existing ordinary unit/integration/doc cases pass, and four new awenBLAS schema/CLI integration tests pass. One pre-existing phase-calibration test remains explicitly ignored. The pre-existing trybuild diagnostic snapshot is sensitive to the local WSL terminal width under Rust 1.97 and is left unchanged for the canonical GitHub Actions environment.
- End-to-end CLI compile emitted eight photonic tiles, 42 device commands, two boundary crossings, and a calibration identity for the 256³ fixture.
- End-to-end CLI benchmark emitted 16 output values and passed its numerical contract; observed maximum absolute error was approximately `0.110236` and maximum relative error approximately `0.007874` for the 8-effective-bit 4×4 fixture.
- All five awenBLAS schemas compile, the request and two backend fixtures validate, and generated CPU result, simulated result, selection plan, and benchmark records validate against their published schemas.
- End-to-end `awenctl kernel`, `kernel-plan`, and `kernel-benchmark` executions produced a three-output transformer Q/K/V result, selected the explicit simulated photonic profile under the latency objective, and passed the declared numerical contract over the complete software measurement boundary.
- The Python framework suite passes against exact PyTorch 2.13.0 and JAX/JAXlib 0.11.0, including installed backend discovery, mixed FX regions, analytic input/weight gradients, dynamic TorchDynamo batches, non-contiguous and batched tensors, portable StableHLO serialization, symbolic JAX batches, and analytic exported gradients.
- NumPy/runtime tests cover strided/batched/complex operations, FFT, caller-owned output buffers, asynchronous futures, buffer ownership, CPU transfer, typed failures, effective-bit rejection, profiler events, deterministic serialization/replay, tamper detection, and the prohibition on subprocess use in the public path.
- Both framework JSON schemas pass Draft 2020-12 meta-validation and generated plans/traces validate. The Rust C ABI unit test passes, and the C++20 example compiles against the generated shared library and returns the expected `19 22 43 50` GEMM result.

These are software conformance results, not hardware performance evidence.

## Repository hygiene and cross-repository findings

The main repository tracks `awen-runtime/libcontrol_v0.rlib`, a generated compiled binary. Its runtime and spec license files contain abbreviated placeholder text instead of complete license terms. Governance, maintainer, contribution, security contact, plugin, PDK, cloud, Studio, marketplace, and multiple AEP documents retain TODO/scaffold language. These are tracked comprehensively in #18 rather than being hidden by phase sign-off documents.

`marcpoliquin5/awen-labs` is primarily a Vite/React site and tracks a 350-byte `.env` containing a Supabase project ID, publishable key, and URL. No value is reproduced in this audit. A publishable browser key is not equivalent to a service-role secret, but the environment file must be removed, privileged history audited, RLS verified, configuration reissued where appropriate, and secret scanning added. That work is `marcpoliquin5/awen-labs#1`.

`marcpoliquin5/awenphotonics.github.io` contains unsupported or unverified product/performance/customer statements, including 50–100× performance, 100× lower energy, 100× faster training, 100× molecular-dynamics acceleration, 12× arbitrage, 8× climate resolution, paid product prices, and strongly characterized research citations. Every retained claim needs direct auditable support or removal. That work is `marcpoliquin5/awenphotonics.github.io#1`.

## Roadmap ownership

- #5: compiler/runtime product epic and definition of done.
- #6: StableHLO/MLIR dialects, lowering, and executable ABI (completed).
- #7: complete first GEMM vertical slice (completed).
- #8: backend capabilities, discovery, health, and conformance (completed).
- #9: crossing-aware graph partitioner and tensor residency (completed).
- #10: measured cost model, uncertainty, and autotuning (completed).
- #11: `awenBLAS`, FFT, convolution/correlation, structured transforms, attention, RF, and reservoir kernels (completed).
- #12: `torch.compile`, JAX, C++, NumPy, buffers/streams/async/autograd (implemented on this branch).
- #13: analog/mixed precision, bit slicing, scaling, saturation, and error attribution.
- #14: calibration-aware mapping, fault remapping, and drift-triggered recompilation.
- #15: hardware-in-the-loop full-system benchmarking and claim generation.
- #16: separate classical and quantum-photonic dialects.
- #17: gdsfactory/circulax and physical-design/PDK boundary.
- #18: repository/product hygiene, licensing, CI consolidation, governance, security, and release truth.
- `awen-labs#1`: tracked environment and Supabase/security cleanup.
- `awenphotonics.github.io#1`: unsupported claims and citation/product integrity.
