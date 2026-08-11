# AWEN

AWEN is an experimental heterogeneous compiler/runtime for programmable physical linear algebra. The repository contains a Rust runtime, a typed tensor-to-photonic compiler slice, specifications, simulator and calibration components, Python experiments, and early Studio/Cloud/Ecosystem scaffolding.

The implemented compiler path is deliberately narrow:

```text
awen.tensor GEMM
  -> shape/layout/precision validation
  -> CPU or photonic placement with conversion-aware cost estimates
  -> M/N/K tiling for a declared photonic matrix core
  -> classical Photonic IR
  -> Device IR command buffer
  -> calibrated reference simulator and CPU comparison
```

It does not yet provide a production MLIR/StableHLO frontend, a real `torch.compile` backend, broad `awenBLAS` coverage, or validated hardware speedups. Those are tracked explicitly in the [compiler roadmap](https://github.com/marcpoliquin5/awen/issues/5). Do not interpret reference capability values or simulator cost estimates as measured product performance.

## Repository layout

- `awen-compiler`: typed Tensor IR, device capabilities, cost/placement logic, GEMM tiling, Photonic IR, Device IR, `awenBLAS` reference GEMM, and calibrated benchmark simulator.
- `awen-runtime`: CLI, engine, HAL, scheduler, calibration, observability, artifacts, plugins, legacy node IR, and quantum experiments.
- `awen-spec`: schemas, specifications, and AWEN Enhancement Proposals.
- `awen-ecosystem`: Python experiments, example PDK data, kernels, marketplace, and plugin templates.
- `awen-studio` and `awen-cloud`: early scaffolding, not shipping products.

## Build and verify

Install the current stable Rust toolchain with `rustfmt` and `clippy`, then run:

```bash
cargo fmt --manifest-path awen-compiler/Cargo.toml --all -- --check
cargo clippy --manifest-path awen-compiler/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path awen-compiler/Cargo.toml --all-features --no-fail-fast

cargo fmt --manifest-path awen-runtime/Cargo.toml --all -- --check
cargo clippy --manifest-path awen-runtime/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path awen-runtime/Cargo.toml --all-features --no-fail-fast
```

## Compile a 256×256 GEMM

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  compile awen-compiler/examples/gemm_256.json \
  --capabilities awen-compiler/capabilities/pace_like_128.json \
  --target photonic \
  --output awen_compilation.json
```

The example lowers to eight 128×128×128 optical GEMM tiles. The output contains placement/cost decisions, typed classical Photonic IR, a calibration reference, and the Device IR command stream.

## Execute the calibrated reference benchmark

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  benchmark awen-compiler/examples/gemm_4x4.json \
  --capabilities awen-compiler/capabilities/reference_2x2.json \
  --target photonic \
  --output awen_benchmark.json
```

This command quantizes literal inputs to the backend's advertised effective precision, executes the emitted tiles, applies the calibration transfer function and inverse compensation, compares against the digital reference GEMM, and fails when the declared accuracy contract is exceeded.

## Status and evidence

The canonical required check is `AWEN required quality gate`. Public performance claims must cite immutable end-to-end benchmark artifacts that include host transfer, reconfiguration, lasers, DAC/ADC, calibration, and digital post-processing. The repository currently makes no validated hardware-acceleration claim.
