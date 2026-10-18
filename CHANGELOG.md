# Changelog

## Unreleased

- Added `awen_models.finetune`, covering LoRA, QLoRA (NF4, CUDA only) and
  knowledge distillation into AWEN-Mini, with committed CPU smoke results on
  SmolLM2-135M. Added `awen_models.compute_plan` and `docs/COMPUTE_PLAN.md`,
  which estimate memory and FLOPs for Qwen3-0.6B/1.7B and SmolLM2 on a
  12 GB GPU. Parameter counts are checked against instantiated models.
- Added `awen-native`, a PyO3/maturin binding to the Rust compiler and the
  photonic GEMM model. `awen_py` operations that target `photonic`, including
  `torch.compile(backend=awen)` with `target="photonic"`, now run their matrix
  products in Rust instead of framework math labeled as simulated (#84).
- `Runtime(device="photonic-sim")` runs every AWEN-Mini transformer linear
  layer on the Rust photonic simulator. A committed demo compares one layer
  and the whole model's validation loss against the CPU oracle across optical
  power (simulated, compact model).
- Added `awen-models`: the AWEN-Mini character-level language model
  (1,016,320 parameters), trained on CPU on Tiny Shakespeare. It ships with a
  bundled safetensors checkpoint (validation loss 1.4795 nats, measured), int8
  inference, `Runtime(device="auto")`, an evaluation command, the
  AWEN-Nonlinear equal-parameter ablation with committed results, and a
  Hugging Face capability report (`trust_remote_code=False`) for SmolLM2-135M.
- `awenctl execute --inputs` runs AWENEXE GEMM dispatches on the photonic
  noise model and reports error and measured effective bits against an exact
  reference (#82).
- Added the physical-unit photonic noise contract
  (`awen.photonic-noise-contract.v1`) and a compact GEMM noise model covering
  shot, thermal, RIN, phase, drift, crosstalk, insertion loss and DAC/ADC
  terms (AEP-0022, #80).
- Added `docs/AUDIT.md` (implementation truth table at `020be48`),
  `docs/ROADMAP.md` (dependency-aware milestones with acceptance tests and
  hardware needs) and `docs/DIRECTIVE_TRACKER.md`; opened labeled issues
  #55–#79 for every missing module.
- Removed about 1,060 lines of never-compiled runtime code (`control_v0`, the
  old simulator module, a duplicate `awenctl`) and 65 tests that only exercised
  mocks; added real plugin-loader, HAL and `awenctl` tests. Runtime line
  coverage rose from 81.2% to 85.6% (#54).
- Updated Rust dependencies: `sha2` 0.11, `ed25519-dalek` 3.0, `serde_json`,
  `flate2`, `log`, `time`; `uuid` held at 1.24.0 because newer releases need
  Rust 1.89 (#53). Superseded Dependabot PRs were closed with explanations.
- Supported PyTorch 2.10 through 2.14 in `awen_py`; CI tests 2.10.0 and 2.14.1
  (#52, supersedes Dependabot #49).
- Pinned the Rust toolchain (1.99.1) in `rust-toolchain.toml`, declared and
  CI-checked a minimum supported Rust version of 1.88, added a weekly scheduled
  quality-gate run, Rust build caching, a prebuilt `cargo-audit`, and upgraded
  GitHub Actions off the deprecated Node 20 runtime.
- Added strict normalized equal-batch rank-three StableHLO `dot_general`
  lowering through typed MLIR dialects, deterministic AWENEXE emission, and
  direct Rust runtime preparation.
- Consolidated repository policy, security, CI, licensing, and release evidence.
- Removed obsolete phase sign-offs and nonfunctional product directories.

## Historical development snapshot

The tag `phase-2.5-gated-2026-01-05` is a historical source snapshot. It is not
a supported product release, does not establish hardware-performance evidence,
and predates the current verified-release policy.
