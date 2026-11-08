"""System energy per lowered MAC from the stored E-036 envelopes and E-024 counts (E-037).

    python -m awen_models.energy_per_mac --registration e037 \
        --totals results/energy-per-token-total/results.json \
        --counts results/conversions-per-token/results.json \
        --out results/energy-per-mac/results.json

Reads two stored results and runs no model. For every grid point (arch, tile_k, positions) it divides each
``scenario_envelope_v1`` bound of E-036 (per transmitter model) by the lowered MACs per token of E-024
(``counts.lowered_macs / positions``, an exact integer). The optical detection floor is reported per MAC
beside it as a labelled component (optics only, a lower bound), never as the system figure.

The denominator is MACs routed to the photonic GEMM only. Host-fallback MACs are not counted, and host CPU
compute is not a term of the E-036 numerator either. The envelope is a sum of labelled published points
within one transmitter model, not one physical device, and not a speedup or savings figure (D-037, D-038).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any, Mapping

CURVE = "awen.energy-per-mac-curve.v1"
TOTALS_ACCOUNTING = "awen.energy-per-token-accounting.v12"
COUNTS_CURVE = "awen.conversions-per-token-curve.v1"
TOTAL_RULE = "scenario_envelope_v1"
TRANSMITTERS = ("dml_laser", "modulator_eo")
# SHA-256 of the stored inputs this registration names (E-036 and E-024 results.json at main d6e1cc4).
REGISTERED_INPUTS = {
    "e037": {"totals": "0f78b7a14dc76f870c0ea7d4108053034a12abd0d3b81a23fcd57bee7f7b7e4b",
             "counts": "6cec0563175b6c0b5fb07bb068a7eb06338b19a3a7a3f67dd98f869da8432267"},
}
REL_TOL = 1e-12
MAC_RULE = ("lowered MACs per token = counts.lowered_macs / positions (E-024; GEMM MACs routed to the photonic "
            "simulator only, host-fallback MACs not counted)")
DISCLOSURE = ("E-036 scenario-envelope bounds divided by lowered MACs per token; sums of labelled published points "
              "within one transmitter model; not one physical device; host CPU compute is not a term; "
              "no speedup or savings claim")


def _key(point: Mapping[str, Any]) -> str:
    return f"{point['arch']}/tile{point['tile_k']}/P{point['positions']}"


def macs_per_token(count_point: Mapping[str, Any]) -> int:
    lowered, positions = int(count_point["counts"]["lowered_macs"]), int(count_point["positions"])
    if positions <= 0 or lowered <= 0 or lowered % positions:
        raise ValueError(f"{_key(count_point)}: lowered_macs {lowered} is not a positive multiple of positions {positions}")
    return lowered // positions


def per_mac_point(total_point: Mapping[str, Any], count_point: Mapping[str, Any]) -> dict[str, Any]:
    macs = macs_per_token(count_point)
    total = total_point.get("total_J")
    if not isinstance(total, Mapping) or total.get("rule") != TOTAL_RULE:
        raise ValueError(f"{_key(total_point)}: total_J is not a {TOTAL_RULE} object")
    by = {}
    for name in TRANSMITTERS:
        env = total["by_transmitter"][name]
        by[name] = {"excluded_terms": list(env["excluded_terms"]), "n_scenarios": env["n_scenarios"],
                    "min_J_per_token": env["min_J"], "max_J_per_token": env["max_J"],
                    "min_J_per_mac": env["min_J"] / macs, "max_J_per_mac": env["max_J"] / macs,
                    "min_picks": env["min_picks"], "max_picks": env["max_picks"]}
    floor = next(t for t in total_point["terms"] if t["term"] == "optical_detection_floor")
    return {"arch": total_point["arch"], "tile_k": total_point["tile_k"], "positions": total_point["positions"],
            "lowered_macs_per_token": macs,
            "conversions_per_token": count_point["per_token"]["total"],
            "conversions_per_mac": count_point["per_token"]["total"] / macs,
            "by_transmitter": by,
            "optical_detection_floor_J_per_mac": {
                "caveat": "optics only, a lower bound on optical energy; a component, not a system figure",
                "by_effective_bits": {bits: v["per_token_J"] / macs for bits, v in floor["values"].items()}}}


def _close(a: float, b: float) -> bool:
    return abs(a - b) <= REL_TOL * max(abs(a), abs(b))


def check(point: Mapping[str, Any]) -> list[str]:
    failures = []
    macs = point["lowered_macs_per_token"]
    if not isinstance(macs, int) or macs <= 0:
        failures.append("lowered MACs per token is not a positive integer")
        return failures
    if set(point["by_transmitter"]) != set(TRANSMITTERS):
        failures.append("transmitter models are not the registered two")
    floor_max = max(point["optical_detection_floor_J_per_mac"]["by_effective_bits"].values())
    for name, env in point["by_transmitter"].items():
        lo, hi = env["min_J_per_mac"], env["max_J_per_mac"]
        if not (math.isfinite(lo) and math.isfinite(hi) and 0 < lo <= hi):
            failures.append(f"{name}: per-MAC bounds are not finite with 0 < min <= max")
        if env["n_scenarios"] < 1:
            failures.append(f"{name}: no scenarios")
        if not (_close(lo * macs, env["min_J_per_token"]) and _close(hi * macs, env["max_J_per_token"])):
            failures.append(f"{name}: per-MAC bounds do not recompute from the per-token envelope")
        if floor_max > hi:
            failures.append(f"{name}: optical detection floor exceeds the system envelope")
    if not _close(point["conversions_per_mac"] * macs, point["conversions_per_token"]):
        failures.append("conversions per MAC do not recompute")
    return failures


def run(totals: Mapping[str, Any], counts: Mapping[str, Any], registration: str = "e037",
        input_sha256: Mapping[str, str] | None = None) -> dict[str, Any]:
    failures: dict[str, list[str]] = {}
    inputs = []
    if totals.get("accounting") != TOTALS_ACCOUNTING or totals.get("verdict") != "pass":
        inputs.append(f"totals are not a passing {TOTALS_ACCOUNTING} result")
    if counts.get("curve") != COUNTS_CURVE or counts.get("verdict") != "pass":
        inputs.append(f"counts are not a passing {COUNTS_CURVE} result")
    if input_sha256 is not None and dict(input_sha256) != REGISTERED_INPUTS[registration]:
        inputs.append("input SHA-256 differs from the registered inputs")
    by_key = {_key(p): p for p in counts["points"]}
    if {_key(p) for p in totals["points"]} != set(by_key) or len(by_key) != len(counts["points"]):
        inputs.append("totals and counts do not cover the same grid points")
    if inputs:
        failures["inputs"] = inputs
    points = []
    for tp in totals["points"]:
        cp = by_key.get(_key(tp))
        if cp is None:
            continue
        if tp["per_token_counts"] != cp["per_token"]:
            failures.setdefault(_key(tp), []).append("per-token counts differ between totals and counts")
        try:
            point = per_mac_point(tp, cp)
        except (ValueError, KeyError, StopIteration) as exc:
            failures.setdefault(_key(tp), []).append(str(exc))
            continue
        if found := check(point):
            failures.setdefault(_key(tp), []).extend(found)
        points.append(point)
    return {"curve": CURVE, "registration": registration, "evidence_level": "L1", "unit": "J per lowered MAC",
            "mac_rule": MAC_RULE, "disclosure": DISCLOSURE, "totals_source": totals.get("accounting"),
            "counts_source": counts.get("curve"), "assumption_set": totals.get("assumption_set"),
            "input_sha256": dict(input_sha256) if input_sha256 is not None else None,
            "claims": "none: per-MAC scenario envelopes only, no speedup or savings figure",
            "verdict": "fail" if failures else "pass", "failures": failures, "points": points}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--registration", choices=sorted(REGISTERED_INPUTS), default="e037")
    parser.add_argument("--totals", required=True, help="stored E-036 results.json")
    parser.add_argument("--counts", required=True, help="stored E-024 results.json")
    parser.add_argument("--out", help="write the JSON result here (default: stdout)")
    args = parser.parse_args(argv)
    raw = {name: Path(path).read_bytes() for name, path in (("totals", args.totals), ("counts", args.counts))}
    sha = {name: hashlib.sha256(data).hexdigest() for name, data in raw.items()}
    result = run(json.loads(raw["totals"]), json.loads(raw["counts"]), args.registration, input_sha256=sha)
    text = json.dumps(result, indent=2, sort_keys=True) + "\n"
    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
