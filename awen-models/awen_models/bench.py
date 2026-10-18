"""AWEN benchmark harness with immutable evidence records (B1).

    python -m awen_models.bench --runs 5 --out-dir benchmarks/results

Backends compared on the same workloads:

* ``cpu``           PyTorch float32 on the host CPU (measured).
* ``cuda``          PyTorch float32 on a CUDA GPU; recorded as ``skipped`` with
                    the reason when no GPU is visible (measured otherwise).
* ``awen-reference`` the AWEN Rust GEMM executor with the ideal (noise-free)
                    contract: exact math through the AWEN software path.
* ``photonic-sim``  the AWEN Rust photonic GEMM model with the default noise
                    contract (compact-model fidelity, simulated).
* ``hybrid``        AWEN-Mini only: feed-forward GEMMs on ``photonic-sim``,
                    attention projections and everything else on the CPU.

Workloads: GEMM at several shapes and AWEN-Mini inference (time to first
token, decode tokens/s, perplexity on fixed validation windows).

Wall-clock numbers for ``awen-reference``, ``photonic-sim`` and ``hybrid`` are
the cost of *running the simulator on this host*. They are not projections of
photonic hardware speed and must never be quoted as such. Accuracy-type
numbers (perplexity, error) on those backends are simulated outcomes.

Statistics: each backend/workload is measured in ``runs`` independent runs of
``repeats`` timed calls after ``warmup`` untimed calls. Percentiles use all
timed samples; 95% confidence intervals are percentile bootstraps (fixed
seed) of the median latency over samples and of the mean over run medians.

The evidence record carries the git SHA (and whether the tree was dirty),
hardware, software environment, the exact settings and a SHA-256 of its own
canonical JSON. Files are write-once: the harness refuses to overwrite one.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import math
import os
import platform
import random
import resource
import subprocess
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Callable, Optional, Sequence

import numpy as np
import torch

EVIDENCE_VERSION = "awen.benchmark-evidence.v1"
BACKENDS = ("cpu", "cuda", "awen-reference", "photonic-sim", "hybrid")
GEMM_SHAPES = ((64, 128, 128), (128, 256, 256), (256, 512, 512))
SIMULATED = {"awen-reference": "software reference (exact math)",
             "photonic-sim": "simulated (compact model)",
             "hybrid": "simulated (compact model) + measured host"}


# --------------------------------------------------------------------------- statistics

def percentile(samples: Sequence[float], q: float) -> float:
    """Linear-interpolated percentile (numpy default), q in [0, 100]."""
    if not samples:
        raise ValueError("no samples")
    return float(np.percentile(np.asarray(samples, dtype=float), q))


def bootstrap_ci(samples: Sequence[float], stat: Callable[[np.ndarray], float] = np.median,
                 resamples: int = 2000, level: float = 0.95, seed: int = 0) -> tuple[float, float]:
    """Percentile bootstrap confidence interval of ``stat``."""
    data = np.asarray(samples, dtype=float)
    if data.size == 0:
        raise ValueError("no samples")
    if data.size == 1:
        return float(data[0]), float(data[0])
    rng = np.random.default_rng(seed)
    idx = rng.integers(0, data.size, size=(resamples, data.size))
    values = np.apply_along_axis(stat, 1, data[idx])
    alpha = (1.0 - level) / 2.0
    return float(np.quantile(values, alpha)), float(np.quantile(values, 1.0 - alpha))


def latency_summary(runs: Sequence[Sequence[float]], seed: int = 0) -> dict[str, Any]:
    """Summarize per-run latency samples (seconds) into ms statistics."""
    flat = [s for run in runs for s in run]
    run_medians = [float(np.median(run)) for run in runs]
    lo, hi = bootstrap_ci(flat, np.median, seed=seed)
    rlo, rhi = bootstrap_ci(run_medians, np.mean, seed=seed + 1)
    ms = 1e3
    return {
        "samples": len(flat),
        "runs": len(runs),
        "p50_ms": percentile(flat, 50) * ms,
        "p95_ms": percentile(flat, 95) * ms,
        "p99_ms": percentile(flat, 99) * ms,
        "mean_ms": float(np.mean(flat)) * ms,
        "min_ms": float(np.min(flat)) * ms,
        "median_ci95_ms": [lo * ms, hi * ms],
        "run_median_mean_ms": float(np.mean(run_medians)) * ms,
        "run_median_ci95_ms": [rlo * ms, rhi * ms],
    }


# --------------------------------------------------------------------------- environment

def git_state(repo: Optional[Path] = None) -> dict[str, Any]:
    cwd = str(repo or Path(__file__).resolve().parents[2])
    try:
        sha = subprocess.run(["git", "rev-parse", "HEAD"], cwd=cwd, capture_output=True, text=True,
                             check=True).stdout.strip()
        dirty = bool(subprocess.run(["git", "status", "--porcelain", "--untracked-files=no"], cwd=cwd,
                                    capture_output=True, text=True, check=True).stdout.strip())
    except (OSError, subprocess.CalledProcessError):
        return {"sha": None, "dirty": None, "note": "not a git checkout"}
    return {"sha": sha, "dirty": dirty}


def hardware() -> dict[str, Any]:
    record: dict[str, Any] = {
        "platform": platform.platform(),
        "machine": platform.machine(),
        "cpu_count": os.cpu_count(),
        "torch_threads": torch.get_num_threads(),
    }
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                record["cpu_model"] = line.split(":", 1)[1].strip()
                break
        for line in Path("/proc/meminfo").read_text().splitlines():
            if line.startswith("MemTotal"):
                record["mem_total_kib"] = int(line.split()[1])
                break
    except OSError:
        pass
    try:
        record["load_average_1_5_15"] = [round(v, 2) for v in os.getloadavg()]
    except OSError:  # pragma: no cover - platform without load average
        pass
    record["cuda_available"] = torch.cuda.is_available()
    if torch.cuda.is_available():
        record["gpu"] = torch.cuda.get_device_name(0)
    return record


def environment() -> dict[str, Any]:
    env: dict[str, Any] = {"python": platform.python_version(), "torch": torch.__version__,
                           "numpy": np.__version__}
    try:
        from importlib.metadata import version

        env["awen_native"] = version("awen-native")
    except Exception:  # pragma: no cover - optional
        env["awen_native"] = None
    for key in ("OMP_NUM_THREADS", "MKL_NUM_THREADS"):
        if key in os.environ:
            env[key] = os.environ[key]
    return env


def max_rss_mib() -> float:
    """Process peak resident set size so far (Linux reports KiB)."""
    return resource.getrusage(resource.RUSAGE_SELF).ru_maxrss / 1024.0


# --------------------------------------------------------------------------- executors

def _contract(kind: str) -> dict[str, Any]:
    import awen_native

    return awen_native.ideal_contract() if kind == "awen-reference" else awen_native.default_contract()


def _contract_or_none(kind: str) -> Optional[dict[str, Any]]:
    try:
        return _contract(kind)
    except ImportError:
        return None


def gemm_callable(backend: str, x: torch.Tensor, w: torch.Tensor, seed: int) -> Callable[[], Any]:
    if backend == "cpu":
        return lambda: x @ w.T
    if backend == "cuda":
        xc, wc = x.cuda(), w.cuda()

        def run() -> Any:
            y = xc @ wc.T
            torch.cuda.synchronize()
            return y
        return run
    from awen.photonic import PhotonicGemmExecutor

    executor = PhotonicGemmExecutor(noise=_contract(backend), seed=seed)
    return lambda: executor(x, w)


def gemm_program(m: int, k: int, n: int) -> dict[str, Any]:
    tensors = [{"id": "lhs", "shape": [m, k], "dtype": "f16", "layout": "row_major"},
               {"id": "rhs", "shape": [k, n], "dtype": "f16", "layout": "row_major"},
               {"id": "output", "shape": [m, n], "dtype": "f16", "layout": "row_major"}]
    ops = [{"op": "gemm", "id": "bench_gemm", "lhs": "lhs", "rhs": "rhs", "output": "output",
            "accuracy": {"max_abs_error": 0.5, "max_rel_error": 0.05, "minimum_effective_bits": 4}}]
    return {"ir_version": "awen.tensor.v1", "tensors": tensors, "ops": ops}


def capabilities() -> dict[str, Any]:
    path = Path(__file__).resolve().parents[2] / "awen-compiler/capabilities/pace_like_128.json"
    return json.loads(path.read_text())


def compile_seconds(m: int, k: int, n: int, repeats: int) -> list[float]:
    import awen_native

    caps, program = capabilities(), gemm_program(m, k, n)
    out = []
    for _ in range(repeats):
        start = time.perf_counter()
        awen_native.compile_program(program, caps)
        out.append(time.perf_counter() - start)
    return out


def time_calls(fn: Callable[[], Any], warmup: int, repeats: int) -> list[float]:
    for _ in range(warmup):
        fn()
    samples = []
    for _ in range(repeats):
        start = time.perf_counter()
        fn()
        samples.append(time.perf_counter() - start)
    return samples


def skipped(reason: str) -> dict[str, Any]:
    return {"status": "skipped", "reason": reason}


def backend_available(backend: str) -> Optional[str]:
    """Return a skip reason, or None when the backend can run here."""
    if backend == "cuda" and not torch.cuda.is_available():
        return "no CUDA device visible to PyTorch on this host"
    if backend in ("awen-reference", "photonic-sim", "hybrid"):
        try:
            import awen_native  # noqa: F401
        except ImportError:
            return "awen_native extension not installed"
    return None


# --------------------------------------------------------------------------- workloads

def bench_gemm(backends: Sequence[str], shapes: Sequence[tuple[int, int, int]], runs: int, repeats: int,
               warmup: int, seed: int) -> list[dict[str, Any]]:
    rows = []
    for m, k, n in shapes:
        gen = torch.Generator().manual_seed(seed)
        x = torch.randn(m, k, generator=gen)
        w = torch.randn(n, k, generator=gen)
        oracle = (x.double() @ w.double().T)
        scale = oracle.abs().max().item()
        flops = 2.0 * m * k * n
        for backend in backends:
            if backend == "hybrid":
                continue
            row: dict[str, Any] = {"workload": "gemm", "shape_mkn": [m, k, n], "backend": backend}
            reason = backend_available(backend)
            if reason:
                rows.append({**row, **skipped(reason)})
                continue
            per_run, errors = [], []
            for r in range(runs):
                fn = gemm_callable(backend, x, w, seed + r)
                per_run.append(time_calls(fn, warmup, repeats))
                y = fn()
                y = y.cpu() if hasattr(y, "cpu") else y
                errors.append(((y.double() - oracle).pow(2).mean().sqrt().item()) / scale)
            lat = latency_summary(per_run, seed)
            row.update({
                "status": "ok",
                "fidelity": SIMULATED.get(backend, "measured"),
                "latency": lat,
                "throughput_gflops": flops / (lat["p50_ms"] / 1e3) / 1e9,
                "throughput_note": "host wall-clock; for simulated backends this is simulator speed, not device speed",
                "relative_rms_error_vs_fp64": {"mean": float(np.mean(errors)), "max": float(np.max(errors))},
                "working_set_bytes": 4 * (m * k + n * k + m * n),
                "process_max_rss_mib": max_rss_mib(),
            })
            if backend in ("awen-reference", "photonic-sim"):
                row["compile"] = latency_summary([compile_seconds(m, k, n, repeats)], seed)
            rows.append(row)
    return rows


def _attach(model, backend: str, seed: int) -> None:
    from awen.photonic import PhotonicGemmExecutor

    executor = None if backend in ("cpu", "cuda") else PhotonicGemmExecutor(noise=_contract(
        "awen-reference" if backend == "awen-reference" else "photonic-sim"), seed=seed)
    for name, layer in model.linear_layers():
        if backend == "hybrid" and ".ffn." not in name:
            layer.matmul_executor = None
        else:
            layer.matmul_executor = executor


@torch.no_grad()
def window_losses(model, val: torch.Tensor, windows: int, block: int, device: torch.device) -> list[float]:
    """Cross-entropy (nats) on ``windows`` evenly spaced, non-overlapping validation windows."""
    stride = max(block + 1, (len(val) - block - 1) // windows)
    out = []
    for i in range(windows):
        start = i * stride
        chunk = val[start:start + block + 1]
        x, y = chunk[:-1].unsqueeze(0).to(device), chunk[1:].unsqueeze(0).to(device)
        _, loss = model(x, y)
        out.append(float(loss.item()))
    return out


@torch.no_grad()
def bench_model(backends: Sequence[str], runs: int, new_tokens: int, windows: int, seed: int,
                corpus: Optional[str] = None, checkpoint: Optional[str] = None) -> list[dict[str, Any]]:
    from .checkpoint import bundled_checkpoint_dir, load_checkpoint
    from .data import download_tiny_shakespeare, split_tokens

    directory = Path(checkpoint) if checkpoint else bundled_checkpoint_dir("awen-mini")
    rows = []
    text = corpus if corpus is not None else download_tiny_shakespeare()
    for backend in backends:
        row: dict[str, Any] = {"workload": "awen-mini-inference", "backend": backend}
        reason = backend_available(backend)
        if reason:
            rows.append({**row, **skipped(reason)})
            continue
        device = torch.device("cuda" if backend == "cuda" else "cpu")
        model, tokenizer, metadata = load_checkpoint(directory, str(device))
        model.eval()
        val = split_tokens(tokenizer.encode(text), 0.1).val
        prompt = torch.tensor([tokenizer.encode("ROMEO:")], dtype=torch.long, device=device)
        ttft_runs, decode_runs, losses = [], [], []
        for r in range(runs):
            _attach(model, backend, seed + r)
            gen = torch.Generator(device=device).manual_seed(seed + r)
            start = time.perf_counter()
            first = model.generate(prompt, 1, temperature=0.8, top_k=40, generator=gen)
            if device.type == "cuda":
                torch.cuda.synchronize()
            ttft_runs.append([time.perf_counter() - start])
            start = time.perf_counter()
            model.generate(first, new_tokens, temperature=0.8, top_k=40, generator=gen)
            if device.type == "cuda":
                torch.cuda.synchronize()
            decode_runs.append([(time.perf_counter() - start) / new_tokens])
            losses.append(window_losses(model, val, windows, model.config.block_size, device))
        _attach(model, "cpu", seed)
        per_window = np.mean(np.asarray(losses), axis=0)
        mean_loss = float(per_window.mean())
        lo, hi = bootstrap_ci(per_window, np.mean, seed=seed)
        tok_s = [1.0 / run[0] for run in decode_runs]
        tlo, thi = bootstrap_ci(tok_s, np.mean, seed=seed)
        row.update({
            "status": "ok",
            "fidelity": SIMULATED.get(backend, "measured"),
            "model": metadata.get("model", directory.name),
            "parameters": sum(p.numel() for p in model.parameters()),
            "time_to_first_token": latency_summary(ttft_runs, seed),
            "decode_latency_per_token": latency_summary(decode_runs, seed),
            "decode_tokens_per_s": {"mean": float(np.mean(tok_s)), "ci95": [tlo, thi]},
            "new_tokens_per_run": new_tokens,
            "eval_windows": windows,
            "val_loss_nats": {"mean": mean_loss, "ci95": [lo, hi]},
            "perplexity": {"mean": math.exp(mean_loss), "ci95": [math.exp(lo), math.exp(hi)]},
            "run_loss_spread_nats": float(np.ptp([np.mean(x) for x in losses])),
            "process_max_rss_mib": max_rss_mib(),
        })
        if backend == "hybrid":
            row["placement"] = "ffn.fc/ffn.proj GEMMs on photonic-sim; attention GEMMs, norms, softmax, head on CPU"
        rows.append(row)
    return rows


# --------------------------------------------------------------------------- evidence

def canonical(obj: Any) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def seal(record: dict[str, Any]) -> dict[str, Any]:
    body = {k: v for k, v in record.items() if k != "evidence_sha256"}
    return {**body, "evidence_sha256": "sha256:" + hashlib.sha256(canonical(body)).hexdigest()}


def verify(record: dict[str, Any]) -> bool:
    return seal(record)["evidence_sha256"] == record.get("evidence_sha256")


def write_once(record: dict[str, Any], out_dir: Path) -> Path:
    digest = record["evidence_sha256"].split(":", 1)[1]
    out_dir.mkdir(parents=True, exist_ok=True)
    path = out_dir / f"evidence-{record['created_at'][:10]}-{digest[:12]}.json"
    with open(path, "x", encoding="utf-8") as handle:  # "x" refuses to overwrite
        handle.write(json.dumps(record, indent=2, sort_keys=True) + "\n")
    return path


def run(backends: Sequence[str] = BACKENDS, runs: int = 5, repeats: int = 20, warmup: int = 3,
        new_tokens: int = 64, windows: int = 32, seed: int = 0,
        shapes: Sequence[tuple[int, int, int]] = GEMM_SHAPES, corpus: Optional[str] = None,
        checkpoint: Optional[str] = None, label: Optional[str] = None) -> dict[str, Any]:
    if runs < 2:
        raise ValueError("runs must be >= 2 for run-level confidence intervals")
    random.seed(seed)
    torch.manual_seed(seed)
    started = time.perf_counter()
    gemm = bench_gemm(backends, shapes, runs, repeats, warmup, seed)
    mini = bench_model(backends, runs, new_tokens, windows, seed, corpus, checkpoint)
    record = {
        "version": EVIDENCE_VERSION,
        "created_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
        "label": label or f"{hardware().get('cpu_model', platform.machine())}",
        "git": git_state(),
        "hardware": hardware(),
        "environment": environment(),
        "settings": {"backends": list(backends), "runs": runs, "repeats": repeats, "warmup": warmup,
                     "new_tokens": new_tokens, "windows": windows, "seed": seed,
                     "gemm_shapes_mkn": [list(s) for s in shapes], "bootstrap_resamples": 2000,
                     "ci_level": 0.95, "photonic_sim_contract": _contract_or_none("photonic-sim"),
                     "awen_reference_contract": _contract_or_none("awen-reference")},
        "evidence_policy": ("cpu/cuda rows are measured on the recorded hardware. awen-reference, photonic-sim "
                            "and hybrid rows time the simulator on this host; their accuracy is simulated. "
                            "No row is a photonic hardware measurement."),
        "results": gemm + mini,
        "wall_seconds": round(time.perf_counter() - started, 3),
    }
    return seal(record)


def main(argv: Optional[Sequence[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--backends", default=",".join(BACKENDS))
    parser.add_argument("--runs", type=int, default=5)
    parser.add_argument("--repeats", type=int, default=20)
    parser.add_argument("--warmup", type=int, default=3)
    parser.add_argument("--new-tokens", type=int, default=64)
    parser.add_argument("--windows", type=int, default=32)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--threads", type=int, default=None)
    parser.add_argument("--label", default=None)
    parser.add_argument("--out-dir", default=None)
    parser.add_argument("--verify", default=None, help="verify an existing evidence file and exit")
    args = parser.parse_args(argv)
    if args.verify:
        ok = verify(json.loads(Path(args.verify).read_text()))
        print("evidence hash OK" if ok else "evidence hash MISMATCH")
        raise SystemExit(0 if ok else 1)
    if args.threads:
        torch.set_num_threads(args.threads)
    record = run([b.strip() for b in args.backends.split(",") if b.strip()], args.runs, args.repeats,
                 args.warmup, args.new_tokens, args.windows, args.seed, label=args.label)
    if args.out_dir:
        print(write_once(record, Path(args.out_dir)))
    else:
        print(json.dumps(record, indent=2, sort_keys=True))


if __name__ == "__main__":
    main()
