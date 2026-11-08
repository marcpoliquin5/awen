"""Unit tests for the E-037 energy-per-MAC curve on synthetic inputs (the registered run is not run here)."""
import copy
import json

import pytest

from awen_models import energy_per_mac as epm


def _env(lo, hi, excluded):
    return {"excluded_terms": [excluded], "min_J": lo, "max_J": hi, "n_scenarios": 4,
            "min_picks": {"adc": {"source": "a"}}, "max_picks": {"adc": {"source": "b"}}}


def _inputs(points=(("llama", 8, 1), ("qwen3", 16, 4)), macs=100):
    counts = {"curve": epm.COUNTS_CURVE, "verdict": "pass", "points": []}
    totals = {"accounting": epm.TOTALS_ACCOUNTING, "verdict": "pass", "assumption_set": "synthetic", "points": []}
    for arch, tile, pos in points:
        per_token = {"adc": 10.0, "dac_input": 5.0, "dac_weight": 20.0 / pos, "total": 15.0 + 20.0 / pos}
        counts["points"].append({"arch": arch, "tile_k": tile, "positions": pos, "per_token": per_token,
                                 "counts": {"lowered_macs": macs * pos}})
        totals["points"].append({
            "arch": arch, "tile_k": tile, "positions": pos, "per_token_counts": per_token,
            "terms": [{"term": "optical_detection_floor",
                       "values": {"4": {"per_token_J": 1e-12}, "8": {"per_token_J": 2e-10}}}],
            "total_J": {"rule": epm.TOTAL_RULE, "by_transmitter": {
                "dml_laser": _env(2e-6, 5e-6, "modulator"), "modulator_eo": _env(1e-6, 4e-6, "laser")}}})
    return totals, counts


def test_pass_and_values_divide_by_lowered_macs_per_token():
    totals, counts = _inputs()
    out = epm.run(totals, counts)
    assert out["verdict"] == "pass", out["failures"]
    assert out["curve"] == epm.CURVE and out["evidence_level"] == "L1"
    for point in out["points"]:
        assert point["lowered_macs_per_token"] == 100
        dml = point["by_transmitter"]["dml_laser"]
        assert dml["min_J_per_mac"] == pytest.approx(2e-8) and dml["max_J_per_mac"] == pytest.approx(5e-8)
        assert point["optical_detection_floor_J_per_mac"]["by_effective_bits"]["8"] == pytest.approx(2e-12)
        assert point["conversions_per_mac"] == pytest.approx(point["conversions_per_token"] / 100)
    assert "no speedup or savings" in out["claims"]


def test_macs_must_divide_positions_exactly():
    totals, counts = _inputs()
    counts["points"][1]["counts"]["lowered_macs"] = 401
    out = epm.run(totals, counts)
    assert out["verdict"] == "fail"
    assert any("positive multiple" in f for f in out["failures"]["qwen3/tile16/P4"])


def test_non_envelope_total_fails():
    totals, counts = _inputs()
    totals["points"][0]["total_J"] = "not defined by this registration"
    out = epm.run(totals, counts)
    assert out["verdict"] == "fail" and "llama/tile8/P1" in out["failures"]


def test_mismatched_grid_and_counts_fail():
    totals, counts = _inputs()
    counts["points"] = counts["points"][:1]
    assert "inputs" in epm.run(totals, counts)["failures"]
    totals, counts = _inputs()
    totals["points"][0]["per_token_counts"] = dict(totals["points"][0]["per_token_counts"], adc=11.0)
    out = epm.run(totals, counts)
    assert any("per-token counts differ" in f for f in out["failures"]["llama/tile8/P1"])


def test_failed_or_wrong_inputs_fail():
    totals, counts = _inputs()
    bad = copy.deepcopy(totals)
    bad["verdict"] = "fail"
    assert "inputs" in epm.run(bad, counts)["failures"]
    out = epm.run(totals, counts, input_sha256={"totals": "0" * 64, "counts": "0" * 64})
    assert out["failures"]["inputs"] == ["input SHA-256 differs from the registered inputs"]


def test_check_catches_tampered_point():
    totals, counts = _inputs()
    point = epm.run(totals, counts)["points"][0]
    point["by_transmitter"]["modulator_eo"]["max_J_per_mac"] *= 2
    assert any("do not recompute" in f for f in epm.check(point))
    point = epm.run(totals, counts)["points"][0]
    point["optical_detection_floor_J_per_mac"]["by_effective_bits"]["8"] = 1.0
    assert any("detection floor exceeds" in f for f in epm.check(point))


def test_cli_records_input_hashes(tmp_path):
    totals, counts = _inputs()
    (tmp_path / "t.json").write_text(json.dumps(totals))
    (tmp_path / "c.json").write_text(json.dumps(counts))
    assert epm.main(["--totals", str(tmp_path / "t.json"), "--counts", str(tmp_path / "c.json"),
                     "--out", str(tmp_path / "o" / "r.json")]) == 0
    out = json.loads((tmp_path / "o" / "r.json").read_text())
    assert set(out["input_sha256"]) == {"totals", "counts"}
    # synthetic inputs are not the registered stored results, so the registered input check fails
    assert out["verdict"] == "fail" and "inputs" in out["failures"]
