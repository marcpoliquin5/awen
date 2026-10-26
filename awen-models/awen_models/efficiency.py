"""AWEN efficiency program: compression/efficiency ablations at a matched budget.

Every variant is an AWEN-Mini-style character model trained on Tiny
Shakespeare with identical data, optimizer, schedule, steps and seeds. The
budget is the *stored* parameter count of the 4-layer GELU baseline (within
3%); for sparse/MoE/quantized variants the extra columns say what is active
or how many bits are stored.

Variants
--------
``baseline``            4 layers, d=128, GELU MLP 512, tied output head.
``untied-head``         separate output head; MLP narrowed to stay in budget.
``low-rank``            factorized MLP (W ~ U V), wider hidden.
``tensor-train``        MLP matrices as 2-core tensor-train (TT) matrices.
``kron3``               MLP matrices as sums of R three-factor Kronecker products
                        (W = sum_r A_r ⊗ B_r ⊗ C_r), hidden 2048, rank 289 (H2 harness).
``moe-top1``            4 experts per layer, top-1 routing (Switch style), load-balance loss.
``sparse-50``           2x wider MLP, gradual magnitude pruning to 50% of MLP weights.
``qat-int4``            baseline trained with int4 fake-quant weights (STE); evaluated at int4.
``ptq-int4``            baseline weights rounded to int4 after training (no QAT), for comparison.
``distill``             baseline student trained with KD from the bundled AWEN-Mini checkpoint.
``recursive``           one shared block applied 4 times, d=256 (weight reuse).

Measured: validation loss (nats/char), stored/active parameters, batch-1
forward latency on this CPU, weight memory. **Estimated** (labeled): energy
per token from MAC counts times a user-settable pJ/MAC (default is a free
parameter, not a literature value), reported only as relative units unless
the caller provides a value.

Example::

    python -m awen_models.efficiency --steps 1500 --seeds 1 2 --out results/efficiency
"""

from __future__ import annotations

import argparse
import copy
import json
import math
import statistics
import time
from pathlib import Path
from typing import Any, Callable, Optional

import torch
import torch.nn as nn
import torch.nn.functional as F
from torch.utils.checkpoint import checkpoint

from .config import MiniConfig
from .data import TINY_SHAKESPEARE, download_tiny_shakespeare, sample_batch, split_tokens
from .model import AwenLinear, AwenMini, FeedForward
from .nonlinear import measure_latency_ms, pareto_front
from .tokenizer import CharTokenizer
from .train import hardware_record

PARAMETER_TOLERANCE = 0.03
BASE = dict(n_layer=4, n_embd=128, n_head=4, ffn="gelu", ffn_hidden=512, dropout=0.0)


# --------------------------------------------------------------------------- layers
class TTLinear(nn.Module):
    """Two-core tensor-train matrix: in = i1*i2, out = o1*o2, rank r.

    W[(a,b),(o,p)] = sum_r G1[a,o,r] G2[r,b,p]. The forward pass materializes W
    (cost i1*i2*o1*o2*r MACs per call, independent of batch) and then runs one
    dense GEMM: on CPUs/GPUs this is far faster than contracting the cores
    per token, but it means TT saves parameters, not per-token compute.
    """

    def __init__(self, in_shape: tuple[int, int], out_shape: tuple[int, int], rank: int, bias: bool = True):
        super().__init__()
        (i1, i2), (o1, o2) = in_shape, out_shape
        self.in_shape, self.out_shape = in_shape, out_shape
        self.g1 = nn.Parameter(torch.randn(i1, o1, rank) * (1.0 / math.sqrt(i1 * rank)) * 0.5)
        self.g2 = nn.Parameter(torch.randn(rank, i2, o2) * (1.0 / math.sqrt(i2)) * 0.5)
        self.bias = nn.Parameter(torch.zeros(o1 * o2)) if bias else None
        self.in_features, self.out_features = i1 * i2, o1 * o2

    def full_weight(self) -> torch.Tensor:
        """Dense [out, in] matrix (for tests and MAC accounting)."""
        w = torch.einsum("aor,rbp->abop", self.g1, self.g2)
        return w.reshape(self.in_features, self.out_features).T

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return F.linear(x, self.full_weight(), self.bias)

    def contract(self, x: torch.Tensor) -> torch.Tensor:
        """Per-token core contraction (reference path; equals forward)."""
        lead = x.shape[:-1]
        x = x.reshape(-1, *self.in_shape)
        t = torch.einsum("nab,aor->nbor", x, self.g1)
        y = torch.einsum("nbor,rbp->nop", t, self.g2).reshape(*lead, self.out_features)
        return y + self.bias if self.bias is not None else y

    def macs_per_token(self) -> int:
        """MACs as executed (dense GEMM on the materialized matrix)."""
        return self.in_features * self.out_features


class TTFeedForward(nn.Module):
    def __init__(self, d: int, in_shape, hid_shape, rank: int):
        super().__init__()
        self.fc = TTLinear(in_shape, hid_shape, rank)
        self.proj = TTLinear(hid_shape, in_shape, rank)

    def forward(self, x):
        return self.proj(F.gelu(self.fc(x)))


class Kron3Linear(nn.Module):
    """Sum of ``rank`` three-factor Kronecker products: W = sum_r A_r ⊗ B_r ⊗ C_r.

    ``d_in = i1*i2*i3`` and ``d_out = o1*o2*o3``; ``A_r`` is ``[o1, i1]``, ``B_r`` is
    ``[o2, i2]`` and ``C_r`` is ``[o3, i3]``. ``forward`` contracts the factors per
    token in a fixed order (C, then B, then A), so :meth:`macs_per_token` is the exact
    multiply count of the executed path, not an estimate. ``dense`` builds the full
    ``[d_out, d_in]`` matrix for tests.

    ``rank_chunk`` (default ``None``: one pass over all ranks, the original path) splits
    the rank sum into chunks of at most ``rank_chunk`` terms and, while autograd is
    recording, recomputes each chunk in backward (activation checkpointing) instead of
    storing its ``[n, r, ...]`` intermediates. The forward multiplies are the same, so
    :meth:`macs_per_token` is unchanged; backward does one extra forward per chunk.
    Peak activation memory then scales with ``rank_chunk`` rather than ``rank``.

    ``dense_weight`` (default ``False``: the per-token contraction above) instead builds
    the dense-equivalent ``[d_out, d_in]`` weight once per forward from the factors
    (:meth:`dense_weight_matrix`, ``sum_r A_r ⊗ B_r ⊗ C_r``) and applies it with one
    matrix product; autograd flows through the build to ``A``, ``B`` and ``C``. The
    model (parameters, function, gradients) is the same up to float rounding;
    ``rank_chunk`` is ignored on this path. :meth:`macs_per_token` then reports
    ``d_in * d_out`` (the executed path); the per-forward build cost, which does not
    depend on the number of tokens, is :meth:`dense_build_macs`.
    """

    def __init__(self, ins: tuple[int, int, int], outs: tuple[int, int, int], rank: int, bias: bool = True,
                 rank_chunk: Optional[int] = None, dense_weight: bool = False):
        super().__init__()
        if len(ins) != 3 or len(outs) != 3 or rank < 1:
            raise ValueError("Kron3Linear needs three input factors, three output factors and rank >= 1")
        if rank_chunk is not None and rank_chunk < 1:
            raise ValueError("rank_chunk must be None or >= 1")
        self.ins, self.outs, self.rank = tuple(ins), tuple(outs), rank
        self.rank_chunk = rank_chunk
        self.dense_weight = bool(dense_weight)
        self.in_features, self.out_features = math.prod(ins), math.prod(outs)
        scale = rank ** (1 / 3)
        self.A = nn.Parameter(torch.randn(rank, outs[0], ins[0]) / math.sqrt(ins[0] * scale))
        self.B = nn.Parameter(torch.randn(rank, outs[1], ins[1]) / math.sqrt(ins[1] * scale))
        self.C = nn.Parameter(torch.randn(rank, outs[2], ins[2]) / math.sqrt(ins[2] * scale))
        self.bias = nn.Parameter(torch.zeros(self.out_features)) if bias else None

    def dense(self) -> torch.Tensor:
        """Equivalent dense ``[d_out, d_in]`` matrix."""
        return sum(torch.kron(torch.kron(self.A[r], self.B[r]), self.C[r]) for r in range(self.rank))

    def dense_weight_matrix(self) -> torch.Tensor:
        """Dense-equivalent ``[d_out, d_in]`` weight built with two contractions (A with B, then C).

        Same matrix as :meth:`dense` (the Kronecker reference), differentiable in ``A``, ``B``, ``C``."""
        ab = torch.einsum("rai,rbj->rabij", self.A, self.B)                  # r*o1*o2*i1*i2 MACs
        w = torch.einsum("rabij,rck->abcijk", ab, self.C)                    # r*d_out*d_in MACs
        return w.reshape(self.out_features, self.in_features)

    def dense_build_macs(self) -> int:
        """Multiplies to build :meth:`dense_weight_matrix` once (per forward on the ``dense_weight`` path)."""
        (i1, i2, _), (o1, o2, _) = self.ins, self.outs
        return self.rank * (o1 * o2 * i1 * i2 + self.out_features * self.in_features)

    @staticmethod
    def _contract(xi: torch.Tensor, A: torch.Tensor, B: torch.Tensor, C: torch.Tensor) -> torch.Tensor:
        t = torch.einsum("nijk,rck->nrijc", xi, C)                 # n*r*i1*i2*i3*o3 MACs
        t = torch.einsum("nrijc,rbj->nricb", t, B)                 # n*r*i1*i2*o3*o2 MACs
        return torch.einsum("nricb,rai->nabc", t, A)               # n*r*i1*o3*o2*o1 MACs

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        if self.dense_weight:
            return F.linear(x, self.dense_weight_matrix(), self.bias)
        lead = x.shape[:-1]
        xi = x.reshape(-1, *self.ins)                               # [n, i1, i2, i3]
        if self.rank_chunk is None or self.rank_chunk >= self.rank:
            y = self._contract(xi, self.A, self.B, self.C)
        else:
            y = None
            for s in range(0, self.rank, self.rank_chunk):
                sl = slice(s, s + self.rank_chunk)
                args = (xi, self.A[sl], self.B[sl], self.C[sl])
                if torch.is_grad_enabled():
                    part = checkpoint(self._contract, *args, use_reentrant=False)
                else:
                    part = self._contract(*args)
                y = part if y is None else y + part
        y = y.reshape(*lead, self.out_features)
        return y + self.bias if self.bias is not None else y

    def macs_per_token(self) -> int:
        """Multiplies per token on the executed path: the contraction (C, then B, then A), or
        ``d_in * d_out`` when ``dense_weight`` is set (build cost excluded, see :meth:`dense_build_macs`)."""
        if self.dense_weight:
            return self.in_features * self.out_features
        (i1, i2, i3), (o1, o2, o3) = self.ins, self.outs
        return self.rank * (i1 * i2 * i3 * o3 + i1 * i2 * o3 * o2 + i1 * o3 * o2 * o1)


class Kron3FeedForward(nn.Module):
    """GELU MLP whose two matrices are :class:`Kron3Linear` layers."""

    def __init__(self, in_factors: tuple[int, int, int], hid_factors: tuple[int, int, int], rank: int,
                 rank_chunk: Optional[int] = None, dense_weight: bool = False):
        super().__init__()
        self.fc = Kron3Linear(in_factors, hid_factors, rank, rank_chunk=rank_chunk, dense_weight=dense_weight)
        self.proj = Kron3Linear(hid_factors, in_factors, rank, rank_chunk=rank_chunk, dense_weight=dense_weight)

    def forward(self, x):
        return self.proj(F.gelu(self.fc(x)))


def set_kron3_rank_chunk(model: nn.Module, rank_chunk: Optional[int]) -> int:
    """Set ``rank_chunk`` on every :class:`Kron3Linear` in ``model``; returns how many were set."""
    if rank_chunk is not None and rank_chunk < 1:
        raise ValueError("rank_chunk must be None or >= 1")
    layers = [m for m in model.modules() if isinstance(m, Kron3Linear)]
    for m in layers:
        m.rank_chunk = rank_chunk
    return len(layers)


def set_kron3_dense_weight(model: nn.Module, enabled: bool) -> int:
    """Set ``dense_weight`` on every :class:`Kron3Linear` in ``model``; returns how many were set."""
    layers = [m for m in model.modules() if isinstance(m, Kron3Linear)]
    for m in layers:
        m.dense_weight = bool(enabled)
    return len(layers)


class MoEFeedForward(nn.Module):
    """Top-1 routed mixture of GELU experts with a Switch-style load-balance loss."""

    def __init__(self, d: int, hidden: int, experts: int, aux_weight: float = 0.01):
        super().__init__()
        self.router = nn.Linear(d, experts, bias=False)
        self.experts = nn.ModuleList(
            nn.Sequential(AwenLinear(d, hidden), nn.GELU(), AwenLinear(hidden, d)) for _ in range(experts)
        )
        self.aux_weight = aux_weight
        self.aux_loss = torch.zeros(())
        self.hidden = hidden

    def forward(self, x):
        shape = x.shape
        flat = x.reshape(-1, shape[-1])
        probs = F.softmax(self.router(flat), dim=-1)
        gate, idx = probs.max(dim=-1)
        out = torch.zeros_like(flat)
        n_e = len(self.experts)
        for e, expert in enumerate(self.experts):
            sel = (idx == e).nonzero(as_tuple=True)[0]
            if sel.numel():
                out.index_add_(0, sel, expert(flat[sel]) * gate[sel, None])
        frac = torch.bincount(idx, minlength=n_e).float() / idx.numel()
        self.aux_loss = self.aux_weight * n_e * torch.sum(frac * probs.mean(dim=0))
        return out.reshape(shape)


def fake_quant(w: torch.Tensor, bits: int) -> torch.Tensor:
    """Symmetric per-output-row uniform quantization with a straight-through gradient."""
    qmax = 2 ** (bits - 1) - 1
    scale = w.detach().abs().amax(dim=1, keepdim=True).clamp_min(1e-12) / qmax
    q = torch.clamp(torch.round(w / scale), -qmax, qmax) * scale
    return w + (q - w).detach()


class FakeQuant(nn.Module):
    def __init__(self, bits: int):
        super().__init__()
        self.bits = bits

    def forward(self, w):
        return fake_quant(w, self.bits)


def quantize_weights_(model: nn.Module, bits: int) -> None:
    """In place: round every AwenLinear weight to `bits` (per-row symmetric)."""
    with torch.no_grad():
        for m in model.modules():
            if isinstance(m, AwenLinear) and not nn.utils.parametrize.is_parametrized(m):
                m.weight.copy_(fake_quant(m.weight, bits))


class UntiedMini(AwenMini):
    def __init__(self, config: MiniConfig):
        super().__init__(config)
        self.head = nn.Linear(config.n_embd, config.vocab_size, bias=False)
        nn.init.normal_(self.head.weight, std=0.02)

    def forward(self, idx, targets=None):
        _, t = idx.shape
        x = self.drop(self.tok_emb(idx) + self.pos_emb(torch.arange(t, device=idx.device)))
        for block in self._layer_sequence():
            x = block(x)
        logits = self.head(self.ln_f(x))
        loss = None if targets is None else F.cross_entropy(logits.reshape(-1, logits.size(-1)), targets.reshape(-1))
        return logits, loss


# --------------------------------------------------------------------------- variants
def _mini(vocab: int, **kw) -> AwenMini:
    return AwenMini(MiniConfig(vocab_size=vocab, **{**BASE, **kw}))


def _replace_ffn(model: AwenMini, factory: Callable[[], nn.Module]) -> AwenMini:
    for block in model.blocks:
        block.ffn = factory()
    return model


def build(name: str, vocab: int) -> AwenMini:
    d = BASE["n_embd"]
    if name in ("baseline", "ptq-int4", "distill", "qat-int4"):
        m = _mini(vocab)
        if name == "qat-int4":
            for lin in [x for x in m.modules() if isinstance(x, AwenLinear)]:
                nn.utils.parametrize.register_parametrization(lin, "weight", FakeQuant(4))
        return m
    if name == "untied-head":
        return UntiedMini(MiniConfig(vocab_size=vocab, **{**BASE, "ffn_hidden": 504}))
    if name == "low-rank":
        return _mini(vocab, ffn="lowrank", ffn_hidden=1024, ffn_rank=56)
    if name == "tensor-train":
        return _replace_ffn(_mini(vocab), lambda: TTFeedForward(d, (8, 16), (32, 32), 85))
    if name == "kron3":
        # d=128=(4,4,8) -> hidden 2048=(8,16,16), rank 289: 131,648 FFN parameters vs 131,712 dense
        # (layer shapes from the H2 pilot in research/experiments/multiplexing-fractal, registered as h2.yaml).
        return _replace_ffn(_mini(vocab), lambda: Kron3FeedForward((4, 4, 8), (8, 16, 16), 289))
    if name == "moe-top1":
        return _replace_ffn(_mini(vocab), lambda: MoEFeedForward(d, 128, 4))
    if name == "sparse-50":
        return _mini(vocab, ffn_hidden=1024)
    if name == "recursive":
        return _mini(vocab, shared_layers=True, n_embd=256, n_head=8, ffn_hidden=1024)
    raise KeyError(name)


VARIANTS = ["baseline", "untied-head", "low-rank", "tensor-train", "kron3", "moe-top1", "sparse-50",
            "qat-int4", "ptq-int4", "distill", "recursive"]


def mlp_weights(model: AwenMini) -> list[torch.Tensor]:
    return [p for n, p in model.named_parameters() if ".ffn." in n and p.dim() == 2]


def stored_parameters(model: nn.Module) -> int:
    seen, total = set(), 0
    for p in model.parameters():
        if id(p) not in seen:
            seen.add(id(p))
            total += p.numel()
    return total


def check_budget(vocab: int) -> dict[str, int]:
    counts = {}
    for n in VARIANTS:
        m = build(n, vocab)
        c = stored_parameters(m)
        if n == "sparse-50":
            mw = sum(p.numel() for p in mlp_weights(m))
            c -= mw // 2  # stored nonzeros after pruning to 50%
        counts[n] = c
    ref = counts["baseline"]
    for n, c in counts.items():
        if abs(c - ref) / ref > PARAMETER_TOLERANCE:
            raise ValueError(f"{n}: {c} parameters outside ±3% of {ref}")
    return counts


def macs_per_token(model: AwenMini, context: int) -> float:
    """Multiply-accumulates per generated token at full context (active weights + attention)."""
    cfg = model.config
    d, L = cfg.n_embd, cfg.n_layer
    total = 0.0
    blocks = list(model._layer_sequence())
    for b in blocks:
        total += b.attn.qkv.in_features * b.attn.qkv.out_features + d * d
        total += 2 * context * d  # QK^T and AV for one query
        f = b.ffn
        if isinstance(f, (TTFeedForward, Kron3FeedForward)):
            total += f.fc.macs_per_token() + f.proj.macs_per_token()
        elif isinstance(f, MoEFeedForward):
            total += 2 * d * f.hidden + d * len(f.experts)  # one expert + router
        else:
            total += sum(m.in_features * m.out_features for m in f.modules() if isinstance(m, nn.Linear))
    total += d * cfg.vocab_size
    assert L == len(blocks)
    return total


# --------------------------------------------------------------------------- training
def _lr(step, steps, warmup, lr=1e-3, lr_min=1e-4):
    if step < warmup:
        return lr * (step + 1) / warmup
    p = (step - warmup) / max(1, steps - warmup)
    return lr_min + 0.5 * (lr - lr_min) * (1 + math.cos(math.pi * p))


@torch.no_grad()
def eval_loss(model, data, batches, batch_size, block, seed=0):
    model.eval()
    g = torch.Generator().manual_seed(seed)
    out = statistics.mean(model(*sample_batch(data, batch_size, block, g))[1].item() for _ in range(batches))
    model.train()
    return out


def load_teacher(text: str) -> AwenMini:
    """Bundled AWEN-Mini (1,016,320 params) as KD teacher; tokenizer must match the student's."""
    from .checkpoint import load_checkpoint
    path = Path(__file__).resolve().parent / "checkpoints" / "awen-mini"
    model, tok, meta = load_checkpoint(path)
    if tok.encode(text[:5000]) != CharTokenizer.from_text(text).encode(text[:5000]):
        raise ValueError("teacher tokenizer differs from the student tokenizer")
    if meta.get("settings", {}).get("val_fraction", 0.1) != 0.1:
        raise ValueError("teacher used a different train/validation split")
    return model


def train_variant(name: str, text: str, steps: int, seed: int, batch_size: int = 32,
                  eval_batches: int = 20, teacher: Optional[AwenMini] = None) -> tuple[AwenMini, dict]:
    tok = CharTokenizer.from_text(text)
    splits = split_tokens(tok.encode(text), 0.1)
    torch.manual_seed(seed)
    model = build(name, tok.vocab_size)
    block = model.config.block_size
    params = list({id(p): p for p in model.parameters()}.values())
    opt = torch.optim.AdamW([{"params": [p for p in params if p.dim() >= 2], "weight_decay": 0.1},
                             {"params": [p for p in params if p.dim() < 2], "weight_decay": 0.0}],
                            lr=1e-3, betas=(0.9, 0.99))
    g = torch.Generator().manual_seed(seed)
    warmup = min(100, max(1, steps // 10))
    masks: dict[int, torch.Tensor] = {}
    started = time.perf_counter()
    for step in range(steps):
        for grp in opt.param_groups:
            grp["lr"] = _lr(step, steps, warmup)
        x, y = sample_batch(splits.train, batch_size, block, g)
        logits, loss = model(x, y)
        if name == "moe-top1":
            loss = loss + sum(b.ffn.aux_loss for b in model.blocks)
        if name == "distill" and teacher is not None:
            T = 2.0
            with torch.no_grad():
                tl, _ = teacher(x)
            kd = F.kl_div(F.log_softmax(logits / T, -1), F.log_softmax(tl / T, -1), log_target=True,
                          reduction="batchmean") * T * T / logits.shape[1]
            loss = 0.5 * loss + 0.5 * kd
        opt.zero_grad(set_to_none=True)
        loss.backward()
        torch.nn.utils.clip_grad_norm_(params, 1.0)
        opt.step()
        if name == "sparse-50":
            # gradual magnitude pruning: cubic schedule to 50% at 60% of training, then fixed
            target = 0.5 * min(1.0, step / (0.6 * steps)) ** 3 if step < 0.6 * steps else 0.5
            with torch.no_grad():
                for w in mlp_weights(model):
                    k = int(target * w.numel())
                    if k:
                        thr = w.abs().flatten().kthvalue(k).values
                        masks[id(w)] = w.abs() > thr
                        w.mul_(masks[id(w)])
    wall = time.perf_counter() - started
    if name == "ptq-int4":
        quantize_weights_(model, 4)
    if name == "qat-int4":
        for lin in [x for x in model.modules() if isinstance(x, AwenLinear)]:
            nn.utils.parametrize.remove_parametrizations(lin, "weight", leave_parametrized=True)
    val = eval_loss(model, splits.val, eval_batches, batch_size, block, 0)
    info = {"train_wall_time_s": round(wall, 1), "val_loss": round(val, 5)}
    if name == "sparse-50":
        mw = mlp_weights(model)
        info["mlp_weight_sparsity"] = round(sum(int((w == 0).sum()) for w in mw) / sum(w.numel() for w in mw), 4)
    return model, info


def weight_bits(name: str, model: nn.Module) -> int:
    n = stored_parameters(model)
    if name in ("qat-int4", "ptq-int4"):
        lin = sum(m.weight.numel() for m in model.modules() if isinstance(m, AwenLinear))
        return lin * 4 + (n - lin) * 32
    if name == "sparse-50":
        mw = sum(p.numel() for p in mlp_weights(model))
        return (n - mw // 2) * 32  # nonzero values only; index overhead not counted (stated)
    return n * 32


def run(steps: int, seeds: list[int], out: Path, variants: Optional[list[str]] = None,
        text: Optional[str] = None, pj_per_mac: Optional[float] = None) -> dict[str, Any]:
    corpus = text if text is not None else download_tiny_shakespeare()
    vocab = len(set(corpus))
    names = variants or VARIANTS
    counts = check_budget(vocab)
    teacher = load_teacher(corpus) if "distill" in names and text is None else None
    runs = []
    for name in names:
        for seed in seeds:
            model, info = train_variant(name, corpus, steps, seed, teacher=teacher)
            macs = macs_per_token(model, model.config.block_size)
            row = {"variant": name, "seed": seed, "stored_parameters": counts[name], **info,
                   "forward_latency_ms_b1": round(measure_latency_ms(model), 3),
                   "weight_memory_kib": round(weight_bits(name, model) / 8 / 1024, 1),
                   "macs_per_token_at_full_context": int(macs)}
            runs.append(row)
            print(json.dumps(row), flush=True)
    return summarize(runs, steps, seeds, counts, out, text is None, len(corpus), pj_per_mac)


def summarize(runs: list[dict], steps: int, seeds: list[int], counts: dict[str, int], out: Path,
              tiny_shakespeare: bool = True, characters: int = 0, pj_per_mac: Optional[float] = None) -> dict:
    names = list(dict.fromkeys(r["variant"] for r in runs))
    summary = {}
    for name in names:
        rows = [r for r in runs if r["variant"] == name]
        losses = [r["val_loss"] for r in rows]
        s = {"stored_parameters": counts[name], "val_loss_mean": round(statistics.mean(losses), 4),
             "val_loss_stdev": round(statistics.stdev(losses), 4) if len(losses) > 1 else None,
             "forward_latency_ms_b1_mean": round(statistics.mean(r["forward_latency_ms_b1"] for r in rows), 3),
             "weight_memory_kib": rows[0]["weight_memory_kib"],
             "macs_per_token_at_full_context": rows[0]["macs_per_token_at_full_context"]}
        s["energy_per_token_estimate"] = (
            {"value_j": s["macs_per_token_at_full_context"] * pj_per_mac * 1e-12, "pj_per_mac": pj_per_mac,
             "label": "estimated from MAC count x user-supplied pJ/MAC"} if pj_per_mac else
            {"relative_to_baseline": None, "label": "estimated: proportional to MACs; no pJ/MAC supplied"})
        summary[name] = s
    base_macs = summary.get("baseline", {}).get("macs_per_token_at_full_context")
    if base_macs and not pj_per_mac:
        for s in summary.values():
            s["energy_per_token_estimate"]["relative_to_baseline"] = round(
                s["macs_per_token_at_full_context"] / base_macs, 3)
    report = {"harness": "awen.efficiency-ablation.v1",
              "dataset": TINY_SHAKESPEARE if tiny_shakespeare else {"name": "custom", "characters": characters},
              "steps": steps, "batch_size": 32, "seeds": seeds, "parameter_tolerance": PARAMETER_TOLERANCE,
              "runs": runs, "summary": summary, "hardware": hardware_record(torch.device("cpu")),
              "evidence": ("val loss, latency measured on the hardware above; weight memory computed from "
                           "stored values (sparse index overhead excluded); energy is an estimate, not measured"),
              "pareto_front_loss_vs_params": pareto_front(
                  [(s["stored_parameters"], s["val_loss_mean"], n) for n, s in summary.items()]),
              "pareto_front_loss_vs_latency": pareto_front(
                  [(s["forward_latency_ms_b1_mean"], s["val_loss_mean"], n) for n, s in summary.items()]),
              "pareto_front_loss_vs_memory": pareto_front(
                  [(s["weight_memory_kib"], s["val_loss_mean"], n) for n, s in summary.items()])}
    out.mkdir(parents=True, exist_ok=True)
    (out / "results.json").write_text(json.dumps(report, indent=2) + "\n")
    plot(report, out / "pareto.png")
    return report


def plot(report: dict, path: Path) -> None:
    import matplotlib
    matplotlib.use("Agg")
    import matplotlib.pyplot as plt
    S = report["summary"]
    axes_spec = [("stored_parameters", "stored parameters", "pareto_front_loss_vs_params"),
                 ("forward_latency_ms_b1_mean", "forward latency b1 (ms, measured)", "pareto_front_loss_vs_latency"),
                 ("weight_memory_kib", "weight memory (KiB, computed)", "pareto_front_loss_vs_memory"),
                 ("macs_per_token_at_full_context", "MACs/token (energy proxy, estimated)", None)]
    fig, axs = plt.subplots(1, 4, figsize=(18, 4.4), dpi=110)
    for ax, (key, label, front_key) in zip(axs, axes_spec):
        front = set(report.get(front_key, [])) if front_key else set()
        for n, s in S.items():
            ax.scatter(s[key], s["val_loss_mean"], s=50, color="#2563eb" if n in front else "#9ca3af", zorder=3)
            ax.annotate(n, (s[key], s["val_loss_mean"]), fontsize=7, xytext=(4, 4), textcoords="offset points")
        ax.set_xlabel(label)
        ax.set_ylabel("val loss (nats/char)")
        ax.grid(alpha=0.3)
    fig.suptitle(f"AWEN efficiency ablations, {report['steps']} steps, seeds {report['seeds']} (blue = Pareto front)")
    fig.tight_layout()
    fig.savefig(path)
    plt.close(fig)


def main(argv=None):
    ap = argparse.ArgumentParser(description="AWEN efficiency ablations at a matched parameter budget.")
    ap.add_argument("--steps", type=int, default=1500)
    ap.add_argument("--seeds", type=int, nargs="+", default=[1, 2])
    ap.add_argument("--variants", nargs="+", default=None)
    ap.add_argument("--pj-per-mac", type=float, default=None)
    ap.add_argument("--out", default="results/efficiency")
    ap.add_argument("--threads", type=int, default=None)
    ap.add_argument("--merge", nargs="+", default=None,
                    help="combine results.json files from runs with different seeds into --out")
    a = ap.parse_args(argv)
    if a.threads:
        torch.set_num_threads(a.threads)
    if a.merge:
        parts = [json.loads(Path(p).read_text()) for p in a.merge]
        runs = [r for p in parts for r in p["runs"]]
        seeds = sorted({r["seed"] for r in runs})
        counts = {r["variant"]: r["stored_parameters"] for r in runs}
        summarize(runs, parts[0]["steps"], seeds, counts, Path(a.out), pj_per_mac=a.pj_per_mac)
        return
    run(a.steps, a.seeds, Path(a.out), a.variants, pj_per_mac=a.pj_per_mac)


if __name__ == "__main__":
    main()
