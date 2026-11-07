"""F3: hierarchical / self-similar weight structure in AWEN-Mini feed-forward layers.

Variants (all trained by ``awen_models.efficiency.train_variant`` with the
same data, optimizer, schedule, steps and seeds as the dense baseline, and
held to the baseline's stored-parameter budget within 3%):

``monarch``   both FFN matrices are Monarch matrices (Dao et al., ICML 2022,
              arXiv:2204.00595): block-diagonal, permute, block-diagonal.
              A 2-level butterfly-like hierarchy; the saved parameters buy a
              wider hidden layer.
``kron3``     both FFN matrices are sums of R three-factor Kronecker products,
              W = sum_r A_r (x) B_r (x) C_r, a three-level self-similar block
              structure (each block of W is itself a scaled Kronecker product).
``recursive-monarch``  the existing ``recursive`` variant (one shared d=256
              block applied 4 times) with Monarch FFN matrices and a wider hidden.

Only measured validation loss, latency and parameter counts are reported.
"""

from __future__ import annotations

import argparse
import json
import math
import statistics
from pathlib import Path

import torch
import torch.nn as nn
import torch.nn.functional as F

from awen_models import efficiency as eff
from awen_models.config import MiniConfig
from awen_models.model import AwenMini


class MonarchLinear(nn.Module):
    """y = reshape(L · perm · R · x): ``d_in = nb·k`` → mid ``nb·m`` → ``d_out = m·c``."""

    def __init__(self, d_in: int, d_out: int, nb: int, m: int, bias: bool = True):
        super().__init__()
        if d_in % nb or d_out % m:
            raise ValueError("d_in must be divisible by nb and d_out by m")
        self.d_in, self.d_out, self.nb, self.k, self.m, self.c = d_in, d_out, nb, d_in // nb, m, d_out // m
        self.R = nn.Parameter(torch.randn(nb, self.k, m) / math.sqrt(self.k))
        self.L = nn.Parameter(torch.randn(m, nb, self.c) / math.sqrt(nb))
        self.bias = nn.Parameter(torch.zeros(d_out)) if bias else None

    def dense(self) -> torch.Tensor:
        """Equivalent dense matrix ``[d_out, d_in]``: W[(m, c), (b, k)] = R[b, k, m] · L[m, b, c]."""
        return torch.einsum("bkm,mbc->mcbk", self.R, self.L).reshape(self.d_out, self.d_in)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        # The structure lives in the parameters; on CPU, materializing W (d_out x d_in) and calling one
        # dense GEMM is faster than two batched block products, so training latency reflects dense compute.
        return F.linear(x, self.dense(), self.bias)

    def forward_factored(self, x: torch.Tensor) -> torch.Tensor:
        """Two block-diagonal products with the Monarch permutation between them (reference path)."""
        s = x.shape[:-1]
        h = torch.bmm(x.reshape(-1, self.nb, self.k).transpose(0, 1), self.R)  # [nb, N, m]
        y = torch.bmm(h.permute(2, 1, 0), self.L).transpose(0, 1).reshape(*s, self.d_out)  # [N, m, c]
        return y if self.bias is None else y + self.bias


class Kron3Linear(nn.Module):
    """W = sum_r A_r ⊗ B_r ⊗ C_r with ``d_out = o1·o2·o3``, ``d_in = i1·i2·i3``."""

    def __init__(self, ins: tuple[int, int, int], outs: tuple[int, int, int], rank: int, bias: bool = True):
        super().__init__()
        self.ins, self.outs, self.rank = ins, outs, rank
        self.d_in, self.d_out = math.prod(ins), math.prod(outs)
        self.A = nn.Parameter(torch.randn(rank, outs[0], ins[0]) / math.sqrt(ins[0] * rank ** (1 / 3)))
        self.B = nn.Parameter(torch.randn(rank, outs[1], ins[1]) / math.sqrt(ins[1] * rank ** (1 / 3)))
        self.C = nn.Parameter(torch.randn(rank, outs[2], ins[2]) / math.sqrt(ins[2] * rank ** (1 / 3)))
        self.bias = nn.Parameter(torch.zeros(self.d_out)) if bias else None

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        s = x.shape[:-1]
        xi = x.reshape(*s, *self.ins)
        y = torch.einsum("...ijk,rai,rbj,rck->...abc", xi, self.A, self.B, self.C).reshape(*s, self.d_out)
        return y if self.bias is None else y + self.bias

    def dense(self) -> torch.Tensor:
        w = sum(torch.kron(torch.kron(self.A[r], self.B[r]), self.C[r]) for r in range(self.rank))
        return w


class StructuredFFN(nn.Module):
    def __init__(self, fc: nn.Module, proj: nn.Module):
        super().__init__()
        self.fc, self.proj = fc, proj

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        return self.proj(F.gelu(self.fc(x)))


def monarch_ffn(d: int, hidden: int, nb_in: int, m_in: int, nb_out: int, m_out: int) -> StructuredFFN:
    return StructuredFFN(MonarchLinear(d, hidden, nb_in, m_in), MonarchLinear(hidden, d, nb_out, m_out))


VARIANTS = ["baseline", "monarch", "kron3", "recursive", "recursive-monarch"]


def build(name: str, vocab: int) -> AwenMini:
    if name in ("baseline", "recursive"):
        return eff.build(name, vocab)
    if name == "monarch":
        # d=128 -> 2048 -> 128 (4x the dense hidden width) at the dense FFN's parameter count.
        return eff._replace_ffn(eff._mini(vocab), lambda: monarch_ffn(128, 2048, 8, 128, 256, 32))
    if name == "kron3":
        return eff._replace_ffn(eff._mini(vocab), lambda: StructuredFFN(
            Kron3Linear((4, 4, 8), (8, 16, 16), 289), Kron3Linear((8, 16, 16), (4, 4, 8), 289)))
    if name == "recursive-monarch":
        m = eff._mini(vocab, shared_layers=True, n_embd=256, n_head=8, ffn_hidden=1024)
        m.blocks[0].ffn = monarch_ffn(256, 2048, 32, 256, 512, 128)
        return m
    raise KeyError(name)


def budget(vocab: int) -> dict[str, int]:
    counts = {n: eff.stored_parameters(build(n, vocab)) for n in VARIANTS}
    ref = counts["baseline"]
    for n, c in counts.items():
        if abs(c - ref) / ref > eff.PARAMETER_TOLERANCE:
            raise ValueError(f"{n}: {c} stored parameters outside ±3% of {ref}")
    return counts


def run(steps: int, seeds: list[int], out: Path, variants: list[str] | None = None) -> dict:
    corpus = eff.download_tiny_shakespeare()
    vocab = len(set(corpus))
    counts = budget(vocab)
    runs = []
    for name in variants or VARIANTS:
        for seed in seeds:
            model, info = eff.train_variant(name, corpus, steps, seed, builder=build)
            row = {"variant": name, "seed": seed, "stored_parameters": counts[name], **info,
                   "forward_latency_ms_b1": round(eff.measure_latency_ms(model), 3)}
            runs.append(row)
            print(json.dumps(row), flush=True)
    summary = {}
    for name in dict.fromkeys(r["variant"] for r in runs):
        rows = [r for r in runs if r["variant"] == name]
        losses = [r["val_loss"] for r in rows]
        summary[name] = {"stored_parameters": counts[name],
                         "val_loss_mean": round(statistics.mean(losses), 4),
                         "val_loss_stdev": round(statistics.stdev(losses), 4) if len(losses) > 1 else None,
                         "forward_latency_ms_b1_mean": round(statistics.mean(r["forward_latency_ms_b1"] for r in rows), 3),
                         "train_wall_time_s_mean": round(statistics.mean(r["train_wall_time_s"] for r in rows), 1)}
    report = {"harness": "awen.fractal-structured-weights.v1", "dataset": eff.TINY_SHAKESPEARE, "steps": steps,
              "batch_size": 32, "seeds": seeds, "parameter_tolerance": eff.PARAMETER_TOLERANCE, "runs": runs,
              "summary": summary, "hardware": eff.hardware_record(torch.device("cpu")),
              "evidence": "validation loss and batch-1 latency measured on the hardware above"}
    out.mkdir(parents=True, exist_ok=True)
    (out / "structured_weights.json").write_text(json.dumps(report, indent=2) + "\n")
    return report


def main(argv=None):
    ap = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    ap.add_argument("--steps", type=int, default=1500)
    ap.add_argument("--seeds", type=int, nargs="+", default=[1, 2])
    ap.add_argument("--variants", nargs="+", default=None)
    ap.add_argument("--threads", type=int, default=None)
    ap.add_argument("--out", default=str(Path(__file__).parent / "results"))
    a = ap.parse_args(argv)
    if a.threads:
        torch.set_num_threads(a.threads)
    run(a.steps, a.seeds, Path(a.out), a.variants)


if __name__ == "__main__":
    main()
