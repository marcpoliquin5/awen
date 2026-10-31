"""AWEN-Mini through torch.compile(backend=awen) on the photonic simulator (L1)."""

import json

import pytest

pytest.importorskip("awen_native")
pytest.importorskip("awen_py")

import awen_native  # noqa: E402

from awen.compile_capability import build, main  # noqa: E402
from awen_models.checkpoint import bundled_checkpoint_dir, load_checkpoint  # noqa: E402


@pytest.fixture(scope="module")
def ideal():
    return build(noise=awen_native.ideal_contract())


def test_every_weight_gemm_and_the_head_lower_and_ideal_matches_cpu(ideal):
    import torch

    model, _, _ = load_checkpoint(bundled_checkpoint_dir("awen-mini"), torch.device("cpu"))
    compile_report = ideal["compile"]
    lowered = [row["node"] for row in compile_report["lowered"]]
    assert len(lowered) == len(list(model.linear_layers())) + 1  # AwenLinear layers plus the tied head
    assert lowered[-1] == "logits"
    deviation = compile_report["deviation_vs_reference"]
    assert deviation["top1_agreement"] == 1.0 and deviation["max_abs_diff"] < 1e-4
    targets = {row["target"]: row for row in compile_report["fallback"]}
    attention = [row for name, row in targets.items() if "scaled_dot_product_attention" in name]
    assert attention and attention[0]["count"] == model.config.n_layer
    assert ideal["contract"] == "ideal" and compile_report["execution"]["physical"] is False


def test_cli_writes_default_contract_report(tmp_path):
    out = tmp_path / "report.json"
    assert main(["--out", str(out), "--text", "ROMEO:"]) == 0
    data = json.loads(out.read_text())
    assert data["contract"] == "default" and data["input"]["positions"] == 6
    assert data["compile"]["deviation_vs_reference"]["max_abs_diff"] > 0
    assert data["compile"]["energy"].startswith("not estimated")
