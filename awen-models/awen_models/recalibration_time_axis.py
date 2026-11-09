"""Recalibration time axis for the stored E-023 drift budgets from a published phase random-walk rate (E-039).

    python -m awen_models.recalibration_time_axis --registration e039 \
        --recalibration results/photon-recalibration/results.json \
        --out results/photon-recalibration-time/results.json

Reads the stored E-023 result and the ``drift.published_designs`` points of ``awen-spec/hardware_assumptions.yaml``;
it trains and evaluates no model. E-023 reports tolerable drift budgets ``s`` (rms random-walk offset of each
normalized programmed weight per recalibration interval). For a weight-unit drift rate ``D`` the interval is
``T = (s / D) ** 2`` (``awen_models.recalibration.interval_for_budget``). Here ``D = k * sigma``: ``sigma`` is a
published Brownian phase rate in rad/sqrt(s) and ``k`` (normalized weight units per rad) is a registered
phase-to-weight sensitivity, an estimate with a stated formula, not a measured device property.

Proxy, stated: the published rate is relative phase between two input fibers of one lab setup, not drift of
programmed mesh phases. E-023 drifts programmed weights only. Treating that rate as if it acted on every weight
independently is the registered assumption of this time axis, so every interval below is an estimate (formula)
under that proxy, not a device prediction, and no hardware or recalibration-time claim follows (D-040).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any, Mapping, Optional, Sequence

CURVE = "awen.recalibration-time-axis.v1"
SOURCE_CURVE = "E-023 results.json (awen_models.recalibration)"
POINT_TYPE = "input_fiber_relative_phase_random_walk"
# Registered inputs: SHA-256 of the stored E-023 results.json, and the published points this registration names.
REGISTERED = {
    "e039": {
        "recalibration_sha256": "12e7db5fa60f594bf4acb0748ea73066b4fc9e9b4b3ffd0f7065040704cc3401",
        "points": {("elmas2025", "brownian_fit_to_phase_distribution"): 0.25,
                   ("elmas2025", "brownian_fit_to_feedback_trace"): 0.15},
        # Normalized weight units per rad. 1.0: |d cos(phi)/d phi| <= 1, the largest first-order change of a
        # unit-amplitude interference term per rad (conservative: shortest interval). 0.5: MZI internal phase
        # theta to field amplitude cos(theta/2), |d/d theta| <= 1/2. Both are estimates (formula), not measured.
        "sensitivities": {"phase_to_amplitude_max": 1.0, "mzi_internal_half_angle_max": 0.5},
    },
}
REL_TOL = 1e-12
DISCLOSURE = ("interval T = (s / (k * sigma))^2 in seconds; sigma is a published Brownian phase rate of input-fiber "
              "relative phase in one lab setup (proxy for programmed-weight drift, not measured weight drift); k is "
              "a registered first-order phase-to-weight sensitivity bound; estimate under the proxy, not a device "
              "prediction; no hardware or recalibration-time claim")


def interval_s(budget: float, sigma: float, k: float) -> float:
    """Seconds until a weight random walk with rate ``k * sigma`` per sqrt(s) reaches rms ``budget``."""
    if sigma <= 0 or k <= 0:
        raise ValueError("sigma and k must be positive")
    if budget < 0:
        raise ValueError("budget must be non-negative")
    return (budget / (k * sigma)) ** 2


def sha256(path: Path) -> str:
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def published_points(assumptions: Mapping[str, Any], registration: str = "e039") -> list[dict[str, Any]]:
    """The ``drift.published_designs`` points, checked against the registered sources, bases and sigmas."""
    block = assumptions["parameters"]["drift"]["published_designs"]
    if block.get("kind") != "points" or block.get("type") != POINT_TYPE or block.get("unit") != "rad/sqrt(s)":
        raise ValueError("drift.published_designs is not the registered points block")
    want = REGISTERED[registration]["points"]
    got = {(p["source"], p["basis"]): float(p["sigma_rad_per_sqrt_s"]) for p in block["points"]}
    if got != want:
        raise ValueError(f"drift points {got} differ from the registered {want}")
    for name, _ in want:
        if name not in assumptions["sources"]:
            raise ValueError(f"source {name} missing from the assumption set")
    return [{"source": s, "basis": b, "sigma_rad_per_sqrt_s": v} for (s, b), v in sorted(want.items())]


def build(recalibration: Mapping[str, Any], points: Sequence[Mapping[str, Any]],
          sensitivities: Mapping[str, float]) -> dict[str, Any]:
    regimes = {}
    for regime, row in recalibration["tolerable_budget"].items():
        budgets = [float(r["budget"]) for r in row["rows"] if float(r["budget"]) > 0]
        tolerable = float(row["tolerable_budget"])
        entries = []
        for p in points:
            for rule, k in sensitivities.items():
                entries.append({
                    "source": p["source"], "basis": p["basis"], "sigma_rad_per_sqrt_s": p["sigma_rad_per_sqrt_s"],
                    "sensitivity_rule": rule, "k_weight_per_rad": k,
                    "drift_rate_weight_per_sqrt_s": k * p["sigma_rad_per_sqrt_s"],
                    "tolerable_interval_s": interval_s(tolerable, p["sigma_rad_per_sqrt_s"], k) if tolerable > 0 else None,
                    "interval_s_by_budget": {f"{b:g}": interval_s(b, p["sigma_rad_per_sqrt_s"], k) for b in budgets},
                })
        regimes[regime] = {"tolerable_budget": tolerable, "entries": entries}
    return {"regimes": regimes}


def check(report: Mapping[str, Any], recalibration: Mapping[str, Any]) -> list[str]:
    """Registered rule: budgets copied unchanged and every interval equal to the closed form."""
    failures = []
    for regime, row in recalibration["tolerable_budget"].items():
        got = report["regimes"].get(regime)
        if got is None:
            failures.append(f"{regime}: missing")
            continue
        if got["tolerable_budget"] != float(row["tolerable_budget"]):
            failures.append(f"{regime}: tolerable budget changed")
        for e in got["entries"]:
            sigma, k = e["sigma_rad_per_sqrt_s"], e["k_weight_per_rad"]
            for b, t in e["interval_s_by_budget"].items():
                if not math.isclose(t, (float(b) / (k * sigma)) ** 2, rel_tol=REL_TOL):
                    failures.append(f"{regime}/{e['basis']}/{e['sensitivity_rule']}/s={b}: interval != closed form")
    return failures


def run(registration: str, recalibration_path: Path, assumptions_path: Path, out: Optional[Path] = None) -> dict[str, Any]:
    import yaml

    reg = REGISTERED[registration]
    digest = sha256(recalibration_path)
    if digest != reg["recalibration_sha256"]:
        raise ValueError(f"E-023 results SHA-256 {digest} != registered {reg['recalibration_sha256']}")
    assumptions = yaml.safe_load(Path(assumptions_path).read_text())
    recalibration = json.loads(Path(recalibration_path).read_text())
    points = published_points(assumptions, registration)
    report = {
        "curve": CURVE, "registration": registration, "fidelity": "L1 (estimate on stored simulation results)",
        "inputs": {"recalibration": {"path": str(recalibration_path), "sha256": digest, "curve": SOURCE_CURVE},
                   "assumptions": {"path": str(assumptions_path), "sha256": sha256(assumptions_path),
                                   "version": assumptions.get("version")}},
        "points": points, "sensitivities": dict(reg["sensitivities"]), "disclosure": DISCLOSURE,
        **build(recalibration, points, reg["sensitivities"]),
    }
    report["failures"] = check(report, recalibration)
    report["verdict"] = "pass" if not report["failures"] else "fail"
    if out is not None:
        Path(out).parent.mkdir(parents=True, exist_ok=True)
        Path(out).write_text(json.dumps(report, indent=2, sort_keys=True) + "\n")
    return report


def main(argv: Optional[Sequence[str]] = None) -> None:
    from .sim2real import find_assumptions

    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--registration", required=True, choices=sorted(REGISTERED))
    parser.add_argument("--recalibration", default="results/photon-recalibration/results.json")
    parser.add_argument("--assumptions", default=None, help="path to awen-spec/hardware_assumptions.yaml")
    parser.add_argument("--out", required=True)
    args = parser.parse_args(argv)
    report = run(args.registration, Path(args.recalibration), Path(args.assumptions or find_assumptions()), Path(args.out))
    print("verdict", report["verdict"])
    for regime, row in report["regimes"].items():
        for e in row["entries"]:
            print(regime, e["basis"], e["sensitivity_rule"], "tolerable_interval_s", e["tolerable_interval_s"])


if __name__ == "__main__":
    main()
