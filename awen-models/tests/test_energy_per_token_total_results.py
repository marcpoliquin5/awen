"""Consistency checks on the stored E-036 result (results/energy-per-token-total)."""
import hashlib
import json
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

from awen_models.energy_accounting import REGISTRATIONS, REQUIRED, check, run  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "energy-per-token-total"
COUNTS = ROOT / "results" / "conversions-per-token" / "results.json"
ASSUMPTIONS = ROOT.parent / "awen-spec" / "hardware_assumptions.yaml"
# Assumption file the stored run read (README.md); a later edit to the file is a new run, not a test failure.
ASSUMPTIONS_SHA256 = "e0965ba697e23559d0638fdbf31a405f497f898215ec75a3349a0330651d246f"
N_SCENARIOS = 288
DML_MIN_LO = 3.24613545984e-07
DML_MIN_HI = 1.31149409134592e-05
DML_MAX_LO = 6.824245632e-07
DML_MAX_HI = 2.6729576168960003e-05
MOD_MIN_LO = 3.24429576064e-07
MOD_MIN_HI = 1.31147569435392e-05
MOD_MAX_LO = 6.820586144000001e-07
MOD_MAX_HI = 2.6729210220160003e-05


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
    _, version = REGISTRATIONS["e036"]
    assert report["accounting"] == version == "awen.energy-per-token-accounting.v12"
    assert report["evidence_level"] == "L1"
    assert report["claims"] == "none: scenario envelopes only, no speedup or savings figure"
    assert report["counts_source"] == json.loads(COUNTS.read_text())["curve"]
    got = sorted((p["arch"], p["tile_k"], p["positions"]) for p in report["points"])
    assert got == sorted(_counts())
    assert len(got) == 40
    assert report["failures"] == {} and report["verdict"] == "pass"


def test_registered_rule_holds_structured_envelopes():
    terms, _ = REGISTRATIONS["e036"]
    for p in _report()["points"]:
        assert check(p) == []
        names = {row["term"] for row in p["terms"]}
        assert set(REQUIRED) <= names == set(terms)
        assert p["not_sourced"] == []
        total = p["total_J"]
        assert total["rule"] == "scenario_envelope_v1"
        assert total["unit"] == "J"
        assert "not one physical device" in total["disclosure"]
        assert "scenario envelopes under scenario_envelope_v1" in p["total_reason"]
        dml = total["by_transmitter"]["dml_laser"]
        mod = total["by_transmitter"]["modulator_eo"]
        assert dml["excluded_terms"] == ["modulator"]
        assert mod["excluded_terms"] == ["laser"]
        assert dml["n_scenarios"] == mod["n_scenarios"] == N_SCENARIOS
        assert dml["min_J"] <= dml["max_J"]
        assert mod["min_J"] <= mod["max_J"]
        for row in p["terms"]:
            assert row["status"] != "not sourced"
            assert "designs" in row or "per_token_J" in row or "values" in row or row.get("status", "").startswith("sourced")


def test_envelope_ranges_and_identical_architectures():
    points = _report()["points"]
    dml_mins = [p["total_J"]["by_transmitter"]["dml_laser"]["min_J"] for p in points]
    dml_maxs = [p["total_J"]["by_transmitter"]["dml_laser"]["max_J"] for p in points]
    mod_mins = [p["total_J"]["by_transmitter"]["modulator_eo"]["min_J"] for p in points]
    mod_maxs = [p["total_J"]["by_transmitter"]["modulator_eo"]["max_J"] for p in points]
    assert min(dml_mins) == DML_MIN_LO and max(dml_mins) == DML_MIN_HI
    assert min(dml_maxs) == DML_MAX_LO and max(dml_maxs) == DML_MAX_HI
    assert min(mod_mins) == MOD_MIN_LO and max(mod_mins) == MOD_MIN_HI
    assert min(mod_maxs) == MOD_MAX_LO and max(mod_maxs) == MOD_MAX_HI
    by_key = {(p["tile_k"], p["positions"]): [] for p in points}
    for p in points:
        by_key[(p["tile_k"], p["positions"])].append(p)
    for pair in by_key.values():
        assert len(pair) == 2
        a, b = pair
        assert a["arch"] != b["arch"]
        assert a["total_J"] == b["total_J"]


def test_minmax_picks_are_identical_at_every_point():
    points = _report()["points"]
    first = points[0]["total_J"]["by_transmitter"]
    for p in points:
        bt = p["total_J"]["by_transmitter"]
        assert bt["dml_laser"]["min_picks"] == first["dml_laser"]["min_picks"]
        assert bt["dml_laser"]["max_picks"] == first["dml_laser"]["max_picks"]
        assert bt["modulator_eo"]["min_picks"] == first["modulator_eo"]["min_picks"]
        assert bt["modulator_eo"]["max_picks"] == first["modulator_eo"]["max_picks"]
    dml_min = first["dml_laser"]["min_picks"]
    dml_max = first["dml_laser"]["max_picks"]
    mod_min = first["modulator_eo"]["min_picks"]
    mod_max = first["modulator_eo"]["max_picks"]
    assert dml_min["adc"]["source"] == "lagos2021"
    assert dml_min["dac_input"]["source"] == dml_min["dac_weight"]["source"] == "sedighi2012"
    assert dml_min["host_transfer"] == {"basis": "phy_transceiver_tx_rx", "source": "li2018", "width": "1"}
    assert dml_min["laser"]["source"] == "diamantopoulos2021"
    assert "modulator" not in dml_min
    assert dml_max["adc"]["source"] == "ramkaj2020"
    assert dml_max["dac_input"]["source"] == dml_max["dac_weight"]["source"] == "palmers2010"
    assert dml_max["host_transfer"]["width"] == "2"
    assert mod_min["modulator"]["source"] == "timurdogan2014"
    assert "laser" not in mod_min
    assert mod_max["modulator"]["basis"] == "per_full_swing_transition"


def test_readme_table_matches_stored_envelopes():
    readme = (RESULTS / "README.md").read_text()
    assert "scenario_envelope_v1" in readme
    assert "0.3246" in readme and "26.73" in readme
    assert "0.3244" in readme and "13.11" in readme
    assert "0.6824" in readme and "0.6821" in readme
    assert "288" in readme
    assert "dml_laser" in readme and "modulator_eo" in readme


def test_stored_result_reproduces_when_assumptions_unchanged():
    if hashlib.sha256(ASSUMPTIONS.read_bytes()).hexdigest() != ASSUMPTIONS_SHA256:
        pytest.skip("hardware_assumptions.yaml changed since the stored run; rerun E-036 as a new registration")
    fresh = run(json.loads(COUNTS.read_text()), yaml.safe_load(ASSUMPTIONS.read_text()), "e036")
    assert json.loads(json.dumps(fresh)) == _report()
