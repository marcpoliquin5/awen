"""Memory and compute estimator for AWEN fine-tuning and distillation runs.

Every number this module produces is an **estimate** from closed-form
formulas, not a measurement. The exception is :func:`measure_matmul_tflops`,
which times dense matrix multiplies on the local device. The formulas and
their sources are listed in ``docs/COMPUTE_PLAN.md``.

* Parameter counts come exactly from the model's architecture config
  (Llama/Qwen3 decoder layout; tests check them against instantiated models).
* NF4 storage is 0.5 byte/parameter plus 0.127 bit/parameter of double-quantized
  block constants (Dettmers et al., 2023, QLoRA, Sec. 3). Embeddings and the output
  head stay in 16-bit, as in ``transformers``' default 4-bit loading.
* LoRA adapters train in fp32 with AdamW: 16 bytes/parameter (weight, grad, two
  moments).
* Activations with full gradient checkpointing keep one 16-bit hidden state per
  layer (``2*B*S*h*L`` bytes) plus one layer's recompute working set, estimated
  with Korthikanti et al. (2022) as ``B*S*h*34`` bytes (fused attention, so no
  ``S^2`` score term). The fp32 logits and their gradient add ``8*B*S*V`` bytes.
* Training compute is ``6*N*D`` FLOPs for a full fine-tune (Kaplan et al., 2020).
  For LoRA with checkpointing it is forward + recompute + input-gradient
  backward, also about ``6*N*D``. Wall time = FLOPs / (peak * utilization), with
  the peak measured or supplied and the utilization an explicit assumption.

Example::

    python -m awen_models.compute_plan --config qwen3-0.6b.json --budget-gb 12 --seq 1024
"""

from __future__ import annotations

import argparse
import json
import time
from dataclasses import asdict, dataclass
from pathlib import Path
from typing import Any, Optional, Sequence

GIB = 1024**3
ALL_PROJECTIONS = ("q_proj", "k_proj", "v_proj", "o_proj", "gate_proj", "up_proj", "down_proj")


@dataclass(frozen=True)
class DecoderShape:
    hidden: int
    intermediate: int
    layers: int
    heads: int
    kv_heads: int
    head_dim: int
    vocab: int
    tied: bool
    qk_norm: bool = False
    attention_bias: bool = False

    @classmethod
    def from_hf_config(cls, c: dict[str, Any]) -> "DecoderShape":
        heads = c["num_attention_heads"]
        return cls(
            hidden=c["hidden_size"],
            intermediate=c["intermediate_size"],
            layers=c["num_hidden_layers"],
            heads=heads,
            kv_heads=c.get("num_key_value_heads") or heads,
            head_dim=c.get("head_dim") or c["hidden_size"] // heads,
            vocab=c["vocab_size"],
            tied=bool(c.get("tie_word_embeddings", False)),
            qk_norm=c.get("model_type") == "qwen3",
            attention_bias=bool(c.get("attention_bias", False)),
        )

    def projection_shapes(self) -> dict[str, tuple[int, int]]:
        """(in, out) of each linear projection in one decoder layer."""
        h, q, kv = self.hidden, self.heads * self.head_dim, self.kv_heads * self.head_dim
        return {
            "q_proj": (h, q), "k_proj": (h, kv), "v_proj": (h, kv), "o_proj": (q, h),
            "gate_proj": (h, self.intermediate), "up_proj": (h, self.intermediate),
            "down_proj": (self.intermediate, h),
        }

    def linear_parameters(self) -> int:
        per_layer = sum(i * o for i, o in self.projection_shapes().values())
        if self.attention_bias:
            shapes = self.projection_shapes()
            per_layer += sum(shapes[n][1] for n in ("q_proj", "k_proj", "v_proj"))
        return per_layer * self.layers

    def norm_parameters(self) -> int:
        per_layer = 2 * self.hidden + (2 * self.head_dim if self.qk_norm else 0)
        return per_layer * self.layers + self.hidden

    def embedding_parameters(self) -> int:
        return self.vocab * self.hidden * (1 if self.tied else 2)

    def total_parameters(self) -> int:
        return self.linear_parameters() + self.norm_parameters() + self.embedding_parameters()

    def lora_parameters(self, rank: int, targets: Sequence[str] = ALL_PROJECTIONS) -> int:
        shapes = self.projection_shapes()
        return self.layers * sum(rank * (shapes[t][0] + shapes[t][1]) for t in targets)


@dataclass
class MemoryEstimate:
    method: str
    weights_gib: float
    trainable_state_gib: float
    activations_gib: float
    logits_gib: float
    total_gib: float
    fits: Optional[bool]


def estimate_memory(
    shape: DecoderShape,
    method: str,
    batch: int,
    seq: int,
    rank: int = 16,
    targets: Sequence[str] = ALL_PROJECTIONS,
    budget_gib: Optional[float] = None,
    reserve_gib: float = 1.0,
) -> MemoryEstimate:
    """Peak training memory estimate. ``method`` is ``full-bf16``, ``lora-bf16`` or ``qlora-nf4``.

    ``reserve_gib`` is an assumed allowance for the CUDA context and allocator
    fragmentation, added to the total.
    """
    linear, other = shape.linear_parameters(), shape.norm_parameters() + shape.embedding_parameters()
    if method == "full-bf16":
        weights = 2 * (linear + other)
        state = 16 * (linear + other)  # fp32 master copy + grad + 2 Adam moments (mixed precision)
    elif method == "lora-bf16":
        weights = 2 * (linear + other)
        state = 16 * shape.lora_parameters(rank, targets)
    elif method == "qlora-nf4":
        weights = linear * (0.5 + 0.127 / 8) + 2 * other
        state = 16 * shape.lora_parameters(rank, targets)
    else:
        raise ValueError(f"unknown method {method!r}")
    acts = 2 * batch * seq * shape.hidden * shape.layers + 34 * batch * seq * shape.hidden
    logits = 8 * batch * seq * shape.vocab
    total = weights + state + acts + logits + reserve_gib * GIB
    return MemoryEstimate(
        method=method,
        weights_gib=weights / GIB,
        trainable_state_gib=state / GIB,
        activations_gib=acts / GIB,
        logits_gib=logits / GIB,
        total_gib=total / GIB,
        fits=None if budget_gib is None else total / GIB <= budget_gib,
    )


def training_flops(parameters: int, tokens: int) -> float:
    return 6.0 * parameters * tokens


def wall_time_hours(flops: float, peak_tflops: float, utilization: float) -> float:
    if not 0 < utilization <= 1:
        raise ValueError("utilization must be in (0, 1]")
    return flops / (peak_tflops * 1e12 * utilization) / 3600.0


def measure_matmul_tflops(n: int = 2048, repeats: int = 10, device: str = "cpu", dtype: str = "float32") -> float:
    """Measured dense matmul throughput on ``device`` (median of ``repeats``)."""
    import torch

    dt = getattr(torch, dtype)
    a = torch.randn(n, n, device=device, dtype=dt)
    b = torch.randn(n, n, device=device, dtype=dt)
    samples = []
    for i in range(repeats + 2):
        if device.startswith("cuda"):
            torch.cuda.synchronize()
        start = time.perf_counter()
        a @ b
        if device.startswith("cuda"):
            torch.cuda.synchronize()
        if i >= 2:
            samples.append(time.perf_counter() - start)
    samples.sort()
    return 2 * n**3 / samples[len(samples) // 2] / 1e12


def plan(config: dict[str, Any], budget_gib: float, batch: int, seq: int, rank: int, tokens: int,
         peak_tflops: Optional[float], utilization: float) -> dict[str, Any]:
    shape = DecoderShape.from_hf_config(config)
    n = shape.total_parameters()
    out: dict[str, Any] = {
        "evidence": "estimated from closed-form formulas; not measured",
        "shape": asdict(shape),
        "parameters": n,
        "lora_parameters": shape.lora_parameters(rank),
        "memory": [asdict(estimate_memory(shape, m, batch, seq, rank, budget_gib=budget_gib))
                   for m in ("full-bf16", "lora-bf16", "qlora-nf4")],
        "training_flops": training_flops(n, tokens),
        "tokens": tokens, "batch": batch, "seq": seq, "rank": rank, "budget_gib": budget_gib,
    }
    if peak_tflops:
        out["assumed_utilization"] = utilization
        out["peak_tflops"] = peak_tflops
        out["wall_time_hours"] = wall_time_hours(out["training_flops"], peak_tflops, utilization)
    return out


def main(argv: Optional[Sequence[str]] = None) -> None:
    p = argparse.ArgumentParser(description="Estimate memory and compute for a fine-tuning run")
    p.add_argument("--config", required=True, help="Hugging Face config.json")
    p.add_argument("--budget-gb", type=float, default=12.0)
    p.add_argument("--batch", type=int, default=1)
    p.add_argument("--seq", type=int, default=1024)
    p.add_argument("--rank", type=int, default=16)
    p.add_argument("--tokens", type=int, default=20_000_000)
    p.add_argument("--peak-tflops", type=float, help="measured or datasheet dense peak of the target device")
    p.add_argument("--utilization", type=float, default=0.3, help="assumed model FLOPs utilization")
    p.add_argument("--measure", action="store_true", help="time a dense matmul on this machine and use it as the peak")
    args = p.parse_args(argv)
    peak = args.peak_tflops
    if args.measure:
        import torch

        device = "cuda" if torch.cuda.is_available() else "cpu"
        peak = measure_matmul_tflops(device=device, dtype="bfloat16" if device == "cuda" else "float32")
    print(json.dumps(plan(json.loads(Path(args.config).read_text()), args.budget_gb, args.batch, args.seq,
                          args.rank, args.tokens, peak, args.utilization), indent=2))


if __name__ == "__main__":
    main()
