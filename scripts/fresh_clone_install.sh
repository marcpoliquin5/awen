#!/usr/bin/env bash
# Fresh-clone check of the pinned CPU install in docs/install.md.
# Clones the current commit into an empty temporary directory, creates a new virtual environment,
# installs requirements/cpu-py312.txt with --require-hashes, installs the eight AWEN Python packages
# with --no-deps, runs pip check and the test suites from "Verify the install".
# The install and verify commands below must stay identical to docs/install.md
# (scripts/tests/test_fresh_clone_install.py checks this).
# Usage: scripts/fresh_clone_install.sh [source-repo]   (PYTHON=python3.12 by default)
set -euo pipefail
SRC="$(cd "${1:-$(dirname "$0")/..}" && pwd)"
PYTHON="${PYTHON:-python3.12}"
COMMIT="$(git -C "$SRC" rev-parse HEAD)"
WORK="$(mktemp -d)"
trap 'rm -rf "$WORK"' EXIT
git clone --quiet --no-checkout "$SRC" "$WORK/awen"
cd "$WORK/awen"
git checkout --quiet --detach "$COMMIT"
echo "fresh clone of $COMMIT in $WORK/awen"
python3.12() { command "$PYTHON" "$@"; }

# Pinned CPU install (docs/install.md)
python3.12 -m venv .venv
. .venv/bin/activate
python -m pip install --require-hashes -r requirements/cpu-py312.txt
python -m pip install --no-deps \
  ./awen-native ./awen-models ./awen-physics ./awen-qphotonic \
  ./awen-systems ./awen-lab ./awen-studio \
  -e ./awen-ecosystem/python_awen
python -m pip check

# Verify the install (docs/install.md)
export OMP_NUM_THREADS=2
python -m pytest -q awen-ecosystem/python_awen/tests awen-native/tests
python -m pytest -q awen-physics/tests awen-qphotonic/tests awen-systems/tests awen-lab/tests
python -m pytest -q awen-studio/tests awen-models/tests

python --version
echo "fresh-clone lockfile install passed at $COMMIT"
