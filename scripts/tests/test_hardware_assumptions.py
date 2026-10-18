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


@pytest.mark.parametrize("name,p", ALL_PARAMS, ids=[n for n, _ in ALL_PARAMS])
def test_every_parameter_has_unit_source_and_notes(name, p):
    assert p.get("unit") and "source" in p, name
    assert isinstance(p.get("notes"), str) and p["notes"].strip(), f"{name}: every parameter needs notes"
    assert p.get("kind") in (None, "sweep", "by_device_length_mm", "points"), name


MODULATOR_TYPES = {"tfln_mzm", "silicon_ring", "silicon_mzm", "silicon_unspecified", "unspecified"}


def test_parameter_count():
    assert len(ALL_PARAMS) == 35


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
