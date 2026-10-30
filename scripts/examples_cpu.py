#!/usr/bin/env python3
"""CPU fallback check for every example in the repository (Master Prompt v2 section 4).

Every file under an ``examples/`` directory must have an entry in ``EXAMPLES``.

* ``--check`` (stdlib only): the manifest lists exactly the example files on disk.
* ``--run``: also runs every Python example with CUDA hidden (``CUDA_VISIBLE_DEVICES=""``), small
  arguments and a timeout, and checks the exit code and the expected output. One JSON line per example.

Non-Python examples are CPU-only code; their entry names the required-gate step that builds or runs them.
This script does not run them.

    python scripts/examples_cpu.py --check
    python scripts/examples_cpu.py --run
"""
from __future__ import annotations

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKIP_DIRS = {".git", "node_modules", "target", ".venv", "__pycache__", "dist", "build"}
TIMEOUT_S = 600

# path -> how its CPU fallback is checked.
#   "run": argv after the interpreter (run from the repo root); "expect": substrings stdout must contain;
#   "jsonl": every non-empty stdout line must be a JSON object without "failed".
#   "gate": required-gate step that covers a non-Python example (not run here).
EXAMPLES: dict[str, dict] = {
    "awen-ecosystem/python_awen/examples/numpy_example.py": {
        "run": [], "expect": ["'device': 'cpu'"]},
    "awen-ecosystem/python_awen/examples/pytorch_example.py": {
        "run": [], "expect": ["grad_fn", '"framework_fallback"']},
    "awen-ecosystem/python_awen/examples/jax_example.py": {
        "run": [], "expect": []},
    "awen-models/examples/generate.py": {
        "run": ["--device", "auto", "--tokens", "20", "--seed", "0"], "expect": ["[cpu, "]},
    "awen-models/examples/kron3_step_memory.py": {
        "run": ["--batch", "1", "--chunks", "none", "17", "--threads", "2", "--cap-gb", "6"], "jsonl": True},
    "awen-models/examples/kron3_step_time.py": {
        "run": ["--rank", "2", "--batch", "1", "--modes", "chunk:1", "dense", "--warmup", "0", "--steps", "1",
                "--threads", "2"], "jsonl": True},
    "awen-compiler/examples/assumptions_before_after.rs": {
        "gate": "Test compiler (cargo test builds examples; not executed)"},
    "awen-compiler/examples/gemm_4x4.json": {
        "gate": "Test runtime (read by awen-runtime/tests/plugin_discovery.rs)"},
    "awen-compiler/examples/gemm_256.json": {
        "gate": "Test compiler and Test runtime (read by awen-compiler/tests/pipeline.rs, awen-runtime/tests/cli_smoke.rs)"},
    "awen-runtime/examples/awen_c_example.c": {
        "gate": "Compile and execute the C SDK example (C11, awen.h)"},
    "awen-runtime/examples/framework_cpp.cpp": {
        "gate": "Compile and execute C++20 ABI consumer"},
}


def discover(root: Path = ROOT) -> list[str]:
    """Every file under a directory named ``examples``, as sorted repo-relative POSIX paths."""
    found = []
    for dirpath, dirnames, filenames in os.walk(root):
        dirnames[:] = [d for d in dirnames if d not in SKIP_DIRS]
        rel = Path(dirpath).relative_to(root)
        if "examples" in rel.parts:
            found += [(rel / f).as_posix() for f in filenames]
    return sorted(found)


def check_manifest(found: list[str], manifest: dict = EXAMPLES) -> list[str]:
    """Problems with the manifest: missing or stale entries, entries with no check or a run on a non-Python file."""
    problems = [f"no CPU fallback entry for {p}" for p in found if p not in manifest]
    problems += [f"manifest entry for missing file {p}" for p in manifest if p not in found]
    for path, entry in manifest.items():
        if ("run" in entry) == ("gate" in entry):
            problems.append(f"{path}: needs exactly one of 'run' or 'gate'")
        if "run" in entry and not path.endswith(".py"):
            problems.append(f"{path}: only Python examples are run")
    return problems


def judge(entry: dict, returncode: int, stdout: str) -> str | None:
    """Failure reason for one finished run, or None when it passed."""
    if returncode != 0:
        return f"exit code {returncode}"
    missing = [s for s in entry.get("expect", []) if s not in stdout]
    if missing:
        return f"expected output missing: {missing}"
    if entry.get("jsonl"):
        lines = [line for line in stdout.splitlines() if line.strip()]
        if not lines:
            return "no JSON lines"
        for line in lines:
            try:
                rec = json.loads(line)
            except json.JSONDecodeError:
                return f"not JSON: {line[:80]}"
            if not isinstance(rec, dict) or rec.get("failed"):
                return f"configuration failed: {line[:200]}"
    return None


def cpu_env() -> dict:
    env = dict(os.environ)
    env["CUDA_VISIBLE_DEVICES"] = ""
    env.setdefault("OMP_NUM_THREADS", "2")
    return env


def run_all(manifest: dict = EXAMPLES) -> int:
    failures = 0
    for path, entry in sorted(manifest.items()):
        if "run" not in entry:
            print(json.dumps({"example": path, "status": "not run here", "covered_by": entry["gate"]}), flush=True)
            continue
        cmd = [sys.executable, path, *entry["run"]]
        started = time.perf_counter()
        try:
            proc = subprocess.run(cmd, cwd=ROOT, env=cpu_env(), capture_output=True, text=True, timeout=TIMEOUT_S)
            reason = judge(entry, proc.returncode, proc.stdout)
            err = proc.stderr
        except subprocess.TimeoutExpired:
            reason, err = f"timeout after {TIMEOUT_S} s", ""
        record = {"example": path, "command": " ".join(["python", path, *entry["run"]]),
                  "cuda_visible_devices": "", "status": "passed" if reason is None else "failed",
                  "seconds": round(time.perf_counter() - started, 2)}
        if reason is not None:
            failures += 1
            record.update(reason=reason, stderr_tail=err.strip().splitlines()[-5:])
        print(json.dumps(record), flush=True)
    ran = sum("run" in e for e in manifest.values())
    print(f"CPU fallback: {ran - failures} of {ran} Python examples passed "
          f"({len(manifest) - ran} non-Python examples covered by gate steps)", flush=True)
    return 1 if failures else 0


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    mode = ap.add_mutually_exclusive_group(required=True)
    mode.add_argument("--check", action="store_true", help="manifest coverage only (no imports)")
    mode.add_argument("--run", action="store_true", help="coverage, then run every Python example on CPU")
    args = ap.parse_args(argv)
    problems = check_manifest(discover())
    for p in problems:
        print(f"error: {p}", file=sys.stderr)
    if problems:
        return 1
    print(f"manifest covers all {len(EXAMPLES)} example files")
    return run_all() if args.run else 0


if __name__ == "__main__":
    raise SystemExit(main())
