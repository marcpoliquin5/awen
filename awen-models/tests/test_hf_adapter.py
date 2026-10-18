import pytest

transformers = pytest.importorskip("transformers")

from awen_models.hf_adapter import analyze_model, format_report, generate_text, load_causal_lm


def test_safe_load_generate_and_report(tiny_llama_dir):
    model, tokenizer = load_causal_lm(str(tiny_llama_dir))
    text = generate_text(model, tokenizer, "abc", max_new_tokens=4)
    assert isinstance(text, str)
    report = analyze_model(model, "tiny-llama")
    summary = report.summary
    # 2 layers x (q,k,v,o,gate,up,down) + lm_head
    assert summary["awen_gemm_layers"] == 15
    targets = {layer.name: layer.target for layer in report.layers}
    assert targets["model.embed_tokens"] == "host"
    assert targets["lm_head"] == "awen-gemm"
    q = next(l for l in report.layers if l.name.endswith("layers.0.self_attn.q_proj"))
    assert q.shape == [32, 32] and q.tiles == 1 and q.macs_per_token == 1024
    assert any(l.target == "host" and "RMSNorm" in l.module_type for l in report.layers)
    assert summary["awen_gemm_mac_fraction_of_dense_layers"] == 1.0
    assert "could lower to AWEN GEMM" in format_report(report)
    assert report.to_dict()["report"] == "awen.model-capability-report.v1"


def test_remote_code_is_never_trusted(monkeypatch):
    import awen_models.hf_adapter as adapter
    seen = {}

    class Recorder:
        @staticmethod
        def from_pretrained(*args, **kwargs):
            seen.update(kwargs)
            raise RuntimeError("stop")

    monkeypatch.setattr(adapter, "_auto_classes", lambda: (Recorder, Recorder))
    with pytest.raises(RuntimeError):
        adapter.load_causal_lm("any/model")
    assert seen["trust_remote_code"] is False
