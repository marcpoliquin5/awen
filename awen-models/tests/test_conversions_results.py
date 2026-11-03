"""Consistency checks on the stored E-024 result (results/conversions-per-token)."""
import hashlib
import json
from pathlib import Path

from awen.compile_llama import ARCHITECTURES
from awen.conversions_curve import ARCHS, CURVE_VERSION, POSITIONS, TILE_KS, expected_counts

RESULTS = Path(__file__).resolve().parents[1] / "results" / "conversions-per-token"


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert hashlib.sha256((RESULTS / name).read_bytes()).hexdigest() == digest, name


def test_registered_grid_was_run_unchanged():
    report = _report()
    assert report["curve"] == CURVE_VERSION
    assert report["evidence_level"] == "L1"
    assert report["use_cache"] is False
    assert report["grid"] == {"archs": list(ARCHS), "tile_ks": list(TILE_KS), "positions": list(POSITIONS)}
    got = sorted((p["arch"], p["tile_k"], p["positions"]) for p in report["points"])
    assert got == sorted((a, t, n) for a in ARCHS for t in TILE_KS for n in POSITIONS)  # 40 points, none dropped
    assert report["energy"].startswith("not estimated")


def test_stored_counts_equal_closed_form_and_verdict_follows():
    report = _report()
    for p in report["points"]:
        config = ARCHITECTURES[p["arch"]][2]
        assert p["counts"] == p["expected"] == expected_counts(config, p["positions"], p["tile_k"])
        assert p["matches_closed_form"] is True
        assert p["report_tile_k"] == p["tile_k"]
    assert report["verdict"] == "pass"


def test_per_token_values_follow_from_totals():
    for p in _report()["points"]:
        n, c, t = p["positions"], p["counts"], p["per_token"]
        for key in ("dac_input", "dac_weight", "adc"):
            assert t[key] == c[key] / n
        assert t["total"] == t["dac_input"] + t["dac_weight"] + t["adc"]
        assert p["conversions_per_mac"] == (c["dac_input"] + c["dac_weight"] + c["adc"]) / c["lowered_macs"]


def test_e020_e021_point_reproduced():
    for p in _report()["points"]:
        if (p["tile_k"], p["positions"]) == (64, 32):
            assert p["counts"]["dac_input"] == 34_816
            assert p["counts"]["dac_weight"] == 90_112
            assert p["counts"]["adc"] == 45_056
