#!/usr/bin/env python3
"""Inventory of device energies and noise still hard-coded in Rust defaults.

Master Prompt v2 section 9: every simulator and cost estimate should read device energies
and noise from awen-spec/hardware_assumptions.yaml, and no other file may hard-code them.
Until the compiler reads that file, this script keeps an exact list of the literals that are
still hard-coded (awen-spec/hardcoded_device_defaults.yaml) and checks it against the code:

* every scanned literal must be listed with the same value (no new silent defaults), and
* every listed entry must still exist in the code (no stale entries), and
* each entry's status must agree with hardware_assumptions.yaml (``unsourced`` only when
  the assumption set has no value for it).

Usage: python scripts/hardcoded_defaults.py [--check]   (prints the inventory table)
"""

from __future__ import annotations

import argparse
import re
import sys
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[1]
INVENTORY = ROOT / "awen-spec" / "hardcoded_device_defaults.yaml"
ASSUMPTIONS = ROOT / "awen-spec" / "hardware_assumptions.yaml"

# Field names that carry a device energy, power, noise, loss or precision value
# (dimensionless `*_factor` multipliers are skipped).
DEVICE_FIELD = re.compile(
    r"(energy|power|noise|rin_|crosstalk|insertion_loss|quantum_efficiency|phase_sigma"
    r"|photons|drift_rad|_bits$|bandwidth_hz|bandwidth_ghz|reconfiguration_latency)"
)
# `name: 1.0,` / `name: Some(-150.0),` / `name: 2_000.0,` on one line (formulas are skipped).
LITERAL = re.compile(r"^\s*([a-z_][a-z0-9_]*):\s*(?:Some\()?(-?[0-9][0-9_]*(?:\.[0-9_]*)?(?:e-?[0-9]+)?)\)?,\s*$")
STATUSES = {"unsourced", "matches_assumption", "differs_from_assumption"}


def block_lines(text: str, anchor: str) -> list[str]:
    """Lines from `anchor` to the closing `}` or `};` at the anchor's own indentation."""
    lines = text.splitlines()
    starts = [i for i, line in enumerate(lines) if line.strip().startswith(anchor)]
    if len(starts) != 1:
        raise ValueError(f"anchor {anchor!r} found {len(starts)} times (need exactly 1)")
    start = starts[0]
    indent = lines[start][: len(lines[start]) - len(lines[start].lstrip())]
    for end in range(start + 1, len(lines)):
        if lines[end] in (indent + "}", indent + "};"):
            return lines[start : end + 1]
    raise ValueError(f"anchor {anchor!r}: no closing brace")


def parse_number(raw: str) -> float:
    return float(raw.replace("_", ""))


def scan(root: Path, blocks: list[dict]) -> dict[tuple[str, str], float]:
    """Return {(block id, field): value} for every device literal in the listed blocks."""
    found: dict[tuple[str, str, str], float] = {}
    for block in blocks:
        text = (root / block["file"]).read_text()
        for line in block_lines(text, block["anchor"]):
            match = LITERAL.match(line)
            if match and DEVICE_FIELD.search(match.group(1)) and not match.group(1).endswith("_factor"):
                key = (block["id"], match.group(1))
                if key in found:
                    raise ValueError(f"duplicate field in block: {key}")
                found[key] = parse_number(match.group(2))
    return found


def assumption_values(assumptions: dict, dotted: str) -> list[float]:
    """All non-null numbers stored under a parameter path (range, sweep, keyed or points)."""
    node = assumptions["parameters"]
    for part in dotted.split("."):
        node = node[part]  # KeyError: the entry names a parameter that does not exist
    values: list[float] = []
    for key in ("low", "typical", "high"):
        if isinstance(node.get(key), (int, float)):
            values.append(float(node[key]))
    for value in (node.get("values") or {}).values():
        if isinstance(value, (int, float)):
            values.append(float(value))
    for point in node.get("points") or []:
        values.extend(float(v) for k, v in point.items() if k.startswith("energy") and isinstance(v, (int, float)))
    return values


def check(root: Path = ROOT) -> list[str]:
    """Return a list of problems (empty when code, inventory and assumptions agree)."""
    inventory = yaml.safe_load((root / INVENTORY.relative_to(ROOT)).read_text())
    assumptions = yaml.safe_load((root / ASSUMPTIONS.relative_to(ROOT)).read_text())
    problems: list[str] = []
    if inventory.get("assumption_set") != assumptions["version"]:
        problems.append(f"inventory assumption_set {inventory.get('assumption_set')!r} != {assumptions['version']!r}")
    found = scan(root, inventory["blocks"])
    listed: dict[tuple[str, str], dict] = {}
    block_ids = [block["id"] for block in inventory["blocks"]]
    if len(block_ids) != len(set(block_ids)):
        problems.append(f"duplicate block ids: {block_ids}")
    for entry in inventory["entries"]:
        key = (entry["block"], entry["field"])
        if entry["block"] not in block_ids:
            problems.append(f"{key}: unknown block {entry['block']!r}")
        if key in listed:
            problems.append(f"listed twice: {key}")
        listed[key] = entry
        for required in ("unit", "status", "notes"):
            if not str(entry.get(required) or "").strip():
                problems.append(f"{key}: missing {required}")
        if entry.get("status") not in STATUSES:
            problems.append(f"{key}: status {entry.get('status')!r} not in {sorted(STATUSES)}")
        if key not in found:
            problems.append(f"stale entry (not in code any more): {key}")
        elif found[key] != float(entry["value"]):
            problems.append(f"{key}: code has {found[key]}, inventory says {entry['value']}")
        target = entry.get("assumption")
        try:
            values = assumption_values(assumptions, target) if target else []
        except KeyError:
            problems.append(f"{key}: assumption {target!r} is not a parameter in {ASSUMPTIONS.name}")
            continue
        if entry.get("status") == "unsourced" and values:
            problems.append(f"{key}: marked unsourced but {target} has values {values}")
        if entry.get("status") in {"matches_assumption", "differs_from_assumption"}:
            if not values:
                problems.append(f"{key}: status {entry['status']} needs an assumption with a value")
            else:
                matches = float(entry["value"]) in values
                if matches != (entry["status"] == "matches_assumption"):
                    problems.append(f"{key}: status {entry['status']} but value in {target} values {values} is {matches}")
    for key in sorted(set(found) - set(listed)):
        problems.append(f"new hard-coded device default not in the inventory: {key} = {found[key]}")
    return problems


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--check", action="store_true", help="exit 1 if code and inventory disagree")
    args = parser.parse_args(argv)
    inventory = yaml.safe_load(INVENTORY.read_text())
    files = {block["id"]: block["file"] for block in inventory["blocks"]}
    counts: dict[str, int] = {}
    for entry in inventory["entries"]:
        counts[entry["status"]] = counts.get(entry["status"], 0) + 1
        print(f"{entry['status']:<24} {entry['field']:<40} {entry['value']!s:>10} {entry['unit']:<14} {files[entry['block']]}")
    print("totals:", ", ".join(f"{k} {v}" for k, v in sorted(counts.items())), f"(entries {len(inventory['entries'])})")
    problems = check()
    for problem in problems:
        print("PROBLEM:", problem, file=sys.stderr)
    return 1 if (args.check and problems) else 0


if __name__ == "__main__":
    raise SystemExit(main())
