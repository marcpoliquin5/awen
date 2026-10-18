"""AWEN-Nonlinear ablation harness.

Compares feed-forward and weight-sharing variants of AWEN-Mini at an
approximately equal parameter budget (within 3%), trained on identical data,
with identical optimizer settings, steps and seeds. For every run it measures
validation loss, parameter count, single-sequence forward latency and peak
training-step memory, then writes ``results.json`` and a Pareto plot.

Example::

    python -m awen_models.nonlinear --steps 1500 --seeds 1 2 --out results/nonlinear

All numbers are measured on the machine that runs the harness.
"""

from __future__ import annotations

import argparse
import json
import statistics
import time
from pathlib import Path
from typing import Any, Optional

import torch

from .config import MiniConfig
from .data import TINY_SHAKESPEARE, download_tiny_shakespeare
from .model import AwenMini
from .train import TrainSettings, hardware_record, train

VARIANTS: dict[str, dict[str, Any]] = {
    "baseline-gelu-mlp": {"n_layer": 4, "ffn": "gelu", "ffn_hidden": 512},
    "gated-multiplicative": {"n_layer": 4, "ffn": "gated", "ffn_hidden": 341},
    "low-rank-wide-mlp": {"n_layer": 4, "ffn": "lowrank", "ffn_hidden": 1024, "ffn_rank": 56},
    "recursive-shared-block": {
        "n_layer": 4,
        "shared_layers": True,
        "n_embd": 256,
        "n_head": 8,
        "ffn_hidden": 1024,
    },
}
PARAMETER_TOLERANCE = 0.03


def variant_config(name: str, vocab_size: int, block_size: int = 128) -> MiniConfig:
    return MiniConfig(vocab_size=vocab_size, block_size=block_size, **VARIANTS[name])


def check_parameter_budget(vocab_size: int) -> dict[str, int]:
    counts = {name: AwenMini(variant_config(name, vocab_size)).num_parameters() for name in VARIANTS}
    reference = counts["baseline-gelu-mlp"]
    for name, count in counts.items():
        if abs(count - reference) / reference > PARAMETER_TOLERANCE:
            raise ValueError(f"{name} has {count} parameters, outside ±3% of {reference}")
    return counts


@torch.no_grad()
def measure_latency_ms(model: AwenMini, repeats: int = 30) -> float:
    """Median wall time of one full-context forward pass for batch size 1."""
    model.eval()
    idx = torch.zeros(1, model.config.block_size, dtype=torch.long)
    for _ in range(5):
        model(idx)
    samples = []
    for _ in range(repeats):
        start = time.perf_counter()
        model(idx)
        samples.append((time.perf_counter() - start) * 1e3)
    return statistics.median(samples)


def measure_training_peak_mib(model: AwenMini, batch_size: int) -> float:
    """Estimated training-step memory in MiB.

    Sum of parameters, gradients and the two AdamW moment buffers (4x the
    parameter bytes) plus the exact bytes of every tensor autograd saves for
    the backward pass, counted with ``saved_tensors_hooks`` during one forward
    pass. Allocator overhead and transient buffers are not included, so this
    is a lower-bound estimate, not an RSS measurement.
    """
    model.train()
    idx = torch.zeros(batch_size, model.config.block_size, dtype=torch.long)
    saved_bytes = 0

    def pack(tensor: torch.Tensor) -> torch.Tensor:
        nonlocal saved_bytes
        saved_bytes += tensor.numel() * tensor.element_size()
        return tensor

    with torch.autograd.graph.saved_tensors_hooks(pack, lambda t: t):
        _, loss = model(idx, idx)
    loss.backward()
    model.zero_grad(set_to_none=True)
    parameter_bytes = sum(p.numel() * p.element_size() for p in model.parameters())
    return (4 * parameter_bytes + saved_bytes) / 2**20


def run(
    steps: int,
    seeds: list[int],
    out: Path,
    batch_size: int = 32,
    variants: Optional[list[str]] = None,
    text: Optional[str] = None,
) -> dict[str, Any]:
    corpus = text if text is not None else download_tiny_shakespeare()
    vocab = len(set(corpus))
    names = variants or list(VARIANTS)
    counts = check_parameter_budget(vocab)
    runs = []
    for name in names:
        for seed in seeds:
            settings = TrainSettings(
                steps=steps,
                batch_size=batch_size,
                eval_interval=max(1, steps // 4),
                eval_batches=20,
                seed=seed,
                device="cpu",
                warmup_steps=min(100, max(1, steps // 10)),
            )
            result = train(corpus, variant_config(name, vocab), settings, log=False)
            entry = {
                "variant": name,
                "seed": seed,
                "parameters": result.model.num_parameters(),
                "val_loss": result.history[-1]["val_loss"],
                "train_loss": result.history[-1]["train_loss"],
                "train_wall_time_s": result.metadata["train_wall_time_s"],
                "forward_latency_ms_b1": round(measure_latency_ms(result.model), 3),
                "train_step_memory_mib_estimate": round(
                    measure_training_peak_mib(result.model, batch_size), 2
                ),
            }
            runs.append(entry)
            print(json.dumps(entry), flush=True)
    summary = {}
    for name in names:
        rows = [r for r in runs if r["variant"] == name]
        losses = [r["val_loss"] for r in rows]
        summary[name] = {
            "parameters": counts[name],
            "val_loss_mean": round(statistics.mean(losses), 4),
            "val_loss_stdev": round(statistics.stdev(losses), 4) if len(losses) > 1 else None,
            "forward_latency_ms_b1_mean": round(
                statistics.mean(r["forward_latency_ms_b1"] for r in rows), 3
            ),
            "train_step_memory_mib_estimate": rows[0]["train_step_memory_mib_estimate"],
        }
    report = {
        "harness": "awen.nonlinear-ablation.v1",
        "dataset": TINY_SHAKESPEARE if text is None else {"name": "custom", "characters": len(corpus)},
        "steps": steps,
        "batch_size": batch_size,
        "seeds": seeds,
        "parameter_tolerance": PARAMETER_TOLERANCE,
        "variants": {name: VARIANTS[name] for name in names},
        "runs": runs,
        "summary": summary,
        "hardware": hardware_record(torch.device("cpu")),
        "evidence": "loss and latency measured on the hardware recorded above; memory is a lower-bound estimate (parameters, gradients, AdamW state, saved activations)",
    }
    out.mkdir(parents=True, exist_ok=True)
    (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    plot_pareto(report, out / "pareto.png")
    return report


def pareto_front(points: list[tuple[float, float, str]]) -> list[str]:
    """Names of points not dominated in (lower x, lower y)."""
    front = []
    for x, y, name in points:
        dominated = any(
            (ox <= x and oy <= y) and (ox < x or oy < y) for ox, oy, other in points if other != name
        )
        if not dominated:
            front.append(name)
    return front


def plot_pareto(report: dict[str, Any], path: Path) -> None:
    import matplotlib

    matplotlib.use("Agg")
    import matplotlib.pyplot as plt

    summary = report["summary"]
    points = [(s["forward_latency_ms_b1_mean"], s["val_loss_mean"], n) for n, s in summary.items()]
    front = set(pareto_front(points))
    fig, ax = plt.subplots(figsize=(6.4, 4.4), dpi=120)
    for run_entry in report["runs"]:
        ax.scatter(run_entry["forward_latency_ms_b1"], run_entry["val_loss"], s=12, alpha=0.35, color="#6b7280")
    for x, y, name in points:
        ax.scatter(x, y, s=60, color="#2563eb" if name in front else "#9ca3af", zorder=3)
        ax.annotate(f"{name}\n{summary[name]['parameters']:,} params", (x, y), fontsize=7,
                    xytext=(5, 5), textcoords="offset points")
    ax.set_xlabel("forward latency, batch 1, full context (ms, measured)")
    ax.set_ylabel("validation loss (nats/char, mean of seeds)")
    ax.set_title(f"AWEN-Nonlinear ablation: {report['steps']} steps, seeds {report['seeds']}", fontsize=9)
    ax.grid(alpha=0.3)
    fig.tight_layout()
    fig.savefig(path)
    plt.close(fig)


def main(argv: Optional[list[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--steps", type=int, default=1500)
    parser.add_argument("--seeds", type=int, nargs="+", default=[1, 2])
    parser.add_argument("--batch-size", type=int, default=32)
    parser.add_argument("--out", required=True)
    args = parser.parse_args(argv)
    run(args.steps, args.seeds, Path(args.out), batch_size=args.batch_size)


if __name__ == "__main__":
    main()
