"""E-039 time axis: closed form, registered pins, published points, synthetic E-023-shaped input."""
import json
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

from awen_models import recalibration_time_axis as rta  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
ASSUMPTIONS = ROOT.parent / "awen-spec" / "hardware_assumptions.yaml"
E023 = ROOT / "results" / "photon-recalibration" / "results.json"


def _synthetic():
    rows = [{"budget": b, "passes": b <= 0.05} for b in (0.0, 0.01, 0.05, 0.1)]
    return {"tolerable_budget": {"standard": {"tolerable_budget": 0.05, "rows": rows},
                                 "flat": {"tolerable_budget": 0.0, "rows": rows}}}


POINTS = [{"source": "x", "basis": "a", "sigma_rad_per_sqrt_s": 0.2}]


def test_interval_closed_form_and_guards():
    assert rta.interval_s(0.05, 0.25, 1.0) == pytest.approx(0.04)
    assert rta.interval_s(0.1, 0.15, 0.5) == pytest.approx((0.1 / 0.075) ** 2)
    assert rta.interval_s(0.0, 0.25, 1.0) == 0.0
    for bad in [(0.1, 0.0, 1.0), (0.1, 0.25, 0.0), (-0.1, 0.25, 1.0)]:
        with pytest.raises(ValueError):
            rta.interval_s(*bad)


def test_matches_e023_interval_helper():
    rec = pytest.importorskip("awen_models.recalibration")
    assert rta.interval_s(0.05, 0.25, 1.0) == pytest.approx(rec.interval_for_budget(0.05, 0.25))


def test_build_and_check_on_synthetic_input():
    recal = _synthetic()
    report = rta.build(recal, POINTS, {"r1": 1.0, "r2": 0.5})
    assert rta.check(report, recal) == []
    std = report["regimes"]["standard"]
    assert len(std["entries"]) == 2 and set(std["entries"][0]["interval_s_by_budget"]) == {"0.01", "0.05", "0.1"}
    e = std["entries"][1]
    assert e["drift_rate_weight_per_sqrt_s"] == pytest.approx(0.1)
    assert e["tolerable_interval_s"] == pytest.approx(0.25)
    assert report["regimes"]["flat"]["entries"][0]["tolerable_interval_s"] is None
    report["regimes"]["standard"]["entries"][0]["interval_s_by_budget"]["0.05"] *= 1.01
    report["regimes"]["flat"]["tolerable_budget"] = 0.01
    failures = rta.check(report, recal)
    assert any("closed form" in f for f in failures) and any("tolerable budget changed" in f for f in failures)


def test_published_points_match_registration():
    data = yaml.safe_load(ASSUMPTIONS.read_text())
    pts = rta.published_points(data)
    assert {(p["basis"], p["sigma_rad_per_sqrt_s"]) for p in pts} == {
        ("brownian_fit_to_phase_distribution", 0.25), ("brownian_fit_to_feedback_trace", 0.15)}
    assert data["sources"]["elmas2025"]["doi"] == "10.1364/AO.560370"
    data["parameters"]["drift"]["published_designs"]["points"][0]["sigma_rad_per_sqrt_s"] = 0.3
    with pytest.raises(ValueError, match="registered"):
        rta.published_points(data)


def test_run_refuses_unpinned_recalibration_input(tmp_path):
    bad = tmp_path / "results.json"
    bad.write_text(json.dumps(_synthetic()))
    with pytest.raises(ValueError, match="SHA-256"):
        rta.run("e039", bad, ASSUMPTIONS)


def test_stored_e023_input_matches_pin():
    assert rta.sha256(E023) == rta.REGISTERED["e039"]["recalibration_sha256"]


def test_sensitivities_are_the_registered_bounds():
    assert rta.REGISTERED["e039"]["sensitivities"] == {"phase_to_amplitude_max": 1.0, "mzi_internal_half_angle_max": 0.5}
    assert "proxy" in rta.DISCLOSURE and "not a device prediction" in rta.DISCLOSURE
