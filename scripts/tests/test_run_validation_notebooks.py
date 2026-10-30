"""Tests for scripts/run_validation_notebooks.py (run: python -m pytest scripts/tests)."""

import importlib.util
import json
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "run_validation_notebooks", Path(__file__).resolve().parents[1] / "run_validation_notebooks.py"
)
runner = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(runner)

WORKFLOW = Path(__file__).resolve().parents[2] / ".github" / "workflows" / "observability-quality-gate.yml"


def write_notebook(directory, name, header, code):
    cells = [{"cell_type": "markdown", "metadata": {}, "source": [header]}]
    cells += [{"cell_type": "code", "metadata": {}, "outputs": [], "execution_count": None, "source": [c]} for c in code]
    (directory / name).write_text(json.dumps({"cells": cells, "metadata": {}, "nbformat": 4, "nbformat_minor": 5}))


def test_repository_has_the_three_notebooks_with_levels():
    names = [p.name for p in runner.notebooks()]
    assert [n[:2] for n in names] == ["a_", "b_", "c_"]
    levels = [runner.declared_evidence_level(json.loads(p.read_text())) for p in runner.notebooks()]
    assert levels == ["L0", "L0", "L1"]


def test_declared_evidence_level_parsing():
    nb = {"cells": [{"cell_type": "markdown", "source": ["# T\n", "**Evidence level.** L1 (simulation).\n"]}]}
    assert runner.declared_evidence_level(nb) == "L1"
    nb["cells"][0]["source"] = ["**Evidence level.** L9\n"]
    assert runner.declared_evidence_level(nb) is None
    assert runner.declared_evidence_level({"cells": [{"cell_type": "markdown", "source": ["no level\n"]}]}) is None


def test_record_captures_output_and_passes(tmp_path):
    write_notebook(tmp_path, "a_ok.ipynb", "**Evidence level.** L0 ok\n", ["x = 2\n", "print('value', x * 3)\n"])
    out = tmp_path / "run.json"
    assert runner.main(["--notebooks", str(tmp_path), "--out", str(out)]) == 0
    record = json.loads(out.read_text())
    assert record["schema"] == "awen_validation_notebooks_run.v1"
    assert record["commit"] and record["environment"]["python"]
    (result,) = record["notebooks"]
    assert result["status"] == "passed" and result["evidence_level"] == "L0"
    assert result["cells"][1]["stdout"] == "value 6\n"
    assert len(result["sha256"]) == 64


def test_failing_assert_fails_the_run(tmp_path):
    write_notebook(tmp_path, "a_bad.ipynb", "**Evidence level.** L0 x\n", ["print('before')\n", "assert 1 == 2, 'tolerance'\n"])
    out = tmp_path / "run.json"
    assert runner.main(["--notebooks", str(tmp_path), "--out", str(out)]) == 1
    (result,) = json.loads(out.read_text())["notebooks"]
    assert result["status"] == "failed" and "tolerance" in result["error"]
    assert result["cells"][0]["stdout"] == "before\n"


def test_missing_evidence_level_fails(tmp_path):
    write_notebook(tmp_path, "a_nolevel.ipynb", "# no level\n", ["pass\n"])
    assert runner.main(["--notebooks", str(tmp_path)]) == 1


def test_native_notebook_skips_or_fails_without_native(tmp_path, monkeypatch):
    write_notebook(tmp_path, "c_native.ipynb", "**Evidence level.** L1 x\n", ["pass\n"])
    real = importlib.util.find_spec
    monkeypatch.setattr(runner.importlib.util, "find_spec", lambda n: None if n == "awen_native" else real(n))
    assert runner.main(["--notebooks", str(tmp_path)]) == 0
    assert runner.main(["--notebooks", str(tmp_path), "--require-native"]) == 1


def test_quality_gate_requires_native_for_the_notebooks():
    text = WORKFLOW.read_text()
    step = text.split("- name: Run physics validation notebooks", 1)[1].split("- name:", 1)[0]
    assert 'AWEN_REQUIRE_NATIVE: "1"' in step
    assert "python scripts/run_validation_notebooks.py --require-native" in step
    assert text.index("Build the awen_native Python extension") < text.index("Run physics validation notebooks")
