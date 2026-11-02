"""Consistency checks on the stored E-023 result (results/photon-recalibration)."""
import hashlib
import json
from pathlib import Path

import pytest

from awen_models.recalibration import REGISTERED_BUDGETS, REGISTERED_TIMES, TOLERANCE, tolerable_budget
from awen_models.sim2real import paired_verdict

RESULTS = Path(__file__).resolve().parents[1] / "results" / "photon-recalibration"
REGIMES = ["standard", "noise-aware-100", "noise-aware-1000"]


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def _curves(report):
    return {(r["regime"], r["seed"]): [p["accuracy"] for p in r["curve"]] for r in report["runs"]}


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert hashlib.sha256((RESULTS / name).read_bytes()).hexdigest() == digest, name


def test_registered_design_was_run_unchanged():
    report = _report()
    assert report["version"] == "awen.photon-recalibration.v1"
    assert report["seeds"] == [1, 2, 3]
    assert report["test_examples"] == 10_000
    assert report["budgets"] == list(REGISTERED_BUDGETS) == [0.0, 0.01, 0.02, 0.05, 0.1, 0.2]
    assert report["times_per_interval"] == REGISTERED_TIMES == 4
    model = report["model_b"]
    assert (model["system_enob"], model["weight_dac_bits"], model["extinction_ratio_db"]) == (7.61, 8, 30.0)
    assert sorted(r["regime"] for r in report["runs"]) == sorted(REGIMES * 3)  # 9 runs, none dropped
    for run in report["runs"]:
        assert [p["budget"] for p in run["curve"]] == report["budgets"]
        for point in run["curve"]:
            assert len(point["accuracy_at_times"]) == 4
            assert point["accuracy"] == pytest.approx(sum(point["accuracy_at_times"]) / 4, abs=1e-12)
        assert len(set(run["curve"][0]["accuracy_at_times"])) == 1  # no drift at s = 0


def test_stored_tolerable_budgets_follow_from_stored_curves():
    report = _report()
    by = _curves(report)
    for regime in REGIMES:
        again = tolerable_budget([by[(regime, s)] for s in report["seeds"]], report["budgets"], TOLERANCE)
        stored = report["tolerable_budget"][regime]
        assert again["tolerable_budget"] == stored["tolerable_budget"]
        assert [r["passes"] for r in again["rows"]] == [r["passes"] for r in stored["rows"]]


def test_stored_regime_comparisons_follow_from_stored_curves():
    report = _report()
    by = _curves(report)
    for name, rows in report["exploratory_regime_comparisons"].items():
        regime, std = name.split(" - ")
        for i, row in enumerate(rows):
            again = paired_verdict([by[(regime, s)][i] - by[(std, s)][i] for s in report["seeds"]])
            assert again["verdict"] == row["verdict"]


def test_reported_verdict():
    tolerable = {k: v["tolerable_budget"] for k, v in _report()["tolerable_budget"].items()}
    assert tolerable == {"standard": 0.05, "noise-aware-100": 0.1, "noise-aware-1000": 0.05}
