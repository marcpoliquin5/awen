# AWEN Runtime

The Rust runtime provides the AWEN CLI, legacy graph engine, HAL, scheduler, calibration/control, observability, plugin registry, artifact storage, reference simulation, reproducible HIL benchmark orchestration, verified claim generation, gradient experiments, and quantum-photonic experiments. It also exposes the first tensor compiler slice through `awenctl compile` and `awenctl benchmark`, plus the compiled `awen.framework-c.v1` C/C++ tensor ABI.

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

## C and C++ framework ABI

Building the library produces Rust, shared, and static library artifacts:

```bash
cargo build --release --manifest-path awen-runtime/Cargo.toml --lib
```

Headers are under `awen-runtime/include/awen`. The C ABI exposes checked,
caller-owned row-major `f32` and `f64` GEMM plus thread-local errors. The C++20
header adds a `std::span` wrapper:

```bash
g++ -std=c++20 -Iawen-runtime/include \
  awen-runtime/examples/framework_cpp.cpp \
  -Lawen-runtime/target/release -lawen_runtime -o framework_cpp
```

The Python/JAX/PyTorch/NumPy in-process runtime is documented in
`awen-ecosystem/python_awen/README.md` and AEP-0016.

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

Physical-design plugins use the manifest's typed
`physical_design_adapters` array. The only kinds are `gdsfactory`,
`circuit_simulator`, and `electromagnetic_simulator`; each fixes the tool/version,
request/response schema version, and supported verification evidence. Duplicate
kinds and version skew fail discovery validation. The runtime/plugin owns
external process or service isolation, while the compiler validates the closed
mapping response and immutable identities. No solver-specific payload or
proprietary PDK data belongs in the manifest.

Every typed backend capability also carries a verified physical-design binding.
Compilation records only identity provenance; PDK and process-corner changes
invalidate reuse and trigger safe recompilation. See AEP-0021 and
`awen-spec/specs/physical-design-boundary.md`.

## Full-system and hardware-in-the-loop benchmarks

Run every backend configured by the canonical portable suite:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  benchmark-suite benchmarks/reference_hil_suite.json \
  --output-dir awen_hil_artifacts \
  --commit-sha "$(git rev-parse HEAD)" \
  --runner-id local-reference
```

The command produces a content-addressed artifact containing raw repetitions,
p50/p95/p99 latency, throughput, energy, power, and error distributions,
full-system component accounting, environment and calibration provenance,
regression findings, and output checksums. CPU and simulator runners are built
in. CUDA devices, lab rigs, and accelerators use the timeout-bounded
`awen.hil-driver.v1` standard-input/standard-output protocol.

Use `.github/workflows/hardware-benchmark.yml` on a controlled self-hosted
runner for physical measurements. Generate public claims only after publishing
a verified artifact at an immutable HTTPS URL whose final path segment contains
its SHA-256 digest and which has no query string or fragment:

```bash
awenctl benchmark-claims benchmark-<sha256>.json \
  --artifact-url https://benchmarks.example/benchmark-<sha256>.json \
  --baseline cpu-baseline \
  --candidate hardware-accelerator \
  --output claims.json \
  --markdown-output claims.md
```

The generator refuses simulated, estimated, vendor-specified, mutable,
inaccurate, uncalibrated, or non-accelerating evidence. See AEP-0019 and
`awen-spec/specs/hardware-benchmarking.md`.

## Typed classical and quantum-photonic programs

The runtime chokepoint accepts the closed `PhotonicProgram` enum, not an
arbitrary operation type string. Its independent contracts are:

- `awen.photonic.program.v1` for calibrated classical analog transforms,
  explicit precision/conversion widths, seeded noise, numerical tolerances,
  and deterministic timing;
- `awen.qphotonic.program.v1` plus `awen.qphotonic.result.v1` for Fock or
  Gaussian CV modes, typed gates/measurements, shots, seed, feed-forward,
  coherence, distribution/mean/fidelity correctness, and replay identity; and
- `awen.photonic-interop.v1` for named measurement readout or named classical
  control of a compatible quantum gate parameter.

Migrate the deprecated mixed V5 document before constructing a typed program:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  migrate-photonic-v5 legacy-v5.json \
  --output migration-report.json
```

The command writes its report even when rejected. Ambiguous unprefixed and
unknown operations are errors; mixed recognized dialects receive an explicit
interop warning. See AEP-0020 and
`awen-spec/specs/photonic-dialect-separation.md`.

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

The tensor compiler writes a single compilation or simulator benchmark JSON file at the caller-provided path. HIL suite execution writes an immutable multi-file artifact set. Reference capability, accounting, and cost values are simulator or estimated inputs, not measured product performance.
