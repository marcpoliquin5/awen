"""H4 registered-run entry point: Fourier (FNO) and Haar-wavelet layers vs dense at matched FLOPs.

Definitions fixed in PREREGISTRATION.md, section H4 (definitions block, 2026-10-26), before any H4
full run. Evidence level: L0 (CPU training, math reference). No device, no energy, no speed claim.

* **Tasks** (``configs/h4.yaml``): 1-D viscous Burgers surrogate u(t=0) -> u(t_final) (integrating-
  factor RK4 pseudo-spectral solver, 2/3 dealiasing, solved on ``solver_grid`` and subsampled to
  ``grid``); 1-D denoising of a sum of 3 linear chirps in white noise; MNIST 28x28 classification.
  Data come from ``data_seed`` only; training seeds change initialization and batch order only.
* **FLOPs per sample** (forward pass, analytic, :func:`flops`): 2 per real multiply-accumulate of
  every linear map; 8 per complex multiply-accumulate of the spectral weights; 2.5 n log2 n per
  real FFT or inverse real FFT of n points (n = H*W in 2-D); 2 per sample per axis pass of each orthonormal
  Haar level, forward or inverse. Biases, activations and pooling are not counted.
* **Compute match**: the dense MLP width is fixed in the config; FNO and wavelet widths are the
  integers whose FLOPs are nearest that dense count, and must fall within ``compute_match.tolerance``.
* **Verdict** (registered rule): candidate c beats dense on a task when the 95% paired-t interval
  over seeds of its improvement (dense error minus c error, or c accuracy minus dense accuracy) lies
  above 0. The hypothesis is killed if on any task neither candidate beats dense; otherwise kept.
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from pathlib import Path

import numpy as np
import torch
import torch.nn as nn

SCHEMA = "awen.fractal-prereg-config.v1"
MODELS = ["dense_mlp", "fno", "haar_wavelet"]
TASKS = ("burgers_1d", "multiscale_denoise_1d", "mnist_28x28")


def load_config(path) -> dict:
    import yaml

    cfg = yaml.safe_load(Path(path).read_text())
    if cfg.get("id") != "H4" or cfg.get("schema") != SCHEMA:
        raise ValueError(f"{path} is not an H4 config")
    if cfg["models"] != MODELS:
        raise ValueError(f"models must be {MODELS}, got {cfg['models']}")
    if tuple(t["name"] for t in cfg["tasks"]) != TASKS:
        raise ValueError(f"tasks must be {list(TASKS)}")
    if len(cfg["seeds"]) < 3 or cfg["compute_match"]["measure"] != "flops_per_sample":
        raise ValueError("need >= 3 seeds and compute_match.measure flops_per_sample")
    for t in cfg["tasks"]:
        n = t["shape"][-1]
        if any(s % 2 ** t["haar_levels"] for s in t["shape"]):
            raise ValueError(f"{t['name']}: shape {t['shape']} not divisible by 2^haar_levels")
        if 2 * t["fno_modes"] > n:
            raise ValueError(f"{t['name']}: fno_modes {t['fno_modes']} above Nyquist for {n} points")
    return cfg


# --------------------------------------------------------------------------- data
def burgers(n: int, t: dict, rng) -> tuple[np.ndarray, np.ndarray]:
    g, kmax = t["solver_grid"], t["ic_modes"]
    k = np.fft.rfftfreq(g, 1.0 / g)
    uh = np.zeros((n, k.size), complex)
    uh[:, 1:kmax + 1] = (rng.standard_normal((n, kmax)) + 1j * rng.standard_normal((n, kmax))) / np.arange(1, kmax + 1) ** 2
    u0 = np.fft.irfft(uh, n=g)
    u0 *= t["ic_amplitude"] / np.abs(u0).max(1, keepdims=True)
    uh, dt, mask = np.fft.rfft(u0), t["dt"], k < g / 3
    e = np.exp(-t["viscosity"] * k ** 2 * dt / 2)

    def nl(v):  # -(u^2/2)_x with 2/3 dealiasing
        return -0.5j * k * np.fft.rfft(np.fft.irfft(v * mask, n=g) ** 2)

    for _ in range(int(round(t["t_final"] / dt))):
        a = nl(uh); b = nl(e * (uh + dt / 2 * a)); c = nl(e * uh + dt / 2 * b); d = nl(e * e * uh + dt * e * c)
        uh = e * e * uh + dt / 6 * (e * e * a + 2 * e * (b + c) + d)
    step = g // t["grid"]
    return u0[:, ::step], np.fft.irfft(uh, n=g)[:, ::step]


def chirps(n: int, t: dict, rng) -> tuple[np.ndarray, np.ndarray]:
    L, (lo, hi) = t["length"], t["freq_range"]
    x = np.arange(L)
    clean = np.zeros((n, L))
    for _ in range(t["n_chirps"]):
        f0, f1 = rng.uniform(lo, hi, (2, n, 1))
        amp, ph = rng.uniform(*t["amp_range"], (n, 1)), rng.uniform(0, 2 * np.pi, (n, 1))
        clean += amp * np.sin(2 * np.pi * (f0 * x + (f1 - f0) * x ** 2 / (2 * L)) + ph)
    std = np.sqrt((clean ** 2).mean(1, keepdims=True) / 10 ** (t["snr_db"] / 10))
    return clean + std * rng.standard_normal((n, L)), clean


def make_data(t: dict, data_seed: int, mnist: dict | None = None) -> dict:
    if t["name"] == "mnist_28x28":
        if mnist is None:
            from awen_models.photon_training import load_mnist
            mnist = load_mnist()
        return {k: v.reshape(-1, 1, 28, 28) if k.endswith("_x") else v for k, v in mnist.items()}
    rng = np.random.default_rng([data_seed, TASKS.index(t["name"])])
    gen = burgers if t["name"] == "burgers_1d" else chirps
    x, y = gen(t["n_train"] + t["n_test"], t, rng)
    f = lambda a: torch.tensor(a, dtype=torch.float32).unsqueeze(1)  # noqa: E731  (N, 1, L)
    s = t["n_train"]
    return {"train_x": f(x[:s]), "train_y": f(y[:s]), "test_x": f(x[s:]), "test_y": f(y[s:])}


# --------------------------------------------------------------------------- models
def haar(x: torch.Tensor, levels: int, inverse: bool = False) -> torch.Tensor:
    """Orthonormal multi-level Haar transform over the last 1 or 2 axes (separable in 2-D). Each level
    transforms the current coarse corner block in place (Mallat layout: coarse half first per axis)."""
    dims, r, x = list(range(2, x.dim())), 1 / math.sqrt(2), x.clone()
    for lv in (range(levels - 1, -1, -1) if inverse else range(levels)):
        sl = (Ellipsis,) + tuple(slice(0, x.shape[d] >> lv) for d in dims)
        blk = x[sl]
        for d in dims:
            n = blk.shape[d]
            if inverse:
                a, b = blk.narrow(d, 0, n // 2), blk.narrow(d, n // 2, n // 2)
                blk = torch.stack([(a + b) * r, (a - b) * r], d + 1).flatten(d, d + 1)
            else:
                a, b = blk.unflatten(d, (n // 2, 2)).unbind(d + 1)
                blk = torch.cat([(a + b) * r, (a - b) * r], d)
        x[sl] = blk
    return x


def haar_bands(shape: tuple, levels: int) -> torch.Tensor:
    """Band of every flattened coefficient: 1 = finest detail ... levels = coarsest detail, 0 = coarse."""
    idx = torch.zeros(shape, dtype=torch.long)
    for lv in range(1, levels + 1):
        idx[tuple(slice(0, s >> (lv - 1)) for s in shape)] = lv
    idx[tuple(slice(0, s >> levels) for s in shape)] = 0
    return idx.flatten()


class Block(nn.Module):
    """FNO layer (truncated spectral weights) or Haar layer (one channel-mixing matrix per band),
    each plus a pointwise skip mix and GELU."""

    def __init__(self, kind: str, w: int, shape: tuple, modes: int, levels: int):
        super().__init__()
        self.kind, self.shape, self.modes, self.levels = kind, shape, modes, levels
        self.skip = nn.Conv1d(w, w, 1)
        if kind == "fno":
            nm = modes * (2 * modes if len(shape) == 2 else 1)
            self.wr = nn.Parameter(torch.randn(nm, w, w) / w)
            self.wi = nn.Parameter(torch.randn(nm, w, w) / w)
        else:
            self.mix = nn.Parameter(torch.randn(levels + 1, w, w) / math.sqrt(w))
            self.register_buffer("band", haar_bands(shape, levels), persistent=False)

    def forward(self, h: torch.Tensor) -> torch.Tensor:  # h: (B, w, *shape)
        b, w = h.shape[:2]
        if self.kind == "fno":
            d = tuple(range(2, h.dim()))
            f = torch.fft.rfftn(h, dim=d)
            m = self.modes
            sel = (f[..., :m] if len(d) == 1 else torch.cat([f[..., :m, :m], f[..., -m:, :m]], -2)).flatten(2)
            wt = torch.complex(self.wr, self.wi)
            o = torch.einsum("bik,kio->bok", sel, wt)
            out = torch.zeros_like(f)
            if len(d) == 1:
                out[..., :m] = o
            else:
                o = o.view(b, w, 2 * m, m)
                out[..., :m, :m], out[..., -m:, :m] = o[..., :m, :], o[..., m:, :]
            y = torch.fft.irfftn(out, s=self.shape, dim=d)
        else:
            c = haar(h, self.levels).flatten(2)
            c = torch.einsum("bik,kio->bok", c, self.mix[self.band].to(c.dtype))
            y = haar(c.view(b, w, *self.shape), self.levels, inverse=True)
        return nn.functional.gelu(y + self.skip(h.flatten(2)).view_as(h))


class Net(nn.Module):
    def __init__(self, kind: str, width: int, t: dict):
        super().__init__()
        self.kind, self.shape, self.classes = kind, tuple(t["shape"]), t.get("classes")
        n, out = math.prod(self.shape), t.get("classes") or math.prod(self.shape)
        if kind == "dense_mlp":
            dims = [n] + [width] * t["dense_hidden_layers"] + [out]
            mods = [m for a, z in zip(dims[:-1], dims[1:]) for m in (nn.Linear(a, z), nn.GELU())][:-1]
            self.body = nn.Sequential(*mods)
        else:
            self.lift = nn.Conv1d(1, width, 1)
            self.blocks = nn.ModuleList(Block(kind, width, self.shape, t["fno_modes"], t["haar_levels"])
                                        for _ in range(t["layers"]))
            self.head = nn.Linear(width, self.classes) if self.classes else nn.Conv1d(width, 1, 1)

    def forward(self, x: torch.Tensor) -> torch.Tensor:
        b = x.shape[0]
        if self.kind == "dense_mlp":
            return self.body(x.flatten(1)).view(b, *([self.classes] if self.classes else x.shape[1:]))
        h = self.lift(x.flatten(2)).view(b, -1, *self.shape)
        for blk in self.blocks:
            h = blk(h)
        return self.head(h.flatten(2).mean(2)) if self.classes else self.head(h.flatten(2)).view_as(x)


def flops(kind: str, width: int, t: dict) -> int:
    """Analytic forward FLOPs per sample under the module-docstring convention."""
    n, out = math.prod(t["shape"]), t.get("classes") or math.prod(t["shape"])
    if kind == "dense_mlp":
        dims = [n] + [width] * t["dense_hidden_layers"] + [out]
        return sum(2 * a * z for a, z in zip(dims[:-1], dims[1:]))
    w = width
    head = 2 * w * t["classes"] if t.get("classes") else 2 * n * w
    per = 2 * n * w * w  # pointwise skip mix
    if kind == "fno":
        nm = t["fno_modes"] * (2 * t["fno_modes"] if len(t["shape"]) == 2 else 1)
        per += 2 * w * 2.5 * n * math.log2(n) + 8 * nm * w * w
    else:
        dims = len(t["shape"])  # per level: 2 FLOPs per sample per axis pass, forward and inverse
        per += 2 * n * w * w + 2 * w * sum(2 * dims * n / 2 ** (dims * lv) for lv in range(t["haar_levels"]))
    return int(round(2 * n * w + t["layers"] * per + head))


def match_widths(t: dict, tol: float) -> dict:
    target = flops("dense_mlp", t["dense_width"], t)
    out = {"dense_mlp": {"width": t["dense_width"], "flops": target, "rel_diff": 0.0}}
    for kind in MODELS[1:]:
        w = min(range(1, 1025), key=lambda v: abs(flops(kind, v, t) - target))
        rel = flops(kind, w, t) / target - 1
        if abs(rel) > tol:
            raise ValueError(f"{t['name']}/{kind}: nearest width {w} is {rel:+.3f} from dense FLOPs")
        out[kind] = {"width": w, "flops": flops(kind, w, t), "rel_diff": rel}
    return out


# --------------------------------------------------------------------------- train, score, verdict
def train_score(kind: str, width: int, t: dict, data: dict, tr: dict, seed: int) -> float:
    torch.manual_seed(seed)
    model = Net(kind, width, t)
    opt = torch.optim.Adam(model.parameters(), lr=tr["lr"])
    cls = bool(t.get("classes"))
    lossf = nn.CrossEntropyLoss() if cls else nn.MSELoss()
    gen = torch.Generator().manual_seed(seed)
    x, y = data["train_x"], data["train_y"]
    for _ in range(tr["epochs"]):
        for idx in torch.randperm(len(x), generator=gen).split(tr["batch_size"]):
            opt.zero_grad()
            lossf(model(x[idx]), y[idx]).backward()
            opt.step()
    with torch.no_grad():
        p = torch.cat([model(c) for c in data["test_x"].split(1000)])
    ty = data["test_y"]
    if cls:
        return float((p.argmax(1) == ty).double().mean())
    return float(((p - ty).flatten(1).norm(dim=1) / ty.flatten(1).norm(dim=1)).mean())


def t975(df: int) -> float:
    """Two-sided 95% Student-t critical value by bisection on a trapezoid-integrated CDF."""
    c = math.gamma((df + 1) / 2) / (math.sqrt(df * math.pi) * math.gamma(df / 2))
    def cdf(x: float) -> float:
        xs = np.linspace(0.0, x, 20001)
        ys = c * (1 + xs ** 2 / df) ** (-(df + 1) / 2)
        return 0.5 + float(np.sum((ys[1:] + ys[:-1]) * np.diff(xs)) / 2)

    lo, hi = 0.0, 100.0
    for _ in range(80):
        mid = (lo + hi) / 2
        lo, hi = (mid, hi) if cdf(mid) < 0.975 else (lo, mid)
    return (lo + hi) / 2


def paired_t(diffs) -> dict:
    d = np.asarray(diffs, dtype=float)
    half = t975(len(d) - 1) * float(d.std(ddof=1)) / math.sqrt(len(d))
    return {"n": len(d), "mean": float(d.mean()), "ci95": [float(d.mean()) - half, float(d.mean()) + half]}


def verdict(rows: list[dict], tasks: list[dict]) -> dict:
    out = {}
    for t in tasks:
        r = {(x["model"], x["seed"]): x["score"] for x in rows if x["task"] == t["name"]}
        seeds = sorted(s for m, s in r if m == "dense_mlp")
        sign = 1 if t["score"] == "test_accuracy" else -1  # improvement > 0 means better than dense
        cands = {}
        for kind in MODELS[1:]:
            ci = paired_t([sign * (r[(kind, s)] - r[("dense_mlp", s)]) for s in seeds])
            cands[kind] = {"improvement_vs_dense": ci, "beats_dense": ci["ci95"][0] > 0}
        out[t["name"]] = {"candidates": cands, "killed": not any(c["beats_dense"] for c in cands.values())}
    return {"tasks": out, "verdict": "killed" if any(v["killed"] for v in out.values()) else "kept"}


def run_config(cfg: dict, mnist: dict | None = None) -> dict:
    from awen_models.train import hardware_record

    torch.set_num_threads(int(cfg.get("threads", 8)))
    rows, widths = [], {}
    for t in cfg["tasks"]:
        widths[t["name"]] = match_widths(t, cfg["compute_match"]["tolerance"])
        data = make_data(t, cfg["data_seed"], mnist)
        for kind in MODELS:
            for seed in cfg["seeds"]:
                score = train_score(kind, widths[t["name"]][kind]["width"], t, data, cfg["training"], seed)
                rows.append({"task": t["name"], "model": kind, "seed": seed, "score": score, "metric": t["score"]})
                print(json.dumps(rows[-1]), flush=True)
    return {"id": "H4", "config": cfg, "compute_match": widths, "runs": rows, "verdict": verdict(rows, cfg["tasks"]),
            "hardware": hardware_record(torch.device("cpu")),
            "evidence": "scores measured on the CPU above (L0); FLOPs counted analytically"}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="H4 registered run (configs/h4.yaml)")
    ap.add_argument("--config", required=True)
    ap.add_argument("--out", default="research/experiments/multiplexing-fractal/results/h4")
    ap.add_argument("--flops-only", action="store_true", help="print the matched widths and FLOPs, no training")
    a = ap.parse_args(argv)
    cfg = load_config(a.config)
    if a.flops_only:
        print(json.dumps({t["name"]: match_widths(t, cfg["compute_match"]["tolerance"]) for t in cfg["tasks"]}, indent=2))
        return 0
    rep = run_config(cfg)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "h4_registered.json").write_text(json.dumps(rep, indent=2) + "\n")
    print(json.dumps(rep["verdict"]["verdict"]))
    return 0


if __name__ == "__main__":
    sys.exit(main())
