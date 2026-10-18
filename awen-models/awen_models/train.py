"""Train AWEN-Mini on a character corpus and record every measured result.

Example::

    python -m awen_models.train --steps 3000 --out runs/awen-mini

All metrics written by this module are measured on the machine that ran it;
``metadata.json`` records that machine, the library versions and the exact
settings.
"""

from __future__ import annotations

import argparse
import json
import math
import os
import platform
import time
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Optional

import torch

from .checkpoint import save_checkpoint
from .config import MiniConfig
from .data import TINY_SHAKESPEARE, download_tiny_shakespeare, sample_batch, split_tokens
from .device import resolve_device
from .model import AwenMini
from .tokenizer import CharTokenizer


@dataclass
class TrainSettings:
    steps: int = 3000
    batch_size: int = 32
    learning_rate: float = 1e-3
    min_learning_rate: float = 1e-4
    warmup_steps: int = 100
    weight_decay: float = 0.1
    grad_clip: float = 1.0
    eval_interval: int = 250
    eval_batches: int = 20
    seed: int = 1337
    val_fraction: float = 0.1
    device: str = "auto"
    threads: Optional[int] = None
    keep_best: bool = True


@dataclass
class TrainResult:
    model: AwenMini
    tokenizer: CharTokenizer
    history: list[dict[str, float]] = field(default_factory=list)
    metadata: dict[str, Any] = field(default_factory=dict)


def learning_rate(step: int, settings: TrainSettings) -> float:
    if step < settings.warmup_steps:
        return settings.learning_rate * (step + 1) / settings.warmup_steps
    progress = (step - settings.warmup_steps) / max(1, settings.steps - settings.warmup_steps)
    cosine = 0.5 * (1.0 + math.cos(math.pi * min(1.0, progress)))
    return settings.min_learning_rate + cosine * (settings.learning_rate - settings.min_learning_rate)


@torch.no_grad()
def estimate_loss(
    model: AwenMini,
    data: torch.Tensor,
    batches: int,
    batch_size: int,
    seed: int,
    device: torch.device,
) -> float:
    """Mean cross-entropy over a fixed, seeded set of windows (identical across runs)."""
    model.eval()
    generator = torch.Generator().manual_seed(seed)
    losses = []
    for _ in range(batches):
        x, y = sample_batch(data, batch_size, model.config.block_size, generator, device)
        _, loss = model(x, y)
        losses.append(loss.item())
    model.train()
    return sum(losses) / len(losses)


def hardware_record(device: torch.device) -> dict[str, Any]:
    record: dict[str, Any] = {
        "device": str(device),
        "platform": platform.platform(),
        "processor": platform.processor() or platform.machine(),
        "cpu_count": os.cpu_count(),
        "torch_threads": torch.get_num_threads(),
        "torch": torch.__version__,
        "python": platform.python_version(),
    }
    try:
        for line in Path("/proc/cpuinfo").read_text().splitlines():
            if line.startswith("model name"):
                record["cpu_model"] = line.split(":", 1)[1].strip()
                break
    except OSError:
        pass
    if device.type == "cuda":
        record["gpu"] = torch.cuda.get_device_name(device)
    return record


def train(
    text: str,
    config: MiniConfig,
    settings: TrainSettings,
    dataset: Optional[dict[str, Any]] = None,
    log: bool = True,
) -> TrainResult:
    if settings.threads:
        torch.set_num_threads(settings.threads)
    device = resolve_device(settings.device)
    torch.manual_seed(settings.seed)
    tokenizer = CharTokenizer.from_text(text)
    if tokenizer.vocab_size != config.vocab_size:
        config = MiniConfig.from_dict({**config.to_dict(), "vocab_size": tokenizer.vocab_size})
    splits = split_tokens(tokenizer.encode(text), settings.val_fraction)
    model = AwenMini(config).to(device)
    decay = [p for n, p in model.named_parameters() if p.dim() >= 2]
    no_decay = [p for n, p in model.named_parameters() if p.dim() < 2]
    optimizer = torch.optim.AdamW(
        [
            {"params": decay, "weight_decay": settings.weight_decay},
            {"params": no_decay, "weight_decay": 0.0},
        ],
        lr=settings.learning_rate,
        betas=(0.9, 0.99),
    )
    generator = torch.Generator().manual_seed(settings.seed)
    history: list[dict[str, float]] = []
    started = time.perf_counter()
    best_val = math.inf
    best_state: Optional[dict[str, torch.Tensor]] = None
    best_step = 0
    for step in range(settings.steps + 1):
        if step % settings.eval_interval == 0 or step == settings.steps:
            val = estimate_loss(model, splits.val, settings.eval_batches, settings.batch_size, 0, device)
            train_eval = estimate_loss(
                model, splits.train, settings.eval_batches, settings.batch_size, 1, device
            )
            entry = {
                "step": step,
                "train_loss": round(train_eval, 5),
                "val_loss": round(val, 5),
                "elapsed_s": round(time.perf_counter() - started, 2),
            }
            history.append(entry)
            if settings.keep_best and val < best_val:
                best_val, best_step = val, step
                best_state = {k: v.detach().clone() for k, v in model.state_dict().items()}
            if log:
                print(json.dumps(entry), flush=True)
        if step == settings.steps:
            break
        for group in optimizer.param_groups:
            group["lr"] = learning_rate(step, settings)
        x, y = sample_batch(splits.train, settings.batch_size, config.block_size, generator, device)
        _, loss = model(x, y)
        optimizer.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(model.parameters(), settings.grad_clip)
        optimizer.step()
    elapsed = time.perf_counter() - started
    selected = history[-1]
    if settings.keep_best and best_state is not None:
        model.load_state_dict(best_state)
        selected = next(e for e in history if e["step"] == best_step)
    tokens = settings.steps * settings.batch_size * config.block_size
    metadata = {
        "model": "awen-mini",
        "parameters": model.num_parameters(),
        "config": config.to_dict(),
        "settings": asdict(settings),
        "dataset": dataset or {"name": "custom", "characters": len(text)},
        "split": {
            "train_tokens": int(len(splits.train)),
            "val_tokens": int(len(splits.val)),
            "method": f"contiguous; last {settings.val_fraction:.0%} is validation",
        },
        "final": history[-1],
        "selected_checkpoint": {
            **selected,
            "rule": "lowest validation loss at an evaluation step" if settings.keep_best else "last step",
        },
        "train_wall_time_s": round(elapsed, 1),
        "train_tokens_processed": tokens,
        "train_tokens_per_s": round(tokens / elapsed, 1),
        "hardware": hardware_record(device),
        "evidence": "measured on the hardware recorded above",
    }
    model.eval()
    return TrainResult(model=model, tokenizer=tokenizer, history=history, metadata=metadata)


def main(argv: Optional[list[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--out", required=True, help="checkpoint output directory")
    parser.add_argument("--steps", type=int, default=TrainSettings.steps)
    parser.add_argument("--batch-size", type=int, default=TrainSettings.batch_size)
    parser.add_argument("--eval-interval", type=int, default=TrainSettings.eval_interval)
    parser.add_argument("--seed", type=int, default=TrainSettings.seed)
    parser.add_argument("--device", default="auto")
    parser.add_argument("--threads", type=int, default=None)
    parser.add_argument("--ffn", default="gelu")
    parser.add_argument("--n-layer", type=int, default=5)
    parser.add_argument("--n-embd", type=int, default=128)
    parser.add_argument("--n-head", type=int, default=4)
    parser.add_argument("--ffn-hidden", type=int, default=512)
    parser.add_argument("--block-size", type=int, default=128)
    parser.add_argument("--dropout", type=float, default=0.0)
    parser.add_argument("--text", help="train on this UTF-8 file instead of Tiny Shakespeare")
    args = parser.parse_args(argv)
    if args.text:
        text = Path(args.text).read_text(encoding="utf-8")
        dataset = {"name": Path(args.text).name, "characters": len(text)}
    else:
        text = download_tiny_shakespeare()
        dataset = TINY_SHAKESPEARE
    config = MiniConfig(
        n_layer=args.n_layer,
        n_embd=args.n_embd,
        n_head=args.n_head,
        ffn=args.ffn,
        ffn_hidden=args.ffn_hidden,
        block_size=args.block_size,
        dropout=args.dropout,
    )
    settings = TrainSettings(
        steps=args.steps,
        batch_size=args.batch_size,
        eval_interval=args.eval_interval,
        seed=args.seed,
        device=args.device,
        threads=args.threads,
    )
    result = train(text, config, settings, dataset=dataset)
    out = save_checkpoint(args.out, result.model, result.tokenizer, result.metadata)
    (out / "loss_history.json").write_text(json.dumps(result.history, indent=1) + "\n")
    print(f"saved {result.metadata['parameters']} parameters to {out}")


if __name__ == "__main__":
    main()
