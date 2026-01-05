AWEN Runtime

Core runtime engine for AWEN. Written in Rust for the core engine. Provides IR loader, engine, HAL, reference simulator, calibration loops, observability, and artifact storage.

Local build & development

Prerequisites
- Ubuntu / macOS / Windows
- Recommended: install `rustup` to manage Rust toolchains

Install Rust toolchain (recommended):

```bash
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
# follow interactive prompts, then reopen shell or run:
source $HOME/.cargo/env
rustup default stable
```

Install optional tools for formatting and linting:

Observability artifacts

After running `awenctl run` or `awenctl gradient`, the run artifact bundle will contain observability exports:

- `traces.jsonl` : newline-delimited spans (one JSON span per line)
- `timeline.json` : lane-based timeline events for Nsight-like viewers
- `metrics.json` : simple counters/gauges summary

These are written into the run directory (awen_run_* or awen_grad_*). The CI validates these files exist and contain required fields.

```bash
cargo install cargo-edit
cargo install cargo-tarpaulin || true
```

Build runtime (release):

```bash
cd awen-runtime
cargo build --release
```

Run CLI locally (after build):

```bash
# run the reference simulator with example IR
./target/debug/awenctl run --input example_ir.json --out-dir /tmp/awen-run

# compute gradients (uses registered providers)
./target/debug/awenctl gradient --input example_ir.json --params mzi_0:phase --provider reference-adjoint
```

Notes
- The environment used by the editor/devcontainer may not have `cargo` installed. Use the above `rustup` steps to install locally, or rely on CI (GitHub Actions) which already has toolchains available.
- Analytic adjoint support in the reference provider currently covers `mzi` node `phase` parameters. Other parameters fall back to finite-difference.

If you want, I can add a small Makefile or shell script to automate the steps above.

Selecting gradient provider from CLI

The CLI supports selecting the gradient backend via the `--strategy` flag to the `gradient` subcommand.

Examples:

```bash
# auto: prefer adjoint if available, otherwise fall back to finite-difference
./target/debug/awenctl gradient --ir example_ir.json --params mzi_0:phase --strategy auto

# force adjoint
./target/debug/awenctl gradient --ir example_ir.json --params mzi_0:phase --strategy adjoint

# force finite-difference
./target/debug/awenctl gradient --ir example_ir.json --params mzi_0:phase --strategy finite_difference
```

Run tests (including adjoint conformance test):

```bash
cd awen-runtime
cargo test
```
