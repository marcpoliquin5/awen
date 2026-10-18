"""Evaluate an AWEN-Mini checkpoint: validation loss (float32 and int8) and generation speed.

    python -m awen_models.evaluate --out awen-mini-eval.json

Validation windows are drawn with a fixed seed from the held-out last 10% of
Tiny Shakespeare, so every variant sees identical text. Numbers are measured on
the machine that runs the command and are recorded with its hardware details.
"""

from __future__ import annotations

import argparse
import json
import math
import time
from pathlib import Path
from typing import Any, Optional

import torch

from .checkpoint import bundled_checkpoint_dir, load_checkpoint
from .data import download_tiny_shakespeare, split_tokens
from .quant import int8_weight_bytes, quantize_model_int8
from .train import estimate_loss, hardware_record


@torch.no_grad()
def tokens_per_second(model, tokenizer, prompt: str, tokens: int, seed: int) -> float:
    ids = torch.tensor([tokenizer.encode(prompt)], dtype=torch.long)
    generator = torch.Generator().manual_seed(seed)
    started = time.perf_counter()
    model.generate(ids, tokens, temperature=0.8, top_k=40, generator=generator)
    return tokens / (time.perf_counter() - started)


@torch.no_grad()
def evaluate(
    checkpoint: Optional[str] = None,
    batches: int = 40,
    batch_size: int = 32,
    seed: int = 2024,
    generate_tokens: int = 200,
    threads: Optional[int] = None,
    corpus: Optional[str] = None,
) -> dict[str, Any]:
    if threads:
        torch.set_num_threads(threads)
    directory = Path(checkpoint) if checkpoint else bundled_checkpoint_dir("awen-mini")
    model, tokenizer, metadata = load_checkpoint(directory, "cpu")
    model.eval()
    ids = tokenizer.encode(corpus if corpus is not None else download_tiny_shakespeare())
    val = split_tokens(ids, 0.1).val
    device = torch.device("cpu")

    variants: dict[str, Any] = {}
    fp32_loss = estimate_loss(model, val, batches, batch_size, seed, device)
    model.eval()
    variants["float32"] = {
        "val_loss_nats": round(fp32_loss, 5),
        "bits_per_char": round(fp32_loss / math.log(2), 5),
        "linear_weight_bytes": sum(
            layer.weight.numel() * layer.weight.element_size() for _, layer in model.linear_layers()
        ),
        "generate_tokens_per_s": round(tokens_per_second(model, tokenizer, "ROMEO:", generate_tokens, seed), 1),
    }
    quantized = quantize_model_int8(model)
    int8_loss = estimate_loss(quantized, val, batches, batch_size, seed, device)
    quantized.eval()
    variants["int8_weights"] = {
        "val_loss_nats": round(int8_loss, 5),
        "bits_per_char": round(int8_loss / math.log(2), 5),
        "linear_weight_bytes": int8_weight_bytes(quantized),
        "generate_tokens_per_s": round(
            tokens_per_second(quantized, tokenizer, "ROMEO:", generate_tokens, seed), 1
        ),
    }
    return {
        "version": "awen.model-evaluation.v1",
        "model": metadata.get("model", directory.name),
        "parameters": metadata.get("parameters"),
        "checkpoint_step": metadata.get("selected_checkpoint", {}).get("step"),
        "uniform_baseline_nats": round(math.log(tokenizer.vocab_size), 5),
        "evaluation": {"split": "validation", "batches": batches, "batch_size": batch_size, "seed": seed},
        "variants": variants,
        "hardware": hardware_record(device),
        "evidence": "measured on the hardware recorded above",
    }


def main(argv: Optional[list[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--checkpoint", default=None, help="checkpoint directory (default: bundled awen-mini)")
    parser.add_argument("--batches", type=int, default=40)
    parser.add_argument("--batch-size", type=int, default=32)
    parser.add_argument("--seed", type=int, default=2024)
    parser.add_argument("--threads", type=int, default=None)
    parser.add_argument("--out", default=None)
    args = parser.parse_args(argv)
    result = evaluate(args.checkpoint, args.batches, args.batch_size, args.seed, threads=args.threads)
    text = json.dumps(result, indent=2)
    print(text)
    if args.out:
        Path(args.out).write_text(text + "\n")


if __name__ == "__main__":
    main()
