"""Execute every code cell of each validation notebook; their asserts are the acceptance tests."""

import importlib.util
import json
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
        pytest.skip("notebook (c) exercises the Rust model; install ./awen-native")
    run_notebook(path)
