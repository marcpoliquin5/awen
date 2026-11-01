"""A tiny random-init transformers Llama through torch.compile(backend=awen) on the photonic simulator (L1)."""

import json

import pytest

pytest.importorskip("transformers")
pytest.importorskip("awen_native")
pytest.importorskip("awen_py")

import awen_native  # noqa: E402

from awen.compile_llama import TINY_LLAMA, build, main  # noqa: E402

POSITIONS = 16


@pytest.fixture(scope="module")
def ideal():
    return build(positions=POSITIONS, noise=awen_native.ideal_contract())


def _expected_lowered_macs(positions):
    hidden, inner, vocab = TINY_LLAMA["hidden_size"], TINY_LLAMA["intermediate_size"], TINY_LLAMA["vocab_size"]
    kv = hidden * TINY_LLAMA["num_key_value_heads"] // TINY_LLAMA["num_attention_heads"]
    per_layer = positions * hidden * (hidden + kv + kv + hidden) + 3 * positions * hidden * inner
    return TINY_LLAMA["num_hidden_layers"] * per_layer + positions * hidden * vocab


def test_every_projection_and_the_head_lower_and_ideal_matches_cpu(ideal):
    compile_report = ideal["compile"]
    layers = TINY_LLAMA["num_hidden_layers"]
    assert compile_report["counts"]["lowered_nodes"] == 7 * layers + 1  # q,k,v,o,gate,up,down per layer + head
    assert compile_report["lowered"][-1]["node"] == "logits"
    assert compile_report["counts"]["lowered_macs"] == _expected_lowered_macs(POSITIONS)
    deviation = compile_report["deviation_vs_reference"]
    assert deviation["top1_agreement"] == 1.0 and deviation["max_abs_diff"] < 1e-5
    assert ideal["contract"] == "ideal" and compile_report["execution"]["physical"] is False
    assert "nothing downloaded" in ideal["model"]["weights"]


def test_every_host_node_is_counted_and_attention_matches_the_dense_rule(ideal):
    counts = ideal["compile"]["counts"]
    assert counts["host_macs_uncounted_nodes"] == []
    heads, head_dim = TINY_LLAMA["num_attention_heads"], TINY_LLAMA["hidden_size"] // TINY_LLAMA["num_attention_heads"]
    attention = TINY_LLAMA["num_hidden_layers"] * heads * POSITIONS * POSITIONS * 2 * head_dim
    assert counts["host_fallback_macs"] == attention
    rows = [row for row in ideal["compile"]["fallback"] if "scaled_dot_product_attention" in row["target"]]
    assert rows and rows[0]["count"] == TINY_LLAMA["num_hidden_layers"]
    total = counts["lowered_macs"] + attention
    assert counts["lowered_mac_fraction"] == pytest.approx(counts["lowered_macs"] / total)


def test_cli_default_contract_is_deterministic_and_deviates(tmp_path):
    first, second = tmp_path / "a.json", tmp_path / "b.json"
    for out in (first, second):
        assert main(["--positions", "8", "--seed", "1", "--out", str(out)]) == 0
    data = json.loads(first.read_text())
    assert first.read_text() == second.read_text()
    assert data["contract"] == "default" and data["input"]["positions"] == 8
    assert data["compile"]["deviation_vs_reference"]["max_abs_diff"] > 0
    assert data["compile"]["energy"].startswith("not estimated")
