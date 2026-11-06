"""Energy-per-token accounting for the E-024 conversion counts, sourced terms only (E-025 to E-033).

    python -m awen_models.energy_accounting \
        --counts results/conversions-per-token/results.json \
        --out results/energy-per-token/results.json

``--registration e026`` selects the E-026 terms: the same as E-025 except that the input and weight DAC
terms read ``dac.published_designs`` (each published design separately, never interpolated). The
default, ``e025``, is unchanged, so the stored E-025 run still reproduces. ``--registration e027`` is E-026
plus the modulator term read from ``modulator.published_designs`` (one contribution per published point).
``--registration e028`` is E-027 plus a counted host-transfer byte figure: every input DAC code, weight DAC code
and ADC read crosses the host boundary once (the simulator accumulates partial sums on the host), packed at 1 or
2 bytes per code. The energy per byte stays "not sourced" while ``system.host_transfer_energy`` is null.
``--registration e029`` is E-028 plus a counted memory-movement figure for the lowered GEMMs under two stated
placements, reported separately: weights streamed from host memory every compiled call, and weights resident on
the device after programming. Under e029 the energy stays "not sourced" (no memory energy applied).
``--registration e030`` is E-029 plus the memory-movement energy read from ``memory.published_designs`` (one
contribution per published point, per placement and byte width; bits-per-byte conversion is 8).
``--registration e031`` is E-030 plus the weight-hold term read from ``phase_shifter.published_designs`` (one
π-reconfiguration energy per weight DAC conversion, per published point).
``--registration e032`` is E-031 plus the laser term read from ``laser.published_designs`` (one DC energy-per-bit
contribution per input DAC conversion, per published point; DML interconnect-style, RF driver excluded).
``--registration e033`` is E-032 plus the thermal-stabilization term read from ``thermal.published_designs`` (one
heater energy-per-bit contribution per input DAC conversion, per published point; heater only, no control ASIC).

Reads the stored E-024 per-token counts and ``awen-spec/hardware_assumptions.yaml`` and, for every
term the system accounting needs (Master Prompt v2 sections 2 and 9: lasers, modulators, DAC,
ADC, control electronics, memory movement, host overhead, plus weight hold, thermal
stabilization and the optical detection floor), reports either a per-token contribution
(per-token count times a sourced per-event energy, with the parameter path, value, unit,
source and source type) or the status ``not sourced`` with the reason. Nothing is filled with a
guess. While any term is not sourced there is no total and no speedup or savings figure.

Power terms (W or mW) need a time model, which is not registered, so they are never converted to
energy here. Whole-system efficiency figures of other devices (``system.*_efficiency_*``) are
listed as excluded: they cannot be composed with per-term counts. Simulated counts (L1); see
``research/experiments/awen-photon-energy-accounting/PREREGISTRATION.md``.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Mapping, Optional

ACCOUNTING_VERSION = "awen.energy-per-token-accounting.v1"
_J_PER_UNIT = {"J": 1.0, "pJ": 1e-12, "fJ": 1e-15}  # unit definitions, not device values
NOT_SOURCED = "not sourced"

# term -> (per-token count key or None, assumption parameter path or None, rule)
TERMS: dict[str, tuple[Optional[str], Optional[str], str]] = {
    "laser": (None, "laser.wall_plug_efficiency", "power"),
    "modulator": ("dac_input", "modulator.energy_per_symbol", "per_event"),
    "dac_input": ("dac_input", "dac.energy_per_conversion", "per_event"),
    "dac_weight": ("dac_weight", "dac.energy_per_conversion", "per_event"),
    "weight_hold": (None, "phase_shifter.thermo_optic_power_per_pi", "power"),
    "adc": ("adc", "adc.published_designs", "points"),
    "optical_detection_floor": ("adc", "shot_noise.shot_noise_energy_per_detection_J", "sweep"),
    "control_electronics": (None, "system.control_electronics_power", "power"),
    "thermal_stabilization": (None, "system.thermal_stabilization_power", "power"),
    "memory_movement": (None, None, "absent"),
    "host_transfer": (None, "system.host_transfer_energy", "per_byte"),
}
EXCLUDED = ("system.efficiency_including_lasers", "system.on_chip_efficiency_reported")
# E-026: DAC terms read the separate published DAC designs (D-027).
TERMS_E026: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS,
    "dac_input": ("dac_input", "dac.published_designs", "points"),
    "dac_weight": ("dac_weight", "dac.published_designs", "points"),
}
# E-027: the modulator term reads the measured published modulator points (D-028).
TERMS_E027: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS_E026,
    "modulator": ("dac_input", "modulator.published_designs", "points"),
}
# E-028: host transfer counts bytes per token from the conversion counts (D-029); energy per byte still needed.
TERMS_E028: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS_E027,
    "host_transfer": ("total", "system.host_transfer_energy", "per_byte_counted"),
}
# Bytes per code crossing the host boundary: 1 for codes of up to 8 bits, 2 for 9 to 16 bits (packing, not a device).
HOST_CODE_BYTES = (1, 2)
HOST_BYTE_RULE = ("one code per input DAC conversion, weight DAC conversion and ADC read crosses the host boundary "
                  "once (weights programmed once per call, partial sums accumulated on the host), packed at "
                  "1 or 2 bytes per code; no headers, framing, compression or protocol overhead")
# E-029: memory movement counts host-memory bytes per token under two placements (D-030); no energy parameter.
TERMS_E029: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS_E028,
    "memory_movement": ("total", None, "memory_counted"),
}
# placement -> per-token count keys read from host memory, and written to host memory (lowered GEMMs only).
MEMORY_PLACEMENTS = {
    "weights_streamed": (("dac_input", "dac_weight"), ("adc",)),
    "weights_resident": (("dac_input",), ("adc",)),
}
MEMORY_RULE = ("lowered GEMMs only: every input element sent to an input DAC is read from host memory once and every "
               "ADC read is written to host memory once; weights are either read once per compiled call "
               "(weights_streamed) or held on the device after programming and never re-read (weights_resident); "
               "1 or 2 bytes per element; host-side ops, accumulator re-reads, KV cache and caches are not counted")
# E-030: memory movement energy from published designs (D-031); E-029 stays reproducible without energy.
TERMS_E030: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS_E029,
    "memory_movement": ("total", "memory.published_designs", "memory_points"),
}
# E-031: weight hold from published thermo-optic π-reconfiguration energy (D-032); E-030 stays reproducible.
TERMS_E031: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS_E030,
    "weight_hold": ("dac_weight", "phase_shifter.published_designs", "points"),
}
# E-032: laser from published DML DC energy-per-bit points (D-033); E-031 stays reproducible.
TERMS_E032: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS_E031,
    "laser": ("dac_input", "laser.published_designs", "points"),
}
# E-033: thermal stabilization from published heater energy-per-bit points (D-034); E-032 stays reproducible.
TERMS_E033: dict[str, tuple[Optional[str], Optional[str], str]] = {
    **TERMS_E032,
    "thermal_stabilization": ("dac_input", "thermal.published_designs", "points"),
}
BITS_PER_BYTE = 8  # packing conversion from bit energy to byte counts; not a device value
REGISTRATIONS = {"e025": (TERMS, ACCOUNTING_VERSION), "e026": (TERMS_E026, "awen.energy-per-token-accounting.v2"),
                 "e027": (TERMS_E027, "awen.energy-per-token-accounting.v3"),
                 "e028": (TERMS_E028, "awen.energy-per-token-accounting.v4"),
                 "e029": (TERMS_E029, "awen.energy-per-token-accounting.v5"),
                 "e030": (TERMS_E030, "awen.energy-per-token-accounting.v6"),
                 "e031": (TERMS_E031, "awen.energy-per-token-accounting.v7"),
                 "e032": (TERMS_E032, "awen.energy-per-token-accounting.v8"),
                 "e033": (TERMS_E033, "awen.energy-per-token-accounting.v9")}
CAVEATS = {
    "adc": "standalone ADC designs; their ENOB and sample rate are not those of the simulated contract",
    "dac_input": "standalone DAC designs; their resolution, rate and load are not those of a photonic driver",
    "dac_weight": "standalone DAC designs; their resolution, rate and load are not those of a photonic driver",
    "modulator": "one binary on-off-keyed modulator without its driver; not the energy of an analog photonic input symbol",
    "memory_movement": "HBM2 access energy (reads); approximate; not a host-link figure; write energy not separately reported",
    "weight_hold": "one π thermal reconfiguration (Pπ×τ) per weight DAC conversion; continuous hold during compute not included; actual phase deltas vary",
    "laser": "membrane III-V-on-Si DML DC energy per net bit (laser bias only, RF driver excluded); one bit per input DAC conversion under a binary optical-input proxy; not a CW external-laser power model; do not sum with modulator.published_designs in one architecture total",
    "thermal_stabilization": "silicon microdisk heater energy per bit for resonant trim/thermal (heater only, no control ASIC or package TEC); one bit per input DAC conversion under a binary optical-input proxy; paper estimate for a stated manufacturing+thermal budget at 10 Gbps; do not sum with modulator.published_designs athermal EO compensation in one architecture total",
}
_BIT_VALUE_KEYS = (("energy_per_bit_pJ", "pJ"),)
_POINT_VALUE_KEYS = (("energy_per_conversion_pJ", "pJ"), ("energy_per_symbol_fJ", "fJ"),
                     ("energy_per_pi_pJ", "pJ"), ("energy_per_bit_pJ", "pJ"), ("energy_per_bit_fJ", "fJ"))
REQUIRED = ("laser", "modulator", "dac_input", "dac_weight", "adc", "control_electronics",
            "memory_movement", "host_transfer")


def find_assumptions(start: Optional[Path] = None) -> Path:
    for parent in [(start or Path.cwd()).resolve(), *(start or Path.cwd()).resolve().parents]:
        candidate = parent / "awen-spec" / "hardware_assumptions.yaml"
        if candidate.exists():
            return candidate
    raise FileNotFoundError("awen-spec/hardware_assumptions.yaml not found; pass --assumptions")


def _entry(doc: Mapping[str, Any], path: str) -> Mapping[str, Any]:
    group, name = path.split(".")
    return doc["parameters"][group][name]


def _label(doc: Mapping[str, Any], entry: Mapping[str, Any], source: str) -> str:
    return entry.get("source_type") or doc["sources"][source]["type"]


def _design(point: Mapping[str, Any], count: float) -> dict[str, Any]:
    extra = {k: point[k] for k in ("enob_bits", "resolution_bits", "basis") if k in point}
    key, unit = next((k, u) for k, u in _POINT_VALUE_KEYS if k in point)
    return {"source": point["source"], **extra, "value": point[key], "unit": unit,
            "derivation": point["derivation"],
            "per_token_J": count * point[key] * _J_PER_UNIT[unit]}


def term_row(doc: Mapping[str, Any], term: str, per_token: Mapping[str, float],
             terms: Mapping[str, tuple[Optional[str], Optional[str], str]] = TERMS) -> dict[str, Any]:
    """One accounting row: a sourced per-token contribution in J, or ``not sourced`` with the reason."""
    count_key, path, rule = terms[term]
    row: dict[str, Any] = {"term": term, "parameter": path, "count": count_key,
                           "count_per_token": per_token[count_key] if count_key else None}
    if rule in ("memory_counted", "memory_points"):
        placements = {}
        for name, (reads, writes) in MEMORY_PLACEMENTS.items():
            read, write = sum(per_token[k] for k in reads), sum(per_token[k] for k in writes)
            placements[name] = {"read_elements": read, "write_elements": write,
                                "bytes_per_token": {str(w): (read + write) * w for w in HOST_CODE_BYTES}}
        counted = {**row, "memory_rule": MEMORY_RULE, "placements": placements}
        if rule == "memory_counted":
            return {**counted, "status": NOT_SOURCED,
                    "reason": "bytes per token counted per placement; energy not applied under this registration"}
        entry = _entry(doc, path)
        designs = []
        for point in entry["points"]:
            key, unit = next((k, u) for k, u in _BIT_VALUE_KEYS if k in point)
            by_placement = {
                name: {str(w): placements[name]["bytes_per_token"][str(w)] * BITS_PER_BYTE
                       * point[key] * _J_PER_UNIT[unit] for w in HOST_CODE_BYTES}
                for name in placements
            }
            extra = {k: point[k] for k in ("technology", "basis") if k in point}
            designs.append({"source": point["source"], **extra, "value": point[key], "unit": unit,
                            "derivation": point["derivation"], "bits_per_byte": BITS_PER_BYTE,
                            "per_token_J_by_placement": by_placement})
        return {**counted, "parameter": path, "status": "sourced (per published design and placement, not interpolated)",
                "source_type": entry["source_type"], "designs": designs, "caveat": CAVEATS[term]}
    if rule == "absent":
        return {**row, "status": NOT_SOURCED, "reason": "no parameter in the assumption set"}
    entry = _entry(doc, path)
    if rule == "power":
        return {**row, "status": NOT_SOURCED,
                "reason": "power term needs a time model (none registered)"
                + ("" if entry.get("typical") is not None else "; value is null")}
    if rule == "per_byte":
        return {**row, "status": NOT_SOURCED,
                "reason": "bytes moved per token are not counted"
                + ("" if entry.get("typical") is not None else "; value is null")}
    if rule == "per_byte_counted":
        codes = sum(per_token[key] for key in ("dac_input", "dac_weight", "adc"))
        counted = {**row, "count_per_token": codes, "byte_rule": HOST_BYTE_RULE,
                   "bytes_per_token": {str(width): codes * width for width in HOST_CODE_BYTES}}
        value, source = entry.get("typical"), entry.get("source")
        if value is None or not source:
            return {**counted, "status": NOT_SOURCED,
                    "reason": "bytes per token counted; energy per byte is null (no checked source)"}
        unit = entry["unit"].removesuffix("/byte")
        values = {str(width): {"bytes_per_token": codes * width, "value": value, "unit": unit,
                               "per_token_J": codes * width * value * _J_PER_UNIT[unit]} for width in HOST_CODE_BYTES}
        return {**counted, "status": "sourced (per packing width)", "source": source,
                "source_type": _label(doc, entry, source), "values": values}
    if rule == "per_event":
        value, source = entry.get("typical"), entry.get("source")
        if value is None or not source:
            return {**row, "status": NOT_SOURCED, "reason": "value is null (no checked source)"}
        return {**row, "status": "sourced", "value": value, "unit": entry["unit"], "source": source,
                "source_type": _label(doc, entry, source),
                "per_token_J": per_token[count_key] * value * _J_PER_UNIT[entry["unit"]]}
    if rule == "points":
        designs = [_design(p, per_token[count_key]) for p in entry["points"]]
        return {**row, "status": "sourced (per published design, not interpolated)",
                "source_type": entry["source_type"], "designs": designs, "caveat": CAVEATS[term]}
    values = {str(bits): {"value": v, "unit": entry["unit"], "per_token_J": per_token[count_key] * v}
              for bits, v in entry["values"].items()}
    return {**row, "status": "sourced (sweep by effective bits)", "source_type": entry["source_type"],
            "sweep_over": entry["sweep_over"], "values": values,
            "caveat": "optics only, one detection per ADC read; a lower bound on optical energy, not a system energy"}


def account(doc: Mapping[str, Any], point: Mapping[str, Any],
            terms: Mapping[str, tuple[Optional[str], Optional[str], str]] = TERMS) -> dict[str, Any]:
    rows = [term_row(doc, term, point["per_token"], terms) for term in terms]
    missing = [row["term"] for row in rows if row["status"] == NOT_SOURCED]
    return {"arch": point["arch"], "tile_k": point["tile_k"], "positions": point["positions"],
            "per_token_counts": dict(point["per_token"]), "terms": rows, "not_sourced": missing,
            "total_J": None if missing else "not defined by this registration",
            "total_reason": f"no total: {len(missing)} terms not sourced" if missing else "all terms sourced; total needs a new registration"}


def check(point: Mapping[str, Any]) -> list[str]:
    """Registered checks for one point; returns the failures (empty means pass)."""
    failures, names = [], [row["term"] for row in point["terms"]]
    failures += [f"missing term {term}" for term in REQUIRED if term not in names]
    for row in point["terms"]:
        count = row["count_per_token"]
        contributions = ([(row["value"], row["unit"], row["per_token_J"])] if "per_token_J" in row else []) + [
            (d["value"], d["unit"], d["per_token_J"]) for d in row.get("designs", [])
            if "per_token_J" in d] + [
            (v["value"], v["unit"], v["per_token_J"]) for v in row.get("values", {}).values()]
        if "bytes_per_token" in row:
            if any(row["bytes_per_token"][str(w)] != count * w for w in HOST_CODE_BYTES):
                failures.append(f"{row['term']}: bytes do not equal codes x bytes per code")
            contributions = []  # host-transfer values are per byte; checked here
            for v in row.get("values", {}).values():
                if abs(v["bytes_per_token"] * v["value"] * _J_PER_UNIT[v["unit"]] - v["per_token_J"]) > 1e-12 * abs(v["per_token_J"]):
                    failures.append(f"{row['term']}: contribution does not recompute")
        if "placements" in row:
            counts = point["per_token_counts"]
            for name, (reads, writes) in MEMORY_PLACEMENTS.items():
                got = row["placements"].get(name, {})
                read, write = sum(counts[k] for k in reads), sum(counts[k] for k in writes)
                if (got.get("read_elements"), got.get("write_elements")) != (read, write):
                    failures.append(f"{row['term']}: {name} elements do not recompute")
                elif got.get("bytes_per_token") != {str(w): (read + write) * w for w in HOST_CODE_BYTES}:
                    failures.append(f"{row['term']}: {name} bytes do not equal elements x bytes per element")
                if any(k for k in got if k not in ("read_elements", "write_elements", "bytes_per_token")):
                    failures.append(f"{row['term']}: {name} carries an unregistered field")
            if set(row["placements"]) != set(MEMORY_PLACEMENTS):
                failures.append(f"{row['term']}: placements are not the registered two")
            for design in row.get("designs", []):
                by = design.get("per_token_J_by_placement", {})
                if set(by) != set(MEMORY_PLACEMENTS):
                    failures.append(f"{row['term']}: design placements are not the registered two")
                bits = design.get("bits_per_byte", BITS_PER_BYTE)
                for name, widths in by.items():
                    for w, joules in widths.items():
                        expected = row["placements"][name]["bytes_per_token"][w] * bits * design["value"] * _J_PER_UNIT[design["unit"]]
                        if abs(expected - joules) > 1e-12 * abs(joules or expected or 1.0):
                            failures.append(f"{row['term']}: {name}@{w} contribution does not recompute")
            # memory designs are checked above; drop them from the generic count*value check
            if row.get("designs") and any("per_token_J_by_placement" in d for d in row["designs"]):
                contributions = []
        for value, unit, joules in contributions:
            if abs(count * value * _J_PER_UNIT[unit] - joules) > 1e-12 * abs(joules):
                failures.append(f"{row['term']}: contribution does not recompute")
        if row["status"] == NOT_SOURCED and (contributions or row.get("values") or row.get("designs")):
            failures.append(f"{row['term']}: not sourced but has a contribution")
    if point["not_sourced"] and point["total_J"] is not None:
        failures.append("total emitted while terms are not sourced")
    return failures


def run(counts: Mapping[str, Any], doc: Mapping[str, Any], registration: str = "e025") -> dict[str, Any]:
    terms, version = REGISTRATIONS[registration]
    points = [account(doc, point, terms) for point in counts["points"]]
    failures = {f"{p['arch']}/tile{p['tile_k']}/P{p['positions']}": f for p in points if (f := check(p))}
    return {"accounting": version, "evidence_level": "L1", "assumption_set": doc["version"],
            "counts_source": counts["curve"], "excluded": {path: "whole-system figure of another device; not composable"
                                                           for path in EXCLUDED},
            "claims": "none: partial table, no total, no speedup or savings figure",
            "verdict": "fail" if failures else "pass", "failures": failures, "points": points}


def main(argv: list[str] | None = None) -> int:
    import yaml  # pyyaml, see the `test` / `assumptions` extras

    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--counts", required=True, help="stored E-024 results.json")
    parser.add_argument("--assumptions", default=None, help="path to awen-spec/hardware_assumptions.yaml")
    parser.add_argument("--registration", choices=sorted(REGISTRATIONS), default="e025",
                        help="term set: e025 (default, unchanged), e026 (DAC from published designs) "
                             "e027 (e026 plus the modulator from published points) "
                             "e028 (e027 plus host-transfer bytes counted per token) "
                             "e029 (e028 plus memory-movement bytes counted per placement) "
                             "e030 (e029 plus memory energy from published designs) "
                             "e031 (e030 plus weight-hold from phase-shifter published designs) "
                             "e032 (e031 plus laser from DML published energy-per-bit points) "
                             "or e033 (e032 plus thermal from heater published energy-per-bit points)")
    parser.add_argument("--out", help="write the JSON result here (default: stdout)")
    args = parser.parse_args(argv)
    doc = yaml.safe_load(Path(args.assumptions or find_assumptions()).read_text())
    counts = json.loads(Path(args.counts).read_text())
    text = json.dumps(run(counts, doc, args.registration), indent=2, sort_keys=True) + "\n"
    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
