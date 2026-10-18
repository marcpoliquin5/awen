"""AWEN-Mini on the simulated photonic GEMM vs the CPU oracle.

    python -m awen_models.photonic_demo --out results/photonic-sim-demo.json

1. Layer study: one AWEN-Mini linear layer (``blocks.0.ffn.fc``) on a real
   validation activation, computed on the CPU (float64 oracle) and on the AWEN
   Rust photonic model at several optical powers. Reports relative RMS error and
   the effective bits it implies.
2. Model study: validation loss of the full model when all 20 transformer
   linear layers run on the photonic model, against the same windows on the CPU.
   The tied output head (an embedding-weight product) stays on the host.

All photonic numbers are simulated (compact-model fidelity) with the stated
noise contract. They are not hardware measurements.
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path
from typing import Any, Optional

import torch

from awen.photonic import PhotonicGemmExecutor

from .checkpoint import bundled_checkpoint_dir, load_checkpoint
from .data import download_tiny_shakespeare, sample_batch, split_tokens

PHOTON_SWEEP = (10.0, 100.0, 1000.0, 10000.0, 100000.0)


def _contract(photons: Optional[float]) -> dict:
    import awen_native

    if photons is None:
        return awen_native.ideal_contract()
    contract = awen_native.default_contract()
    contract["photons_per_mac"] = photons
    return contract


def _effective_bits(relative_rms: float) -> Optional[float]:
    import awen_native

    return None if relative_rms <= 0 else round(awen_native.effective_bits_from_sigma(relative_rms), 3)


@torch.no_grad()
def layer_study(model, val: torch.Tensor, seed: int) -> list[dict[str, Any]]:
    generator = torch.Generator().manual_seed(seed)
    x, _ = sample_batch(val, 1, model.config.block_size, generator, torch.device("cpu"))
    captured: dict[str, torch.Tensor] = {}
    layer = dict(model.linear_layers())["blocks.0.ffn.fc"]
    def capture(module, inputs, output) -> None:
        captured.setdefault("x", inputs[0].detach())

    handle = layer.register_forward_hook(capture)
    model(x)
    handle.remove()
    activation = captured["x"].reshape(-1, captured["x"].shape[-1]).double()
    weight = layer.weight.detach().double()
    oracle = activation @ weight.T
    scale = oracle.abs().max().item()
    rows = []
    for photons in (None,) + PHOTON_SWEEP:
        executor = PhotonicGemmExecutor(noise=_contract(photons), seed=seed)
        y = executor(activation, weight)
        rms = (y - oracle).pow(2).mean().sqrt().item() / scale
        rows.append({
            "photons_per_mac": photons,
            "contract": "ideal" if photons is None else "default with photons_per_mac overridden",
            "relative_rms_error": rms,
            "effective_bits": _effective_bits(rms),
        })
    return rows


@torch.no_grad()
def model_study(model, val: torch.Tensor, batches: int, batch_size: int, seed: int) -> list[dict[str, Any]]:
    def loss_with(executor: Optional[PhotonicGemmExecutor]) -> float:
        for _, layer in model.linear_layers():
            layer.matmul_executor = executor
        generator = torch.Generator().manual_seed(seed)
        losses = []
        for _ in range(batches):
            x, y = sample_batch(val, batch_size, model.config.block_size, generator, torch.device("cpu"))
            _, loss = model(x, y)
            losses.append(loss.item())
        for _, layer in model.linear_layers():
            layer.matmul_executor = None
        return sum(losses) / len(losses)

    cpu = loss_with(None)
    rows = [{"target": "cpu (oracle)", "fidelity": "measured", "val_loss_nats": round(cpu, 5)}]
    for photons in (None,) + PHOTON_SWEEP:
        loss = loss_with(PhotonicGemmExecutor(noise=_contract(photons), seed=seed))
        rows.append({
            "target": "photonic-sim",
            "fidelity": "simulated (compact model)",
            "photons_per_mac": photons,
            "val_loss_nats": round(loss, 5),
            "delta_vs_cpu_nats": round(loss - cpu, 5),
        })
    return rows


def run(
    checkpoint: Optional[str] = None,
    batches: int = 4,
    batch_size: int = 8,
    seed: int = 7,
    corpus: Optional[str] = None,
) -> dict[str, Any]:
    import awen_native

    directory = Path(checkpoint) if checkpoint else bundled_checkpoint_dir("awen-mini")
    model, tokenizer, metadata = load_checkpoint(directory, "cpu")
    model.eval()
    ids = tokenizer.encode(corpus if corpus is not None else download_tiny_shakespeare())
    val = split_tokens(ids, 0.1).val
    return {
        "version": "awen.photonic-demo.v1",
        "model": metadata.get("model", directory.name),
        "fidelity": "simulated (compact model); CPU rows measured",
        "evidence": "simulated with the AWEN photonic noise model; not a hardware measurement",
        "base_contract": awen_native.default_contract(),
        "tile_k": 64,
        "seed": seed,
        "layer_study": layer_study(model, val, seed),
        "model_study": {
            "batches": batches,
            "batch_size": batch_size,
            "rows": model_study(model, val, batches, batch_size, seed),
        },
    }


def main(argv: Optional[list[str]] = None) -> None:
    parser = argparse.ArgumentParser(description=__doc__, formatter_class=argparse.RawDescriptionHelpFormatter)
    parser.add_argument("--checkpoint", default=None)
    parser.add_argument("--batches", type=int, default=4)
    parser.add_argument("--batch-size", type=int, default=8)
    parser.add_argument("--seed", type=int, default=7)
    parser.add_argument("--out", default=None)
    args = parser.parse_args(argv)
    result = run(args.checkpoint, args.batches, args.batch_size, args.seed)
    text = json.dumps(result, indent=2)
    print(text)
    if args.out:
        Path(args.out).write_text(text + "\n")


if __name__ == "__main__":
    main()
