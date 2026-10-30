"""Phase 1 P1-3: ``Runtime(device="auto" | "cpu" | "cuda").load_model(...).generate()``.

The CPU tests run everywhere. ``auto`` is checked with CUDA hidden, so the CPU
fallback path is exercised even on a GPU machine. An explicit ``cuda`` request
must fail loudly when CUDA is absent instead of silently running on CPU. The
CUDA test only runs where ``torch.cuda.is_available()`` is true (not on the CI
build box).
"""

import pytest
import torch

from awen import Runtime
from awen_models.checkpoint import save_checkpoint
from awen_models.config import MiniConfig
from awen_models.model import AwenMini
from awen_models.tokenizer import CharTokenizer

PROMPT = "ROMEO:"
TINY_TEXT = "ROMEO: to be or not to be, that is the question.\n"


@pytest.fixture
def no_cuda(monkeypatch):
    monkeypatch.setattr(torch.cuda, "is_available", lambda: False)


@pytest.fixture
def tiny_checkpoint(tmp_path):
    """A tiny untrained AWEN-Mini checkpoint (CI config), saved with safetensors."""
    torch.manual_seed(0)
    tokenizer = CharTokenizer.from_text(TINY_TEXT)
    config = MiniConfig(vocab_size=tokenizer.vocab_size, block_size=16, n_layer=1, n_head=2,
                        n_embd=16, ffn_hidden=32)
    model = AwenMini(config)
    save_checkpoint(tmp_path, model, tokenizer, {"parameters": sum(p.numel() for p in model.parameters())})
    return tmp_path


def test_auto_without_cuda_falls_back_to_cpu_and_generates(no_cuda):
    model = Runtime(device="auto").load_model("awen-mini")
    assert model.execution.device == "cpu"
    assert model.execution.gemm_backend == "torch-cpu"
    assert model.execution.fidelity == "measured"
    text = model.generate(PROMPT, max_new_tokens=24, seed=5)
    assert text.startswith(PROMPT) and len(text) == len(PROMPT) + 24
    reference = Runtime(device="cpu").load_model("awen-mini").generate(PROMPT, max_new_tokens=24, seed=5)
    assert text == reference


@pytest.mark.parametrize("device", ["cuda", "cuda:0"])
def test_explicit_cuda_without_cuda_raises_instead_of_running_on_cpu(no_cuda, device):
    runtime = Runtime(device=device)
    with pytest.raises(RuntimeError, match="CUDA was requested"):
        runtime.load_model("awen-mini")


def test_tiny_config_checkpoint_generates_on_auto_and_cpu(no_cuda, tiny_checkpoint):
    outputs = {}
    for device in ("auto", "cpu"):
        model = Runtime(device=device).load_model(str(tiny_checkpoint))
        assert model.execution.device == "cpu"
        assert model.model.config.n_layer == 1
        outputs[device] = model.generate(PROMPT, max_new_tokens=20, temperature=0, seed=None)
    assert outputs["auto"] == outputs["cpu"]
    assert outputs["cpu"].startswith(PROMPT) and len(outputs["cpu"]) == len(PROMPT) + 20


def test_tiny_config_generation_past_block_size(tiny_checkpoint):
    model = Runtime(device="cpu").load_model(str(tiny_checkpoint))
    text = model.generate(PROMPT, max_new_tokens=3 * model.model.config.block_size, seed=1)
    assert len(text) == len(PROMPT) + 3 * model.model.config.block_size
    assert set(text) <= set(TINY_TEXT)


def test_greedy_generation_is_deterministic_without_seed():
    model = Runtime(device="cpu").load_model("awen-mini")
    first = model.generate(PROMPT, max_new_tokens=16, temperature=0, seed=None)
    second = model.generate(PROMPT, max_new_tokens=16, temperature=0, seed=None)
    assert first == second


def test_empty_prompt_is_rejected():
    model = Runtime(device="cpu").load_model("awen-mini")
    with pytest.raises(ValueError, match="prompt"):
        model.generate("", max_new_tokens=4)


@pytest.mark.skipif(not torch.cuda.is_available(), reason="CUDA not available on this host")
def test_cuda_generates_on_cuda():
    model = Runtime(device="cuda").load_model("awen-mini")
    assert model.execution.device.startswith("cuda")
    assert model.execution.gemm_backend == "torch-cuda"
    text = model.generate(PROMPT, max_new_tokens=24, seed=5)
    assert text.startswith(PROMPT) and len(text) == len(PROMPT) + 24
