import pytest
import torch
from torch import nn

pytest.importorskip("transformers")

from awen_models.finetune import (  # noqa: E402
    LoRALinear,
    apply_lora,
    distill,
    kd_loss,
    load_base,
    lora_finetune,
    qlora_load_kwargs,
    trainable_parameters,
)

TEXT = "the quick brown fox jumps over the lazy dog " * 120


def test_lora_starts_as_identity_and_merges_exactly():
    base = nn.Linear(16, 8)
    layer = LoRALinear(base, r=4, alpha=8)
    x = torch.randn(3, 16)
    assert torch.allclose(layer(x), base(x))
    with torch.no_grad():
        layer.lora_b.weight.normal_()
    merged = nn.functional.linear(x, layer.merged_weight(), base.bias)
    assert torch.allclose(layer(x), merged, atol=1e-5)
    assert not base.weight.requires_grad


def test_apply_lora_freezes_base_and_wraps_targets(tiny_llama_dir):
    model, _ = load_base(str(tiny_llama_dir))
    total = sum(p.numel() for p in model.parameters())
    wrapped = apply_lora(model, ("q_proj", "v_proj"), r=2)
    assert len(wrapped) == 4  # 2 layers x (q, v)
    # q: 32->32, v: 32->16 (2 kv heads of 8); r=2 adds 2*(in+out) per module
    assert trainable_parameters(model) == 2 * (2 * (32 + 32) + 2 * (32 + 16))
    assert sum(p.numel() for p in model.parameters()) == total + trainable_parameters(model)
    with pytest.raises(ValueError):
        apply_lora(nn.Sequential(nn.ReLU()), ("q_proj",))


def test_lora_finetune_reduces_validation_loss(tiny_llama_dir):
    model, tok = load_base(str(tiny_llama_dir))
    report = lora_finetune(model=model, tokenizer=tok, text=TEXT, steps=60, batch_size=4, block_size=16,
                           lr=1e-2, r=4, eval_windows=8, model_id="tiny-llama")
    assert report["val_loss_after"] < report["val_loss_before"]
    assert report["trainable_parameters"] < report["total_parameters"] / 5


def test_kd_loss_is_zero_kl_for_identical_logits():
    logits = torch.randn(2, 5, 11)
    targets = torch.randint(11, (2, 5))
    ce = nn.functional.cross_entropy(logits.flatten(0, 1), targets.flatten())
    assert torch.allclose(kd_loss(logits, logits, targets, alpha=0.5), 0.5 * ce, atol=1e-6)


def test_distill_runs_both_arms(tiny_llama_dir):
    teacher, tok = load_base(str(tiny_llama_dir))
    report = distill(teacher=teacher, tokenizer=tok, text=TEXT, steps=5, batch_size=2, block_size=16,
                     student={"n_layer": 1, "n_embd": 16, "n_head": 2, "ffn_hidden": 32}, eval_windows=4,
                     teacher_id="tiny-llama")
    assert set(report["results"]) == {"kd", "ce-only"}
    assert report["student_config"]["vocab_size"] == 64


@pytest.mark.skipif(torch.cuda.is_available(), reason="checks the CPU refusal path")
def test_qlora_refuses_without_cuda():
    with pytest.raises(RuntimeError, match="CUDA"):
        qlora_load_kwargs()


def test_parameter_counts_match_instantiated_models():
    from transformers import LlamaConfig, LlamaForCausalLM, Qwen3Config, Qwen3ForCausalLM

    from awen_models.compute_plan import DecoderShape

    for cfg_cls, model_cls, tied in ((LlamaConfig, LlamaForCausalLM, True), (Qwen3Config, Qwen3ForCausalLM, False)):
        cfg = cfg_cls(vocab_size=96, hidden_size=48, intermediate_size=80, num_hidden_layers=2,
                      num_attention_heads=4, num_key_value_heads=2, head_dim=16, tie_word_embeddings=tied)
        model = model_cls(cfg)
        shape = DecoderShape.from_hf_config(cfg.to_dict())
        assert shape.total_parameters() == sum(p.numel() for p in model.parameters())
        wrapped_model = model_cls(cfg)
        apply_lora(wrapped_model, ("q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj"), r=3)
        assert shape.lora_parameters(3) == trainable_parameters(wrapped_model)


def test_memory_estimate_ordering_and_measured_peak():
    from awen_models.compute_plan import DecoderShape, estimate_memory, measure_matmul_tflops

    shape = DecoderShape(hidden=1024, intermediate=3072, layers=28, heads=16, kv_heads=8, head_dim=128,
                         vocab=151936, tied=True, qk_norm=True)
    full, lora, qlora = (estimate_memory(shape, m, 1, 1024, budget_gib=12) for m in ("full-bf16", "lora-bf16", "qlora-nf4"))
    assert qlora.weights_gib < lora.weights_gib == full.weights_gib
    assert lora.trainable_state_gib < full.trainable_state_gib
    assert measure_matmul_tflops(n=128, repeats=3) > 0
