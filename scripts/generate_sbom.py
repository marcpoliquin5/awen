#!/usr/bin/env python3
"""Generate CycloneDX SBOMs for AWEN's Rust crates (and optionally the Python env).

Rust: ``cargo cyclonedx`` (CycloneDX 1.5 JSON, all dependencies, from each
crate's committed ``Cargo.lock``). Python: ``pip-audit -f cyclonedx-json`` over
the current environment when ``--python`` is given.

SOURCE_DATE_EPOCH defaults to the HEAD commit time so reruns on the same commit
produce the same timestamps. Each output is checked to be a CycloneDX document
with a non-empty component list; ``SBOM-SUMMARY.json`` records component counts.

Usage: python scripts/generate_sbom.py --out dist/sbom [--python]
"""

from __future__ import annotations

import argparse
import json
import os
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
CRATES = ("awen-compiler", "awen-runtime", "awen-native")


def commit_epoch() -> str:
    try:
        return subprocess.run(
            ["git", "log", "-1", "--format=%ct"], cwd=ROOT, check=True,
            capture_output=True, text=True,
        ).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "0"


def check_bom(path: Path) -> int:
    bom = json.loads(path.read_text())
    if bom.get("bomFormat") != "CycloneDX":
        raise SystemExit(f"{path}: not a CycloneDX document")
    components = bom.get("components") or []
    if not components:
        raise SystemExit(f"{path}: SBOM has no components")
    return len(components)


def rust_sbom(crate: str, out: Path, env: dict[str, str]) -> Path:
    manifest = ROOT / crate / "Cargo.toml"
    name = crate.replace("-", "_") + ".rust"
    subprocess.run(
        ["cargo", "cyclonedx", "--manifest-path", str(manifest), "-f", "json",
         "--spec-version", "1.5", "--all", "--override-filename", name],
        cwd=ROOT, check=True, env=env,
    )
    produced = ROOT / crate / f"{name}.json"
    target = out / f"{crate}.cdx.json"
    shutil.move(produced, target)
    return target


def python_sbom(out: Path) -> Path:
    target = out / "python-environment.cdx.json"
    # pip-audit exits non-zero when it finds vulnerabilities; the SBOM is still
    # written, and the separate audit step in CI is what enforces "no vulns".
    subprocess.run(
        [sys.executable, "-m", "pip_audit", "--local", "--skip-editable",
         "-f", "cyclonedx-json", "-o", str(target)],
        cwd=ROOT, check=False,
    )
    if not target.exists():
        raise SystemExit("pip-audit did not write a CycloneDX SBOM")
    return target


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--out", type=Path, required=True)
    parser.add_argument("--python", action="store_true", help="also SBOM the Python environment")
    args = parser.parse_args()
    args.out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ)
    env.setdefault("SOURCE_DATE_EPOCH", commit_epoch())
    summary = {}
    for crate in CRATES:
        path = rust_sbom(crate, args.out, env)
        summary[path.name] = check_bom(path)
    if args.python:
        path = python_sbom(args.out)
        summary[path.name] = check_bom(path)
    (args.out / "SBOM-SUMMARY.json").write_text(json.dumps(
        {"format": "CycloneDX", "source_date_epoch": env["SOURCE_DATE_EPOCH"], "components": summary},
        indent=2, sort_keys=True) + "\n")
    for name, count in summary.items():
        print(f"{name}: {count} components")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
