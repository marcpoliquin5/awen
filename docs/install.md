# Installing AWEN

This page covers the pinned CPU install of AWEN's Python packages. The Rust crates build with Cargo as shown in
the README ([Build and verify](../README.md#build-and-verify)). A pinned CUDA 12.9 lockfile exists but has not been
installed or run by the project yet; see [GPU](#gpu-cuda-129-lockfile-not-run-yet) below.

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

These are the same suites the required CI gate runs. The gate also runs this page's install and verify commands
from the lockfile in a fresh clone of the commit under test: `scripts/fresh_clone_install.sh` (step "Fresh-clone install
from the pinned CPU lockfile"). You can run that script locally; it needs `python3.12` (or set `PYTHON`) and network
access to PyPI and the PyTorch CPU index.

## Check every example on CPU

```bash
python scripts/examples_cpu.py --run
```

Every file under an `examples/` directory has an entry in `scripts/examples_cpu.py`. The command runs each Python
example with CUDA hidden (`CUDA_VISIBLE_DEVICES=""`), small arguments and a 600 s timeout, checks the exit code and
the expected output (for `awen-models/examples/generate.py --device auto`, that it ran on `cpu`), and prints one JSON
line per example. The Rust, C, C++ and JSON examples are CPU-only; their entries name the required-gate step that
builds or runs them, and this command does not run them. `python scripts/examples_cpu.py --check` only checks that
the list matches the files on disk (standard library only). The fresh-clone script runs this check after the suites
above. A new example without an entry fails the check.

## Run the three Phase 1 devices

```bash
python scripts/phase1_devices.py --out phase1-devices.json
python scripts/phase1_devices.py --devices cpu,photonic-sim --ideal --out phase1-devices-ideal.json
```

The first command loads the bundled tiny `awen-mini` checkpoint through `Runtime` on `cpu`, `auto` and
`photonic-sim` (and on `cuda` when `torch.cuda.is_available()`; otherwise `cuda` is recorded as skipped), generates 32
characters from `ROMEO:` with seed 0, and writes a run record: commit, environment, and for each device the resolved
device, fidelity label, GEMM backend, generated text and its SHA-256, plus for `photonic-sim` a summary of the
capability report (lowered GEMMs, host fallbacks, conversion counts, deviation from the CPU reference). It fails if a
device does not load or generate, if `auto` does not resolve to `cuda` when present and to `cpu` otherwise, or if
`auto` on CPU does not reproduce the `cpu` text. The second command runs `photonic-sim` under the noise-free contract
and fails unless its text equals the `cpu` text. `photonic-sim` is simulated (L1); no command here produces an energy,
latency or speedup figure. The fresh-clone script runs both commands and the gate uploads both records as the
`awen-phase1-devices-<commit>` artifact.

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

## GPU: CUDA 12.9 lockfile (not run yet)

`requirements/cu129-py312.txt` is the same lockfile resolved with PyTorch wheels built for CUDA 12.9 (cu129):
the same packages, extras, target and resolution date as the CPU lockfile, generated by `scripts/lock_cu129.sh`.
It pins `torch==2.13.0+cu129`, the newest cu129 build on the PyTorch index for CPython 3.12 on x86_64 Linux when
this page was written (the CPU lockfile pins 2.14.1), plus `triton` and the NVIDIA CUDA 12.9 runtime wheels.

Why not cu128: the newest cu128 build, torch 2.11.0, requires `setuptools<82`, and setuptools before 83.0.0 is
affected by advisory [GHSA-h35f-9h28-mq5c](https://github.com/advisories/GHSA-h35f-9h28-mq5c) (moderate), which the
required gate's dependency review rejects. cu129 stays on the CUDA 12 line and its torch has no such cap.

The planned target is Marc's RTX 5070 Ti laptop (Blackwell, sm_120). Per the project brief, sm_120 needs CUDA 12.8 or
newer and PyTorch wheels built for cu128 or newer; cu121 and cu126 builds do not support it. You also need an NVIDIA
driver that supports CUDA 12.9; the wheels bring their own CUDA runtime.

Status: resolved only. The build box has no GPU, so no install from this lockfile and no CUDA run is recorded yet
(not run). The commands for the laptop are:

```bash
python3.12 -m venv .venv-cu129
. .venv-cu129/bin/activate
python -m pip install --require-hashes -r requirements/cu129-py312.txt
python -m pip install --no-deps \
  ./awen-native ./awen-models ./awen-physics ./awen-qphotonic \
  ./awen-systems ./awen-lab ./awen-studio \
  -e ./awen-ecosystem/python_awen
python -m pip check
python -c "import torch; print(torch.__version__, torch.version.cuda, torch.cuda.is_available(), torch.cuda.get_device_capability(0))"
```

Then run the suites from [Verify the install](#verify-the-install). Regenerate with `./scripts/lock_cu129.sh` (same
rules as the CPU lockfile); `scripts/tests/test_cu129_lockfile.py` checks it. A logged laptop run and a CPU fallback
check for every example are done (see [Check every example on CPU](#check-every-example-on-cpu)); the logged laptop
run is not run yet. On the laptop, after the install above, the Phase 1 CUDA log is:

```bash
python scripts/phase1_devices.py --require-cuda --out phase1-devices-cuda.json
```

`--require-cuda` makes a missing GPU a failure instead of a skip.
