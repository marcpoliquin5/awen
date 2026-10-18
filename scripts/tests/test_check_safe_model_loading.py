"""Tests for scripts/check_safe_model_loading.py (run: python -m pytest scripts/tests)."""

import importlib.util
from pathlib import Path

SPEC = importlib.util.spec_from_file_location(
    "check_safe_model_loading", Path(__file__).resolve().parents[1] / "check_safe_model_loading.py"
)
checker = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(checker)


def problems(tmp_path, source):
    path = tmp_path / "mod.py"
    path.write_text(source)
    return checker.check_file(path)


def test_safe_patterns_pass(tmp_path):
    assert problems(tmp_path, (
        "import torch\n"
        "m = AutoModel.from_pretrained('x', trust_remote_code=False, use_safetensors=True)\n"
        "t = torch.load('w.pt', weights_only=True)\n"
    )) == []


def test_remote_code_true_fails(tmp_path):
    assert any("trust_remote_code" in p for p in problems(
        tmp_path, "AutoModel.from_pretrained('x', trust_remote_code=True)\n"))


def test_remote_code_variable_fails(tmp_path):
    assert problems(tmp_path, "AutoModel.from_pretrained('x', trust_remote_code=flag)\n")


def test_missing_remote_code_flag_fails(tmp_path):
    assert any("explicitly" in p for p in problems(tmp_path, "AutoModel.from_pretrained('x')\n"))


def test_pickle_and_unsafe_torch_load_fail(tmp_path):
    found = problems(tmp_path, "import pickle, torch\npickle.load(f)\ntorch.load('w.pt')\n")
    assert len(found) == 2


def test_repository_passes():
    assert checker.main() == 0
