"""Consistency checks on the stored E-039 result (results/photon-recalibration-time)."""
import hashlib
import json
import math
from pathlib import Path

from awen_models import recalibration_time_axis as rta

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "photon-recalibration-time"
E023 = ROOT / "results" / "photon-recalibration" / "results.json"
RUN_COMMIT = "6a043ba7a4ba509df4a311c6c76f7519f56abf45"


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def _sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert _sha(RESULTS / name) == digest, name


def test_verdict_pass_and_registered_inputs():
    report = _report()
    assert report["curve"] == rta.CURVE and report["registration"] == "e039"
    assert report["verdict"] == "pass" and report["failures"] == []
    assert report["inputs"]["recalibration"]["sha256"] == rta.REGISTERED["e039"]["recalibration_sha256"] == _sha(E023)
    assert report["inputs"]["assumptions"]["version"] == "awen.hardware-assumptions.v0"
    assert report["sensitivities"] == rta.REGISTERED["e039"]["sensitivities"]
    got = {(p["source"], p["basis"]): p["sigma_rad_per_sqrt_s"] for p in report["points"]}
    assert got == rta.REGISTERED["e039"]["points"]


def test_proxy_disclosed_in_stored_result():
    report = _report()
    assert report["disclosure"] == rta.DISCLOSURE
    assert "proxy" in report["disclosure"] and "not a device prediction" in report["disclosure"]
    assert report["fidelity"].startswith("L1")


def test_budgets_copied_unchanged_from_e023():
    e023 = json.loads(E023.read_text())["tolerable_budget"]
    report = _report()
    assert set(report["regimes"]) == set(e023)
    for regime, row in e023.items():
        stored = report["regimes"][regime]
        assert stored["tolerable_budget"] == float(row["tolerable_budget"])
        grid = {f"{float(r['budget']):g}" for r in row["rows"] if float(r["budget"]) > 0}
        assert len(stored["entries"]) == 4  # 2 published points x 2 sensitivity bounds
        for e in stored["entries"]:
            assert set(e["interval_s_by_budget"]) == grid


def test_every_interval_recomputes_independently():
    for regime, row in _report()["regimes"].items():
        for e in row["entries"]:
            k, sigma = e["k_weight_per_rad"], e["sigma_rad_per_sqrt_s"]
            assert math.isclose(e["drift_rate_weight_per_sqrt_s"], k * sigma, rel_tol=1e-12)
            assert math.isclose(e["tolerable_interval_s"], (row["tolerable_budget"] / (k * sigma)) ** 2, rel_tol=1e-12)
            for b, t in e["interval_s_by_budget"].items():
                assert math.isclose(t, (float(b) / (k * sigma)) ** 2, rel_tol=1e-12), (regime, b)


def test_headline_values_in_readme():
    """The README table values are the stored tolerable intervals, rounded."""
    report = _report()
    tol = sorted(e["tolerable_interval_s"] for r in report["regimes"].values() for e in r["entries"])
    assert math.isclose(tol[0], 0.04, rel_tol=1e-9) and math.isclose(tol[-1], 16 / 9, rel_tol=1e-9)
    std = {(e["sigma_rad_per_sqrt_s"], e["k_weight_per_rad"]): e["tolerable_interval_s"]
           for e in report["regimes"]["standard"]["entries"]}
    assert math.isclose(std[(0.15, 0.5)], 4 / 9, rel_tol=1e-9) and math.isclose(std[(0.25, 0.5)], 0.16, rel_tol=1e-9)
    readme = (RESULTS / "README.md").read_text()
    for text in ("0.04 s", "1.778 s", "0.4444 s", "proxy", RUN_COMMIT):
        assert text in readme


def test_rerun_from_stored_input_reproduces_regimes(tmp_path):
    """Running the registered module again gives the same regimes, points and verdict as the stored file."""
    fresh = rta.run("e039", E023, ROOT.parent / "awen-spec" / "hardware_assumptions.yaml", tmp_path / "r.json")
    stored = _report()
    for key in ("regimes", "points", "sensitivities", "verdict", "failures", "disclosure", "curve"):
        assert fresh[key] == stored[key], key
