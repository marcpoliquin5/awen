"""Tests for scripts/examples_cpu.py, the CPU fallback check for every example (run: python -m pytest scripts/tests)."""

import importlib.util
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
SPEC = importlib.util.spec_from_file_location("examples_cpu", ROOT / "scripts" / "examples_cpu.py")
ex = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(ex)

from test_fresh_clone_install import doc_section_commands, script_commands  # noqa: E402


def test_manifest_covers_every_example_on_disk():
    found = ex.discover()
    assert found, "no examples found"
    assert ex.check_manifest(found) == []
    assert sorted(ex.EXAMPLES) == found


def test_every_python_example_is_run_and_others_name_a_gate_step():
    for path, entry in ex.EXAMPLES.items():
        if path.endswith(".py"):
            assert "run" in entry, path
        else:
            assert entry.get("gate"), path


def test_check_manifest_reports_missing_stale_and_bad_entries():
    found = ["a/examples/new.py", "a/examples/x.c"]
    manifest = {"a/examples/x.c": {"run": []}, "a/examples/gone.py": {"run": [], "gate": "g"}}
    problems = ex.check_manifest(found, manifest)
    assert "no CPU fallback entry for a/examples/new.py" in problems
    assert "manifest entry for missing file a/examples/gone.py" in problems
    assert "a/examples/x.c: only Python examples are run" in problems
    assert "a/examples/gone.py: needs exactly one of 'run' or 'gate'" in problems


def test_discover_finds_nested_examples_and_skips_build_dirs(tmp_path):
    (tmp_path / "pkg/examples/sub").mkdir(parents=True)
    (tmp_path / "pkg/examples/sub/b.py").write_text("")
    (tmp_path / "pkg/examples/a.rs").write_text("")
    (tmp_path / "pkg/src").mkdir()
    (tmp_path / "pkg/src/not_example.py").write_text("")
    (tmp_path / "ui/node_modules/dep/examples").mkdir(parents=True)
    (tmp_path / "ui/node_modules/dep/examples/c.js").write_text("")
    assert ex.discover(tmp_path) == ["pkg/examples/a.rs", "pkg/examples/sub/b.py"]


def test_judge_exit_code_and_expected_output():
    assert ex.judge({"expect": ["[cpu, "]}, 0, "text\n[cpu, measured] 20 tokens") is None
    assert ex.judge({"expect": []}, 3, "") == "exit code 3"
    assert ex.judge({"expect": ["[cpu, "]}, 0, "[cuda, measured]").startswith("expected output missing")


def test_judge_jsonl_rejects_failed_configurations():
    ok = json.dumps({"mode": "dense", "step_s_median": 0.1})
    assert ex.judge({"jsonl": True}, 0, ok + "\n") is None
    bad = json.dumps({"mode": "dense", "failed": True, "returncode": -9})
    assert ex.judge({"jsonl": True}, 0, ok + "\n" + bad).startswith("configuration failed")
    assert ex.judge({"jsonl": True}, 0, "") == "no JSON lines"
    assert ex.judge({"jsonl": True}, 0, "hello").startswith("not JSON")


def test_cpu_env_hides_cuda(monkeypatch):
    monkeypatch.setenv("CUDA_VISIBLE_DEVICES", "0")
    assert ex.cpu_env()["CUDA_VISIBLE_DEVICES"] == ""


def test_run_reports_failures_with_nonzero_exit(tmp_path, monkeypatch, capsys):
    monkeypatch.setattr(ex, "ROOT", tmp_path)
    (tmp_path / "ok.py").write_text("print('fine')\n")
    (tmp_path / "bad.py").write_text("raise SystemExit(2)\n")
    manifest = {"ok.py": {"run": [], "expect": ["fine"]}, "bad.py": {"run": []}, "x.c": {"gate": "step"}}
    assert ex.run_all(manifest) == 1
    lines = capsys.readouterr().out.splitlines()
    records = {r["example"]: r for r in map(json.loads, lines[:-1])}
    assert records["ok.py"]["status"] == "passed"
    assert records["bad.py"]["status"] == "failed" and records["bad.py"]["reason"] == "exit code 2"
    assert records["x.c"]["status"] == "not run here"
    assert lines[-1].startswith("CPU fallback: 1 of 2 Python examples passed")


def test_fresh_clone_script_runs_the_documented_example_check_after_verify():
    doc = doc_section_commands("Check every example on CPU")
    assert doc == ["python scripts/examples_cpu.py --run"]
    script = script_commands()
    assert script.count(doc[0]) == 1
    assert script.index(doc[0]) > script.index("python -m pytest -q awen-studio/tests awen-models/tests")
