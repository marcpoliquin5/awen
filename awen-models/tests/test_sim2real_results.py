"""Consistency checks on the stored E-022 result (results/photon-sim2real)."""
import hashlib
import json
from pathlib import Path

import pytest

from awen_models.sim2real import EVAL_SEED_OFFSET, REGISTERED_ENOB_SWEEP, REGISTERED_SEEDS, paired_verdict

RESULTS = Path(__file__).resolve().parents[1] / "results" / "photon-sim2real"


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert hashlib.sha256((RESULTS / name).read_bytes()).hexdigest() == digest, name


def test_registered_design_was_run_unchanged():
    report = _report()
    assert report["version"] == "awen.photon-sim2real.v1"
    assert report["seeds"] == list(REGISTERED_SEEDS) == [1, 2, 3]
    assert report["test_examples"] == 10_000
    assert [p["system_enob"] for i, p in enumerate(report["model_b_points"]) if REGISTERED_ENOB_SWEEP[i] is not None] == [
        e for e in REGISTERED_ENOB_SWEEP if e is not None
    ]
    primary = report["model_b_points"][report["primary_point_index"]]
    assert REGISTERED_ENOB_SWEEP[report["primary_point_index"]] is None
    assert (primary["system_enob"], primary["weight_dac_bits"], primary["extinction_ratio_db"]) == (7.61, 8, 30.0)
    assert sorted({r["regime"] for r in report["runs"]}) == ["noise-aware-100", "noise-aware-1000", "standard"]
    assert len(report["runs"]) == 9  # no run dropped
    assert EVAL_SEED_OFFSET == 20_000


def test_stored_verdicts_follow_from_stored_accuracies():
    report = _report()
    by = {(r["regime"], r["seed"]): r["accuracy_model_b"] for r in report["runs"]}
    for name, rows in report["verdicts_model_b"].items():
        regime, std = name.split(" - ")
        for i, row in enumerate(rows):
            again = paired_verdict([by[(regime, s)][i] - by[(std, s)][i] for s in report["seeds"]])
            assert again["verdict"] == row["verdict"]
            assert again["mean"] == pytest.approx(row["mean"], abs=1e-12)


def test_primary_point_verdict_is_reverses_for_both_regimes():
    report = _report()
    i = report["primary_point_index"]
    assert [rows[i]["verdict"] for rows in report["verdicts_model_b"].values()] == ["reverses", "reverses"]
