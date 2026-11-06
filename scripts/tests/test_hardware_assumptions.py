"""awen-spec/hardware_assumptions.yaml: structure, sources and the derived estimates."""

from pathlib import Path

import pytest
import yaml  # required: CI pins pyyaml==6.0.3; a missing yaml must fail, not skip

PATH = Path(__file__).resolve().parents[2] / "awen-spec" / "hardware_assumptions.yaml"
DATA = yaml.safe_load(PATH.read_text())


def leaves(node, prefix=""):
    for key, value in node.items():
        if isinstance(value, dict) and "unit" in value:
            yield prefix + key, value
        elif isinstance(value, dict):
            yield from leaves(value, prefix + key + ".")


ALL_PARAMS = list(leaves(DATA["parameters"]))
PARAMS = [(n, p) for n, p in ALL_PARAMS if "kind" not in p]  # low/typical/high entries


def test_version_and_sources():
    assert DATA["version"] == "awen.hardware-assumptions.v0"
    for name, src in DATA["sources"].items():
        assert src["type"] in {"measured", "vendor", "estimate"}, name
        assert src["doi"].startswith("10."), name


@pytest.mark.parametrize("name,p", PARAMS, ids=[n for n, _ in PARAMS])
def test_parameter_has_range_unit_and_source(name, p):
    assert {"low", "typical", "high", "unit", "source"} <= set(p), name
    values = [p[k] for k in ("low", "typical", "high") if p[k] is not None]
    assert values == sorted(values), f"{name}: low <= typical <= high"
    sources = p["source"] if isinstance(p["source"], list) else [p["source"]]
    if values:  # a number needs a checked source or an estimate formula
        assert p.get("source_type") == "estimate" or all(s in DATA["sources"] for s in sources), name
    for s in sources:
        assert s is None or s in DATA["sources"], f"{name}: unknown source {s}"
    assert isinstance(p.get("notes"), str) and p["notes"].strip(), f"{name}: every parameter needs notes"


def test_channel_counts_are_not_assumed():
    ch = DATA["parameters"]["channels"]
    assert ch["wavelengths"] is None and ch["modes"] is None and ch["polarizations"] is None


def test_shot_noise_is_a_per_detection_sweep_by_bit_depth():
    h, c = 6.62607015e-34, 299792458.0
    photon = h * c / 1550e-9
    p = DATA["parameters"]["shot_noise"]["shot_noise_energy_per_detection_J"]
    assert p["notes"].strip() and p["source_type"] == "estimate"
    assert p["kind"] == "sweep" and p["sweep_over"] == "effective_bits" and p["unit"] == "J"
    assert not {"low", "typical", "high"} & set(p), "a sweep is not an uncertainty range"
    assert set(p["values"]) == {4, 6, 8}
    for bits, energy in p["values"].items():
        assert energy == pytest.approx(photon * 4 ** bits, rel=2e-3)
    assert "/ N" in p["per_mac_formula"]
    assert "optical_energy_per_mac" not in DATA["parameters"]


def test_system_enob_is_not_an_adc_spec():
    assert DATA["parameters"]["system"]["pace_enob"]["typical"] == 7.61
    assert DATA["parameters"]["system"]["pace_enob"]["source"] == "hua2025"
    adc = DATA["parameters"]["adc"]["published_designs"]["points"]
    assert 7.61 not in [pt["enob_bits"] for pt in adc]
    assert {pt["source"] for pt in adc} == {"ramkaj2020", "lagos2021"}


def test_adc_designs_are_separate_points_matching_formulas():
    p = DATA["parameters"]["adc"]["published_designs"]
    assert p["kind"] == "points" and not {"low", "typical", "high"} & set(p)
    by = {pt["source"]: pt for pt in p["points"]}
    assert by["lagos2021"]["energy_per_conversion_pJ"] == pytest.approx(6.2e-15 * 2 ** 10.0 * 1e12, rel=1e-3)
    assert by["ramkaj2020"]["energy_per_conversion_pJ"] == pytest.approx(158.6e-3 / 5e9 * 1e12, rel=1e-3)
    for pt in p["points"]:
        assert {"enob_bits", "sample_rate_GSps", "energy_per_conversion_pJ"} <= set(pt)


def test_dac_designs_are_separate_points_matching_formulas():
    dac = DATA["parameters"]["dac"]
    assert dac["energy_per_conversion"]["typical"] is None, "no single DAC value; designs stay separate"
    p = dac["published_designs"]
    assert p["kind"] == "points" and not {"low", "typical", "high"} & set(p)
    assert p["source_type"] == "estimate" and set(p["source"]) == {pt["source"] for pt in p["points"]}
    for pt in p["points"]:
        assert DATA["sources"][pt["source"]]["type"] == "measured"
        assert "enob_bits" not in pt, "nominal DAC resolution is not an effective-bits figure"
        assert pt["energy_per_conversion_pJ"] == pytest.approx(pt["power_mW"] / pt["sample_rate_GSps"], rel=1e-3)
        assert f"{pt['power_mW']:g} mW / {pt['sample_rate_GSps']:g} GS/s" in pt["derivation"]


@pytest.mark.parametrize("name,p", ALL_PARAMS, ids=[n for n, _ in ALL_PARAMS])
def test_every_parameter_has_unit_source_and_notes(name, p):
    assert p.get("unit") and "source" in p, name
    assert isinstance(p.get("notes"), str) and p["notes"].strip(), f"{name}: every parameter needs notes"
    assert p.get("kind") in (None, "sweep", "by_device_length_mm", "points"), name


MODULATOR_TYPES = {"tfln_mzm", "silicon_ring", "silicon_mzm", "silicon_unspecified", "unspecified"}


def test_parameter_count():
    assert len(ALL_PARAMS) == 43


def test_modulator_entries_have_device_type_and_separate_bandwidth():
    mod = DATA["parameters"]["modulator"]
    for name, p in mod.items():
        assert p.get("type") in MODULATOR_TYPES, name
    bw = mod["tfln_electro_optic_bandwidth_3dB"]
    assert bw["unit"] == "GHz" and bw["source"] == "wang2018"
    assert bw["kind"] == "by_device_length_mm" and bw["values"] == {5: 100.0, 10: 80.0, 20: None}
    vpi = mod["tfln_half_wave_voltage"]
    assert vpi["kind"] == "by_device_length_mm" and vpi["values"] == {5: 4.4, 10: 2.3, 20: 1.4}
    assert mod["tfln_data_rate"]["unit"] == "Gbit/s"  # a data rate is not a bandwidth


def test_modulator_points_are_measured_and_energy_per_symbol_stays_null():
    mod = DATA["parameters"]["modulator"]
    assert mod["energy_per_symbol"]["typical"] is None, "no single modulator value; points stay separate"
    p = mod["published_designs"]
    assert p["kind"] == "points" and not {"low", "typical", "high"} & set(p) and p["unit"] == "fJ"
    assert set(p["source"]) == {pt["source"] for pt in p["points"]} == {"timurdogan2014"}
    by = {pt["basis"]: pt for pt in p["points"]}
    assert by["per_full_swing_transition"]["energy_per_symbol_fJ"] == 3.65
    # The paper's per-bit figure is switching energy / 4 (equiprobable NRZ transitions).
    assert by["per_bit_nrz_ook"]["energy_per_symbol_fJ"] == pytest.approx(3.65 / 4, abs=0.005)
    assert "not an analog" in p["notes"]


def test_memory_hbm2_point_matches_oconnor2017_quote():
    mem = DATA["parameters"]["memory"]
    assert mem["access_energy_per_bit"]["typical"] is None, "no single memory value; points stay separate"
    p = mem["published_designs"]
    assert p["kind"] == "points" and not {"low", "typical", "high"} & set(p) and p["unit"] == "pJ/bit"
    assert p["source_type"] == "measured" and set(p["source"]) == {pt["source"] for pt in p["points"]} == {"oconnor2017"}
    pt = p["points"][0]
    assert pt["technology"] == "HBM2" and pt["basis"] == "access_per_bit"
    assert pt["energy_per_bit_pJ"] == 3.97
    assert "3.97 pJ/bit" in pt["derivation"]
    assert DATA["sources"]["oconnor2017"]["doi"] == "10.1145/3123939.3124545"
    assert DATA["parameters"]["system"]["host_transfer_energy"]["typical"] is None


def test_phase_shifter_harris2014_values_and_published_pi_energy():
    ps = DATA["parameters"]["phase_shifter"]
    power = ps["thermo_optic_power_per_pi"]
    assert power["typical"] == 24.77 and power["low"] == 24.34 and power["high"] == 25.20
    assert power["source"] == "harris2014" and power["unit"] == "mW"
    time = ps["thermo_optic_reconfiguration_time"]
    assert time["typical"] == 2.69 and time["source"] == "harris2014" and time["unit"] == "us"
    p = ps["published_designs"]
    assert p["kind"] == "points" and not {"low", "typical", "high"} & set(p) and p["unit"] == "pJ"
    assert p["source_type"] == "estimate" and set(p["source"]) == {pt["source"] for pt in p["points"]} == {"harris2014"}
    pt = p["points"][0]
    assert pt["basis"] == "pi_reconfiguration_P_times_tau"
    assert pt["power_per_pi_mW"] == 24.77 and pt["reconfiguration_time_us"] == 2.69
    assert pt["energy_per_pi_pJ"] == 66.9
    assert "Table I" in pt["derivation"] and "66.9" in pt["derivation"]
    assert DATA["sources"]["harris2014"]["doi"] == "10.1364/OE.22.010487"
    assert "24.77" in DATA["sources"]["harris2014"]["note"]


def test_laser_diamantopoulos2021_wpe_and_published_energy_per_bit():
    laser = DATA["parameters"]["laser"]
    wpe = laser["wall_plug_efficiency"]
    assert wpe["typical"] == 0.0644 and wpe["source"] == "diamantopoulos2021" and wpe["unit"] == "fraction"
    p = laser["published_designs"]
    assert p["kind"] == "points" and not {"low", "typical", "high"} & set(p) and p["unit"] == "pJ/bit"
    assert p["source_type"] == "estimate" and set(p["source"]) == {pt["source"] for pt in p["points"]} == {"diamantopoulos2021"}
    assert len(p["points"]) == 2
    by = {pt["basis"]: pt for pt in p["points"]}
    assert by["dc_energy_per_bit_25C"]["energy_per_bit_pJ"] == 0.17
    assert by["dc_energy_per_bit_50C"]["energy_per_bit_pJ"] == 0.34
    assert by["dc_energy_per_bit_25C"]["dc_power_mW"] == 19.565
    assert by["dc_energy_per_bit_50C"]["dc_power_mW"] == 38.656
    assert "0.17" in by["dc_energy_per_bit_25C"]["derivation"]
    assert DATA["sources"]["diamantopoulos2021"]["doi"] == "10.3390/photonics8020031"
    assert "19.565" in DATA["sources"]["diamantopoulos2021"]["note"]

def test_thermal_zortman2012_heater_efficiency_and_published_energy_per_bit():
    thermal = DATA["parameters"]["thermal"]
    eff = thermal["heater_efficiency"]
    assert eff["typical"] == 7.0 and eff["source"] == "zortman2012" and eff["unit"] == "uW/GHz"
    p = thermal["published_designs"]
    assert p["kind"] == "points" and p["type"] == "silicon_microdisk_heater" and p["unit"] == "fJ/bit"
    assert p["source_type"] == "estimate" and set(p["source"]) == {pt["source"] for pt in p["points"]} == {"zortman2012"}
    assert len(p["points"]) == 1
    pt = p["points"][0]
    assert pt["basis"] == "realistic_trim_plus_thermal_10Gbps"
    assert pt["energy_per_bit_fJ"] == 180.0
    assert pt["heater_efficiency_uW_per_GHz"] == 7.0
    assert pt["data_rate_Gbps"] == 10.0
    assert "180" in pt["derivation"] and "7" in pt["derivation"]
    assert DATA["sources"]["zortman2012"]["doi"] == "10.1364/ofc.2012.ow4i.5"
    assert "7" in DATA["sources"]["zortman2012"]["note"] and "180" in DATA["sources"]["zortman2012"]["note"]
    # Continuous Watt field stays null; e032-style power readers stay not-sourced without a time model.
    assert DATA["parameters"]["system"]["thermal_stabilization_power"]["typical"] is None
