"""Consistency checks on the stored E-035 result (results/energy-per-token-host-transfer-energy)."""
import hashlib
import json
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

from awen_models.energy_accounting import NOT_SOURCED, REGISTRATIONS, REQUIRED, check, run  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "energy-per-token-host-transfer-energy"
COUNTS = ROOT / "results" / "conversions-per-token" / "results.json"
ASSUMPTIONS = ROOT.parent / "awen-spec" / "hardware_assumptions.yaml"
# Assumption file the stored run read (README.md); a later edit to the file is a new run, not a test failure.
ASSUMPTIONS_SHA256 = "e0965ba697e23559d0638fdbf31a405f497f898215ec75a3349a0330651d246f"
DESIGNS = {"li2018"}
PJ_TO_J = 1e-12
TOTAL_NOT_DEFINED = "not defined by this registration"


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
    _, version = REGISTRATIONS["e035"]
    assert report["accounting"] == version == "awen.energy-per-token-accounting.v11"
    assert report["evidence_level"] == "L1"
    assert report["counts_source"] == json.loads(COUNTS.read_text())["curve"]
    got = sorted((p["arch"], p["tile_k"], p["positions"]) for p in report["points"])
    assert got == sorted(_counts())
    assert len(got) == 40
    assert report["failures"] == {} and report["verdict"] == "pass"


def test_registered_rule_holds_all_sourced_total_not_defined():
    terms, _ = REGISTRATIONS["e035"]
    for p in _report()["points"]:
        assert check(p) == []
        names = {row["term"] for row in p["terms"]}
        assert set(REQUIRED) <= names == set(terms)
        assert p["total_J"] == TOTAL_NOT_DEFINED
        assert p["not_sourced"] == []
        for row in p["terms"]:
            assert row["status"] != NOT_SOURCED
            assert "designs" in row or "per_token_J" in row or "values" in row or row.get("status", "").startswith("sourced")


def test_host_transfer_energy_recomputes_from_bytes_and_published_points():
    counts = _counts()
    for p in _report()["points"]:
        per_token = counts[(p["arch"], p["tile_k"], p["positions"])]
        row = next(r for r in p["terms"] if r["term"] == "host_transfer")
        assert row["status"].startswith("sourced")
        assert row["parameter"] == "host_link.published_designs"
        assert row["count"] == "total"
        assert row["count_per_token"] == per_token["total"]
        assert row["bytes_per_token"]["1"] == per_token["total"]
        assert row["bytes_per_token"]["2"] == per_token["total"] * 2
        assert {d["source"] for d in row["designs"]} == DESIGNS
        for design in row["designs"]:
            assert design["unit"] == "pJ"
            assert design["bits_per_byte"] == 8
            for width, bytes_ in row["bytes_per_token"].items():
                expected = bytes_ * design["bits_per_byte"] * design["value"] * PJ_TO_J
                assert design["per_token_J_by_width"][width] == pytest.approx(expected, rel=1e-12)


def test_readme_table_matches_stored_host_transfer_energy():
    readme = (RESULTS / "README.md").read_text()
    assert "3,072" in readme and "102,464" in readme
    assert "184.32" in readme and "6.148" in readme
    assert "368.64" in readme and "12.30" in readme
    assert "7.5" in readme and "60.0" in readme


def test_stored_result_reproduces_when_assumptions_unchanged():
    if hashlib.sha256(ASSUMPTIONS.read_bytes()).hexdigest() != ASSUMPTIONS_SHA256:
        pytest.skip("hardware_assumptions.yaml changed since the stored run; rerun E-035 as a new registration")
    fresh = run(json.loads(COUNTS.read_text()), yaml.safe_load(ASSUMPTIONS.read_text()), "e035")
    assert json.loads(json.dumps(fresh)) == _report()
