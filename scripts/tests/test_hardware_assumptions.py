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
    assert len(ALL_PARAMS) == 49


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


def test_phase_shifter_harris2014_insertion_loss_is_per_device_not_mzi_or_path():
    loss = DATA["parameters"]["phase_shifter"]["thermo_optic_insertion_loss"]
    assert (loss["low"], loss["typical"], loss["high"]) == (0.10, 0.23, 0.36)
    assert loss["low"] == pytest.approx(0.23 - 0.13) and loss["high"] == pytest.approx(0.23 + 0.13)
    assert loss["source"] == "harris2014" and loss["unit"] == "dB" and "source_type" not in loss
    assert "21 devices" in loss["notes"] and "not an MZI insertion loss" in loss["notes"]
    assert "0.23 ± 0.13 dB" in DATA["sources"]["harris2014"]["note"]
    # a per-heater loss is not the MZI loss nor the contract's total laser-to-detector loss
    assert DATA["parameters"]["mzi"]["insertion_loss"]["typical"] is None


def test_mzi_demarinis2021_extinction_ratio_is_not_a_crosstalk():
    mzi = DATA["parameters"]["mzi"]
    er = mzi["extinction_ratio"]
    assert (er["low"], er["typical"], er["high"]) == (22.9, 26.8, 42.5)
    assert er["low"] < er["typical"] < er["high"]
    assert er["unit"] == "dB" and er["source"] == "demarinis2021" and "source_type" not in er
    assert "not mean ± sigma" in er["notes"] and "not a crosstalk" in er["notes"]
    src = DATA["sources"]["demarinis2021"]
    assert src["doi"] == "10.3390/app11136232" and src["type"] == "measured"
    assert "Applied Sciences 11(13), 6232 (2021)" in src["citation"]
    for quoted in ("26.8 dB", "42.5 dB", "22.9 dB", "sigma = 4.2 dB", "approximately 0.7 dB"):
        assert quoted in src["note"], quoted
    # the contract's tile-level crosstalk and the MZI loss stay unsourced (D-043)
    for name in ("crosstalk", "insertion_loss"):
        assert (mzi[name]["low"], mzi[name]["typical"], mzi[name]["high"]) == (None, None, None), name
    assert "D-043" in mzi["crosstalk"]["notes"]


def test_laser_vaskasi2022_rin_bound_and_linewidth_points():
    laser = DATA["parameters"]["laser"]
    rin, lw = laser["rin"], laser["linewidth"]
    # a stated upper bound, not a level: only the high (noisiest) corner is filled (D-044)
    assert (rin["low"], rin["typical"], rin["high"]) == (None, None, -150.0)
    assert rin["unit"] == "dB/Hz" and rin["source"] == "vaskasi2022" and "source_type" not in rin
    assert "up to 6 GHz" in rin["notes"] and "extrapolation" in rin["notes"] and "D-044" in rin["notes"]
    assert (lw["low"], lw["typical"], lw["high"]) == (28000.0, None, 440000.0)
    assert lw["unit"] == "Hz" and lw["source"] == "vaskasi2022" and "not a spread" in lw["notes"]
    src = DATA["sources"]["vaskasi2022"]
    assert src["doi"] == "10.1364/OE.462051" and src["type"] == "measured"
    assert "Optics Express 30(15), 27983 (2022)" in src["citation"]
    for quoted in ("below -150 dB/Hz up to 6 GHz", "440 kHz", "28 kHz", "8870 Hz^2/Hz", "140 x 10^3 Hz^2/Hz"):
        assert quoted in src["note"], quoted
    # the wall-plug efficiency entry keeps its earlier source
    assert laser["wall_plug_efficiency"]["source"] == "diamantopoulos2021"


def test_photodetector_chen2016_bandwidth_and_dark_current():
    pd = DATA["parameters"]["photodetector"]
    bw, dark = pd["bandwidth"], pd["dark_current"]
    assert (bw["low"], bw["typical"], bw["high"]) == (None, 67.0, None)
    assert bw["unit"] == "GHz" and bw["source"] == "chen2016" and "source_type" not in bw
    assert "-1 V, 1550 nm" in bw["notes"] and "not a whole-receiver" in bw["notes"]
    assert (dark["low"], dark["typical"], dark["high"]) == (2.0, 2.4, 2.8)
    assert dark["low"] == pytest.approx(2.4 - 0.4) and dark["high"] == pytest.approx(2.4 + 0.4)
    assert dark["unit"] == "nA" and dark["source"] == "chen2016" and "source_type" not in dark
    src = DATA["sources"]["chen2016"]
    assert src["doi"] == "10.1364/OE.24.004622" and src["type"] == "measured"
    assert "Optics Express 24(5), 4622 (2016)" in src["citation"]
    assert "2.4 nA (sd 0.4 nA)" in src["note"] and "~67 GHz at -1 V, 1550 nm" in src["note"]
    # responsivity keeps its hua2025 value; chen2016 responsivity is noted in the source only
    assert pd["responsivity"]["source"] == "hua2025"


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


def test_control_sun2016_tuner_logic_power_and_published_energy_per_bit():
    control = DATA["parameters"]["control"]
    logic = control["tuner_logic_power"]
    assert logic["typical"] == 0.74 and logic["source"] == "sun2016" and logic["unit"] == "mW"
    p = control["published_designs"]
    assert p["kind"] == "points" and p["type"] == "microring_tuner_logic" and p["unit"] == "fJ/bit"
    assert p["source_type"] == "estimate" and set(p["source"]) == {pt["source"] for pt in p["points"]} == {"sun2016"}
    assert len(p["points"]) == 1
    pt = p["points"][0]
    assert pt["basis"] == "tuner_logic_power_over_5Gbps"
    assert pt["energy_per_bit_fJ"] == 148.0
    # The point is the stated formula: logic power / data rate.
    assert pt["logic_power_mW"] * 1e-3 / (pt["data_rate_Gbps"] * 1e9) * 1e15 == pytest.approx(pt["energy_per_bit_fJ"])
    assert "0.74" in pt["derivation"] and "148" in pt["derivation"]
    src = DATA["sources"]["sun2016"]
    assert src["doi"] == "10.1109/JSSC.2016.2519390" and src["type"] == "measured"
    assert "0.74 mW" in src["note"] and "1721.1/99784" in src["note"]
    # Continuous Watt field stays null; e033-style power readers stay not-sourced without a time model.
    assert DATA["parameters"]["system"]["control_electronics_power"]["typical"] is None



def test_host_link_li2018_published_energy_per_bit():
    assert DATA["parameters"]["system"]["host_transfer_energy"]["typical"] is None, (
        "scalar stays null so e028-e034 keep host energy not sourced; points are separate"
    )
    p = DATA["parameters"]["host_link"]["published_designs"]
    assert p["kind"] == "points" and p["type"] == "pcie_gen4_phy_transceiver" and p["unit"] == "pJ/bit"
    assert p["source_type"] == "measured" and set(p["source"]) == {pt["source"] for pt in p["points"]} == {"li2018"}
    assert len(p["points"]) == 1
    pt = p["points"][0]
    assert pt["basis"] == "phy_transceiver_tx_rx"
    assert pt["energy_per_bit_pJ"] == 7.5
    assert pt["energy_per_byte_pJ"] == 60.0
    assert pt["energy_per_bit_pJ"] * 8 == pt["energy_per_byte_pJ"]
    assert "7.5" in pt["derivation"] and "60" in pt["derivation"]
    src = DATA["sources"]["li2018"]
    assert src["doi"] == "10.1109/ASSCC.2018.8579314" and src["type"] == "measured"
    assert "7.5" in src["note"] and "transceiver" in src["note"].lower()
    notes = DATA["parameters"]["system"]["host_transfer_energy"]["notes"]
    assert "li2018" in notes and "1.93" in notes and "167.9" in notes


def test_drift_elmas2025_published_phase_random_walk_points():
    p = DATA["parameters"]["drift"]["published_designs"]
    assert p["kind"] == "points" and p["type"] == "input_fiber_relative_phase_random_walk"
    assert p["unit"] == "rad/sqrt(s)" and p["source_type"] == "measured"
    assert set(p["source"]) == {pt["source"] for pt in p["points"]} == {"elmas2025"}
    assert sorted(pt["sigma_rad_per_sqrt_s"] for pt in p["points"]) == [0.15, 0.25]
    for pt in p["points"]:
        assert pt["basis"].startswith("brownian_fit_to_") and "Fig." in pt["evidence"]
    src = DATA["sources"]["elmas2025"]
    assert src["doi"] == "10.1364/AO.560370" and src["type"] == "measured"
    assert "2606.25196" in src["note"] and "not a directly reported measured statistic" in src["note"]
    # A proxy for the E-023 time axis only: not on-chip weight drift.
    assert "not drift of programmed" in p["notes"]
