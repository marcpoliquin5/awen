import json
import subprocess
from pathlib import Path
from typing import List, Dict, Any, Optional


def compute_gradients(ir_path: str, params: List[str], strategy: str = "finite_difference", seed: Optional[int] = None, samples: int = 1) -> Dict[str, Any]:
    """Call awenctl gradient and return parsed gradients.json

    This helper assumes `awenctl` is on PATH (CI or runtime installation).
    """
    params_csv = ",".join(params)
    cmd = ["awenctl", "gradient", ir_path, params_csv, "--strategy", strategy, "--samples", str(samples)]
    if seed is not None:
        cmd += ["--seed", str(seed)]
    subprocess.run(cmd, check=True)

    # find latest awen_grad_* directory
    cwd = Path.cwd()
    candidates = list(cwd.glob("awen_grad_*"))
    if not candidates:
        raise RuntimeError("no gradient artifact directory found")
    latest = max(candidates, key=lambda p: p.stat().st_mtime)
    grad_file = latest / "gradients.json"
    if not grad_file.exists():
        raise RuntimeError(f"gradients.json not found in {latest}")
    return json.loads(grad_file.read_text())


def run_ir(ir_path: str, seed: Optional[int] = None) -> Dict[str, Any]:
    """Run awenctl run and return a mapping of artifact files.

    Returns a dict with paths to ir.json, results.json, trace.json, metadata.json
    """
    cmd = ["awenctl", "run", ir_path]
    if seed is not None:
        cmd += ["--seed", str(seed)]
    subprocess.run(cmd, check=True)
    cwd = Path.cwd()
    candidates = list(cwd.glob("awen_run_*"))
    if not candidates:
        raise RuntimeError("no run artifact directory found")
    latest = max(candidates, key=lambda p: p.stat().st_mtime)
    files = {}
    for name in ["ir.json", "results.json", "trace.json", "metadata.json"]:
        p = latest / name
        files[name] = str(p) if p.exists() else None
    return files
