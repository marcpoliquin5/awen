"""Consistency checks on the stored E-025 result (results/energy-per-token)."""
import hashlib
import json
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

from awen_models.energy_accounting import ACCOUNTING_VERSION, NOT_SOURCED, REQUIRED, TERMS, check, run  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "energy-per-token"
COUNTS = ROOT / "results" / "conversions-per-token" / "results.json"
ASSUMPTIONS = ROOT.parent / "awen-spec" / "hardware_assumptions.yaml"
# Assumption file the stored run read (README.md); a later edit to the file is a new run, not a test failure.
ASSUMPTIONS_SHA256 = "a895ea024c6caa34083a713c5174f77d1901f99b310189b210b2d223fc831f0d"


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert hashlib.sha256((RESULTS / name).read_bytes()).hexdigest() == digest, name


def test_all_e024_points_accounted_and_verdict_pass():
    report, counts = _report(), json.loads(COUNTS.read_text())
    assert report["accounting"] == ACCOUNTING_VERSION
    assert report["evidence_level"] == "L1"
    assert report["counts_source"] == counts["curve"]
    got = sorted((p["arch"], p["tile_k"], p["positions"]) for p in report["points"])
    assert got == sorted((p["arch"], p["tile_k"], p["positions"]) for p in counts["points"])
    assert len(got) == 40
    assert report["failures"] == {} and report["verdict"] == "pass"


def test_registered_rule_holds_at_every_stored_point():
    for p in _report()["points"]:
        assert check(p) == []
        terms = {row["term"] for row in p["terms"]}
        assert set(REQUIRED) <= terms == set(TERMS)
        assert p["total_J"] is None  # no total while any term is not sourced
        for row in p["terms"]:
            if row["status"] == NOT_SOURCED:
                assert "designs" not in row and "values" not in row and "per_token_J" not in row


def test_adc_terms_recompute_from_e024_counts():
    counts = {(p["arch"], p["tile_k"], p["positions"]): p["per_token"]
              for p in json.loads(COUNTS.read_text())["points"]}
    for p in _report()["points"]:
        adc_reads = counts[(p["arch"], p["tile_k"], p["positions"])]["adc"]
        row = next(r for r in p["terms"] if r["term"] == "adc")
        assert row["count_per_token"] == adc_reads
        for design in row["designs"]:
            assert design["unit"] == "pJ"
            assert design["per_token_J"] == pytest.approx(adc_reads * design["value"] * 1e-12, rel=1e-12)


def test_stored_result_reproduces_when_assumptions_unchanged():
    if hashlib.sha256(ASSUMPTIONS.read_bytes()).hexdigest() != ASSUMPTIONS_SHA256:
        pytest.skip("hardware_assumptions.yaml changed since the stored run; rerun E-025 as a new registration")
    fresh = run(json.loads(COUNTS.read_text()), yaml.safe_load(ASSUMPTIONS.read_text()))
    assert json.loads(json.dumps(fresh)) == _report()
