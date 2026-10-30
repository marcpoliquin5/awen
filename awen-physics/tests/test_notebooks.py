"""Execute every code cell of each validation notebook; their asserts are the acceptance tests."""

import importlib.util
import json
import os
from pathlib import Path

import pytest

NOTEBOOKS = sorted((Path(__file__).resolve().parents[1] / "notebooks").glob("*.ipynb"))


def run_notebook(path: Path) -> None:
    notebook = json.loads(path.read_text())
    namespace: dict = {"__name__": "__notebook__"}
    for index, cell in enumerate(notebook["cells"]):
        if cell["cell_type"] != "code":
            continue
        source = "".join(cell["source"])
        exec(compile(source, f"{path.name}[cell {index}]", "exec"), namespace)  # noqa: S102


def test_three_validation_notebooks_exist():
    assert [p.name[:2] for p in NOTEBOOKS] == ["a_", "b_", "c_"]


@pytest.mark.parametrize("path", NOTEBOOKS, ids=lambda p: p.stem)
def test_notebook_meets_its_stated_tolerance(path):
    if path.name.startswith("c_") and importlib.util.find_spec("awen_native") is None:
        if os.environ.get("AWEN_REQUIRE_NATIVE") == "1":
            pytest.fail("AWEN_REQUIRE_NATIVE=1 but awen_native is not installed; notebook (c) would not run")
        pytest.skip("notebook (c) exercises the Rust model; install ./awen-native")
    run_notebook(path)


@pytest.mark.parametrize("path", NOTEBOOKS, ids=lambda p: p.stem)
def test_notebook_declares_evidence_level_and_sources(path):
    notebook = json.loads(path.read_text())
    header = "".join(notebook["cells"][0]["source"])
    expected = "L1" if path.name.startswith("c_") else "L0"
    assert f"**Evidence level.** {expected} " in header
    assert "**Sources.**" in header and "measured by us" in header
    assert "**Fidelity.**" in header


@pytest.mark.parametrize("path", NOTEBOOKS, ids=lambda p: p.stem)
def test_notebook_is_stored_without_outputs(path):
    # Outputs live in the run record (scripts/run_validation_notebooks.py), not in the notebook.
    for cell in json.loads(path.read_text())["cells"]:
        if cell["cell_type"] == "code":
            assert cell["outputs"] == [] and cell["execution_count"] is None


def test_shot_noise_notebook_checks_photons_per_extra_bit():
    path = next(p for p in NOTEBOOKS if p.name.startswith("c_"))
    code = "".join("".join(c["source"]) for c in json.loads(path.read_text())["cells"] if c["cell_type"] == "code")
    assert "photons per extra bit" in code and "assert low < measured_factor < high" in code
