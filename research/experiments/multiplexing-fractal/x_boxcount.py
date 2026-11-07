"""F4 (exploratory): box-counting dimension of loss-landscape slices and heavy tails of weight spectra.

* Loss slices follow Li et al., "Visualizing the Loss Landscape of Neural Nets" (NeurIPS 2018,
  arXiv:1712.09913): a random Gaussian direction, filter-normalized to the trained weights,
  and the loss L(θ + α d) on a fixed batch. The graph {(α, L)} rescaled to the unit square is
  box-counted (``frontier_checks.box_counting_dimension``). A C¹ curve has dimension 1; a value
  clearly above 1 over a range of scales would indicate roughness at those scales.
* Weight spectra: eigenvalues of WᵀW/N per weight matrix; tail exponent α of the empirical
  spectral density by the Hill estimator on the top 10%. Martin & Mahoney (JMLR 2021,
  arXiv:1810.01075) report heavy-tailed ESDs in trained networks; random Gaussian matrices
  follow Marchenko–Pastur with no power-law tail.
"""

from __future__ import annotations

import argparse
import json
import sys
from pathlib import Path

import numpy as np
import torch

sys.path.insert(0, str(Path(__file__).resolve().parents[2] / "frontier-physics"))
from frontier_checks import box_counting_dimension  # noqa: E402

from awen_models.checkpoint import load_checkpoint  # noqa: E402
from awen_models.data import download_tiny_shakespeare, sample_batch, split_tokens  # noqa: E402
from awen_models.model import AwenMini  # noqa: E402

CHECKPOINT = Path(__import__("awen_models").__file__).resolve().parent / "checkpoints" / "awen-mini"


def graph_dimension(x: np.ndarray, y: np.ndarray, scales: list[int]) -> float:
    """Box-counting dimension of the graph of y(x) rescaled to [0, 1)² (endpoints nudged inside)."""
    xs = (x - x.min()) / (np.ptp(x) or 1.0) * (1 - 1e-12)
    ys = (y - y.min()) / (np.ptp(y) or 1.0) * (1 - 1e-12)
    # Densify linearly between samples so a box is counted when the segment passes through it.
    k = 8
    t = np.linspace(0, 1, k, endpoint=False)
    xd = (xs[:-1, None] + np.outer(np.diff(xs), t)).ravel()
    yd = (ys[:-1, None] + np.outer(np.diff(ys), t)).ravel()
    return box_counting_dimension(np.stack([xd, yd], 1), scales)


def filter_normalized_direction(model: torch.nn.Module, seed: int) -> list[torch.Tensor]:
    g = torch.Generator().manual_seed(seed)
    out = []
    for p in model.parameters():
        d = torch.randn(p.shape, generator=g)
        if p.dim() >= 2:
            d = d * (p.norm(dim=1, keepdim=True) / (d.norm(dim=1, keepdim=True) + 1e-12))
        else:
            d = torch.zeros_like(p)  # biases / norms held fixed (Li et al. §4)
        out.append(d)
    return out


@torch.no_grad()
def loss_slice(model: AwenMini, x: torch.Tensor, y: torch.Tensor, direction, alphas: np.ndarray) -> np.ndarray:
    base = [p.detach().clone() for p in model.parameters()]
    vals = []
    for a in alphas:
        for p, b, d in zip(model.parameters(), base, direction):
            p.copy_(b + float(a) * d)
        vals.append(model(x, y)[1].item())
    for p, b in zip(model.parameters(), base):
        p.copy_(b)
    return np.array(vals)


def hill_tail_exponent(eigs: np.ndarray, fraction: float = 0.1) -> float:
    """Hill estimator of the ESD tail exponent α (density ~ λ^-α) on the top ``fraction``."""
    e = np.sort(eigs)[::-1]
    k = max(5, int(fraction * e.size))
    top = e[:k + 1]
    return float(1 + k / np.sum(np.log(top[:k] / top[k])))


def esd_report(model: torch.nn.Module) -> dict[str, float]:
    out = {}
    for name, p in model.named_parameters():
        if p.dim() == 2 and min(p.shape) >= 64:
            w = p.detach().double().numpy()
            eig = np.linalg.svd(w, compute_uv=False) ** 2 / max(w.shape)
            out[name] = round(hill_tail_exponent(eig), 3)
    return out


def study(points: int = 1025, seeds: tuple[int, ...] = (0, 1, 2)) -> dict:
    torch.set_grad_enabled(False)
    trained, tok, _ = load_checkpoint(CHECKPOINT)
    # float64 throughout, so fine-scale slices are not float32 round-off (eps 1.2e-7 relative).
    trained = trained.double().eval()
    torch.manual_seed(0)
    random_init = AwenMini(trained.config).double().eval()
    splits = split_tokens(tok.encode(download_tiny_shakespeare()), 0.1)
    x, y = sample_batch(splits.val, 8, trained.config.block_size, torch.Generator().manual_seed(0))
    scales = [4, 8, 16, 32, 64, 128]
    slices = []
    for label, model in (("trained", trained), ("random-init", random_init)):
        for seed in seeds:
            d = filter_normalized_direction(model, seed)
            for width in (1.0, 1e-2, 1e-4):
                a = np.linspace(-width, width, points)
                ls = loss_slice(model, x, y, d, a)
                slices.append({"model": label, "direction_seed": seed, "alpha_half_width": width,
                               "loss_range": [float(ls.min()), float(ls.max())],
                               "loss_span_over_float64_eps": float(np.ptp(ls) / (np.finfo(float).eps * abs(ls.mean()))),
                               "box_dimension": round(graph_dimension(a, ls, scales), 4)})
                print(json.dumps(slices[-1]), flush=True)
    return {"study": "awen.fractal-boxcount.v1", "checkpoint": "bundled awen-mini", "points_per_slice": points,
            "box_scales": scales, "slices": slices,
            "esd_hill_alpha": {"trained": esd_report(trained), "random-init": esd_report(random_init)},
            "dtype": "float64",
            "evidence": "measured on the bundled checkpoint, one fixed validation batch (8 x 128 chars), CPU"}


def main(argv=None):
    ap = argparse.ArgumentParser(description="Exploratory box-counting / ESD measurements")
    ap.add_argument("--out", default=str(Path(__file__).parent / "results"))
    a = ap.parse_args(argv)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    rep = study()
    (out / "boxcount.json").write_text(json.dumps(rep, indent=2) + "\n")
    print(json.dumps(rep["esd_hill_alpha"], indent=1))


if __name__ == "__main__":
    main()
