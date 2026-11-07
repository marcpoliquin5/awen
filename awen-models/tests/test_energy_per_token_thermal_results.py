"""Consistency checks on the stored E-033 result (results/energy-per-token-thermal)."""
import hashlib
import json
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

from awen_models.energy_accounting import NOT_SOURCED, REGISTRATIONS, REQUIRED, check, run  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "energy-per-token-thermal"
COUNTS = ROOT / "results" / "conversions-per-token" / "results.json"
ASSUMPTIONS = ROOT.parent / "awen-spec" / "hardware_assumptions.yaml"
# Assumption file the stored run read (README.md); a later edit to the file is a new run, not a test failure.
ASSUMPTIONS_SHA256 = "0214ea6ef0b53bd591729f1c40aa2b158ddf7e5b2c06f29e407975a49e0d7154"
STILL_NOT_SOURCED = {"control_electronics", "host_transfer"}
DESIGNS = {"zortman2012"}
FJ_TO_J = 1e-15


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def _counts():
    return {(p["arch"], p["tile_k"], p["positions"]): p["per_token"]
            for p in json.loads(COUNTS.read_text())["points"]}


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert hashlib.sha256((RESULTS / name).read_bytes()).hexdigest() == digest, name


def test_all_e024_points_accounted_and_verdict_pass():
    report = _report()
    _, version = REGISTRATIONS["e033"]
    assert report["accounting"] == version == "awen.energy-per-token-accounting.v9"
    assert report["evidence_level"] == "L1"
    assert report["counts_source"] == json.loads(COUNTS.read_text())["curve"]
    got = sorted((p["arch"], p["tile_k"], p["positions"]) for p in report["points"])
    assert got == sorted(_counts())
    assert len(got) == 40
    assert report["failures"] == {} and report["verdict"] == "pass"


def test_registered_rule_holds_and_no_total():
    terms, _ = REGISTRATIONS["e033"]
    for p in _report()["points"]:
        assert check(p) == []
        names = {row["term"] for row in p["terms"]}
        assert set(REQUIRED) <= names == set(terms)
        assert p["total_J"] is None
        assert set(p["not_sourced"]) == STILL_NOT_SOURCED
        for row in p["terms"]:
            if row["status"] == NOT_SOURCED:
                assert "designs" not in row and "values" not in row and "per_token_J" not in row


def test_thermal_energy_recomputes_from_dac_input_and_published_points():
    counts = _counts()
    for p in _report()["points"]:
        per_token = counts[(p["arch"], p["tile_k"], p["positions"])]
        row = next(r for r in p["terms"] if r["term"] == "thermal_stabilization")
        assert row["status"].startswith("sourced")
        assert row["parameter"] == "thermal.published_designs"
        assert row["count"] == "dac_input"
        assert row["count_per_token"] == per_token["dac_input"]
        assert {d["source"] for d in row["designs"]} == DESIGNS
        for design in row["designs"]:
            assert design["unit"] == "fJ"
            expected = per_token["dac_input"] * design["value"] * FJ_TO_J
            assert design["per_token_J"] == pytest.approx(expected, rel=1e-12)


def test_readme_table_matches_stored_thermal_energy():
    readme = (RESULTS / "README.md").read_text()
    assert "1,088" in readme
    assert "0.1958" in readme and "195.84" in readme
    assert "180" in readme


def test_stored_result_reproduces_when_assumptions_unchanged():
    if hashlib.sha256(ASSUMPTIONS.read_bytes()).hexdigest() != ASSUMPTIONS_SHA256:
        pytest.skip("hardware_assumptions.yaml changed since the stored run; rerun E-033 as a new registration")
    fresh = run(json.loads(COUNTS.read_text()), yaml.safe_load(ASSUMPTIONS.read_text()), "e033")
    assert json.loads(json.dumps(fresh)) == _report()
