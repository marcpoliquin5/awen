# Evidence for AWEN 0.1.0

This file records how the 0.1.0 state of the repository was checked: the
exact commands, the test counts per package, the measured results with the
hardware they were measured on, what is simulated or estimated, and what is
blocked. Every number here comes from a command in this file or from a
result file committed in the repository; nothing is extrapolated.

AWEN 0.1.0 is **simulator-only**. No result in this repository was measured
on photonic hardware, and no GPU was used.

## Where the evidence was produced

| Item | Value |
| --- | --- |
| Date | October 18, 2026 (times in ET) |
| Tree | `main` plus the open pull requests #114, #115, #118 and #119 merged together at `7bbf779` (the same content that landed on `main` through those pull requests) |
| CPU | Intel Xeon (8 vCPU), 15 GiB RAM, Linux 6.12 x86-64 |
| GPU | none |
| Photonic hardware | none |
| Rust | rustc 1.99.1 (pinned in `rust-toolchain.toml`; MSRV 1.88 checked in CI) |
| Python | 3.13.5 locally; 3.12 in CI and in the release build |
| PyTorch | 2.14.1+cpu |
| LLVM / MLIR | 20 (clang-20) |
| Node.js | 20.19 locally; 22 in CI and in the release build |

The required GitHub check ("AWEN required quality gate") runs the same suites
on every pull request and on `main`; its runs are the authoritative record for
each merged commit.

## Commands and test counts

Run from the repository root with a virtual environment holding the
development dependencies (`pip install -e` of each Python package with
`--no-deps`, plus `pip install ./awen-native`), `OMP_NUM_THREADS=2` and
`HF_HUB_OFFLINE=1`:

| Package / check | Command | Result |
| --- | --- | --- |
| Repository policy | `python scripts/validate_repository.py` | passed: 541 files, 3 workflows |
| Specs and schemas | `python scripts/validate_json.py` | 90 JSON documents parsed, 37 schemas meta-validated, 22 instances valid |
| `awen-compiler` | `cargo test --manifest-path awen-compiler/Cargo.toml --all-features --no-fail-fast` | 129 passed, 0 failed (11 test binaries) |
| `awen-runtime` (+ `awenctl`) | `cargo test --manifest-path awen-runtime/Cargo.toml --all-features --no-fail-fast` | 305 passed, 0 failed (29 test binaries) |
| `awen-native` (PyO3) | `pip install ./awen-native && python -m pytest awen-native/tests -q` | 23 passed |
| `awen_py` | `python -m pytest awen-ecosystem/python_awen/tests -q` | 48 passed |
| `awen-models` (+ `awen-bench`) | `python -m pytest awen-models/tests -q` | 75 passed, 2 skipped (the `awenctl` conversion test skips when the binary is not built; one runtime test skips when the native extension is installed because it checks the error raised when the extension is missing) |
| `awen-physics` | `python -m pytest awen-physics/tests -q` | 51 passed |
| `awen-qphotonic` | `python -m pytest awen-qphotonic/tests -q` | 61 passed |
| `awen-systems` | `python -m pytest awen-systems/tests -q` | 26 passed |
| `awen-lab` | `python -m pytest awen-lab/tests -q` | 11 passed |
| `research` | `python -m pytest research/tests -q` | 16 passed |
| `awen-studio` frontend | `cd awen-studio/frontend && npm ci && npm run build` | built |
| `awen-studio` API + end to end | `AWEN_STUDIO_REQUIRE_FRONTEND=1 python -m pytest awen-studio/tests -q` | 14 passed |
| Repository scripts | `python -m pytest scripts/tests -q` | 10 passed |
| Safe model loading | `python scripts/check_safe_model_loading.py` | passed for 82 shipped Python files |
| Licenses and sources | `cargo deny --manifest-path <crate>/Cargo.toml --config deny.toml check licenses sources` for compiler, runtime, native | licenses ok, sources ok (all three) |
| Vulnerabilities | `cargo audit --file <crate>/Cargo.lock` for compiler, runtime, native | 0 vulnerabilities (27, 172 and 45 crates) |
| MLIR | `cmake -S awen-mlir -B build -G Ninja -DMLIR_DIR=/usr/lib/llvm-20/lib/cmake/mlir -DLLVM_DIR=/usr/lib/llvm-20/lib/cmake/llvm -DCMAKE_C_COMPILER=clang-20 -DCMAKE_CXX_COMPILER=clang++-20 && cmake --build build --target awen-opt awen-compile && ctest --test-dir build` | 1/1 CTest passed (wraps the lit-style cases) |

Totals: 434 Rust tests and 345 Python tests passed, 0 failed.

The C/C++ ABI consumers, the C11 SDK example, the MSRV build, `clippy`,
`rustfmt`, `pip-audit`, the minimum-supported framework test (PyTorch 2.10), the SBOM
generation and gitleaks run only in the required CI gate; they are green on
every merged pull request.

## Release artifact check

```bash
scripts/build_release_artifacts.sh 0.1.0-rc.1 /tmp/rel-rc1
python scripts/verify_release_artifacts.py /tmp/rel-rc1
```

Result on the box: 17 assets plus `SHA256SUMS`, verification passed. Two
independent builds of `awenctl` with `--remap-path-prefix` and
`SOURCE_DATE_EPOCH` produced the identical SHA-256
(`6d67e676035a70ee01d0e5ba505cd59c2c0d7d78b925fff4a070f5a03fe0047c` for that
commit). The verifier checks every checksum, runs `awenctl`, confirms the
AWEN-Mini checkpoint tarball matches the weights in the `awen_models` wheel,
and in a fresh virtual environment compiles a program, runs a photonic-sim
GEMM and generates text with AWEN-Mini on CPU and on photonic-sim. The
published release is built by `.github/workflows/release.yml` on GitHub
runners from the tagged commit, with a build-provenance attestation; its own
`SHA256SUMS` and `BUILD-INFO.json` are attached to the release.

## Measured results

"Measured" here means measured on the CPU listed above. None of these is a
photonic-hardware measurement.

| Result | Value | Source |
| --- | --- | --- |
| AWEN-Mini (1.0M params, char-level, Tiny Shakespeare) | validation loss 1.4795 nats at step 5,750 (training run) | `awen-models` checkpoint metadata; test reproduces it |
| AWEN-Mini fp32 vs int8 | val loss 1.47167 vs 1.47161; 548.7 vs 482.2 tokens/s with 2 torch threads | `awen-models/results/awen-mini-eval.json` |
| AWEN-Nonlinear (2 seeds, matched ~818k params) | baseline GELU MLP 1.7181, gated multiplicative 1.6252, recursive shared block 1.6114, low-rank wide MLP 1.9742 (negative result) | `awen-models/results/nonlinear/results.json` |
| Efficiency ablations (10 variants, seeds 1 and 2) | baseline 1.718, MoE top-1 1.696, tensor-train 1.706, QAT int4 1.730, PTQ int4 1.744, low-rank 1.974; energy only as MACs relative to baseline | `awen-models/results/efficiency/results.json` |
| LoRA on SmolLM2-135M (r=8, 200 steps, CPU) | val loss 3.592 → 3.353 nats/token | `awen-models/results/finetune/lora-SmolLM2-135M.json` |
| Distillation from SmolLM2-135M (7.1M-param student, 300 steps, seed 0) | KD 5.209 vs CE-only 5.426 nats/token | `awen-models/results/finetune/distill-SmolLM2-135M-seed0.json` |
| Benchmarks (`awen-bench`, run 2026-10-18 06:27 ET) | GEMM p50 on CPU 0.014 / 0.022 / 0.121 ms for 64×128×128 / 128×256×256 / 256×512×512; AWEN-Mini perplexity CPU 4.31, AWEN software reference 4.31 | `docs/BENCHMARKS.md` and its evidence record |

## Simulated or estimated

| Result | Label | Source |
| --- | --- | --- |
| Photonic-sim GEMM error | simulated (compact noise model): relative RMS 0.167 / 0.157 / 0.127 for 64×128×128 / 128×256×256 / 256×512×512 under the default contract (1000 photons/MAC, 8-bit DAC/ADC, 6 dB loss); simulator wall-clock is not device speed | `docs/BENCHMARKS.md` |
| AWEN-Mini on photonic-sim | simulated: perplexity 67.6 (photonic-sim) and 45.2 (hybrid) vs 4.31 on CPU under the default contract; with the ideal contract the photonic path matches CPU exactly | `docs/BENCHMARKS.md`, `awen-models/results/photonic-sim-demo.json` |
| AWEN-Photon noise-aware training (MNIST MLP) | simulated: accuracy measured on the MNIST test set, but GEMMs run on the Rust noise model; at 300 photons/MAC standard training 0.357, noise-aware (100) 0.809, noise-aware (1000) 0.570; at 10⁵ photons/MAC 0.968 / 0.869 / 0.954 | `awen-models/results/photon-training/results.json` |
| Optical energy per MAC | estimated: photon energy only (1.28e-18 J at 10 photons/MAC, 1550 nm) | same file |
| Efficiency energy column | estimated: proportional to MACs, no pJ/MAC supplied | `awen-models/results/efficiency/results.json` |
| Quantum kernel vs RBF | simulated: photonic kernel worse than RBF (moons 0.832 vs 0.993, circles 0.718 vs 0.927, mean of 3 seeds); no quantum advantage | `awen-qphotonic/results/kernel_vs_rbf.json` |
| Memory and fabric studies | modeled: only three literature values (cited per value), the rest are stated user parameters | `awen-systems/results/` |
| Physics notebooks | numerically validated against analytic references (slab modes \|Δn_eff\| ≤ 4.7e-6, Clements mesh error ≈ 5e-16, ENOB within 0.05 bit) | `awen-physics/` |
| Cost model, autotuner, awenBLAS photonic paths | compact model / estimated | `awen-compiler`, `awen-runtime` |
| AWEN Studio | a local app; every response is labeled measured, simulated or estimated by the backend it calls | `awen-studio/` |

## Blocked

| Boundary | Why | What exists instead |
| --- | --- | --- |
| Photonic hardware | no device is available to this project | compact noise model, simulated HAL device, hardware description schemas, vendor integration guide; every hardware action in `awen-lab` is refused |
| GPU / CUDA | the build host has no GPU, so every CUDA path is untested | CUDA benchmark rows are skipped with a recorded reason; QLoRA and Qwen3 fine-tune scripts are written and tested on a tiny local Llama on CPU only |
| Research ledger file | the 72-paper ledger file was not available | ledger schema and importer (unchecked rows are downgraded to `citation pending`), 15 seed directions with checked citations (#77) |
| Full OpenXLA PJRT plugin | needs MLIR/StableHLO bytecode deserialization, StableHLO is not built here | JAX-lowered StableHLO executes on AWEN for `dot_general` and supporting ops; boundary in `docs/PJRT_BOUNDARY.md` (#61) |
| Paid-GPU fine-tunes | the directive forbids paid compute | `docs/COMPUTE_PLAN.md` estimates (labeled), CPU-scale LoRA and distillation results above |
| Hosted cloud | not offered | `docs/CLOUD_ARCHITECTURE.md`, design only |

Items still partial or open are listed with their issues in
`docs/DIRECTIVE_TRACKER.md` and `docs/AUDIT.md`.
