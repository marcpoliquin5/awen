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
    TERMS_E027,
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


def test_e027_modulator_term_uses_each_published_point(doc):
    result = run(COUNTS, doc, "e027")
    assert result["accounting"] == "awen.energy-per-token-accounting.v3" and result["verdict"] == "pass"
    point = result["points"][0]
    mod = rows(point)["modulator"]
    pts = doc["parameters"]["modulator"]["published_designs"]["points"]
    assert mod["parameter"] == "modulator.published_designs" and mod["count"] == "dac_input"
    assert mod["source_type"] == "measured" and "not the energy of an analog" in mod["caveat"]
    assert [(d["source"], d["basis"]) for d in mod["designs"]] == [(p["source"], p["basis"]) for p in pts]
    for d, p in zip(mod["designs"], pts):
        assert d["unit"] == "fJ" and d["per_token_J"] == pytest.approx(10.0 * p["energy_per_symbol_fJ"] * 1e-15)
    assert "modulator" not in point["not_sourced"] and len(point["not_sourced"]) == 6
    assert point["total_J"] is None


def test_e027_changes_only_the_modulator_term_from_e026(doc):
    assert {t for t in TERMS if TERMS_E026[t] != TERMS_E027[t]} == {"modulator"}
    e026, e027 = run(COUNTS, doc, "e026"), run(COUNTS, doc, "e027")
    for term in set(TERMS) - {"modulator"}:
        assert rows(e026["points"][0])[term] == rows(e027["points"][0])[term], term
    assert rows(e026["points"][0])["modulator"]["status"] == NOT_SOURCED


def test_e027_check_catches_a_tampered_modulator_point(doc):
    point = account(doc, POINT, TERMS_E027)
    rows(point)["modulator"]["designs"][1]["per_token_J"] *= 3
    assert any(f.startswith("modulator") for f in check(point))


def test_e028_counts_host_transfer_bytes_and_keeps_energy_not_sourced(doc):
    from awen_models.energy_accounting import HOST_CODE_BYTES, TERMS_E028

    host = rows(account(doc, POINT, TERMS_E028))["host_transfer"]
    assert host["count_per_token"] == 114.0  # 10 input + 100 weight + 4 ADC codes per token
    assert host["bytes_per_token"] == {str(w): 114.0 * w for w in HOST_CODE_BYTES} == {"1": 114.0, "2": 228.0}
    assert host["status"] == NOT_SOURCED and "values" not in host and "per_token_J" not in host
    assert check(account(doc, POINT, TERMS_E028)) == []


def test_e028_changes_only_the_host_transfer_term_from_e027(doc):
    from awen_models.energy_accounting import TERMS_E028

    a, b = rows(account(doc, POINT, TERMS_E027)), rows(account(doc, POINT, TERMS_E028))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["host_transfer"]
    assert run(COUNTS, doc, "e028")["verdict"] == "pass"
    assert run(COUNTS, doc, "e028")["accounting"] == "awen.energy-per-token-accounting.v4"


def test_e028_uses_a_sourced_energy_per_byte_per_packing_width(doc):
    from awen_models.energy_accounting import TERMS_E028

    sourced = copy.deepcopy(doc)
    sourced["parameters"]["system"]["host_transfer_energy"].update(typical=5.0, source="hua2025")
    host = rows(account(sourced, POINT, TERMS_E028))["host_transfer"]
    assert host["status"].startswith("sourced")
    assert host["values"]["1"]["per_token_J"] == pytest.approx(114.0 * 5.0e-12)
    assert host["values"]["2"]["per_token_J"] == pytest.approx(228.0 * 5.0e-12)
    assert check(account(sourced, POINT, TERMS_E028)) == []


def test_e028_check_catches_tampered_bytes_and_hidden_values(doc):
    from awen_models.energy_accounting import TERMS_E028

    point = account(doc, POINT, TERMS_E028)
    bad = copy.deepcopy(point)
    rows(bad)["host_transfer"]["bytes_per_token"]["2"] = 200.0
    assert any("bytes do not equal" in f for f in check(bad))
    hidden = copy.deepcopy(point)
    rows(hidden)["host_transfer"]["values"] = {"1": {"bytes_per_token": 114.0, "value": 1.0, "unit": "pJ",
                                                     "per_token_J": 114.0e-12}}
    assert any("not sourced but has a contribution" in f for f in check(hidden))


def test_e029_counts_memory_bytes_per_placement_and_keeps_energy_not_sourced(doc):
    from awen_models.energy_accounting import TERMS_E029

    mem = rows(account(doc, POINT, TERMS_E029))["memory_movement"]
    assert mem["status"] == NOT_SOURCED and mem["parameter"] is None and "per_token_J" not in mem
    streamed, resident = mem["placements"]["weights_streamed"], mem["placements"]["weights_resident"]
    assert (streamed["read_elements"], streamed["write_elements"]) == (110.0, 4.0)  # 10 input + 100 weight, 4 ADC
    assert streamed["bytes_per_token"] == {"1": 114.0, "2": 228.0}
    assert (resident["read_elements"], resident["write_elements"]) == (10.0, 4.0)  # weights never re-read
    assert resident["bytes_per_token"] == {"1": 14.0, "2": 28.0}
    assert check(account(doc, POINT, TERMS_E029)) == []


def test_e029_changes_only_the_memory_term_from_e028_and_old_registrations_are_unchanged(doc):
    from awen_models.energy_accounting import TERMS_E028, TERMS_E029

    a, b = rows(account(doc, POINT, TERMS_E028)), rows(account(doc, POINT, TERMS_E029))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["memory_movement"]
    assert a["memory_movement"]["reason"] == "no parameter in the assumption set"
    result = run(COUNTS, doc, "e029")
    assert result["verdict"] == "pass" and result["accounting"] == "awen.energy-per-token-accounting.v5"
    assert result["points"][0]["total_J"] is None
    assert run(COUNTS, doc, "e028")["accounting"] == "awen.energy-per-token-accounting.v4"


def test_e029_check_catches_tampered_counts_bytes_and_hidden_energy(doc):
    from awen_models.energy_accounting import TERMS_E029

    point = account(doc, POINT, TERMS_E029)
    elements = copy.deepcopy(point)
    rows(elements)["memory_movement"]["placements"]["weights_resident"]["read_elements"] = 110.0
    assert any("elements do not recompute" in f for f in check(elements))
    size = copy.deepcopy(point)
    rows(size)["memory_movement"]["placements"]["weights_streamed"]["bytes_per_token"]["2"] = 200.0
    assert any("bytes do not equal" in f for f in check(size))
    hidden = copy.deepcopy(point)
    rows(hidden)["memory_movement"]["placements"]["weights_streamed"]["per_token_J"] = 1e-9
    assert any("unregistered field" in f for f in check(hidden))
    dropped = copy.deepcopy(point)
    del rows(dropped)["memory_movement"]["placements"]["weights_resident"]
    assert any("elements do not recompute" in f for f in check(dropped))
    assert any("registered two" in f for f in check(dropped))


def test_e030_applies_published_memory_energy_per_placement(doc):
    from awen_models.energy_accounting import BITS_PER_BYTE, TERMS_E030

    mem = rows(account(doc, POINT, TERMS_E030))["memory_movement"]
    assert mem["status"].startswith("sourced") and mem["parameter"] == "memory.published_designs"
    assert mem["source_type"] == "measured"
    pts = doc["parameters"]["memory"]["published_designs"]["points"]
    assert len(mem["designs"]) == len(pts) == 1
    design = mem["designs"][0]
    assert design["source"] == "oconnor2017" and design["value"] == 3.97 and design["unit"] == "pJ"
    assert design["bits_per_byte"] == BITS_PER_BYTE == 8
    streamed = mem["placements"]["weights_streamed"]["bytes_per_token"]
    resident = mem["placements"]["weights_resident"]["bytes_per_token"]
    assert design["per_token_J_by_placement"]["weights_streamed"]["1"] == pytest.approx(
        streamed["1"] * 8 * 3.97e-12)
    assert design["per_token_J_by_placement"]["weights_resident"]["2"] == pytest.approx(
        resident["2"] * 8 * 3.97e-12)
    assert check(account(doc, POINT, TERMS_E030)) == []


def test_e030_changes_only_the_memory_term_from_e029_and_old_registrations_are_unchanged(doc):
    from awen_models.energy_accounting import TERMS_E029, TERMS_E030

    a, b = rows(account(doc, POINT, TERMS_E029)), rows(account(doc, POINT, TERMS_E030))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["memory_movement"]
    assert a["memory_movement"]["status"] == NOT_SOURCED
    assert "designs" not in a["memory_movement"]
    result = run(COUNTS, doc, "e030")
    assert result["verdict"] == "pass" and result["accounting"] == "awen.energy-per-token-accounting.v6"
    assert result["points"][0]["total_J"] is None
    assert set(result["points"][0]["not_sourced"]) == {
        "laser", "weight_hold", "control_electronics", "thermal_stabilization", "host_transfer"}
    assert run(COUNTS, doc, "e029")["accounting"] == "awen.energy-per-token-accounting.v5"


def test_e030_check_catches_tampered_energy_and_rejects_hidden_total(doc):
    from awen_models.energy_accounting import TERMS_E030

    point = account(doc, POINT, TERMS_E030)
    bad = copy.deepcopy(point)
    rows(bad)["memory_movement"]["designs"][0]["per_token_J_by_placement"]["weights_streamed"]["1"] = 1e-6
    assert any("contribution does not recompute" in f for f in check(bad))
    hidden = copy.deepcopy(point)
    rows(hidden)["memory_movement"]["status"] = NOT_SOURCED
    assert any("not sourced but has a contribution" in f for f in check(hidden))
    total = copy.deepcopy(point)
    total["not_sourced"] = ["laser"]
    total["total_J"] = 1e-9
    assert any("total emitted" in f for f in check(total))


def test_e031_applies_published_weight_hold_energy_per_weight_dac(doc):
    from awen_models.energy_accounting import TERMS_E031

    hold = rows(account(doc, POINT, TERMS_E031))["weight_hold"]
    assert hold["status"].startswith("sourced") and hold["parameter"] == "phase_shifter.published_designs"
    assert hold["source_type"] == "estimate"
    assert hold["count_per_token"] == POINT["per_token"]["dac_weight"]
    pts = doc["parameters"]["phase_shifter"]["published_designs"]["points"]
    assert len(hold["designs"]) == len(pts) == 1
    design = hold["designs"][0]
    assert design["source"] == "harris2014" and design["value"] == 66.9 and design["unit"] == "pJ"
    assert design["per_token_J"] == pytest.approx(POINT["per_token"]["dac_weight"] * 66.9e-12)
    assert check(account(doc, POINT, TERMS_E031)) == []


def test_e031_changes_only_the_weight_hold_term_from_e030_and_old_registrations_are_unchanged(doc):
    from awen_models.energy_accounting import TERMS_E030, TERMS_E031

    a, b = rows(account(doc, POINT, TERMS_E030)), rows(account(doc, POINT, TERMS_E031))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["weight_hold"]
    assert a["weight_hold"]["status"] == NOT_SOURCED
    assert "designs" not in a["weight_hold"]
    # Filling thermo_optic_power_per_pi does not source e030 weight_hold (still a power term).
    assert "time model" in a["weight_hold"]["reason"]
    result = run(COUNTS, doc, "e031")
    assert result["verdict"] == "pass" and result["accounting"] == "awen.energy-per-token-accounting.v7"
    assert result["points"][0]["total_J"] is None
    assert set(result["points"][0]["not_sourced"]) == {
        "laser", "control_electronics", "thermal_stabilization", "host_transfer"}
    assert run(COUNTS, doc, "e030")["accounting"] == "awen.energy-per-token-accounting.v6"
    assert run(COUNTS, doc, "e025")["verdict"] == "pass"


def test_e031_check_catches_tampered_energy_and_rejects_hidden_total(doc):
    from awen_models.energy_accounting import TERMS_E031

    point = account(doc, POINT, TERMS_E031)
    bad = copy.deepcopy(point)
    rows(bad)["weight_hold"]["designs"][0]["per_token_J"] = 1e-6
    assert any("contribution does not recompute" in f for f in check(bad))
    hidden = copy.deepcopy(point)
    rows(hidden)["weight_hold"]["status"] = NOT_SOURCED
    assert any("not sourced but has a contribution" in f for f in check(hidden))
    total = copy.deepcopy(point)
    total["not_sourced"] = ["laser"]
    total["total_J"] = 1e-9
    assert any("total emitted" in f for f in check(total))


def test_e032_applies_published_laser_energy_per_input_dac(doc):
    from awen_models.energy_accounting import TERMS_E032

    laser = rows(account(doc, POINT, TERMS_E032))["laser"]
    assert laser["status"].startswith("sourced") and laser["parameter"] == "laser.published_designs"
    assert laser["source_type"] == "estimate"
    assert laser["count"] == "dac_input"
    assert laser["count_per_token"] == POINT["per_token"]["dac_input"]
    pts = doc["parameters"]["laser"]["published_designs"]["points"]
    assert len(laser["designs"]) == len(pts) == 2
    by_basis = {d["basis"]: d for d in laser["designs"]}
    assert by_basis["dc_energy_per_bit_25C"]["value"] == 0.17
    assert by_basis["dc_energy_per_bit_50C"]["value"] == 0.34
    assert by_basis["dc_energy_per_bit_25C"]["unit"] == "pJ"
    assert by_basis["dc_energy_per_bit_25C"]["per_token_J"] == pytest.approx(
        POINT["per_token"]["dac_input"] * 0.17e-12)
    assert by_basis["dc_energy_per_bit_50C"]["per_token_J"] == pytest.approx(
        POINT["per_token"]["dac_input"] * 0.34e-12)
    assert check(account(doc, POINT, TERMS_E032)) == []


def test_e032_changes_only_the_laser_term_from_e031_and_old_registrations_are_unchanged(doc):
    from awen_models.energy_accounting import TERMS_E031, TERMS_E032

    a, b = rows(account(doc, POINT, TERMS_E031)), rows(account(doc, POINT, TERMS_E032))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["laser"]
    assert a["laser"]["status"] == NOT_SOURCED
    assert "designs" not in a["laser"]
    # Filling wall_plug_efficiency does not source e031 laser (still a power term).
    assert "time model" in a["laser"]["reason"]
    result = run(COUNTS, doc, "e032")
    assert result["verdict"] == "pass" and result["accounting"] == "awen.energy-per-token-accounting.v8"
    assert result["points"][0]["total_J"] is None
    assert set(result["points"][0]["not_sourced"]) == {
        "control_electronics", "thermal_stabilization", "host_transfer"}
    assert run(COUNTS, doc, "e031")["accounting"] == "awen.energy-per-token-accounting.v7"
    assert run(COUNTS, doc, "e025")["verdict"] == "pass"


def test_e032_check_catches_tampered_energy_and_rejects_hidden_total(doc):
    from awen_models.energy_accounting import TERMS_E032

    point = account(doc, POINT, TERMS_E032)
    bad = copy.deepcopy(point)
    rows(bad)["laser"]["designs"][0]["per_token_J"] = 1e-6
    assert any("contribution does not recompute" in f for f in check(bad))
    hidden = copy.deepcopy(point)
    rows(hidden)["laser"]["status"] = NOT_SOURCED
    assert any("not sourced but has a contribution" in f for f in check(hidden))
    total = copy.deepcopy(point)
    total["not_sourced"] = ["control_electronics"]
    total["total_J"] = 1e-9
    assert any("total emitted" in f for f in check(total))


def test_e033_applies_published_thermal_energy_per_input_dac(doc):
    from awen_models.energy_accounting import TERMS_E033

    thermal = rows(account(doc, POINT, TERMS_E033))["thermal_stabilization"]
    assert thermal["status"].startswith("sourced") and thermal["parameter"] == "thermal.published_designs"
    assert thermal["source_type"] == "estimate"
    assert thermal["count"] == "dac_input"
    assert thermal["count_per_token"] == POINT["per_token"]["dac_input"]
    pts = doc["parameters"]["thermal"]["published_designs"]["points"]
    assert len(thermal["designs"]) == len(pts) == 1
    design = thermal["designs"][0]
    assert design["source"] == "zortman2012" and design["basis"] == "realistic_trim_plus_thermal_10Gbps"
    assert design["value"] == 180.0 and design["unit"] == "fJ"
    assert design["per_token_J"] == pytest.approx(POINT["per_token"]["dac_input"] * 180.0e-15)
    assert check(account(doc, POINT, TERMS_E033)) == []


def test_e033_changes_only_the_thermal_term_from_e032_and_old_registrations_are_unchanged(doc):
    from awen_models.energy_accounting import TERMS_E032, TERMS_E033

    a, b = rows(account(doc, POINT, TERMS_E032)), rows(account(doc, POINT, TERMS_E033))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["thermal_stabilization"]
    assert a["thermal_stabilization"]["status"] == NOT_SOURCED
    assert "designs" not in a["thermal_stabilization"]
    # Filling thermal.published_designs does not source e032 thermal (still a power term).
    assert "time model" in a["thermal_stabilization"]["reason"]
    result = run(COUNTS, doc, "e033")
    assert result["verdict"] == "pass" and result["accounting"] == "awen.energy-per-token-accounting.v9"
    assert result["points"][0]["total_J"] is None
    assert set(result["points"][0]["not_sourced"]) == {"control_electronics", "host_transfer"}
    assert run(COUNTS, doc, "e032")["accounting"] == "awen.energy-per-token-accounting.v8"
    assert run(COUNTS, doc, "e025")["verdict"] == "pass"


def test_e033_check_catches_tampered_energy_and_rejects_hidden_total(doc):
    from awen_models.energy_accounting import TERMS_E033

    point = account(doc, POINT, TERMS_E033)
    bad = copy.deepcopy(point)
    rows(bad)["thermal_stabilization"]["designs"][0]["per_token_J"] = 1e-6
    assert any("contribution does not recompute" in f for f in check(bad))
    hidden = copy.deepcopy(point)
    rows(hidden)["thermal_stabilization"]["status"] = NOT_SOURCED
    assert any("not sourced but has a contribution" in f for f in check(hidden))
    total = copy.deepcopy(point)
    total["not_sourced"] = ["control_electronics"]
    total["total_J"] = 1e-9
    assert any("total emitted" in f for f in check(total))


def test_e034_applies_published_tuner_logic_energy_per_input_dac(doc):
    from awen_models.energy_accounting import TERMS_E034

    control = rows(account(doc, POINT, TERMS_E034))["control_electronics"]
    assert control["status"].startswith("sourced") and control["parameter"] == "control.published_designs"
    assert control["source_type"] == "estimate"
    assert control["count"] == "dac_input"
    assert control["count_per_token"] == POINT["per_token"]["dac_input"]
    pts = doc["parameters"]["control"]["published_designs"]["points"]
    assert len(control["designs"]) == len(pts) == 1
    design = control["designs"][0]
    assert design["source"] == "sun2016" and design["basis"] == "tuner_logic_power_over_5Gbps"
    assert design["value"] == 148.0 and design["unit"] == "fJ"
    assert design["per_token_J"] == pytest.approx(POINT["per_token"]["dac_input"] * 148.0e-15)
    assert "logic only" in control["caveat"]
    assert check(account(doc, POINT, TERMS_E034)) == []


def test_e034_changes_only_the_control_term_from_e033_and_old_registrations_are_unchanged(doc):
    from awen_models.energy_accounting import TERMS_E033, TERMS_E034

    a, b = rows(account(doc, POINT, TERMS_E033)), rows(account(doc, POINT, TERMS_E034))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["control_electronics"]
    assert a["control_electronics"]["status"] == NOT_SOURCED
    assert "designs" not in a["control_electronics"]
    # Filling control.published_designs does not source e033 control (still a power term).
    assert "time model" in a["control_electronics"]["reason"]
    result = run(COUNTS, doc, "e034")
    assert result["verdict"] == "pass" and result["accounting"] == "awen.energy-per-token-accounting.v10"
    assert result["points"][0]["total_J"] is None
    assert result["points"][0]["not_sourced"] == ["host_transfer"]
    assert run(COUNTS, doc, "e033")["accounting"] == "awen.energy-per-token-accounting.v9"
    assert set(run(COUNTS, doc, "e033")["points"][0]["not_sourced"]) == {"control_electronics", "host_transfer"}
    assert run(COUNTS, doc, "e025")["verdict"] == "pass"


def test_e034_check_catches_tampered_energy_and_rejects_hidden_total(doc):
    from awen_models.energy_accounting import TERMS_E034

    point = account(doc, POINT, TERMS_E034)
    bad = copy.deepcopy(point)
    rows(bad)["control_electronics"]["designs"][0]["per_token_J"] = 1e-6
    assert any("contribution does not recompute" in f for f in check(bad))
    hidden = copy.deepcopy(point)
    rows(hidden)["control_electronics"]["status"] = NOT_SOURCED
    assert any("not sourced but has a contribution" in f for f in check(hidden))
    total = copy.deepcopy(point)
    total["not_sourced"] = ["host_transfer"]
    total["total_J"] = 1e-9
    assert any("total emitted" in f for f in check(total))


def test_e035_applies_published_host_link_energy_per_byte_width(doc):
    from awen_models.energy_accounting import BITS_PER_BYTE, HOST_CODE_BYTES, TERMS_E035

    host = rows(account(doc, POINT, TERMS_E035))["host_transfer"]
    assert host["status"].startswith("sourced") and host["parameter"] == "host_link.published_designs"
    assert host["source_type"] == "measured"
    assert host["count_per_token"] == 114.0
    assert host["bytes_per_token"] == {str(w): 114.0 * w for w in HOST_CODE_BYTES}
    pts = doc["parameters"]["host_link"]["published_designs"]["points"]
    assert len(host["designs"]) == len(pts) == 1
    design = host["designs"][0]
    assert design["source"] == "li2018" and design["basis"] == "phy_transceiver_tx_rx"
    assert design["value"] == 7.5 and design["unit"] == "pJ"
    assert design["bits_per_byte"] == BITS_PER_BYTE == 8
    assert design["energy_per_byte_pJ"] == 60.0
    assert design["per_token_J_by_width"]["1"] == pytest.approx(114.0 * 8 * 7.5e-12)
    assert design["per_token_J_by_width"]["2"] == pytest.approx(228.0 * 8 * 7.5e-12)
    assert "PHY transceiver" in host["caveat"] or "PCIe Gen4" in host["caveat"]
    assert check(account(doc, POINT, TERMS_E035)) == []


def test_e035_changes_only_the_host_transfer_term_from_e034_and_old_registrations_are_unchanged(doc):
    from awen_models.energy_accounting import TERMS_E034, TERMS_E035

    a, b = rows(account(doc, POINT, TERMS_E034)), rows(account(doc, POINT, TERMS_E035))
    assert set(a) == set(b)
    assert [t for t in a if a[t] != b[t]] == ["host_transfer"]
    assert a["host_transfer"]["status"] == NOT_SOURCED
    assert "designs" not in a["host_transfer"]
    # Filling host_link.published_designs does not source e034 host (still null scalar).
    assert "energy per byte is null" in a["host_transfer"]["reason"]
    result = run(COUNTS, doc, "e035")
    assert result["verdict"] == "pass" and result["accounting"] == "awen.energy-per-token-accounting.v11"
    # All required terms sourced under e035 on synthetic counts; total still not emitted as a number.
    assert result["points"][0]["not_sourced"] == []
    assert result["points"][0]["total_J"] == "not defined by this registration"
    assert run(COUNTS, doc, "e034")["accounting"] == "awen.energy-per-token-accounting.v10"
    assert run(COUNTS, doc, "e034")["points"][0]["not_sourced"] == ["host_transfer"]
    assert run(COUNTS, doc, "e025")["verdict"] == "pass"


def test_e035_check_catches_tampered_energy_and_rejects_hidden_total(doc):
    from awen_models.energy_accounting import TERMS_E035

    point = account(doc, POINT, TERMS_E035)
    bad = copy.deepcopy(point)
    rows(bad)["host_transfer"]["designs"][0]["per_token_J_by_width"]["1"] = 1e-6
    assert any("contribution does not recompute" in f for f in check(bad))
    hidden = copy.deepcopy(point)
    rows(hidden)["host_transfer"]["status"] = NOT_SOURCED
    assert any("not sourced but has a contribution" in f for f in check(hidden))
    total = copy.deepcopy(point)
    total["not_sourced"] = ["host_transfer"]
    total["total_J"] = 1e-9
    assert any("total emitted" in f for f in check(total))


def test_e036_emits_structured_total_with_both_transmitters_when_all_sourced(doc):
    from awen_models.energy_accounting import TERMS_E035, TERMS_E036

    assert TERMS_E036 == TERMS_E035
    result = run(COUNTS, doc, "e036")
    assert result["accounting"] == "awen.energy-per-token-accounting.v12" and result["verdict"] == "pass"
    assert "scenario envelopes only" in result["claims"]
    assert "no speedup or savings" in result["claims"]
    point = result["points"][0]
    assert point["not_sourced"] == []
    total = point["total_J"]
    assert isinstance(total, dict)
    assert total["rule"] == "scenario_envelope_v1"
    assert total["unit"] == "J"
    assert "not one physical device" in total["disclosure"]
    assert "no speedup/savings" in total["disclosure"]
    assert set(total["by_transmitter"]) == {"dml_laser", "modulator_eo"}
    assert "scenario envelopes under scenario_envelope_v1" in point["total_reason"]
    assert check(point) == []


def test_e036_dml_laser_excludes_modulator_and_modulator_eo_excludes_laser(doc):
    point = run(COUNTS, doc, "e036")["points"][0]
    r = rows(point)
    dml = point["total_J"]["by_transmitter"]["dml_laser"]
    eom = point["total_J"]["by_transmitter"]["modulator_eo"]
    assert dml["excluded_terms"] == ["modulator"]
    assert eom["excluded_terms"] == ["laser"]
    laser_js = [d["per_token_J"] for d in r["laser"]["designs"]]
    mod_js = [d["per_token_J"] for d in r["modulator"]["designs"]]
    assert min(laser_js) > 0 and min(mod_js) > 0
    # Common axes are shared, so the envelope gap equals the transmitter-term gap.
    assert dml["min_J"] - eom["min_J"] == pytest.approx(min(laser_js) - min(mod_js))
    assert dml["max_J"] - eom["max_J"] == pytest.approx(max(laser_js) - max(mod_js))
    for env in (dml, eom):
        for picks in (env["min_picks"], env["max_picks"]):
            assert picks["dac_input"]["source"] == picks["dac_weight"]["source"]
            assert "thermal_stabilization" in picks
    assert "modulator" not in dml["min_picks"] and "modulator" not in dml["max_picks"]
    assert "laser" in dml["min_picks"] and "laser" in dml["max_picks"]
    assert "laser" not in eom["min_picks"] and "laser" not in eom["max_picks"]
    assert "modulator" in eom["min_picks"] and "modulator" in eom["max_picks"]


def test_e036_min_le_max_and_n_scenarios_matches_product(doc):
    point = run(COUNTS, doc, "e036")["points"][0]
    r = rows(point)
    n_dac = len({d["source"] for d in r["dac_input"]["designs"]}
                & {d["source"] for d in r["dac_weight"]["designs"]})
    n_adc = len(r["adc"]["designs"])
    n_floor = len(r["optical_detection_floor"]["values"])
    n_hold = len(r["weight_hold"]["designs"])
    n_control = len(r["control_electronics"]["designs"])
    n_thermal = len(r["thermal_stabilization"]["designs"])
    n_memory = sum(len(widths) for d in r["memory_movement"]["designs"]
                   for widths in d["per_token_J_by_placement"].values())
    n_host = sum(len(d["per_token_J_by_width"]) for d in r["host_transfer"]["designs"])
    n_laser = len(r["laser"]["designs"])
    n_mod = len(r["modulator"]["designs"])
    expected_dml = n_dac * n_adc * n_floor * n_hold * n_control * n_thermal * n_memory * n_host * n_laser
    expected_eom = n_dac * n_adc * n_floor * n_hold * n_control * n_thermal * n_memory * n_host * n_mod
    assert (n_dac, n_adc, n_floor, n_memory, n_host, n_laser, n_mod) == (3, 2, 3, 4, 2, 2, 2)
    dml = point["total_J"]["by_transmitter"]["dml_laser"]
    eom = point["total_J"]["by_transmitter"]["modulator_eo"]
    assert dml["n_scenarios"] == expected_dml == 288
    assert eom["n_scenarios"] == expected_eom == 288
    assert dml["min_J"] <= dml["max_J"] and eom["min_J"] <= eom["max_J"]


def test_e036_leaves_older_registrations_unchanged(doc):
    e036 = run(COUNTS, doc, "e036")
    e035 = run(COUNTS, doc, "e035")
    e034 = run(COUNTS, doc, "e034")
    e025 = run(COUNTS, doc)
    assert e035["accounting"] == "awen.energy-per-token-accounting.v11"
    assert e035["points"][0]["not_sourced"] == []
    assert e035["points"][0]["total_J"] == "not defined by this registration"
    assert "partial table" in e035["claims"]
    assert e034["accounting"] == "awen.energy-per-token-accounting.v10"
    assert e034["points"][0]["not_sourced"] == ["host_transfer"]
    assert e034["points"][0]["total_J"] is None
    assert e025["verdict"] == "pass" and e025["accounting"] == "awen.energy-per-token-accounting.v1"
    assert e025["points"][0]["total_J"] is None
    assert isinstance(e036["points"][0]["total_J"], dict)


def test_e036_check_catches_tampered_minmax_and_rejects_total_while_not_sourced(doc):
    point = run(COUNTS, doc, "e036")["points"][0]
    assert check(point) == []
    bad_min = copy.deepcopy(point)
    bad_min["total_J"]["by_transmitter"]["dml_laser"]["min_J"] *= 2
    assert any("min_J" in f and "envelope" in f for f in check(bad_min))
    bad_max = copy.deepcopy(point)
    bad_max["total_J"]["by_transmitter"]["modulator_eo"]["max_J"] = 0.0
    assert any("max_J" in f and "envelope" in f for f in check(bad_max))
    missing = copy.deepcopy(point)
    missing["not_sourced"] = ["host_transfer"]
    assert any("total emitted" in f for f in check(missing))
    hidden = copy.deepcopy(point)
    hidden["not_sourced"] = ["host_transfer"]
    hidden["total_J"] = 1e-9
    assert any("total emitted" in f for f in check(hidden))
