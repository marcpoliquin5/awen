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
| P0.1 | `docs/AUDIT.md` truth table and baseline | implemented | this PR (Phase 0 docs) | doc lists every component with evidence | |
| P0.2 | `docs/ROADMAP.md` dependency-aware with acceptance tests and hardware needs | implemented | this PR (Phase 0 docs) | every milestone has a CI acceptance test | |
| P0.3 | Labeled issues for missing modules | implemented | issues #55–#79 | one open issue per missing module | |
| P0.4 | `docs/EVIDENCE.md` with exact commands and results | proposed | | | |

## Models

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| M1 | `awen-models` installable package | implemented | this PR | `pip install ./awen-models` + pytest | |
| M2 | AWEN-Mini ~1M-param char LM trained for real on Tiny Shakespeare (public domain), checkpoint, generate, int8, device=auto | implemented (val loss 1.4795 nats at step 5,750, measured on CPU) | this PR | bundled checkpoint reproduces recorded val loss | |
| M3 | AWEN-Nonlinear candidates vs matched baseline, ≥2 seeds, Pareto plot | implemented (3 candidates, 2 seeds; low-rank is a negative result) | this PR | harness test; committed results.json | |
| M4 | External HF adapter with capability report (trust_remote_code=False) | implemented (SmolLM2-135M report committed) | this PR | tiny local Llama test | Full SmolLM2-135M run is a manual demo (270 MB download) |
| M5 | Photonic execution demo + `Runtime(device='photonic-sim')` | implemented (simulated; ideal contract matches CPU exactly) | this PR | per-layer error vs CPU oracle within contract | needs #59 |
| M6 | AWEN-Mini fine-tune/distill scripts (Qwen3-0.6B/1.7B, SmolLM2; QLoRA for 12 GB RTX 5070 Ti, cu128+) | implemented (LoRA + KD measured on CPU with SmolLM2-135M; QLoRA path needs CUDA) | this PR | `tests/test_finetune.py` (tiny Llama: LoRA lowers loss, exact merge, KD both arms); committed `results/finetune/` | QLoRA and Qwen3 runs need the user's GPU |
| M7 | AWEN-Photon noise-injection training with accuracy-vs-ENOB and accuracy-vs-optical-energy curves | proposed | | toy MNIST/MLP run committed | #73 |
| M8 | Cost/compute plan doc | implemented (`docs/COMPUTE_PLAN.md`; estimates labeled) | this PR | parameter counts equal instantiated Llama/Qwen3 models | memory model not yet checked on a GPU |

## Truth layer and physics

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| T1 | Physical-unit precision/noise contracts (schema, Rust types, docs) | numerically validated | #80 | round-trip + rejection tests | #56 |
| T2 | Noise model: shot, thermal, RIN, phase, ADC/DAC, insertion loss, crosstalk, drift | numerically validated (Monte Carlo vs closed form) | #80 | Monte Carlo vs analytic variance | values need measured devices; #57 |
| T3 | Notebook (a): slab waveguide TE modes vs analytic dispersion | proposed | | stated tolerance in pytest | #58 |
| T4 | Notebook (b): MZI mesh vs Clements 2016 | proposed | | unitarity + reconstruction error | #58 |
| T5 | Notebook (c): shot-noise-limited ENOB vs theory | proposed | | stated tolerance | #58 |
| T6 | awen-physics: TMM/S-matrix, mode solver, 1D/2D FDTD + convergence, BPM, dispersion, ring/cavity, thermo/electro-optic, chi2 SHG, adjoint check, gdsfactory/Meep/Circulax adapters | proposed | | analytic comparisons | fabricated devices; #68 |
| T7 | Quantum-photonic track (Fock/Gaussian, permanents, boson sampling, detection, resource model, small kernel vs classical baseline) | proposed | | permanent vs brute force | single-photon hardware; #69 |
| T8 | Memory subsystem (cited parameters, simulator, KV-cache study) | proposed | | | #70 |
| T9 | Fabric simulator, optical vs electrical, labeled modeled | proposed | | analytic queueing checks | #71 |

## Compute path and standards

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| C1 | PyO3/maturin binding to compiler/simulator | implemented | #84 | wheel builds in CI; numeric match | #59 |
| C2 | `awenctl execute` runs math with noise model | implemented | #82 | AWENEXE result vs NumPy | #59 |
| C3 | `torch.compile(backend=awen)` routes GEMMs through Rust | implemented | #84 | call assertion + numeric match | #59 |
| C4 | PyTorch PrivateUse1 `awen` device | proposed | | op tests vs CPU | #60 |
| C5 | OpenXLA PJRT plugin for dot/dot_general | proposed | | JAX dot vs CPU | #61 |
| C6 | Virtual Photonic ISA + ABI, codec, validator, quantum domain | proposed | | round-trip + negative tests | #55 |
| C7 | Compiler ops beyond GEMM with 4-level labels + ONNX import + compatibility report | proposed | | per-op oracle tests | hardware levels; #62, #63 |
| C8 | Runtime extensions (discovery, streams/events, multi-device, residency, leases, remap, fallback, deterministic mode, recording, profiling, plugin isolation) | proposed | | | #64 |
| C9 | awenBLAS extensions | proposed | | per-kernel oracle | #65 |
| C10 | SDKs: Python, C/C++ header + example, CLI tools, conversion utility, compatibility metadata | proposed | | | #66 |
| C11 | Vendor-neutral hardware capability schemas, reference simulator drivers, vendor integration guide | proposed | | schema tests | vendor SDKs; #67 |

## Research, tooling, efficiency, security

| # | Item | Status | PR | Acceptance test | Blocked boundary |
| --- | --- | --- | --- | --- | --- |
| R1 | Research ledger schema + import script; 15 directions as unverified stubs | proposed | | schema validation | 72-paper ledger file unavailable; #77 |
| R2 | Research-to-code pipeline | proposed | | | #77 |
| R3 | Frontier-physics hypotheses (Casimir, toroidal/fractal optics, information capacity) with falsification criteria | proposed | | | #77 |
| E1 | Efficiency program ablations + Pareto (energy estimated) | proposed | | | #72 |
| E2 | AWEN-Photon differentiable MZI layers vs independent reference; six nonlinearity categories | proposed | | | #73 |
| B1 | Benchmarks with immutable evidence, CIs, multiple seeds, LM metrics | proposed | | | #74 |
| S1 | Studio local web app wired to real backends | proposed | | | #75 |
| S2 | Lab Agent (simulation only, approval gates) | proposed | | | hardware actions disabled; #76 |
| S3 | Cloud architecture doc, marked not available | proposed | | | no service |
| X1a | SECURITY.md, ed25519 plugin-manifest signing, dependency license scan, gitleaks | implemented | before #50 | `scripts/check_dependency_licenses.py`, plugin signature tests | |
| X1b | SBOM, release checksums, reproducible builds, safe model loading, compatibility docs | proposed | | | #78 |
| X2 | Release automation with real artifacts; v0.1.0 | proposed | | release workflow attaches wheels/binaries | #78 |
