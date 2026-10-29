# Installing AWEN

This page covers the pinned CPU install of AWEN's Python packages. The Rust crates build with Cargo as shown in
the README ([Build and verify](../README.md#build-and-verify)). A pinned CUDA install is not written yet; see
[GPU](#gpu-not-locked-yet) below.

Status: experimental, pre-1.0. Everything here runs on a CPU; nothing needs a GPU or photonic hardware.

## Prerequisites

- CPython 3.12 on x86_64 Linux for the pinned install. The packages themselves declare Python 3.10 or newer, but the
  lockfile is resolved for 3.12 only (the version CI uses).
- The Rust toolchain from `rust-toolchain.toml` (rustup installs it automatically). `awen-native` is a PyO3 extension
  compiled from source when you install it.
- Node.js 22 and npm, only for the AWEN Studio frontend.

## Pinned CPU install

`requirements/cpu-py312.txt` pins every third-party dependency of the eight AWEN Python packages (runtime
dependencies, their `test` extras and the `frameworks` extra of `awen-ecosystem`), with SHA-256 hashes. PyTorch comes
from the CPU-only wheel index. From the repository root:

```bash
python3.12 -m venv .venv
. .venv/bin/activate
python -m pip install --require-hashes -r requirements/cpu-py312.txt
python -m pip install --no-deps \
  ./awen-native ./awen-models ./awen-physics ./awen-qphotonic \
  ./awen-systems ./awen-lab ./awen-studio \
  -e ./awen-ecosystem/python_awen
python -m pip check
```

`--no-deps` keeps pip from replacing any pinned version while it installs the AWEN packages from the checkout.
Building `awen-native` fetches its build backend (maturin) from PyPI in an isolated build environment; that build
tool is not covered by the lockfile hashes.

For the Studio frontend: `(cd awen-studio/frontend && npm ci && npm run build)`; `package-lock.json` pins it.

## Verify the install

```bash
export OMP_NUM_THREADS=2
python -m pytest -q awen-ecosystem/python_awen/tests awen-native/tests
python -m pytest -q awen-physics/tests awen-qphotonic/tests awen-systems/tests awen-lab/tests
python -m pytest -q awen-studio/tests awen-models/tests
```

These are the same suites the required CI gate runs. The CI gate does not install from the lockfile yet; a
fresh-clone lockfile install job is a planned follow-up.

## Updating the lockfile

Never edit `requirements/cpu-py312.txt` by hand. Change the ranges in the packages' `pyproject.toml` files, then run:

```bash
./scripts/lock_cpu.sh                                    # needs uv
EXCLUDE_NEWER=2026-11-30T00:00:00Z ./scripts/lock_cpu.sh  # move the resolution date on purpose
```

The script resolves with `uv pip compile` up to a fixed resolution date, so a rerun gives the same pins.
`scripts/tests/test_cpu_lockfile.py` checks that every pin is exact and hashed, that every declared dependency is
pinned inside its declared range, that the script lists every Python package in the repository, and that PyTorch is
the CPU build.

## GPU (not locked yet)

There is no pinned CUDA lockfile yet, and no CUDA path has been tested by the project (the build box has no GPU).
The planned target is Marc's RTX 5070 Ti laptop (Blackwell, sm_120). Per the project brief, sm_120 needs CUDA 12.8 or
newer and PyTorch wheels built for cu128 or newer; cu121 and cu126 builds do not support it. A cu128 lockfile and a
logged laptop run are later slices of this task, as is checking that every example has a CPU fallback.
