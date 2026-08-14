#!/usr/bin/env python3
"""Scan Cargo dependency metadata for missing or prohibited licenses."""

from __future__ import annotations

import json
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
MANIFESTS = ("awen-compiler/Cargo.toml", "awen-runtime/Cargo.toml")
PROHIBITED = ("AGPL", "BUSL", "COMMONS-CLAUSE", "SSPL")
packages: dict[tuple[str, str], dict[str, object]] = {}

for manifest in MANIFESTS:
    result = subprocess.run(
        [
            "cargo", "metadata", "--locked", "--format-version", "1",
            "--manifest-path", manifest,
        ],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    for package in json.loads(result.stdout)["packages"]:
        packages[(package["name"], package["version"])] = package

failures: list[str] = []
for (name, version), package in sorted(packages.items()):
    expression = package.get("license")
    license_file = package.get("license_file")
    if not expression and not license_file:
        failures.append(f"{name} {version}: no license expression or license file")
        continue
    normalized = str(expression or "").upper()
    for prohibited in PROHIBITED:
        if prohibited in normalized:
            failures.append(f"{name} {version}: prohibited license {expression}")

if failures:
    print("dependency license scan failed:", file=sys.stderr)
    print("\n".join(f"- {failure}" for failure in failures), file=sys.stderr)
    raise SystemExit(1)

print(f"dependency license scan passed for {len(packages)} Rust packages")
