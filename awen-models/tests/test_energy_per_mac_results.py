"""Consistency checks on the stored E-037 result (results/energy-per-mac)."""
import hashlib
import json
from pathlib import Path

from awen_models.energy_per_mac import CURVE, REGISTERED_INPUTS, TRANSMITTERS, check, run

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "energy-per-mac"
TOTALS = ROOT / "results" / "energy-per-token-total" / "results.json"
COUNTS = ROOT / "results" / "conversions-per-token" / "results.json"
MACS_PER_TOKEN = 90112
N_SCENARIOS = 288
# Overall span per transmitter (min of mins, max of maxes) as stored by the run, J per lowered MAC.
SPAN = {"dml_laser": (3.602334272727273e-12, 2.966261559943182e-10),
        "modulator_eo": (3.600292703125e-12, 2.966220949502841e-10)}


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def _sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert _sha(RESULTS / name) == digest, name


def test_inputs_are_the_registered_pins():
    report = _report()
    assert report["input_sha256"] == REGISTERED_INPUTS["e037"]
    assert _sha(TOTALS) == REGISTERED_INPUTS["e037"]["totals"]
    assert _sha(COUNTS) == REGISTERED_INPUTS["e037"]["counts"]


def test_verdict_pass_at_all_40_points():
    report = _report()
    assert report["curve"] == CURVE and report["registration"] == "e037"
    assert report["evidence_level"] == "L1"
    assert report["claims"] == "none: per-MAC scenario envelopes only, no speedup or savings figure"
    assert report["verdict"] == "pass" and report["failures"] == {}
    keys = {(p["arch"], p["tile_k"], p["positions"]) for p in report["points"]}
    assert len(keys) == len(report["points"]) == 40
    for p in report["points"]:
        assert check(p) == []
        assert p["lowered_macs_per_token"] == MACS_PER_TOKEN
        assert set(p["by_transmitter"]) == set(TRANSMITTERS)
        assert all(env["n_scenarios"] == N_SCENARIOS for env in p["by_transmitter"].values())


def test_rerun_from_stored_inputs_reproduces_result():
    raw = {"totals": TOTALS.read_bytes(), "counts": COUNTS.read_bytes()}
    sha = {k: hashlib.sha256(v).hexdigest() for k, v in raw.items()}
    again = run(json.loads(raw["totals"]), json.loads(raw["counts"]), "e037", input_sha256=sha)
    text = json.dumps(again, indent=2, sort_keys=True) + "\n"
    assert text == (RESULTS / "results.json").read_text()


def test_stored_spans_and_architectures_agree():
    points = _report()["points"]
    for name, (lo, hi) in SPAN.items():
        assert min(p["by_transmitter"][name]["min_J_per_mac"] for p in points) == lo
        assert max(p["by_transmitter"][name]["max_J_per_mac"] for p in points) == hi
    by = {}
    for p in points:
        by.setdefault((p["tile_k"], p["positions"]), []).append(json.dumps(
            {k: v for k, v in p.items() if k != "arch"}, sort_keys=True))
    assert all(len(v) == 2 and v[0] == v[1] for v in by.values())


def test_detection_floor_is_labelled_component_below_system_bounds():
    for p in _report()["points"]:
        floor = p["optical_detection_floor_J_per_mac"]
        assert "optics only" in floor["caveat"] and "not a system figure" in floor["caveat"]
        assert set(floor["by_effective_bits"]) == {"4", "6", "8"}
        lowest = min(env["min_J_per_mac"] for env in p["by_transmitter"].values())
        assert max(floor["by_effective_bits"].values()) <= 7.25e-5 * lowest * 1.001


def _pj(x):
    return f"{x * 1e12:.4g}"


def test_readme_figures_match_results():
    readme = (RESULTS / "README.md").read_text()
    points = _report()["points"]
    for name, (lo, hi) in SPAN.items():
        assert f"{float(_pj(lo)):.3f} pJ – {_pj(hi)} pJ |" in readme, name
    for p in points:
        if p["arch"] != "llama":
            continue
        env = p["by_transmitter"]["dml_laser"]
        cell = f"{_pj(env['min_J_per_mac'])} – {_pj(env['max_J_per_mac'])} pJ"
        assert cell in readme or cell.replace(" pJ", "0 pJ") in readme, (p["tile_k"], p["positions"], cell)
    assert "5acc49156f2c973ea49375261b294042b7cfcf7a" in readme
    assert REGISTERED_INPUTS["e037"]["totals"] in readme and REGISTERED_INPUTS["e037"]["counts"] in readme
