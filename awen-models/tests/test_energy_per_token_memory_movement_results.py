"""Consistency checks on the stored E-029 result (results/energy-per-token-memory-movement)."""
import hashlib
import json
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

from awen_models.energy_accounting import NOT_SOURCED, REGISTRATIONS, REQUIRED, check, run  # noqa: E402

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "energy-per-token-memory-movement"
COUNTS = ROOT / "results" / "conversions-per-token" / "results.json"
ASSUMPTIONS = ROOT.parent / "awen-spec" / "hardware_assumptions.yaml"
# Assumption file the stored run read (README.md); a later edit to the file is a new run, not a test failure.
ASSUMPTIONS_SHA256 = "0b7ed6abec9637b0eef875020a572be30ba13b7be9576cac4116f07952ecb930"
STILL_NOT_SOURCED = {"laser", "weight_hold", "control_electronics", "thermal_stabilization", "memory_movement",
                     "host_transfer"}


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
    _, version = REGISTRATIONS["e029"]
    assert report["accounting"] == version == "awen.energy-per-token-accounting.v5"
    assert report["evidence_level"] == "L1"
    assert report["counts_source"] == json.loads(COUNTS.read_text())["curve"]
    got = sorted((p["arch"], p["tile_k"], p["positions"]) for p in report["points"])
    assert got == sorted(_counts())
    assert len(got) == 40
    assert report["failures"] == {} and report["verdict"] == "pass"


def test_registered_rule_holds_and_no_total():
    terms, _ = REGISTRATIONS["e029"]
    for p in _report()["points"]:
        assert check(p) == []
        names = {row["term"] for row in p["terms"]}
        assert set(REQUIRED) <= names == set(terms)
        assert p["total_J"] is None
        assert set(p["not_sourced"]) == STILL_NOT_SOURCED
        for row in p["terms"]:
            if row["status"] == NOT_SOURCED:
                assert "designs" not in row and "values" not in row and "per_token_J" not in row


def test_memory_movement_placements_match_counts():
    counts = _counts()
    for p in _report()["points"]:
        per_token = counts[(p["arch"], p["tile_k"], p["positions"])]
        row = next(r for r in p["terms"] if r["term"] == "memory_movement")
        assert row["status"] == NOT_SOURCED
        assert row["parameter"] is None
        streamed = row["placements"]["weights_streamed"]
        resident = row["placements"]["weights_resident"]
        assert streamed["read_elements"] == per_token["dac_input"] + per_token["dac_weight"]
        assert streamed["write_elements"] == per_token["adc"]
        assert resident["read_elements"] == per_token["dac_input"]
        assert resident["write_elements"] == per_token["adc"]
        for placement in (streamed, resident):
            elements = placement["read_elements"] + placement["write_elements"]
            assert placement["bytes_per_token"] == {"1": elements, "2": 2 * elements}
        # streamed element total equals E-028 host-transfer code count by construction
        assert streamed["read_elements"] + streamed["write_elements"] == per_token["total"]


def test_readme_table_matches_stored_streamed_bytes():
    readme = (RESULTS / "README.md").read_text()
    for p in _report()["points"]:
        row = next(r for r in p["terms"] if r["term"] == "memory_movement")
        streamed = int(row["placements"]["weights_streamed"]["bytes_per_token"]["1"])
        resident = int(row["placements"]["weights_resident"]["bytes_per_token"]["1"])
        assert f"{streamed:,}" in readme
        assert f"{resident:,}" in readme


def test_stored_result_reproduces_when_assumptions_unchanged():
    if hashlib.sha256(ASSUMPTIONS.read_bytes()).hexdigest() != ASSUMPTIONS_SHA256:
        pytest.skip("hardware_assumptions.yaml changed since the stored run; rerun E-029 as a new registration")
    fresh = run(json.loads(COUNTS.read_text()), yaml.safe_load(ASSUMPTIONS.read_text()), "e029")
    assert json.loads(json.dumps(fresh)) == _report()
