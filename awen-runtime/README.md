# AWEN Runtime

The Rust runtime provides the AWEN CLI, legacy graph engine, HAL, scheduler, calibration/control, observability, plugin registry, artifact storage, reference simulation, gradient experiments, and quantum-photonic experiments. It also exposes the first tensor compiler slice through `awenctl compile` and `awenctl benchmark`.

## Prerequisites

Install the current stable Rust toolchain with `rustfmt` and `clippy`:

```bash
rustup toolchain install stable --component rustfmt,clippy
rustup default stable
```

## Build and verify

From the repository root:

```bash
cargo fmt --manifest-path awen-runtime/Cargo.toml --all -- --check
cargo clippy --manifest-path awen-runtime/Cargo.toml --all-targets --all-features -- -D warnings
cargo test --manifest-path awen-runtime/Cargo.toml --all-features --no-fail-fast
cargo build --release --manifest-path awen-runtime/Cargo.toml --bin awenctl
```

## Tensor compiler commands

Compile a typed GEMM into placement decisions, classical Photonic IR, and Device IR:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  compile awen-compiler/examples/gemm_256.json \
  --capabilities awen-compiler/capabilities/pace_like_128.json \
  --health awen-compiler/capabilities/pace_like_128.health.json \
  --optimize-for latency \
  --target photonic \
  --output awen_compilation.json
```

Execute literal tensor data through the emitted tiles and calibrated reference simulator:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  benchmark awen-compiler/examples/gemm_4x4.json \
  --capabilities awen-compiler/capabilities/reference_2x2.json \
  --health awen-compiler/capabilities/reference_2x2.health.json \
  --target photonic \
  --output awen_benchmark.json
```

`--optimize-for` accepts `latency`, `energy`, `accuracy`, or `throughput`. `--target` accepts `auto`, `cpu`, or `photonic`. Forced photonic compilation fails when the backend cannot satisfy the operation.

Discover a plugin-provided backend and query its current health:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  backends awen-runtime/plugins/reference_sim --allow-unverified
```

Unsigned manifests are restricted to the explicit development flag. Production
discovery requires a valid Ed25519 signature. Health paths are sandboxed inside
the plugin directory and re-read on every query.

## Legacy graph commands

Run the reference graph engine:

```bash
cd awen-runtime
cargo run --bin awenctl -- run example_ir.json --seed 42
```

Compute gradients:

```bash
cd awen-runtime
cargo run --bin awenctl -- gradient example_ir.json mzi_0:phase,mzi_1:phase \
  --strategy auto --seed 42 --samples 1
```

Gradient strategies are `auto`, `adjoint`, and `finite_difference`. The reference adjoint provider currently covers MZI phase parameters; finite differences are a prototype/debug fallback.

## Artifacts

Legacy run and gradient commands write directories named `awen_run_*` and `awen_grad_*`. Depending on the command, artifacts include input IR, results, quantum states, measurements, gradients, traces, timelines, metrics, calibration state, and provenance.

The tensor compiler writes a single compilation or benchmark JSON file at the caller-provided path. Reference capability and cost values are simulator inputs, not measured product performance.
