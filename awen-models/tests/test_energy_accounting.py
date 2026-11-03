"""E-025 energy-per-token accounting: sourced terms only, no total while any term is not sourced."""

import copy
import json
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

from awen_models.energy_accounting import (  # noqa: E402
    NOT_SOURCED,
    REQUIRED,
    TERMS,
    TERMS_E026,
    account,
    check,
    find_assumptions,
    main,
    run,
)

ASSUMPTIONS = Path(__file__).resolve().parents[2] / "awen-spec" / "hardware_assumptions.yaml"
# Synthetic counts (not E-024 output): one point, small round numbers.
POINT = {"arch": "llama", "tile_k": 16, "positions": 2,
         "per_token": {"dac_input": 10.0, "dac_weight": 100.0, "adc": 4.0, "total": 114.0}}
COUNTS = {"curve": "synthetic", "points": [POINT]}


@pytest.fixture()
def doc():
    return yaml.safe_load(ASSUMPTIONS.read_text())


def rows(result):
    return {row["term"]: row for row in result["terms"]}


def test_every_required_term_is_listed_and_nulls_stay_not_sourced(doc):
    r = rows(account(doc, POINT))
    assert set(REQUIRED) <= set(r) == set(TERMS)
    for term in ("laser", "modulator", "dac_input", "dac_weight", "weight_hold", "control_electronics",
                 "thermal_stabilization", "memory_movement", "host_transfer"):
        assert r[term]["status"] == NOT_SOURCED and "per_token_J" not in r[term], term


def test_adc_term_uses_each_published_design_without_interpolation(doc):
    adc = rows(account(doc, POINT))["adc"]
    points = doc["parameters"]["adc"]["published_designs"]["points"]
    assert [d["source"] for d in adc["designs"]] == [p["source"] for p in points]
    for d, p in zip(adc["designs"], points):
        assert d["per_token_J"] == pytest.approx(4.0 * p["energy_per_conversion_pJ"] * 1e-12)
    assert adc["source_type"] == "estimate"


def test_detection_floor_is_the_sweep_times_adc_reads(doc):
    floor = rows(account(doc, POINT))["optical_detection_floor"]
    sweep = doc["parameters"]["shot_noise"]["shot_noise_energy_per_detection_J"]["values"]
    assert {k: v["per_token_J"] for k, v in floor["values"].items()} == pytest.approx(
        {str(b): 4.0 * e for b, e in sweep.items()})
    assert "not a system energy" in floor["caveat"]


def test_no_total_and_pass_on_the_current_assumption_set(doc):
    result = run(COUNTS, doc)
    point = result["points"][0]
    assert point["total_J"] is None and point["not_sourced"]
    assert result["verdict"] == "pass" and result["failures"] == {}
    assert result["assumption_set"] == "awen.hardware-assumptions.v0"
    assert "no speedup or savings" in result["claims"]


def test_a_sourced_per_event_value_is_used_with_its_label(doc):
    doc = copy.deepcopy(doc)
    doc["parameters"]["dac"]["energy_per_conversion"].update(typical=2.0, source="hua2025")
    dac = rows(account(doc, POINT))["dac_weight"]
    assert dac["status"] == "sourced" and dac["source_type"] == "measured"
    assert dac["per_token_J"] == pytest.approx(100.0 * 2.0e-12)


def test_check_catches_tampering(doc):
    point = account(doc, POINT)
    bad = copy.deepcopy(point)
    rows(bad)["adc"]["designs"][0]["per_token_J"] *= 2
    bad["total_J"] = 1.0
    bad["terms"] = [row for row in bad["terms"] if row["term"] != "laser"]
    failures = check(bad)
    assert any("recompute" in f for f in failures)
    assert any("total emitted" in f for f in failures)
    assert "missing term laser" in failures
    assert check(point) == []


def test_cli_writes_json(tmp_path):
    counts = tmp_path / "counts.json"
    counts.write_text(json.dumps(COUNTS))
    out = tmp_path / "out" / "r.json"
    assert main(["--counts", str(counts), "--assumptions", str(ASSUMPTIONS), "--out", str(out)]) == 0
    assert json.loads(out.read_text())["verdict"] == "pass"
    assert find_assumptions(Path(__file__).parent) == ASSUMPTIONS.resolve()


def test_e026_dac_terms_use_each_published_design(doc):
    result = run(COUNTS, doc, "e026")
    assert result["accounting"] == "awen.energy-per-token-accounting.v2" and result["verdict"] == "pass"
    point = result["points"][0]
    r = rows(point)
    designs = doc["parameters"]["dac"]["published_designs"]["points"]
    for term, count in (("dac_input", 10.0), ("dac_weight", 100.0)):
        assert r[term]["parameter"] == "dac.published_designs" and r[term]["source_type"] == "estimate"
        assert [d["source"] for d in r[term]["designs"]] == [p["source"] for p in designs]
        for d, p in zip(r[term]["designs"], designs):
            assert d["resolution_bits"] == p["resolution_bits"] and "enob_bits" not in d
            assert d["per_token_J"] == pytest.approx(count * p["energy_per_conversion_pJ"] * 1e-12)
        assert "not those of a photonic driver" in r[term]["caveat"]
    assert "dac_input" not in point["not_sourced"] and "dac_weight" not in point["not_sourced"]
    assert point["total_J"] is None and "modulator" in point["not_sourced"]


def test_e026_changes_only_the_dac_terms_and_e025_is_the_default(doc):
    assert {t for t in TERMS if TERMS[t] != TERMS_E026[t]} == {"dac_input", "dac_weight"}
    e025, e026 = run(COUNTS, doc), run(COUNTS, doc, "e026")
    assert e025["accounting"] == "awen.energy-per-token-accounting.v1"
    assert rows(e025["points"][0])["dac_weight"]["status"] == NOT_SOURCED
    for term in set(TERMS) - {"dac_input", "dac_weight"}:
        assert rows(e025["points"][0])[term] == rows(e026["points"][0])[term], term
    assert len(e025["points"][0]["not_sourced"]) - len(e026["points"][0]["not_sourced"]) == 2


def test_cli_registration_flag(tmp_path):
    counts = tmp_path / "counts.json"
    counts.write_text(json.dumps(COUNTS))
    out = tmp_path / "r.json"
    assert main(["--counts", str(counts), "--assumptions", str(ASSUMPTIONS), "--registration", "e026",
                 "--out", str(out)]) == 0
    assert json.loads(out.read_text())["accounting"] == "awen.energy-per-token-accounting.v2"
