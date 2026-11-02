"""Conversions-per-token curve (E-024): closed form, report agreement and CLI on a small grid (CPU, L1)."""

import json

import pytest

pytest.importorskip("transformers")
pytest.importorskip("awen_native")
pytest.importorskip("awen_py")

from awen.compile_llama import TINY_LLAMA, TINY_QWEN3  # noqa: E402
from awen.conversions_curve import expected_counts, main, point, projections, run  # noqa: E402


def test_projections_cover_every_lowered_linear():
    for config in (TINY_LLAMA, TINY_QWEN3):
        shapes = projections(config)
        assert len(shapes) == 7 * config["num_hidden_layers"] + 1
        assert shapes[-1] == (config["hidden_size"], config["vocab_size"])


def test_closed_form_matches_stored_e021_report_at_tile_64():
    # E-021 stored report (qwen3-tiny-7466505.json): 32 positions, tile_k 64.
    assert expected_counts(TINY_QWEN3, 32, 64) == {"dac_input": 34816, "dac_weight": 90112, "adc": 45056,
                                                   "lowered_macs": 2883584}


def test_closed_form_scaling():
    small, large = expected_counts(TINY_LLAMA, 4, 8), expected_counts(TINY_LLAMA, 4, 128)
    assert small["dac_input"] == large["dac_input"] and small["dac_weight"] == large["dac_weight"]
    assert small["adc"] > large["adc"]  # more tiles per output, more ADC reads
    one, many = expected_counts(TINY_LLAMA, 1, 64), expected_counts(TINY_LLAMA, 8, 64)
    assert many["dac_input"] == 8 * one["dac_input"] and many["adc"] == 8 * one["adc"]
    assert many["dac_weight"] == one["dac_weight"]  # programmed once per call


@pytest.mark.parametrize("arch", ["llama", "qwen3"])
@pytest.mark.parametrize("tile_k", [16, 128])
def test_report_counts_match_closed_form(arch, tile_k):
    row = point(arch, tile_k, 4)
    assert row["report_tile_k"] == tile_k
    assert row["matches_closed_form"], (row["counts"], row["expected"])
    assert row["per_token"]["dac_weight"] == row["counts"]["dac_weight"] / 4


def test_run_and_cli(tmp_path):
    result = run(["llama"], [32], [2])
    assert result["verdict"] == "pass" and result["evidence_level"] == "L1"
    assert "not estimated" in result["energy"]
    out = tmp_path / "curve.json"
    assert main(["--archs", "qwen3", "--tile-ks", "64", "--positions", "2", "--out", str(out)]) == 0
    stored = json.loads(out.read_text())
    assert stored["verdict"] == "pass" and len(stored["points"]) == 1
