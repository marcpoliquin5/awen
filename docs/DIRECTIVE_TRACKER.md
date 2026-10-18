# Directive tracker

Every item from the October 2026 directive, with its current status, the pull
request that delivered it, and its acceptance test. Updated in each pull
request that changes an item.

Status vocabulary: **proposed** → **under investigation** → **implemented** →
**numerically validated** (passes analytic/reference checks in CI) →
**experimentally validated** (compared with measured lab data) → **hardware
validated** (runs on photonic hardware) → **released** (in a tagged GitHub
Release). Nothing here is experimentally or hardware validated: no lab data or
photonic hardware is available to this project.

Items that need unavailable hardware, credentials, datasets or files have
their testable portion implemented and the blocked boundary written down.

## Phase 0 and foundation

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| F1 | Pinned toolchain, weekly gate, caching, MSRV 1.88 | implemented | #51 | gate green; policy script checks pin and MSRV | |
| F2 | PyTorch 2.14 support (supersedes Dependabot #49) | implemented | #52 | pytest on torch 2.10.0 and 2.14.1 | |
| F3 | Dependabot backlog resolved | implemented | #51, #53 | obsolete PRs closed with reasons; updates merged green | |
| F4 | Dead code and mock-only tests removed; loader/CLI/HAL coverage raised | implemented | #54 | runtime coverage 81.2% → 85.6% | |
| F5 | jsonschema upgrade | proposed | | schema tests pass on current jsonschema | #79 |
| P0.1 | `docs/AUDIT.md` truth table and baseline | implemented | #81, #90 | doc lists every component with evidence | |
| P0.2 | `docs/ROADMAP.md` dependency-aware with acceptance tests and hardware needs | implemented | #81 | every milestone has a CI acceptance test | |
| P0.3 | Labeled issues for missing modules | implemented | issues #55–#79 | one open issue per missing module | |
| P0.4 | `docs/EVIDENCE.md` with exact commands and results | proposed | | | |

## Models

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| M1 | `awen-models` installable package | implemented | #83 | `pip install ./awen-models` + pytest | |
| M2 | AWEN-Mini ~1M-param char LM trained for real on Tiny Shakespeare (public domain), checkpoint, generate, int8, device=auto | implemented (val loss 1.4795 nats at step 5,750, measured on CPU) | #83 | bundled checkpoint reproduces recorded val loss | |
| M3 | AWEN-Nonlinear candidates vs matched baseline, ≥2 seeds, Pareto plot | implemented (3 candidates, 2 seeds; low-rank is a negative result) | #83 | harness test; committed results.json | |
| M4 | External HF adapter with capability report (trust_remote_code=False) | implemented (SmolLM2-135M report committed) | #83 | tiny local Llama test | Full SmolLM2-135M run is a manual demo (270 MB download) |
| M5 | Photonic execution demo + `Runtime(device='photonic-sim')` | implemented (simulated; ideal contract matches CPU exactly) | #85 | per-layer error vs CPU oracle within contract | |
| M6 | AWEN-Mini fine-tune/distill scripts (Qwen3-0.6B/1.7B, SmolLM2; QLoRA for 12 GB RTX 5070 Ti, cu128+) | implemented (LoRA + KD measured on CPU with SmolLM2-135M; QLoRA path needs CUDA) | #91 | `tests/test_finetune.py` (tiny Llama: LoRA lowers loss, exact merge, KD both arms); committed `results/finetune/` | QLoRA and Qwen3 runs need the user's GPU |
| M7 | AWEN-Photon noise-injection training with accuracy-vs-ENOB and accuracy-vs-optical-energy curves | implemented (MNIST MLP, 3 regimes × 2 seeds, evaluated on the Rust noise model; simulated) | #88 | `tests/test_photon_training.py`: surrogate vs Rust error std within 5%; harness end to end; committed `results/photon-training/results.json` | differentiable MZI-mesh layers (E2) still open; #73 |
| M8 | Cost/compute plan doc | implemented (`docs/COMPUTE_PLAN.md`; estimates labeled) | #91 | parameter counts equal instantiated Llama/Qwen3 models | memory model not yet checked on a GPU |

## Truth layer and physics

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| T1 | Physical-unit precision/noise contracts (schema, Rust types, docs) | numerically validated | #80 | round-trip + rejection tests | #56 |
| T2 | Noise model: shot, thermal, RIN, phase, ADC/DAC, insertion loss, crosstalk, drift | numerically validated (Monte Carlo vs closed form) | #80 | Monte Carlo vs analytic variance | values need measured devices; #57 |
| T3 | Notebook (a): slab waveguide TE modes vs analytic dispersion | numerically validated (\|Δn_eff\| 1.1e-6 / 4.7e-6 vs tolerance 1e-5; FD convergence order 2.00) | #86 | `awen-physics/tests/test_notebooks.py` runs every cell | |
| T4 | Notebook (b): MZI mesh vs Clements 2016 | numerically validated (reconstruction/unitarity error ≈ 5e-16 for N ≤ 32, tolerance 1e-12) | #86 | `awen-physics/tests/test_notebooks.py` | |
| T5 | Notebook (c): shot-noise-limited ENOB vs theory | numerically validated (Rust model within 0.05 bit of IEEE 1241 theory over 4 decades, tolerance 0.1 bit) | #86 | `awen-physics/tests/test_notebooks.py` | |
| T6 | awen-physics: TMM/S-matrix, mode solver, 1D/2D FDTD + convergence, BPM, dispersion, ring/cavity, thermo/electro-optic, chi2 SHG, adjoint check, gdsfactory/Meep/Circulax adapters | partial: TMM + S-matrix, microrings, 1-D FDTD (2nd order vs TMM), 2-D paraxial BPM, χ⁽²⁾ SHG (undepleted/depleted/QPM), 1-D FDFD adjoint gradient check done; 2-D FDTD, dispersion models, thermo/electro-optic, external adapters open | #94, #95, #96 | `awen-physics/tests/test_tmm_rings.py` (21), `test_fdtd_bpm.py` (7), `test_shg_adjoint.py` (13) | fabricated devices; #68 |
| T7 | Quantum-photonic track (Fock/Gaussian, permanents, boson sampling, detection, resource model, small kernel vs classical baseline) | numerically validated (simulation only): permanents (Ryser), Fock linear optics, small-n boson sampling, HOM dip, photon counting with loss/efficiency/dark counts, Gaussian covariance formalism with homodyne, resource model, quantum kernel vs RBF (photonic kernel worse than RBF on moons 0.832 vs 0.993 and circles 0.718 vs 0.927; no advantage) | #97, #98, #99 | `awen-qphotonic/tests/` (61: Ryser vs brute force n<=6, amplitudes vs polynomial oracle, HOM vs closed form, loss vs environment-mode oracle, Gaussian vs analytic and truncated Fock, resource Monte Carlo and TVD bound, kernel PSD, coherent kernel = RBF) | single-photon sources/detectors; quantum VPISA execution not wired yet; #69 |
| T8 | Memory subsystem (cited parameters, simulator, KV-cache study) | proposed | | | #70 |
| T9 | Fabric simulator, optical vs electrical, labeled modeled | proposed | | analytic queueing checks | #71 |

## Compute path and standards

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| C1 | PyO3/maturin binding to compiler/simulator | implemented | #84 | wheel builds in CI; numeric match | #59 |
| C2 | `awenctl execute` runs math with noise model | implemented | #82 | AWENEXE result vs NumPy | #59 |
| C3 | `torch.compile(backend=awen)` routes GEMMs through Rust | implemented | #84 | call assertion + numeric match | #59 |
| C4 | PyTorch PrivateUse1 `awen` device | implemented (inference: mm/addmm/bmm on the Rust photonic model, add/relu electronic; simulated device, host memory) | #87 | 6 op tests vs CPU within 1e-5 under the ideal contract | backward/reductions not implemented; no hardware driver |
| C5 | OpenXLA PJRT plugin for dot/dot_general | partial: JAX-lowered StableHLO executes on AWEN (dot_general on the f64 reference or the photonic noise model, simulated; supporting shape/elementwise ops electronic; everything else refused by name). Not a PJRT plugin yet: boundary in `docs/PJRT_BOUNDARY.md` | #100 | `awen-ecosystem/python_awen/tests/test_stablehlo_exec.py` (8: jnp.dot, batched einsum, transposed matmul + bias, dense + ReLU vs JAX CPU rtol 1e-5; ideal photonic-sim matches; unsupported ops refused) | PJRT C API needs MLIR/StableHLO bytecode deserialization (StableHLO not built here); #61 |
| C6 | Virtual Photonic ISA + ABI, codec, validator, quantum domain | implemented (VPISA v1; classical domain executes on the noise model, quantum domain validated but not executed) | #89 | `awen-compiler/tests/isa.rs`: 2,000-program byte round trip, all truncations rejected, per-rule negative tests, domain separation, decomposed = fused GEMM | quantum execution needs the T7 simulator; #55 |
| C7 | Compiler ops beyond GEMM with 4-level labels + ONNX import + compatibility report | implemented (12-op library: gemm hardware_lowerable, 11 photonic_sim, none measured; minimal ONNX import: MatMul/Gemm/Conv + 13 electronic ops, report + execution) | #92, #93 | `awen-compiler/tests/ops.rs` (7), `awen-native/tests/test_ops.py` (15, NumPy oracles), `awen-models/tests/test_onnx_import.py` (7, onnx ReferenceEvaluator oracle) | hardware levels need a device; ONNX LayerNorm/Gather/attention fusions not mapped yet |
| C8 | Runtime extensions (discovery, streams/events, multi-device, residency, leases, remap, fallback, deterministic mode, recording, profiling, plugin isolation) | partial (`awen_runtime::devrt`: ordered streams + events with sticky errors, cross-device event dependencies on two simulated devices, memory residency with LRU eviction/pinning/transfer accounting, leases with logical-clock expiry and operation/energy/wall budgets; health-aware remap with CPU fallback, SplitMix64 deterministic seeding, experiment records with SHA-256 request/output digests and bit-exact replay check, wall-clock profiler with Chrome trace export) | #101, #102 | `awen-runtime/tests/devrt_streams_residency_leases.rs` (5), `devrt_remap_record_profile.rs` (5) | real multi-device and device-side timing need hardware; plugin out-of-process isolation not started; #64 |
| C9 | awenBLAS extensions | implemented (25 kinds: added `batched_complex_gemm`, `scaled_dot_product_attention` (softmax electronic), `feed_forward` (tanh-GELU electronic); independent oracles for complex/batched/batched-complex GEMM, FFT, low-rank, reservoir, attention, FFN; reference + simulator only, none hardware-measured) | #103 | `awen-compiler/tests/awenblas_oracles.rs` (8): real block embedding, per-batch naive, naive DFT + Parseval + inverse, explicit U·Vᵀ, leaky-tanh formula, softmax oracle + `ops::attention`/`ops::ffn` cross-checks, simulator error vs noise | causal masking not in the kernel request yet; hardware evidence needs a device; #65 |
| C10 | SDKs: Python, C/C++ header + example, CLI tools, conversion utility, compatibility metadata | partial (C SDK: `include/awen/awen.h` kernel ABI `awen.kernel-c.v1` with kind listing, interleaved complex GEMM and JSON kernel execution on the CPU reference or simulator; C11 example compiled with `-Werror -pedantic` and run in CI; CLI: `awenctl caps` capability explorer for device capabilities, plugin manifests, hardware descriptions and awenBLAS backend profiles; `awenctl profile` host wall-clock kernel profiler with Chrome trace; Python SDK from #84/#85) | #104, #107 | `awen-runtime/tests/ffi_kernels.rs` (3) + CI step checking the C example output; `awen-runtime/tests/cli_tools.rs` (2) | conversion utility in #106; #66 |
| C11 | Vendor-neutral hardware capability schemas, reference simulator drivers, vendor integration guide | implemented (`awen.hardware-description.v1`: mzi_mesh, microring_bank, wdm, diffractive, detector, adc, dac, laser, reservoir, memory, quantum, with required provenance; reference-simulator fixture covering all 11 classes; reference simulator drivers for Clements MZI mesh, all-pass microring bank and detector+ADC on the existing `Device` trait; `docs/VENDOR_INTEGRATION.md` with plugin ABI and acceptance checklist) | #105 | `scripts/validate_json.py` (fixture) + `awen-runtime/tests/hardware_description.rs` (5): schema accepts fixture and rejects 10 malformed documents, mesh unitarity + power conservation, MZI closed-form splitting with loss, ring all-pass formula + critical coupling, ADC quantization/saturation | real drivers need vendor hardware/SDKs; no vendor drivers written; #67 |

## Research, tooling, efficiency, security

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| R1 | Research ledger schema + import script; 15 directions as unverified stubs | implemented (`research/ledger/`: schema `awen.research-ledger.v1`, CSV/Markdown importer that downgrades unchecked rows to `citation pending`, 15 seed directions each with a Crossref-checked citation) | #PRN | `research/tests/test_ledger.py` (5) | 72-paper ledger file unavailable (import path ready); #77 |
| R2 | Research-to-code pipeline | implemented (`docs/RESEARCH_PIPELINE.md`: 7 stages with exit tests, provenance rules) | #PRN | doc | #77 |
| R3 | Frontier-physics hypotheses (Casimir, toroidal/fractal optics, information capacity) with falsification criteria | implemented (H1–H3 in `research/frontier-physics/` with math, assumptions, observables, method, comparison and falsification; `frontier_checks.py` reproduces Casimir 1948 pressure, zero closed-cycle work, Miller mode count, Sierpinski dimension, anapole cancellation, capacity bound vs simulator ENOB) | #PRN | `research/tests/test_frontier.py` (11) | hypotheses untested against measured devices; #77 |
| E1 | Efficiency program ablations + Pareto (energy estimated) | proposed | | | #72 |
| E2 | AWEN-Photon differentiable MZI layers vs independent reference; six nonlinearity categories | proposed | | | #73 |
| B1 | Benchmarks with immutable evidence, CIs, multiple seeds, LM metrics | proposed | | | #74 |
| S1 | Studio local web app wired to real backends | proposed | | | #75 |
| S2 | Lab Agent (simulation only, approval gates) | proposed | | | hardware actions disabled; #76 |
| S3 | Cloud architecture doc, marked not available | proposed | | | no service |
| X1a | SECURITY.md, ed25519 plugin-manifest signing, dependency license scan, gitleaks | implemented | before #50 | `scripts/check_dependency_licenses.py`, plugin signature tests | |
| X1b | SBOM, release checksums, reproducible builds, safe model loading, compatibility docs | proposed | | | #78 |
| X2 | Release automation with real artifacts; v0.1.0 | proposed | | release workflow attaches wheels/binaries | #78 |
