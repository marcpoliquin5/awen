#!/usr/bin/env python3
"""Run the three awen-physics validation notebooks and store a run record.

Each notebook's code cells are executed in order in one namespace (no Jupyter
kernel needed); the notebooks' own asserts are the acceptance checks. The record
names the commit, environment, notebook SHA-256 and the printed output of every
cell, so each number a notebook prints has a command and a stored output.

    python scripts/run_validation_notebooks.py --require-native \
        --out awen-physics/results/notebooks/run.json
"""

from __future__ import annotations

import argparse
import contextlib
import hashlib
import importlib.util
import io
import json
import platform
import subprocess
import sys
import time
import traceback
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NOTEBOOK_DIR = ROOT / "awen-physics" / "notebooks"
EVIDENCE_LEVELS = ("L0", "L1", "L2", "L3")
# Notebook (c) drives the Rust model through the awen_native extension.
NEEDS_NATIVE = ("c_",)


def notebooks(directory: Path = NOTEBOOK_DIR) -> list[Path]:
    return sorted(directory.glob("*.ipynb"))


def declared_evidence_level(notebook: dict) -> str | None:
    """Return the level in the first markdown cell's "**Evidence level.** Lx" line."""
    for cell in notebook["cells"]:
        if cell["cell_type"] != "markdown":
            continue
        for line in "".join(cell["source"]).splitlines():
            if line.startswith("**Evidence level.**"):
                rest = line.removeprefix("**Evidence level.**").split()
                level = rest[0].rstrip(".,") if rest else ""
                return level if level in EVIDENCE_LEVELS else None
        return None
    return None


def run_notebook(path: Path) -> dict:
    notebook = json.loads(path.read_text(encoding="utf-8"))
    namespace: dict = {"__name__": "__notebook__"}
    cells = []
    status, error = "passed", None
    start = time.perf_counter()
    for index, cell in enumerate(notebook["cells"]):
        if cell["cell_type"] != "code":
            continue
        buffer = io.StringIO()
        try:
            with contextlib.redirect_stdout(buffer):
                code = compile("".join(cell["source"]), f"{path.name}[cell {index}]", "exec")
                exec(code, namespace)  # noqa: S102 - repository notebooks only
        except Exception:  # noqa: BLE001 - recorded, then reported as a failure
            status, error = "failed", traceback.format_exc(limit=3)
            cells.append({"cell": index, "stdout": buffer.getvalue()})
            break
        cells.append({"cell": index, "stdout": buffer.getvalue()})
    return {
        "notebook": path.name,
        "sha256": hashlib.sha256(path.read_bytes()).hexdigest(),
        "evidence_level": declared_evidence_level(notebook),
        "status": status,
        "error": error,
        "seconds": round(time.perf_counter() - start, 3),
        "cells": cells,
    }


def git_commit() -> str:
    try:
        out = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True)
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def environment() -> dict:
    versions = {}
    for module in ("numpy", "awen_physics", "awen_native"):
        spec = importlib.util.find_spec(module)
        if spec is None:
            versions[module] = None
            continue
        imported = __import__(module)
        versions[module] = getattr(imported, "__version__", "installed")
    return {"python": platform.python_version(), "platform": platform.platform(), "packages": versions}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", type=Path, help="write the JSON run record here")
    parser.add_argument(
        "--require-native", action="store_true", help="fail instead of skipping notebook (c) when awen_native is missing"
    )
    parser.add_argument("--notebooks", type=Path, default=NOTEBOOK_DIR, help=argparse.SUPPRESS)
    args = parser.parse_args(argv)

    have_native = importlib.util.find_spec("awen_native") is not None
    paths = notebooks(args.notebooks)
    results = []
    for path in paths:
        if path.name.startswith(NEEDS_NATIVE) and not have_native:
            if args.require_native:
                results.append({"notebook": path.name, "status": "failed", "error": "awen_native is not installed"})
            else:
                results.append({"notebook": path.name, "status": "skipped", "error": "awen_native is not installed"})
            continue
        results.append(run_notebook(path))

    record = {
        "schema": "awen_validation_notebooks_run.v1",
        "command": "python scripts/run_validation_notebooks.py" + (" --require-native" if args.require_native else ""),
        "commit": git_commit(),
        "environment": environment(),
        "source_label": "measured by us (simulator and solver output on the build box)",
        "notebooks": results,
    }
    for result in results:
        if result["status"] == "passed" and result.get("evidence_level") is None:
            result["status"], result["error"] = "failed", "no '**Evidence level.** L0-L3' line in the first markdown cell"
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    for result in results:
        print(f"{result['notebook']}: {result['status']}" + (f" ({result['error'].strip()})" if result["error"] else ""))
    failed = [r for r in results if r["status"] == "failed"]
    if not paths:
        print("no notebooks found", file=sys.stderr)
        return 1
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
