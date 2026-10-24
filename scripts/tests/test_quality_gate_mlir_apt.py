"""Checks that the quality gate caches the MLIR apt packages and installs the listed set."""

from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
WORKFLOW = ROOT / ".github/workflows/observability-quality-gate.yml"
PACKAGES = ROOT / ".github/mlir-apt-packages.txt"
EXPECTED = {
    "clang-20", "clang-format-20", "cmake", "libmlir-20-dev",
    "lld-20", "llvm-20-dev", "mlir-20-tools", "ninja-build",
}


def steps():
    workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
    (job,) = workflow["jobs"].values()
    return job["steps"]


def step(name):
    matches = [s for s in steps() if s.get("name") == name]
    assert len(matches) == 1, name
    return matches[0]


def test_package_list_is_the_previous_set():
    listed = [line.strip() for line in PACKAGES.read_text(encoding="utf-8").splitlines() if line.strip()]
    assert len(listed) == len(set(listed))
    assert set(listed) == EXPECTED


def test_cache_restored_before_install():
    names = [s.get("name") for s in steps()]
    cache = names.index("Restore MLIR apt package cache")
    install = names.index("Install MLIR compiler dependencies")
    assert cache == install - 1
    restore = step("Restore MLIR apt package cache")
    assert restore["uses"].startswith("actions/cache@")
    assert restore["with"]["path"] == "~/mlir-apt-archives"
    assert "hashFiles('.github/mlir-apt-packages.txt')" in restore["with"]["key"]


def test_install_uses_list_cache_dir_and_retries():
    run = step("Install MLIR compiler dependencies")["run"]
    assert "xargs -a .github/mlir-apt-packages.txt sudo apt-get install -y" in run
    assert 'Dir::Cache::Archives="$HOME/mlir-apt-archives"' in run
    assert "Acquire::Retries=" in run
    # The cache action saves as the runner user, so apt's root-owned files must be handed back.
    assert "chown -R" in run
    for package in EXPECTED:
        assert package not in run


def test_mlir_configure_still_follows_install():
    names = [s.get("name") for s in steps()]
    assert names.index("Configure AWEN MLIR compiler") == names.index("Install MLIR compiler dependencies") + 1
