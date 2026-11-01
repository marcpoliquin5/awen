"""A tiny random-init transformers Qwen3 through torch.compile(backend=awen) on the photonic simulator (L1)."""

import json

import pytest

pytest.importorskip("transformers")
pytest.importorskip("awen_native")
pytest.importorskip("awen_py")

import awen_native  # noqa: E402

from awen.compile_llama import ARCHITECTURES, TINY_LLAMA, TINY_QWEN3, build, main  # noqa: E402

POSITIONS = 16


@pytest.fixture(scope="module")
def ideal():
    return build(positions=POSITIONS, noise=awen_native.ideal_contract(), arch="qwen3")


def _rows(report, short):
    return [row for row in report["compile"]["fallback"] if row["target"].split(".")[-1] == short]


def test_every_projection_and_the_head_lower_and_ideal_matches_cpu(ideal):
    compile_report = ideal["compile"]
    layers, hidden, inner = TINY_QWEN3["num_hidden_layers"], TINY_QWEN3["hidden_size"], TINY_QWEN3["intermediate_size"]
    q = TINY_QWEN3["num_attention_heads"] * TINY_QWEN3["head_dim"]
    kv = TINY_QWEN3["num_key_value_heads"] * TINY_QWEN3["head_dim"]
    per_layer = POSITIONS * hidden * (q + kv + kv) + POSITIONS * q * hidden + 3 * POSITIONS * hidden * inner
    assert compile_report["counts"]["lowered_nodes"] == 7 * layers + 1  # q,k,v,o,gate,up,down per layer + head
    assert compile_report["lowered"][-1]["node"] == "logits"
    assert compile_report["counts"]["lowered_macs"] == layers * per_layer + POSITIONS * hidden * TINY_QWEN3["vocab_size"]
    deviation = compile_report["deviation_vs_reference"]
    assert deviation["top1_agreement"] == 1.0 and deviation["max_abs_diff"] < 1e-5
    assert ideal["model"]["architecture"] == "Qwen3ForCausalLM"
    assert "Qwen3Config" in ideal["model"]["weights"] and "nothing downloaded" in ideal["model"]["weights"]
    assert compile_report["execution"]["physical"] is False


def test_per_head_qk_norms_stay_on_the_host_and_count_zero(ideal):
    layers = TINY_QWEN3["num_hidden_layers"]
    # Llama has 2 RMS norms per layer plus the final one; Qwen3 adds q_norm and k_norm per layer.
    for short in ("rsqrt", "pow"):
        rows = _rows(ideal, short)
        assert len(rows) == 1 and rows[0]["count"] == 4 * layers + 1 and rows[0]["macs"] == 0


def test_every_host_node_is_counted_and_attention_matches_the_dense_rule(ideal):
    counts = ideal["compile"]["counts"]
    assert counts["host_macs_uncounted_nodes"] == []
    heads, head_dim = TINY_QWEN3["num_attention_heads"], TINY_QWEN3["head_dim"]
    attention = TINY_QWEN3["num_hidden_layers"] * heads * POSITIONS * POSITIONS * 2 * head_dim
    assert counts["host_fallback_macs"] == attention
    total = counts["lowered_macs"] + attention
    assert counts["lowered_mac_fraction"] == pytest.approx(counts["lowered_macs"] / total)


def test_llama_stays_the_default_and_unknown_arch_is_rejected():
    assert ARCHITECTURES["llama"][2] is TINY_LLAMA and "head_dim" not in TINY_LLAMA
    with pytest.raises(ValueError):
        build(positions=4, arch="gpt2")


def test_cli_qwen3_default_contract_is_deterministic_and_deviates(tmp_path):
    first, second = tmp_path / "a.json", tmp_path / "b.json"
    for out in (first, second):
        assert main(["--arch", "qwen3", "--positions", "8", "--seed", "1", "--out", str(out)]) == 0
    data = json.loads(first.read_text())
    assert first.read_text() == second.read_text()
    assert data["model"]["architecture"] == "Qwen3ForCausalLM" and data["model"]["config"] == TINY_QWEN3
    assert data["compile"]["deviation_vs_reference"]["max_abs_diff"] > 0
    assert data["compile"]["energy"].startswith("not estimated")
