"""Phase 1 P1-5: capability report and CPU deviation for ``device="photonic-sim"``.

Tiny untrained config so the suite runs in CI on CPU. ``ffn_hidden`` 96 makes
the FFN output projection span two ``tile_k`` 64 contraction tiles, so the ADC
count rule is checked across tiles. Everything here is simulated (L1).
"""

import json

import pytest
import torch

awen_native = pytest.importorskip("awen_native")

from awen import Runtime  # noqa: E402
from awen.capability import REPORT_VERSION, main  # noqa: E402
from awen_models.checkpoint import save_checkpoint  # noqa: E402
from awen_models.config import MiniConfig  # noqa: E402
from awen_models.model import AwenMini  # noqa: E402
from awen_models.tokenizer import CharTokenizer  # noqa: E402

TEXT = "ROMEO: to be or not to be, that is the question.\n"
WIDTH, HIDDEN, LAYERS, BLOCK = 16, 96, 2, 16


@pytest.fixture
def tiny_checkpoint(tmp_path):
    torch.manual_seed(0)
    tokenizer = CharTokenizer.from_text(TEXT)
    config = MiniConfig(vocab_size=tokenizer.vocab_size, block_size=BLOCK, n_layer=LAYERS, n_head=2,
                        n_embd=WIDTH, ffn_hidden=HIDDEN)
    model = AwenMini(config)
    save_checkpoint(tmp_path, model, tokenizer, {"parameters": model.num_parameters()})
    return tmp_path


def _report(path, noise=None, seed=0, text=TEXT):
    return Runtime(device="photonic-sim", noise=noise, seed=seed).load_model(str(path)).capability_report(text)


def test_every_linear_layer_is_lowered_once_per_pass_with_analytic_counts(tiny_checkpoint):
    report = _report(tiny_checkpoint)
    assert report["report"] == REPORT_VERSION
    names = {row["layer"] for row in report["lowered"]}
    expected = {f"blocks.{i}.{n}" for i in range(LAYERS) for n in ("attn.qkv", "attn.proj", "ffn.fc", "ffn.proj")}
    assert names == expected
    rows = BLOCK  # text is longer than block_size, so one pass covers BLOCK positions
    assert report["input"]["positions"] == rows
    for row in report["lowered"]:
        tiles = -(-row["inner"] // 64)
        assert row["calls"] == 1 and row["rows"] == rows and row["tiles_per_output"] == tiles
        assert row["macs"] == rows * row["inner"] * row["outputs"]
        assert row["dac_input_conversions"] == rows * row["inner"]
        assert row["dac_weight_conversions"] == row["outputs"] * row["inner"]
        assert row["adc_conversions"] == rows * row["outputs"] * tiles
    assert {row["tiles_per_output"] for row in report["lowered"] if row["layer"].endswith("ffn.proj")} == {2}
    per_block = rows * WIDTH * (3 * WIDTH + WIDTH + 2 * HIDDEN)
    assert report["mac_split"]["photonic_sim_macs"] == LAYERS * per_block
    assert report["conversions"]["adc"] == sum(r["adc_conversions"] for r in report["lowered"])


def test_fallbacks_name_every_host_op_with_a_reason(tiny_checkpoint):
    report = _report(tiny_checkpoint)
    ops = [row["op"] for row in report["fallback"]]
    assert {"tok_emb", "pos_emb", "ln_f"} <= set(ops)
    assert sum(op.endswith("scaled_dot_product_attention") for op in ops) == LAYERS
    assert any(op.startswith("head") for op in ops)
    assert all(row["reason"] for row in report["fallback"])
    head = next(row for row in report["fallback"] if row["op"].startswith("head"))
    vocab = report["model"]["config"]["vocab_size"]
    assert head["macs"] == BLOCK * WIDTH * vocab
    split = report["mac_split"]
    total = split["photonic_sim_macs"] + split["host_macs"]
    assert split["photonic_sim_fraction"] == pytest.approx(split["photonic_sim_macs"] / total)


def test_report_is_labeled_simulated_with_no_energy_figure(tiny_checkpoint):
    report = _report(tiny_checkpoint)
    execution = report["execution"]
    assert execution["physical"] is False and execution["evidence_level"] == "L1"
    assert execution["device"] == "photonic-sim"
    assert report["energy"].startswith("not estimated")
    precision = report["precision_and_calibration"]
    contract = awen_native.default_contract()
    assert precision["dac_bits"] == contract["dac_bits"] and precision["adc_bits"] == contract["adc_bits"]
    assert precision["min_tile_effective_bits"] > 0


def test_ideal_contract_matches_cpu_reference(tiny_checkpoint):
    deviation = _report(tiny_checkpoint, noise=awen_native.ideal_contract())["deviation_vs_cpu"]
    assert deviation["top1_agreement"] == 1.0
    assert deviation["logits_max_abs_diff"] < 1e-5
    assert abs(deviation["nll_diff_nats"]) < 1e-5


def test_default_noise_deviates_more_than_ideal(tiny_checkpoint):
    ideal = _report(tiny_checkpoint, noise=awen_native.ideal_contract())["deviation_vs_cpu"]
    noisy = _report(tiny_checkpoint)["deviation_vs_cpu"]
    assert noisy["logits_rms_diff"] > ideal["logits_rms_diff"]
    assert noisy["cpu_mean_nll_nats"] == pytest.approx(ideal["cpu_mean_nll_nats"])


def test_report_is_reproducible_and_resets_counters(tiny_checkpoint):
    model = Runtime(device="photonic-sim", seed=3).load_model(str(tiny_checkpoint))
    first = model.capability_report(TEXT)
    model.generate("ROMEO:", max_new_tokens=3, seed=0)
    second = model.capability_report(TEXT)
    assert first == second
    assert first["execution"]["seed"] == 3


def test_report_needs_photonic_sim_and_text(tiny_checkpoint):
    with pytest.raises(RuntimeError, match="photonic-sim"):
        Runtime(device="cpu").load_model(str(tiny_checkpoint)).capability_report(TEXT)
    with pytest.raises(ValueError):
        _report(tiny_checkpoint, text="")


def test_cli_writes_json_report(tiny_checkpoint, tmp_path):
    out = tmp_path / "report.json"
    assert main(["--model", str(tiny_checkpoint), "--text", TEXT, "--ideal", "--out", str(out)]) == 0
    data = json.loads(out.read_text())
    assert data["report"] == REPORT_VERSION
    assert data["deviation_vs_cpu"]["top1_agreement"] == 1.0
