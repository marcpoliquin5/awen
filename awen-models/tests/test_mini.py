import math

import pytest
import torch
import torch.nn.functional as F

from awen_models import AwenMini, CharTokenizer, MiniConfig, load_checkpoint, save_checkpoint
from awen_models.data import sample_batch, split_tokens
from awen_models.device import resolve_device
from awen_models.quant import Int8Linear, int8_weight_bytes, quantize_model_int8, quantize_tensor_int8
from awen_models.train import TrainSettings, learning_rate, train


def tiny_config(vocab: int, **overrides) -> MiniConfig:
    base = dict(vocab_size=vocab, block_size=32, n_layer=2, n_head=2, n_embd=32, ffn_hidden=64)
    base.update(overrides)
    return MiniConfig(**base)


def test_tokenizer_roundtrip_and_persistence(tmp_path, corpus):
    tokenizer = CharTokenizer.from_text(corpus)
    assert tokenizer.decode(tokenizer.encode(corpus[:200])) == corpus[:200]
    assert tokenizer.itos == sorted(set(corpus))
    tokenizer.save(tmp_path / "tok.json")
    assert CharTokenizer.load(tmp_path / "tok.json").itos == tokenizer.itos
    with pytest.raises(KeyError):
        tokenizer.encode("§")


def test_config_validation():
    with pytest.raises(ValueError):
        MiniConfig(ffn="relu-squared")
    with pytest.raises(ValueError):
        MiniConfig(n_embd=30, n_head=4)


def test_initial_loss_is_near_uniform(corpus):
    tokenizer = CharTokenizer.from_text(corpus)
    torch.manual_seed(0)
    model = AwenMini(tiny_config(tokenizer.vocab_size))
    splits = split_tokens(tokenizer.encode(corpus))
    x, y = sample_batch(splits.train, 8, 32, torch.Generator().manual_seed(0))
    logits, loss = model(x, y)
    assert logits.shape == (8, 32, tokenizer.vocab_size)
    assert abs(loss.item() - math.log(tokenizer.vocab_size)) < 0.3


@pytest.mark.parametrize("variant", [{}, {"ffn": "gated"}, {"ffn": "lowrank", "ffn_rank": 8}, {"shared_layers": True}])
def test_variants_forward_and_generate(variant, corpus):
    tokenizer = CharTokenizer.from_text(corpus)
    model = AwenMini(tiny_config(tokenizer.vocab_size, **variant))
    idx = torch.zeros(1, 3, dtype=torch.long)
    out = model.generate(idx, 10, temperature=0)
    assert out.shape == (1, 13)
    assert torch.equal(out, model.generate(idx, 10, temperature=0))
    sampled = model.generate(idx, 40, temperature=1.0, top_k=5, generator=torch.Generator().manual_seed(1))
    assert sampled.shape == (1, 43)


def test_shared_layers_reuse_parameters(corpus):
    vocab = len(set(corpus))
    shared = AwenMini(tiny_config(vocab, shared_layers=True, n_layer=4))
    unshared = AwenMini(tiny_config(vocab, n_layer=4))
    assert len(shared.blocks) == 1 and len(unshared.blocks) == 4
    assert shared.num_parameters() < unshared.num_parameters()


def test_learning_rate_schedule():
    settings = TrainSettings(steps=100, warmup_steps=10, learning_rate=1e-3, min_learning_rate=1e-4)
    assert learning_rate(0, settings) == pytest.approx(1e-4)
    assert learning_rate(9, settings) == pytest.approx(1e-3)
    assert learning_rate(100, settings) == pytest.approx(1e-4)


def test_training_reduces_validation_loss_and_checkpoint_roundtrips(tmp_path, corpus):
    settings = TrainSettings(steps=60, batch_size=8, eval_interval=30, eval_batches=4, warmup_steps=5, device="cpu", seed=3)
    result = train(corpus, tiny_config(len(set(corpus))), settings, log=False)
    history = result.history
    assert [h["step"] for h in history] == [0, 30, 60]
    assert history[-1]["val_loss"] < history[0]["val_loss"] - 0.5
    assert result.metadata["selected_checkpoint"]["val_loss"] == min(h["val_loss"] for h in history)
    assert result.metadata["hardware"]["device"] == "cpu"
    save_checkpoint(tmp_path / "ckpt", result.model, result.tokenizer, result.metadata)
    loaded, tokenizer, metadata = load_checkpoint(tmp_path / "ckpt")
    idx = torch.tensor([tokenizer.encode("To be")])
    assert torch.equal(loaded(idx)[0], result.model(idx)[0])
    assert metadata["parameters"] == result.model.num_parameters()


def test_training_is_deterministic_for_a_seed(corpus):
    settings = TrainSettings(steps=10, batch_size=4, eval_interval=10, eval_batches=2, warmup_steps=2, device="cpu", seed=11)
    a = train(corpus, tiny_config(len(set(corpus))), settings, log=False)
    b = train(corpus, tiny_config(len(set(corpus))), settings, log=False)
    assert a.history[-1]["val_loss"] == b.history[-1]["val_loss"]


def test_int8_quantization_error_is_bounded(corpus):
    torch.manual_seed(0)
    weight = torch.randn(16, 24)
    q, scale = quantize_tensor_int8(weight)
    assert q.dtype == torch.int8 and q.abs().max() <= 127
    error = (q.float() * scale[:, None] - weight).abs()
    assert torch.all(error <= scale[:, None] / 2 + 1e-7)
    model = AwenMini(tiny_config(len(set(corpus)))).eval()
    quantized = quantize_model_int8(model)
    assert sum(isinstance(m, Int8Linear) for m in quantized.modules()) == len(list(model.linear_layers()))
    idx = torch.zeros(1, 16, dtype=torch.long)
    full, _ = model(idx)
    approx, _ = quantized(idx)
    assert (full - approx).abs().max() < 0.05
    assert int8_weight_bytes(quantized) < sum(l.weight.numel() * 4 for _, l in model.linear_layers()) / 3


def test_external_executor_matches_torch_linear(corpus):
    model = AwenMini(tiny_config(len(set(corpus)))).eval()
    idx = torch.zeros(2, 8, dtype=torch.long)
    reference, _ = model(idx)
    calls = []

    def executor(x, weight):
        calls.append(tuple(x.shape))
        return x @ weight.T

    for _, layer in model.linear_layers():
        layer.matmul_executor = executor
    routed, _ = model(idx)
    assert calls and all(len(shape) == 2 for shape in calls)
    assert torch.allclose(routed, reference, atol=1e-6)


def test_device_resolution():
    assert resolve_device("cpu").type == "cpu"
    assert resolve_device("auto").type == ("cuda" if torch.cuda.is_available() else "cpu")
    if not torch.cuda.is_available():
        with pytest.raises(RuntimeError):
            resolve_device("cuda")
