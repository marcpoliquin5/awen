"""H2 registered-run entry point (--config): config checks, conversion counter, verdict rules, tiny run."""

import json
import string
import sys
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")
torch = pytest.importorskip("torch")
eff = pytest.importorskip("awen_models.efficiency")

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
H2_YAML = FRACTAL / "configs" / "h2.yaml"
sys.path.insert(0, str(FRACTAL))
import h2_structured_weights as h2  # noqa: E402

# 65 distinct characters, the vocabulary size the harness budget is checked at (awen-models tests).
ALPHABET = (string.ascii_letters + string.digits + " .,;:!?'-\n")[:65]


def _cfg(tmp_path, **over):
    cfg = yaml.safe_load(H2_YAML.read_text())
    cfg.update(over)
    p = tmp_path / "h2.yaml"
    p.write_text(yaml.safe_dump(cfg))
    return p


def test_registered_config_loads():
    cfg = h2.load_config(H2_YAML)
    assert cfg["seeds"] == [101, 102, 103] and cfg["variants"] == h2.REGISTERED_VARIANTS


def test_config_rejects_pilot_seeds_wrong_id_and_variants(tmp_path):
    with pytest.raises(ValueError, match="pilot seeds"):
        h2.load_config(_cfg(tmp_path, seeds=[2, 101, 102]))
    with pytest.raises(ValueError, match="not an H2"):
        h2.load_config(_cfg(tmp_path, id="H1"))
    with pytest.raises(ValueError, match="variants"):
        h2.load_config(_cfg(tmp_path, variants=["baseline", "kron3", "monarch"]))


def test_dense_layer_counts_one_dac_per_input_one_adc_per_output():
    rep = h2.layer_conversions(torch.nn.Linear(128, 512))
    assert rep["useful_macs"] == 128 * 512 and rep["conversions"] == 128 + 512 and rep["legal"]


def test_factorized_counts_follow_rank_and_match_dense_equivalent():
    k = eff.Kron3Linear((2, 2, 4), (4, 2, 4), rank=3)
    rep = h2.layer_conversions(k)
    assert k.dense().shape == (rep["d_out"], rep["d_in"]) == (32, 16)
    assert rep["conversions"] == 3 * (16 + 32) and rep["conversions_lower_bound"] == 16 + 32
    assert rep["legal"] and rep["channel_requirements"] == {
        "spatial_modes": 4, "polarization_states": 2, "wavelengths": 4, "weight_banks": 4}
    tt = eff.TTLinear((8, 16), (32, 32), 5)
    rep = h2.layer_conversions(tt)
    assert tt.full_weight().shape == (1024, 128) and rep["conversions"] == 5 * (128 + 1024) and rep["legal"]


def test_polarization_factor_above_two_is_illegal():
    rep = h2.layer_conversions(eff.Kron3Linear((4, 4, 8), (8, 16, 16), rank=1))
    assert not rep["legal"] and "polarization" in rep["illegal_reasons"][0]


def test_registered_variants_conversion_report():
    rep = {v: h2.conversion_report(eff.build(v, 65)) for v in h2.REGISTERED_VARIANTS}
    assert rep["baseline"]["conversions_per_useful_mac"] == pytest.approx(2 * (128 + 512) / (2 * 128 * 512))
    assert rep["baseline"]["legal"] and rep["tensor-train"]["legal"] and not rep["kron3"]["legal"]


def test_verdict_rules():
    rows = [{"variant": v, "seed": s, "val_loss": loss + 0.01 * s, "forward_latency_ms_b1": lat}
            for v, loss, lat in [("baseline", 1.0, 1.0), ("kron3", 2.0, 1.0), ("tensor-train", 1.0, 1.1)]
            for s in (1, 2, 3)]
    conv = {"baseline": {"legal": True, "conversions_per_useful_mac": 0.01},
            "kron3": {"legal": False, "conversions_per_useful_mac": 2.0},
            "tensor-train": {"legal": True, "conversions_per_useful_mac": 0.005}}
    v = h2.verdict(rows, conv, {"baseline": 100, "kron3": 99, "tensor-train": 100})
    assert v["kron3"]["rules"]["accuracy"]["result"] == "killed" and v["kron3"]["verdict"] == "killed"
    assert v["kron3"]["rules"]["conversions"]["result"].startswith("not evaluable")
    assert v["tensor-train"]["verdict"] == "kept"
    assert h2.t_quantile(0.975, 2) == pytest.approx(4.3027, abs=1e-3)


def test_tiny_run_end_to_end(tmp_path):
    cfg = h2.load_config(H2_YAML)
    cfg.update(steps=2, seeds=[101, 102], threads=1, latency={"batch_size": 1, "concurrent_runs": 1, "repeats": 2})
    text = "".join(ALPHABET[(i * 7 + i // 13) % 65] for i in range(6000))
    rep = h2.run_config(cfg, text=text, batch_size=1)  # registered 32 needs > 13 GiB for kron3
    assert len(rep["runs"]) == 6 and set(rep["verdict"]) == {"kron3", "tensor-train"}
    assert all(r["val_loss"] > 0 and r["forward_latency_ms_b1"] > 0 for r in rep["runs"])
    json.dumps(rep)
