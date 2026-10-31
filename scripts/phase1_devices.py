#!/usr/bin/env python3
"""Phase 1 device check: load awen-mini through Runtime on every device, generate, store a run record.

Phase 1 (Master Prompt v2) is done when a fresh clone runs ``auto``, ``cpu`` and ``photonic-sim`` on CPU in CI with
a tiny config, and on CUDA on Marc's laptop (logged). The tiny config is the bundled ``awen-mini`` checkpoint; the
AWEN-Mini v0 base model is Marc's decision (D-008). ``cuda`` runs when ``torch.cuda.is_available()``, else it is
skipped (``--require-cuda`` makes that a failure). ``photonic-sim`` also stores a capability-report summary;
with ``--ideal`` it uses the noise-free contract and must reproduce the ``cpu`` text. ``auto`` must resolve to
``cuda`` when present, else to ``cpu`` with the ``cpu`` text. Simulated or host execution only: no energy, latency
or speedup figure; ``seconds`` is host wall-clock for load plus generate. Checks are documented in docs/install.md.

    python scripts/phase1_devices.py --out phase1-devices.json
    python scripts/phase1_devices.py --devices cpu,photonic-sim --ideal
    python scripts/phase1_devices.py --require-cuda --out phase1-devices-cuda.json  # laptop with CUDA
"""

from __future__ import annotations

import argparse
import hashlib
import importlib.util
import json
import platform
import shlex
import subprocess
import sys
import time
from pathlib import Path
from typing import Any, Callable

ROOT = Path(__file__).resolve().parents[1]
SCHEMA = "awen.phase1-devices-run.v1"
DEFAULT_DEVICES = ("cpu", "auto", "photonic-sim", "cuda")
DEFAULT_PROMPT = "ROMEO:"
SOURCE_LABEL = "measured by us (host execution; photonic-sim is simulated, L1)"


def git_commit() -> str:
    try:
        out = subprocess.run(["git", "rev-parse", "HEAD"], cwd=ROOT, capture_output=True, text=True, check=True)
        return out.stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return "unknown"


def environment() -> dict[str, Any]:
    versions: dict[str, Any] = {}
    for module in ("torch", "numpy", "awen", "awen_models", "awen_native"):
        if importlib.util.find_spec(module) is None:
            versions[module] = None
            continue
        imported = __import__(module)
        versions[module] = getattr(imported, "__version__", "installed")
    env: dict[str, Any] = {"python": platform.python_version(), "platform": platform.platform(),
                           "packages": versions, "cuda_available": False, "cuda_device": None}
    if versions["torch"] is not None:
        import torch

        env["cuda_available"] = bool(torch.cuda.is_available())
        env["torch_cuda_build"] = torch.version.cuda
        if env["cuda_available"]:
            env["cuda_device"] = torch.cuda.get_device_name(0)
    return env


def summarize_capability(report: dict[str, Any]) -> dict[str, Any]:
    """The capability-report fields the run record keeps (the full report is `python -m awen.capability`)."""
    execution = report["execution"]
    return {
        "report": report["report"],
        "evidence_level": execution["evidence_level"],
        "physical": execution["physical"],
        "fidelity": execution["fidelity"],
        "lowered_gemms": len(report["lowered"]),
        "host_fallbacks": len(report["fallback"]),
        "photonic_sim_fraction": report["mac_split"]["photonic_sim_fraction"],
        "conversions": {k: report["conversions"][k] for k in ("dac_input", "dac_weight", "adc")},
        "deviation_vs_cpu": report["deviation_vs_cpu"],
        "energy": report["energy"],
    }


def check_result(result: dict[str, Any], prompt: str, max_new_tokens: int) -> list[str]:
    """Problems with one device result (empty when it passes)."""
    problems: list[str] = []
    text = result.get("text") or ""
    if not text.startswith(prompt) or len(text) != len(prompt) + max_new_tokens:
        problems.append(f"generated {len(text)} characters, expected the prompt plus {max_new_tokens}")
    requested, fidelity = result["requested"], result.get("fidelity")
    if requested in ("cpu", "cuda") and fidelity != "measured":
        problems.append(f"fidelity {fidelity!r}, expected 'measured'")
    if requested == "cpu" and result.get("device") != "cpu":
        problems.append(f"resolved to {result.get('device')!r}, expected 'cpu'")
    if requested == "cuda" and not str(result.get("device", "")).startswith("cuda"):
        problems.append(f"resolved to {result.get('device')!r}, expected cuda")
    if requested == "photonic-sim":
        cap = result.get("capability") or {}
        if fidelity != "simulated":
            problems.append(f"fidelity {fidelity!r}, expected 'simulated'")
        if cap.get("evidence_level") != "L1" or cap.get("physical") is not False:
            problems.append("capability report must say evidence level L1 and physical false")
        if not cap.get("lowered_gemms"):
            problems.append("no GEMM was lowered to the photonic simulator")
    return problems


def check_auto(results: dict[str, dict[str, Any]], cuda_available: bool) -> list[str]:
    auto, cpu = results.get("auto"), results.get("cpu")
    if not auto or auto["status"] != "passed":
        return []
    expected = "cuda" if cuda_available else "cpu"
    if not str(auto["device"]).startswith(expected):
        return [f"auto resolved to {auto['device']!r}, expected {expected!r}"]
    if expected == "cpu" and cpu and cpu["status"] == "passed" and auto["text"] != cpu["text"]:
        return ["auto resolved to cpu but its text differs from the cpu text (same checkpoint and seed)"]
    return []


def check_ideal(results: dict[str, dict[str, Any]]) -> list[str]:
    sim, cpu = results.get("photonic-sim"), results.get("cpu")
    if not sim or sim["status"] != "passed" or not cpu or cpu["status"] != "passed":
        return []
    if sim["text"] != cpu["text"]:
        return ["photonic-sim under the ideal contract generated different text from cpu"]
    return []


def run_device(device: str, model: str, prompt: str, max_new_tokens: int, seed: int,
               runtime_factory: Callable[..., Any] | None = None, noise: Any = None) -> dict[str, Any]:
    if runtime_factory is None:
        from awen import Runtime as runtime_factory  # noqa: N813 - imported lazily so --help needs no torch
    start = time.perf_counter()
    kwargs = {"noise": noise} if device == "photonic-sim" and noise is not None else {}
    loaded = runtime_factory(device=device, seed=seed, **kwargs).load_model(model)
    text = loaded.generate(prompt, max_new_tokens=max_new_tokens, seed=seed)
    result: dict[str, Any] = {
        "requested": device,
        "device": loaded.execution.device,
        "fidelity": loaded.execution.fidelity,
        "gemm_backend": loaded.execution.gemm_backend,
        "text": text,
        "text_sha256": hashlib.sha256(text.encode("utf-8")).hexdigest(),
    }
    if device == "photonic-sim":
        result["capability"] = summarize_capability(loaded.capability_report(prompt))
    result["seconds"] = round(time.perf_counter() - start, 3)
    return result


def run_all(devices: list[str], model: str, prompt: str, max_new_tokens: int, seed: int, require_cuda: bool,
            cuda_available: bool, runtime_factory: Callable[..., Any] | None = None,
            noise: Any = None) -> dict[str, dict[str, Any]]:
    """Run every device; ``noise`` (a contract dict) means the ideal-contract check applies to photonic-sim."""
    results: dict[str, dict[str, Any]] = {}
    for device in devices:
        if device == "cuda" and not cuda_available:
            status = "failed" if require_cuda else "skipped"
            results[device] = {"requested": device, "status": status,
                               "error": "torch.cuda.is_available() is False"}
            continue
        try:
            result = run_device(device, model, prompt, max_new_tokens, seed, runtime_factory, noise)
        except Exception as error:  # noqa: BLE001 - recorded, then reported as a failure
            results[device] = {"requested": device, "status": "failed", "error": f"{type(error).__name__}: {error}"}
            continue
        problems = check_result(result, prompt, max_new_tokens)
        result["status"] = "failed" if problems else "passed"
        result["error"] = "; ".join(problems) or None
        results[device] = result
    auto_problems = check_auto(results, cuda_available)
    if auto_problems:
        results["auto"]["status"] = "failed"
        results["auto"]["error"] = "; ".join(auto_problems)
    ideal_problems = check_ideal(results) if noise is not None else []
    if ideal_problems:
        results["photonic-sim"]["status"] = "failed"
        results["photonic-sim"]["error"] = "; ".join(ideal_problems)
    return results


def parse_devices(value: str) -> list[str]:
    devices = [d.strip() for d in value.split(",") if d.strip()]
    allowed = set(DEFAULT_DEVICES)
    unknown = [d for d in devices if d not in allowed]
    if unknown or not devices or len(set(devices)) != len(devices):
        raise argparse.ArgumentTypeError(f"devices must be distinct values from {', '.join(DEFAULT_DEVICES)}")
    # cpu first: auto is compared with it.
    return sorted(devices, key=DEFAULT_DEVICES.index)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--devices", type=parse_devices, default=list(DEFAULT_DEVICES),
                        help="comma-separated subset of cpu,auto,photonic-sim,cuda (default: all)")
    parser.add_argument("--require-cuda", action="store_true", help="fail instead of skipping cuda when it is absent")
    parser.add_argument("--model", default="awen-mini", help="bundled name or checkpoint directory")
    parser.add_argument("--prompt", default=DEFAULT_PROMPT)
    parser.add_argument("--max-new-tokens", type=int, default=32)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--ideal", action="store_true",
                        help="photonic-sim uses the noise-free contract and must match the cpu text (needs cpu)")
    parser.add_argument("--out", type=Path, help="write the JSON run record here")
    args = parser.parse_args(argv)
    if not args.prompt or args.max_new_tokens < 1:
        parser.error("--prompt must be non-empty and --max-new-tokens at least 1")
    if args.require_cuda and "cuda" not in args.devices:
        parser.error("--require-cuda needs cuda in --devices")
    if args.ideal and not {"cpu", "photonic-sim"} <= set(args.devices):
        parser.error("--ideal needs cpu and photonic-sim in --devices")
    noise = None
    if args.ideal:
        import awen_native

        noise = awen_native.ideal_contract()

    env = environment()
    results = run_all(args.devices, args.model, args.prompt, args.max_new_tokens, args.seed,
                      args.require_cuda, env["cuda_available"], noise=noise)
    command = shlex.join(["python", "scripts/phase1_devices.py", *(sys.argv[1:] if argv is None else argv)])
    record = {
        "schema": SCHEMA,
        "command": command,
        "commit": git_commit(),
        "environment": env,
        "source_label": SOURCE_LABEL,
        "model": args.model,
        "prompt": args.prompt,
        "max_new_tokens": args.max_new_tokens,
        "seed": args.seed,
        "photonic_sim_contract": "ideal (noise-free)" if args.ideal else "default awen_native contract",
        "devices": [results[d] for d in args.devices],
    }
    if args.out:
        args.out.parent.mkdir(parents=True, exist_ok=True)
        args.out.write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    for result in record["devices"]:
        where = f" -> {result['device']} ({result['fidelity']})" if "device" in result else ""
        error = f" ({result['error']})" if result.get("error") else ""
        print(f"{result['requested']}{where}: {result['status']}{error}")
    return 1 if any(r["status"] == "failed" for r in record["devices"]) else 0


if __name__ == "__main__":
    sys.exit(main())
