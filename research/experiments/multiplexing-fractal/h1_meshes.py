"""F1: self-similar (butterfly / FFT-like) MZI meshes versus rectangular Clements meshes.

A butterfly stage of stride ``2**s`` couples modes ``i`` and ``i + 2**s`` (bit ``s`` of
``i`` clear). One butterfly *layer* is ``log2 N`` stages of strides ``1, 2, 4, …``:
the recursive structure of the radix-2 FFT, where each half of the network is a
copy of the whole network at half the size. A Clements column couples adjacent
modes ``(0,1),(2,3),…`` or ``(1,2),(3,4),…`` alternately (Clements et al. 2016,
doi:10.1364/OPTICA.3.001460).

Everything here is a lossless-unitary model plus a dB loss bookkeeping model; it
says nothing about a fabricated chip. Each 2×2 unit uses the same convention as
``awen_physics.clements.mzi``: ``T(θ, φ) = [[e^{iφ}cosθ, −sinθ], [e^{iφ}sinθ, cosθ]]``.

Fidelity of an implemented ``U`` against a target ``V``: ``F = |Tr(V† U)|² / N²``
(1 means equal up to a global phase).
"""

from __future__ import annotations

import argparse
import json
import math
import sys
from dataclasses import dataclass
from pathlib import Path

import numpy as np
import torch

Column = list[tuple[int, int]]


# --------------------------------------------------------------------------- topologies
def butterfly_columns(n: int, layers: int = 1) -> list[Column]:
    p = int(round(math.log2(n)))
    if 2 ** p != n:
        raise ValueError("butterfly meshes need N = 2**p modes")
    cols: list[Column] = []
    for _ in range(layers):
        for s in range(p):
            stride = 2 ** s
            cols.append([(i, i + stride) for i in range(n) if not (i >> s) & 1])
    return cols


def clements_columns(n: int, depth: int) -> list[Column]:
    return [[(i, i + 1) for i in range(c % 2, n - 1, 2)] for c in range(depth)]


def mzi_count(cols: list[Column]) -> int:
    return sum(len(c) for c in cols)


def crossings_per_column(n: int, col: Column) -> list[int]:
    """Waveguide crossings each mode passes to bring every pair of ``col`` side by side and back.

    The planar layout routes the modes through a permutation that places pair partners adjacent
    (partners keep their order; pairs ordered by first mode, unpaired modes keep their slot
    order) and back through its inverse. A waveguide crosses each other waveguide whose relative
    order flips, once on the way in and once on the way out.
    """
    paired = {m for pair in col for m in pair}
    slots: list[int] = []
    for i in range(n):
        if i in paired:
            if any(a == i for a, _ in col):
                b = next(b for a, b in col if a == i)
                slots += [i, b]
        else:
            slots.append(i)
    pos = {m: k for k, m in enumerate(slots)}
    out = []
    for i in range(n):
        flips = sum(1 for j in range(n) if j != i and (i < j) != (pos[i] < pos[j]))
        out.append(2 * flips)
    return out


@dataclass(frozen=True)
class MeshBudget:
    name: str
    modes: int
    depth: int
    mzis: int
    max_mzis_per_path: int
    max_crossings_per_path: int

    def path_loss_db(self, mzi_db: float, crossing_db: float) -> float:
        """Worst-path insertion loss for user-supplied per-MZI and per-crossing losses (dB)."""
        return self.max_mzis_per_path * mzi_db + self.max_crossings_per_path * crossing_db


def budget(name: str, n: int, cols: list[Column]) -> MeshBudget:
    mz = [0] * n
    cr = [0] * n
    for col in cols:
        for a, b in col:
            mz[a] += 1
            mz[b] += 1
        for m, c in enumerate(crossings_per_column(n, col)):
            cr[m] += c
    return MeshBudget(name, n, len(cols), mzi_count(cols), max(mz), max(cr))


# --------------------------------------------------------------------------- simulation
def haar_unitary(n: int, rng: np.random.Generator) -> np.ndarray:
    """Haar-random U(n) by QR with phase correction (Mezzadri, Notices AMS 54, 592 (2007))."""
    z = (rng.standard_normal((n, n)) + 1j * rng.standard_normal((n, n))) / math.sqrt(2)
    q, r = np.linalg.qr(z)
    d = np.diag(r)
    return q * (d / abs(d))


def mesh_unitary(n: int, cols: list[Column], theta: torch.Tensor, phi: torch.Tensor,
                 out_phase: torch.Tensor) -> torch.Tensor:
    """Batched mesh unitary ``[B, N, N]``; ``theta, phi: [B, mzis]``, ``out_phase: [B, N]``."""
    batch = theta.shape[0]
    eye = torch.eye(n, dtype=torch.complex128).expand(batch, n, n)
    rows = [eye[:, i, :] for i in range(n)]
    k = 0
    for col in cols:
        for a, b in col:
            c = torch.cos(theta[:, k]).to(torch.complex128)[:, None]
            s = torch.sin(theta[:, k]).to(torch.complex128)[:, None]
            e = torch.exp(1j * phi[:, k].to(torch.complex128))[:, None]
            ra, rb = rows[a], rows[b]
            rows[a], rows[b] = e * c * ra - s * rb, e * s * ra + c * rb
            k += 1
    u = torch.stack(rows, dim=1)
    return torch.exp(1j * out_phase.to(torch.complex128))[:, :, None] * u


def fidelity(target: torch.Tensor, u: torch.Tensor) -> torch.Tensor:
    n = u.shape[-1]
    tr = torch.einsum("bij,bij->b", target.conj(), u)
    return (tr.abs() ** 2) / n ** 2


def fit(n: int, cols: list[Column], targets: np.ndarray, steps: int = 800, restarts: int = 3,
        seed: int = 0, lr: float = 0.05) -> dict:
    """Best fidelity per target over random restarts (Adam on all phases); returns phases too."""
    g = torch.Generator().manual_seed(seed)
    t = torch.as_tensor(targets, dtype=torch.complex128).repeat(restarts, 1, 1)
    m, b = mzi_count(cols), t.shape[0]
    theta = (torch.rand(b, m, generator=g, dtype=torch.float64) * math.pi).requires_grad_()
    phi = (torch.rand(b, m, generator=g, dtype=torch.float64) * 2 * math.pi).requires_grad_()
    out = (torch.rand(b, n, generator=g, dtype=torch.float64) * 2 * math.pi).requires_grad_()
    opt = torch.optim.Adam([theta, phi, out], lr=lr)
    for _ in range(steps):
        loss = (1 - fidelity(t, mesh_unitary(n, cols, theta, phi, out))).sum()
        opt.zero_grad()
        loss.backward()
        opt.step()
    with torch.no_grad():
        f = fidelity(t, mesh_unitary(n, cols, theta, phi, out)).reshape(restarts, -1)
        best = f.argmax(0)
        idx = best * len(targets) + torch.arange(len(targets))
    return {"fidelity": f.max(0).values.numpy(), "theta": theta.detach()[idx], "phi": phi.detach()[idx],
            "out": out.detach()[idx]}


def phase_noise_infidelity(n: int, cols: list[Column], theta, phi, out, sigma: float, draws: int,
                           seed: int = 0) -> float:
    """Mean ``1 − F(U_noisy, U_ideal)`` with i.i.d. Gaussian errors of std ``sigma`` on every θ and φ."""
    g = torch.Generator().manual_seed(seed)
    ideal = mesh_unitary(n, cols, theta, phi, out)
    vals = []
    for _ in range(draws):
        dt = torch.randn(theta.shape, generator=g, dtype=torch.float64) * sigma
        dp = torch.randn(phi.shape, generator=g, dtype=torch.float64) * sigma
        vals.append(1 - fidelity(ideal, mesh_unitary(n, cols, theta + dt, phi + dp, out)))
    return float(torch.cat(vals).mean())


def first_order_infidelity(n: int, mzis: int, theta: torch.Tensor, sigma: float) -> float:
    """Small-σ prediction ``1 − F ≈ σ² Σ_k Var_k`` (derived in README §F1).

    An error δ on the φ of a unit acting on modes (a, b) multiplies mode a by e^{iδ}: generator
    P_a, contributing (1/N)(1 − 1/N). An error δ on θ generates a real rotation in the (a, b)
    plane, whose generator has trace 0 and normalized Hilbert–Schmidt norm 2/N, contributing 2/N.
    Summed: σ² · mzis · (1/N − 1/N² + 2/N). Independent of θ, kept as an argument for clarity.
    """
    del theta
    return sigma ** 2 * mzis * ((1 / n) * (1 - 1 / n) + 2 / n)


# --------------------------------------------------------------------------- study
def study(n: int = 8, targets: int = 16, steps: int = 800, restarts: int = 3, seed: int = 0,
          sigmas: tuple[float, ...] = (0.005, 0.01, 0.02, 0.05), noise_draws: int = 8) -> dict:
    rng = np.random.default_rng(seed)
    tg = np.stack([haar_unitary(n, rng) for _ in range(targets)])
    p = int(math.log2(n))
    configs = [("clements", d, clements_columns(n, d)) for d in range(p, n + 1)]
    configs += [("butterfly", L * p, butterfly_columns(n, L)) for L in range(1, n // p + 2)]
    rows = []
    for name, depth, cols in configs:
        r = fit(n, cols, tg, steps, restarts, seed)
        b = budget(name, n, cols)
        f = r["fidelity"]
        row = {"mesh": name, "modes": n, "depth": depth, "mzis": b.mzis,
               "real_parameters": 2 * b.mzis + n, "max_mzis_per_path": b.max_mzis_per_path,
               "max_crossings_per_path": b.max_crossings_per_path,
               "fidelity_mean": round(float(f.mean()), 6), "fidelity_min": round(float(f.min()), 6),
               "fraction_fidelity_ge_0.99": round(float((f >= 0.99).mean()), 4),
               "fraction_fidelity_ge_0.999": round(float((f >= 0.999).mean()), 4)}
        noise = {}
        for s in sigmas:
            noise[str(s)] = {"measured": round(phase_noise_infidelity(n, cols, r["theta"], r["phi"], r["out"], s,
                                                                         noise_draws, seed), 8),
                             "first_order_prediction": round(first_order_infidelity(n, b.mzis, r["theta"], s), 8)}
        row["phase_noise_infidelity"] = noise
        rows.append(row)
        print(json.dumps({k: v for k, v in row.items() if k != "phase_noise_infidelity"}), flush=True)
    return {"study": "awen.fractal-meshes.v1", "modes": n, "haar_targets": targets, "optimizer": "Adam lr 0.05",
            "steps": steps, "restarts": restarts, "seed": seed, "rows": rows,
            "evidence": "lossless unitary simulation (float64); loss figures are bookkeeping, not measured"}


# --------------------------------------------------------------------------- registered run (--config)
PILOT_SEEDS = {0, 1, 2}
H1_REQUIRED = ("id", "modes", "meshes", "tasks", "primary_point", "free_parameters", "optimizer",
               "fabrication_noise_seeds", "seeds", "analysis")
# Families and tasks this entry point can run. Anything registered but missing here is listed under
# "not_built" in the report instead of being skipped silently.
BUILT_FAMILIES = ("butterfly", "clements", "low_rank_svd")
BUILT_TASKS = ("haar_unitary", "dft_matrix", "mnist_pca16_linear")
BASELINES = ("clements", "low_rank_svd")


def load_config(path) -> dict:
    """Read and check a registered H1 config (``configs/h1.yaml``)."""
    import yaml

    cfg = yaml.safe_load(Path(path).read_text())
    missing = [k for k in H1_REQUIRED if k not in cfg]
    if missing:
        raise ValueError(f"H1 config missing keys: {missing}")
    if cfg["id"] != "H1":
        raise ValueError(f"not an H1 config (id={cfg['id']!r})")
    for key in ("seeds", "fabrication_noise_seeds"):
        if PILOT_SEEDS & set(cfg[key]):
            raise ValueError(f"{key} reuse pilot seeds {sorted(PILOT_SEEDS & set(cfg[key]))}")
    if set(cfg["seeds"]) & set(cfg["fabrication_noise_seeds"]):
        raise ValueError("initialization and fabrication-noise seeds overlap")
    for n in cfg["modes"]:
        if n < 2 or n & (n - 1):
            raise ValueError(f"modes must be powers of 2, got {n}")
    pp, fp = cfg["primary_point"], cfg["free_parameters"]
    if pp["modes"] not in cfg["modes"]:
        raise ValueError("primary_point.modes not in modes")
    for key, grid in (("il_mzi_db", "il_mzi_db"), ("il_crossing_db", "il_crossing_db"),
                      ("budget_db", "loss_budgets_db"), ("phase_sigma_rad", "phase_sigma_rad"),
                      ("splitter_imbalance", "splitter_imbalance")):
        if pp[key] not in fp[grid]:
            raise ValueError(f"primary_point.{key}={pp[key]} not in free_parameters.{grid}")
    return cfg


def family_columns(family: str, n: int, depth: int) -> list[Column]:
    """``depth`` is in butterfly layers (log2 N stages each) for butterfly, in columns for Clements.
    For ``low_rank_svd`` it is the rank r and the columns are the input stage then the output stage."""
    if family == "butterfly":
        return butterfly_columns(n, depth)
    if family == "clements":
        return clements_columns(n, depth)
    if family == "low_rank_svd":
        return low_rank_stage_columns(n, depth) + low_rank_stage_columns(n, depth)[::-1]
    raise ValueError(f"mesh family {family!r} not built")


def family_depths(cfg: dict, family: str) -> list[int]:
    spec = cfg["meshes"][family]
    if family == "butterfly":
        return list(spec["depths_in_layers_of_log2N_stages"])
    return list(spec["ranks"] if family == "low_rank_svd" else spec["depths"])


# --------------------------------------------------------------------------- low-rank SVD mesh
def low_rank_stage_columns(n: int, r: int) -> list[Column]:
    """Reck-style triangle that sets the first ``r`` rows of an N-mode unitary: for row k < r, MZIs on
    (N-2, N-1), ..., (k, k+1), one per column. ``r N - r (r + 1) / 2`` MZIs, adjacent pairs, no crossings.
    Reversed, the same list sets the first ``r`` columns (output stage)."""
    if not 1 <= r < n:
        raise ValueError(f"rank must be in [1, {n - 1}], got {r}")
    return [[(j, j + 1)] for k in range(r) for j in range(n - 2, k - 1, -1)]


def low_rank_budget(n: int, r: int) -> MeshBudget:
    """MZI count and worst per-mode path of input stage + r attenuator MZIs (modes < r) + output stage.
    Same per-mode proxy as ``budget``: the attenuators couple mode i to a dump port, adding one MZI."""
    stage = low_rank_stage_columns(n, r)
    per_mode = [0] * n
    for col in stage + stage[::-1]:
        for a, b in col:
            per_mode[a] += 1
            per_mode[b] += 1
    for i in range(r):
        per_mode[i] += 1
    return MeshBudget("low_rank_svd", n, r, 2 * mzi_count(stage) + r, max(per_mode), 0)


def family_budget(family: str, n: int, depth: int) -> MeshBudget:
    if family == "low_rank_svd":
        return low_rank_budget(n, depth)
    return budget(family, n, family_columns(family, n, depth))


def init_params(family: str, n: int, depth: int, batch: int, g: torch.Generator) -> list[torch.Tensor]:
    """Uniform random phases. Meshes: [theta, phi, out] (same draw order as ``fit``). Low rank: input
    stage [theta, phi, out], attenuators [theta, phi], output stage [theta, phi, out]."""
    def draw(*shape, scale):
        return (torch.rand(batch, *shape, generator=g, dtype=torch.float64) * scale).requires_grad_()

    if family != "low_rank_svd":
        m = mzi_count(family_columns(family, n, depth))
        return [draw(m, scale=math.pi), draw(m, scale=2 * math.pi), draw(n, scale=2 * math.pi)]
    k = mzi_count(low_rank_stage_columns(n, depth))
    stage = lambda: [draw(k, scale=math.pi), draw(k, scale=2 * math.pi), draw(n, scale=2 * math.pi)]  # noqa: E731
    return stage() + [draw(depth, scale=math.pi), draw(depth, scale=2 * math.pi)] + stage()


def implemented(family: str, n: int, depth: int, params, phase_sigma: float = 0.0, imbalance: float = 0.0,
                seed: int | None = None) -> torch.Tensor:
    """Matrix ``[B, N, N]`` the mesh applies; ideal if ``seed`` is None, else one fabrication draw.
    Low rank: ``M = U2[:, :r] diag(a) U1[:r, :]`` with attenuator amplitude ``a`` = element [0, 0] of the
    two-coupler MZI. Fabrication draws for U1, the attenuators and U2 use seeds seed, seed + 7e6, seed + 14e6."""
    def unitary(cols, th, ph, out, s):
        if s is None:
            return mesh_unitary(n, cols, th, ph, out)
        return fabricated_unitary(n, cols, th, ph, out, phase_sigma, imbalance, s)

    if family != "low_rank_svd":
        return unitary(family_columns(family, n, depth), *params, seed)
    r, stage = depth, low_rank_stage_columns(n, depth)
    u1 = unitary(stage, *params[0:3], seed)
    u2 = unitary(stage[::-1], *params[5:8], None if seed is None else seed + 7_000_000)
    th, ph = params[3], params[4]
    if seed is None:
        a = torch.exp(1j * ph.to(torch.complex128)) * torch.cos(th).to(torch.complex128)
    else:
        g = torch.Generator().manual_seed(seed + 14_000_000)
        th = th + torch.randn(th.shape, generator=g, dtype=torch.float64) * phase_sigma
        ph = ph + torch.randn(ph.shape, generator=g, dtype=torch.float64) * phase_sigma
        eta = (torch.randn((*th.shape, 2), generator=g, dtype=torch.float64) * imbalance).clamp(-0.49, 0.49)
        a = physical_mzi(th, ph, eta[..., 0], eta[..., 1])[..., 0, 0]
    return u2[:, :, :r] @ (a[:, :, None] * u1[:, :r, :])


def fit_family(family: str, n: int, depth: int, objective, batch: int, extra: int, opt: dict, seed: int):
    """Adam on all phases (+ ``extra`` real readout parameters per batch row, initialised to 0) minimising
    ``objective(M, extra_params) -> [batch]``; returns parameters and the final objective."""
    g = torch.Generator().manual_seed(seed)
    params = init_params(family, n, depth, batch, g)
    ext = torch.zeros(batch, extra, dtype=torch.float64, requires_grad=True)
    adam = torch.optim.Adam(params + [ext], lr=opt["lr"])
    for _ in range(opt["steps"]):
        loss = objective(implemented(family, n, depth, params), ext).sum()
        adam.zero_grad()
        loss.backward()
        adam.step()
    with torch.no_grad():
        final = objective(implemented(family, n, depth, params), ext)
    return [p.detach() for p in params], ext.detach(), final


# --------------------------------------------------------------------------- MNIST-PCA16 task
def mnist_pca(features: int, data: dict | None = None) -> dict:
    """PCA of MNIST to ``features`` dims, fit on the training split only (train mean removed, top
    eigenvectors of the train covariance, sign fixed so each component's largest-|.| entry is positive),
    then every sample scaled to unit L2 norm (amplitude encoding into ``features`` modes)."""
    if data is None:
        sys.path.insert(0, str(Path(__file__).resolve().parents[3] / "awen-models"))
        from awen_models.photon_training import load_mnist

        data = load_mnist()
    tx = data["train_x"].to(torch.float64)
    mean = tx.mean(0)
    cov = (tx - mean).T @ (tx - mean) / (len(tx) - 1)
    _, vecs = torch.linalg.eigh(cov)
    comp = vecs[:, -features:].flip(1)
    comp = comp * torch.sign(comp.gather(0, comp.abs().argmax(0, keepdim=True)))
    proj = lambda x: torch.nn.functional.normalize((x.to(torch.float64) - mean) @ comp, dim=1)  # noqa: E731
    return {"train_x": proj(data["train_x"]), "train_y": data["train_y"],
            "test_x": proj(data["test_x"]), "test_y": data["test_y"]}


def mnist_logits(m: torch.Tensor, x: torch.Tensor, log_gain: torch.Tensor) -> torch.Tensor:
    """One photonic layer plus detection: intensities ``|M x|^2`` of output modes 0-9, times an electronic
    gain ``10 * exp(log_gain)`` (the only non-photonic parameter). ``m: [B, N, N]``, ``x: [S, N]``."""
    y = torch.einsum("bij,sj->bsi", m, x.to(torch.complex128))
    return (y.abs() ** 2)[:, :, :10] * (10 * torch.exp(log_gain))[:, :, None]


def deepest_within_budget(budgets: dict[int, MeshBudget], il_mzi_db: float, il_crossing_db: float,
                          budget_db: float) -> int | None:
    """Deepest configured depth whose worst-path loss is at most ``budget_db`` (None if none fits)."""
    ok = [d for d, b in budgets.items() if b.path_loss_db(il_mzi_db, il_crossing_db) <= budget_db + 1e-12]
    return max(ok) if ok else None


def coupler(alpha: torch.Tensor) -> torch.Tensor:
    """Lossless directional coupler ``[[cos a, i sin a], [i sin a, cos a]]``; 50:50 at ``a = pi/4``."""
    c, s = torch.cos(alpha).to(torch.complex128), torch.sin(alpha).to(torch.complex128)
    return torch.stack([torch.stack([c, 1j * s], -1), torch.stack([1j * s, c], -1)], -2)


def physical_mzi(theta: torch.Tensor, phi: torch.Tensor, eta_in: torch.Tensor, eta_out: torch.Tensor) -> torch.Tensor:
    """Two-coupler MZI ``[..., 2, 2]`` whose couplers have power split ``0.5 + eta``.

    ``U = g * C(a2) diag(e^{i d}, 1) C(a1) diag(e^{i phi}, -1)`` with ``d = pi - 2 theta``,
    ``g = -i e^{-i d / 2}`` and ``sin^2 a = 0.5 + eta``. At ``eta = 0`` this equals the
    ``T(theta, phi)`` convention used by ``mesh_unitary`` exactly (checked in the tests).
    """
    a1 = torch.asin(torch.sqrt(0.5 + eta_in))
    a2 = torch.asin(torch.sqrt(0.5 + eta_out))
    d = (math.pi - 2 * theta).to(torch.complex128)
    zero = torch.zeros_like(d)
    inner = torch.diag_embed(torch.stack([torch.exp(1j * d), torch.exp(zero)], -1))
    pin = torch.diag_embed(torch.stack([torch.exp(1j * phi.to(torch.complex128)), -torch.exp(zero)], -1))
    g = (-1j * torch.exp(-0.5j * d))[..., None, None]
    return g * coupler(a2) @ inner @ coupler(a1) @ pin


def fabricated_unitary(n: int, cols: list[Column], theta, phi, out, phase_sigma: float, imbalance: float,
                       seed: int) -> torch.Tensor:
    """Mesh unitary with one fabrication draw: i.i.d. N(0, phase_sigma) errors on every phase shifter
    (theta, phi, output phases) and coupler split errors eta ~ N(0, imbalance), clipped to |eta| <= 0.49."""
    g = torch.Generator().manual_seed(seed)
    batch, m = theta.shape
    t = theta + torch.randn(theta.shape, generator=g, dtype=torch.float64) * phase_sigma
    p = phi + torch.randn(phi.shape, generator=g, dtype=torch.float64) * phase_sigma
    o = out + torch.randn(out.shape, generator=g, dtype=torch.float64) * phase_sigma
    eta = (torch.randn((batch, m, 2), generator=g, dtype=torch.float64) * imbalance).clamp(-0.49, 0.49)
    units = physical_mzi(t, p, eta[..., 0], eta[..., 1])  # [B, m, 2, 2]
    rows = [torch.eye(n, dtype=torch.complex128).expand(batch, n, n)[:, i, :] for i in range(n)]
    k = 0
    for col in cols:
        for a, b in col:
            u = units[:, k]
            ra, rb = rows[a], rows[b]
            rows[a] = u[:, 0, 0, None] * ra + u[:, 0, 1, None] * rb
            rows[b] = u[:, 1, 0, None] * ra + u[:, 1, 1, None] * rb
            k += 1
    return torch.exp(1j * o.to(torch.complex128))[:, :, None] * torch.stack(rows, dim=1)


def dft_unitary(n: int) -> np.ndarray:
    k = np.arange(n)
    return np.exp(-2j * math.pi * np.outer(k, k) / n) / math.sqrt(n)


def task_targets(task: dict, n: int, seed: int) -> np.ndarray:
    if task["name"] == "haar_unitary":
        rng = np.random.default_rng(seed)
        return np.stack([haar_unitary(n, rng) for _ in range(task["targets"])])
    if task["name"] == "dft_matrix":
        return dft_unitary(n)[None]
    raise ValueError(f"task {task['name']!r} not built")


def t_quantile(p: float, df: int) -> float:
    """Student-t quantile by bisection on a numerically integrated CDF (no scipy on the build box)."""
    c = math.gamma((df + 1) / 2) / (math.sqrt(df * math.pi) * math.gamma(df / 2))

    def cdf(x: float) -> float:
        xs = np.linspace(0.0, x, 20001)
        ys = c * (1 + xs ** 2 / df) ** (-(df + 1) / 2)
        return 0.5 + float(np.sum((ys[1:] + ys[:-1]) * np.diff(xs)) / 2)

    lo, hi = 0.0, 100.0
    for _ in range(80):
        mid = (lo + hi) / 2
        lo, hi = (mid, hi) if cdf(mid) < p else (lo, mid)
    return (lo + hi) / 2


def paired_t(diffs) -> dict:
    """95% two-sided paired-t interval of per-seed differences, Cohen's d_z."""
    d = np.asarray(diffs, dtype=float)
    n = len(d)
    mean, sd = float(d.mean()), float(d.std(ddof=1)) if n > 1 else float("nan")
    half = t_quantile(0.975, n - 1) * sd / math.sqrt(n) if n > 1 else float("nan")
    return {"n": n, "mean": mean, "ci95": [mean - half, mean + half],
            "d_z": mean / sd if sd and sd > 0 else None, "beats": bool(n > 1 and mean - half > 0)}


def score_seed(task: dict, family: str, n: int, depth: int, seed: int, cfg: dict, mnist: dict | None) -> float:
    """Fit without noise from init seed ``seed``, then mean score over the fabrication draws.
    Unitary tasks: best restart per target by fidelity. MNIST: best restart by final train loss."""
    pp, opt = cfg["primary_point"], cfg["optimizer"]
    restarts = opt["restarts"]
    fab_seeds = [f * 1000 + seed for f in cfg["fabrication_noise_seeds"]]
    if task["name"] == "mnist_pca16_linear":
        tx, ty = mnist["train_x"], mnist["train_y"]
        ce = lambda m, ext: torch.stack([torch.nn.functional.cross_entropy(z, ty)  # noqa: E731
                                         for z in mnist_logits(m, tx, ext)])
        params, ext, final = fit_family(family, n, depth, ce, restarts, 1, opt, seed)
        b = int(final.argmin())
        best = [p[b:b + 1] for p in params]
        accs = [float((mnist_logits(implemented(family, n, depth, best, pp["phase_sigma_rad"],
                                                pp["splitter_imbalance"], fs), mnist["test_x"], ext[b:b + 1])[0]
                       .argmax(1) == mnist["test_y"]).double().mean()) for fs in fab_seeds]
        return float(np.mean(accs))
    tg = torch.as_tensor(task_targets(task, n, seed), dtype=torch.complex128)
    k = len(tg)
    target = tg.repeat(restarts, 1, 1)
    params, _, final = fit_family(family, n, depth, lambda m, ext: 1 - fidelity(target, m), restarts * k, 0, opt, seed)
    idx = final.reshape(restarts, k).argmin(0) * k + torch.arange(k)
    best = [p[idx] for p in params]
    return float(np.mean([float(fidelity(tg, implemented(family, n, depth, best, pp["phase_sigma_rad"],
                                                         pp["splitter_imbalance"], fs)).mean()) for fs in fab_seeds]))


def registered_run(cfg: dict, mnist_raw: dict | None = None) -> dict:
    """Score every built family/depth/task at the primary noise point; select per loss budget; compare
    butterfly with each baseline; apply the registered kill rule. ``mnist_raw`` overrides MNIST (tests)."""
    pp = cfg["primary_point"]
    scores: dict = {}  # scores[task][modes][family][depth] = per-init-seed mean over fabrication seeds
    skipped: dict = {}
    budgets = {n: {f: {d: family_budget(f, n, d) for d in family_depths(cfg, f)} for f in BUILT_FAMILIES}
               for n in cfg["modes"]}
    mnist = None
    for task in cfg["tasks"]:
        if task["name"] not in BUILT_TASKS:
            continue
        if task["name"] == "mnist_pca16_linear":
            mnist = mnist_pca(task["features"], mnist_raw)
        for n in cfg["modes"]:
            if task["name"] == "mnist_pca16_linear" and n != task["features"]:
                skipped.setdefault(task["name"], {})[n] = f"not applicable: {task['features']} features need {task['features']} modes"
                continue
            for f in BUILT_FAMILIES:
                for d in family_depths(cfg, f):
                    per_seed = [score_seed(task, f, n, d, s, cfg, mnist) for s in cfg["seeds"]]
                    scores.setdefault(task["name"], {}).setdefault(n, {}).setdefault(f, {})[d] = per_seed
    sel = {f: deepest_within_budget(budgets[pp["modes"]][f], pp["il_mzi_db"], pp["il_crossing_db"], pp["budget_db"])
           for f in BUILT_FAMILIES}
    comparisons, beats_all = {}, []
    for name, by_n in scores.items():
        fam = by_n.get(pp["modes"])
        if fam is None or any(sel[f] is None for f in BUILT_FAMILIES):
            comparisons[name] = {"status": "no configuration within budget or task not run at primary modes",
                                 "selected": sel}
            beats_all.append(None)
            continue
        comparisons[name] = {"selected": sel}
        for base in BASELINES:
            comparisons[name][f"butterfly_minus_{base}"] = paired_t(
                np.subtract(fam["butterfly"][sel["butterfly"]], fam[base][sel[base]]))
        beats_all.append(all(comparisons[name][f"butterfly_minus_{b}"]["beats"] for b in BASELINES))
    not_built = {"families": [f for f in cfg["meshes"] if f not in BUILT_FAMILIES],
                 "tasks": [t["name"] for t in cfg["tasks"] if t["name"] not in BUILT_TASKS]}
    if not_built["families"] or not_built["tasks"] or None in beats_all or len(beats_all) < len(cfg["tasks"]):
        verdict = "not decided: a registered family or task is missing or has no configuration within budget"
    elif all(beats_all):
        verdict = "kept: butterfly beats Clements and low-rank on every task at the primary point"
    else:
        lost = [t for t, b in zip(scores, beats_all) if not b]
        verdict = f"killed: butterfly does not beat both baselines on {lost}"
    return {
        "study": "awen.fractal-h1-registered.v2", "id": "H1", "primary_point": pp,
        "budgets": {n: {f: {d: {"mzis": b.mzis, "max_mzis_per_path": b.max_mzis_per_path,
                                "max_crossings_per_path": b.max_crossings_per_path,
                                "loss_db_primary": b.path_loss_db(pp["il_mzi_db"], pp["il_crossing_db"])}
                            for d, b in fb.items()} for f, fb in fam.items()} for n, fam in budgets.items()},
        "scores_per_init_seed": scores, "skipped": skipped, "comparisons_at_primary_point": comparisons,
        "not_built": not_built, "verdict": verdict,
        "evidence": "L1 simulation (float64); loss is bookkeeping with free parameters, not a device prediction",
    }


def main(argv=None):
    ap = argparse.ArgumentParser(description="Butterfly vs Clements (and, with --config, low-rank SVD) MZI mesh study")
    ap.add_argument("--modes", type=int, nargs="+", default=[8])
    ap.add_argument("--targets", type=int, default=16)
    ap.add_argument("--steps", type=int, default=800)
    ap.add_argument("--restarts", type=int, default=3)
    ap.add_argument("--out", default=None)
    ap.add_argument("--config", default=None, help="registered H1 config (configs/h1.yaml)")
    a = ap.parse_args(argv)
    if a.config:
        out = Path(a.out or Path(__file__).parent / "results" / "h1")
        out.mkdir(parents=True, exist_ok=True)
        rep = registered_run(load_config(a.config))
        (out / "h1_meshes_registered.json").write_text(json.dumps(rep, indent=2) + "\n")
        return
    out = Path(a.out or Path(__file__).parent / "results" / "pilot")
    out.mkdir(parents=True, exist_ok=True)
    for n in a.modes:
        rep = study(n, a.targets, a.steps, a.restarts)
        (out / f"meshes_n{n}.json").write_text(json.dumps(rep, indent=2) + "\n")


if __name__ == "__main__":
    main()
