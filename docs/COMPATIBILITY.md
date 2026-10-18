# Compatibility

What each AWEN component is built and tested against. "Tested" means the
required quality gate runs it on every pull request; anything else is listed
as untested.

## Platforms

| Platform | Status |
|---|---|
| Linux x86_64 (Ubuntu 24.04, CI runner) | tested |
| Other Linux distributions, macOS, Windows | untested; no CI job |
| Photonic accelerator hardware | none available; every photonic path runs on the software simulator |
| NVIDIA GPU (CUDA) | code paths exist (benchmarks, QLoRA) but no GPU is available in CI or on the build host, so they are untested |

## Toolchains

| Component | Requirement | CI coverage |
|---|---|---|
| Rust crates (`awen-compiler`, `awen-runtime`, `awen-native`) | MSRV 1.88 (`rust-version`) | builds/tests on the pinned toolchain in `rust-toolchain.toml`; `cargo check` on 1.88.0 |
| MLIR dialect (`awen-mlir`) | LLVM/MLIR 20 | built and `ctest`ed with clang-20 |
| C / C++ SDK headers | C11, C++20 | example programs compiled with `-Werror` and executed |
| Python packages | `requires-python >= 3.10` | tested on Python 3.12 only |

## Python framework ranges

| Package | Declared range | Tested in CI |
|---|---|---|
| `torch` | `>=2.10,<2.15` | 2.10.0 (minimum) and 2.14.1 (current) |
| `jax` | `>=0.9,<0.12` (Python >= 3.12) | 0.9.0 and 0.11.0 |
| `numpy` | `>=1.26,<3` | whatever the above resolve to |

## Data and schema compatibility

* Every JSON contract is versioned in its name (`awen.<kind>.vN`) and checked
  by `scripts/validate_json.py`. A breaking change gets a new `vN`; readers
  reject unknown versions instead of guessing.
* Plugin manifests are `awen.plugin-manifest.v1`; the signature covers the
  canonical manifest bytes described in `SECURITY.md`.
* Model checkpoints are safetensors plus JSON config; there is no pickle
  format to stay compatible with.

## Versioning

Packages are pre-1.0 (`0.x`). Until 1.0, minor versions may change public
APIs; every change is recorded in `CHANGELOG.md`.
