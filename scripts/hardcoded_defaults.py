#!/usr/bin/env python3
"""Inventory of device energies and noise still hard-coded in Rust defaults.

Master Prompt v2 section 9: every simulator and cost estimate should read device energies
and noise from awen-spec/hardware_assumptions.yaml, and no other file may hard-code them.
Until the compiler reads that file, this script keeps an exact list of the literals that are
still hard-coded (awen-spec/hardcoded_device_defaults.yaml) and checks it against every
non-test Rust file in the inventory's `scope` (awen-compiler/src and awen-runtime/src):

* every device literal found must be listed with the same value (no new silent defaults), and
  so must every float literal passed to a fallback, floor or cap inside an expression
  (``.map_or(0.05, ...)``, ``.unwrap_or(0.0)``, ``.max(0.001)``, ``.min(1.0)``, ``.clamp(a, b)``),
  listed under the field ``expr:<method>``, and
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

# Field names that carry a device energy, power, noise, loss, latency, voltage, fidelity, calibration or
# precision value (dimensionless `*_factor` multipliers are skipped).
DEVICE_FIELD = re.compile(
    r"(energy|power|noise|rin_|crosstalk|insertion_loss|quantum_efficiency|phase_sigma|phase_error"
    r"|photons|drift|_bits$|bandwidth_hz|bandwidth_ghz|reconfiguration_latency|_uj$"
    r"|^gain$|^offset$|uncertainty|temperature"
    r"|dark_count|dark_current|saturation|hysteresis|_loss_|coherence_time|latency|voltage|fidelity)"
)
# `name: 1.0,` / `name: Some(-150.0),` / `name: 2_000.0,` on one line (formulas are skipped).
LITERAL = re.compile(r"^\s*([a-z_][a-z0-9_]*):\s*(?:Some\()?(-?[0-9][0-9_]*(?:\.[0-9_]*)?(?:e-?[0-9]+)?)\)?,\s*$")
# `.map_or(0.05, f)` / `.unwrap_or(0.0)` / `.max(0.001)` / `.min(1.0)` / `.clamp(-1.0, 1.0)` anywhere
# in a line. Only float literals (with a `.` or exponent) count: integer guards such as `.max(1)` on
# counts and `.min(1024)` on capacities are not device values. `fold(0.0, ...)` accumulators,
# arithmetic factors (`x * 0.01`) and named constants are not matched.
FLOAT = r"-?[0-9][0-9_]*(?:\.[0-9_]*(?:e-?[0-9]+)?|e-?[0-9]+)(?:_f64|_f32)?"
FALLBACK = re.compile(rf"\.(map_or|unwrap_or|max|min|clamp)\(\s*({FLOAT})\s*[,)]")
ITEM = re.compile(r"^(\s*)(?:pub(?:\([a-z]+\))?\s+)?(fn|impl)\b")
FN_NAME = re.compile(r"\bfn\s+([A-Za-z0-9_]+)")
STATUSES = {"unsourced", "matches_assumption", "differs_from_assumption", "definitional", "numeric_guard"}


def parse_number(raw: str) -> float:
    return float(raw.removesuffix("_f64").removesuffix("_f32").replace("_", ""))


def is_device_field(name: str) -> bool:
    return bool(DEVICE_FIELD.search(name)) and not name.endswith("_factor")


def literals(text: str) -> dict[tuple[str, str], float]:
    """{(context, field): value} for device literals in non-test Rust source.

    The context is the enclosing function, prefixed by its `impl` header when it is a method
    (e.g. ``impl Default for PhotonicNoiseContract::default``). A field that appears more than
    once in one context gets ``#2``, ``#3``... in source order. Scanning stops at the
    ``#[cfg(test)] mod tests`` module.
    """
    found: dict[tuple[str, str], float] = {}
    lines = text.splitlines()
    impl_header, context = None, "<file scope>"
    for index, line in enumerate(lines):
        if line.strip() == "#[cfg(test)]" and index + 1 < len(lines) and lines[index + 1].lstrip().startswith("mod tests"):
            break
        if line == "}":
            impl_header, context = None, "<file scope>"
            continue
        item = ITEM.match(line)
        if item:
            if item.group(2) == "impl" and not item.group(1):
                impl_header = " ".join(line.strip().rstrip("{").split())
                context = impl_header
            elif item.group(2) == "fn" and (name := FN_NAME.search(line)):
                context = f"{impl_header}::{name.group(1)}" if item.group(1) and impl_header else name.group(1)
            continue
        if line.lstrip().startswith("//"):
            continue
        match = LITERAL.match(line)
        if match and is_device_field(match.group(1)):
            add(found, context, match.group(1), parse_number(match.group(2)))
        for fallback in FALLBACK.finditer(line):
            add(found, context, f"expr:{fallback.group(1)}", parse_number(fallback.group(2)))
    return found


def add(found: dict[tuple[str, str], float], context: str, field: str, value: float) -> None:
    n = 1
    while (context, field if n == 1 else f"{field}#{n}") in found:
        n += 1
    found[(context, field if n == 1 else f"{field}#{n}")] = value


def scan(root: Path, scope: list[str]) -> dict[tuple[str, str, str], float]:
    """{(file, context, field): value} for every device literal in the scoped source trees."""
    found: dict[tuple[str, str, str], float] = {}
    for tree in scope:
        for path in sorted((root / tree).rglob("*.rs")):
            rel = path.relative_to(root).as_posix()
            for (context, field), value in literals(path.read_text()).items():
                found[(rel, context, field)] = value
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
    found = scan(root, inventory["scope"])
    blocks = {block["id"]: (block["file"], block["context"]) for block in inventory["blocks"]}
    if len(blocks) != len(inventory["blocks"]) or len(set(blocks.values())) != len(blocks):
        problems.append("duplicate block ids or contexts")
    listed: dict[tuple[str, str, str], dict] = {}
    for entry in inventory["entries"]:
        if entry["block"] not in blocks:
            problems.append(f"{entry['field']}: unknown block {entry['block']!r}")
            continue
        key = (*blocks[entry["block"]], entry["field"])
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
        if entry.get("status") in {"unsourced", "definitional"} and values:
            problems.append(f"{key}: marked {entry['status']} but {target} has values {values}")
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
    print("scope:", ", ".join(inventory["scope"]), "| not yet inventoried:", "; ".join(inventory["not_yet_inventoried"]))
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
