"""awen-spec/hardware_assumptions.yaml: structure, sources and the derived estimates."""

from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")

PATH = Path(__file__).resolve().parents[2] / "awen-spec" / "hardware_assumptions.yaml"
DATA = yaml.safe_load(PATH.read_text())


def leaves(node, prefix=""):
    for key, value in node.items():
        if isinstance(value, dict) and "unit" in value:
            yield prefix + key, value
        elif isinstance(value, dict):
            yield from leaves(value, prefix + key + ".")


PARAMS = list(leaves(DATA["parameters"]))


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
        assert p.get("type") == "estimate" or all(s in DATA["sources"] for s in sources), name
    for s in sources:
        assert s is None or s in DATA["sources"], f"{name}: unknown source {s}"
    if p.get("type") == "estimate":
        assert p.get("notes"), f"{name}: estimates must state their formula"


def test_channel_counts_are_not_assumed():
    ch = DATA["parameters"]["channels"]
    assert ch["wavelengths"] is None and ch["modes"] is None and ch["polarizations"] is None


def test_shot_noise_estimate_matches_formula():
    h, c = 6.62607015e-34, 299792458.0
    photon = h * c / 1550e-9
    p = DATA["parameters"]["optical_energy_per_mac"]["shot_noise_bound"]
    for key, bits in (("low", 4), ("typical", 6), ("high", 8)):
        assert p[key] == pytest.approx(photon * 4 ** bits, rel=2e-3)


def test_adc_estimate_matches_formula():
    p = DATA["parameters"]["adc"]["energy_per_conversion"]
    assert p["low"] == pytest.approx(6.2e-15 * 2 ** 10.0 * 1e12, rel=1e-3)
    assert p["high"] == pytest.approx(158.6e-3 / 5e9 * 1e12, rel=1e-3)
