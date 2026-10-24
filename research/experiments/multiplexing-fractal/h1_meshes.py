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
# Families and tasks this entry point can run so far. The low-rank SVD mesh and the MNIST task are
# registered but not built yet; a run lists them under "not_built" instead of skipping them silently.
BUILT_FAMILIES = ("butterfly", "clements")
BUILT_TASKS = ("haar_unitary", "dft_matrix")


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
    """``depth`` is in butterfly layers (log2 N stages each) for butterfly, in columns for Clements."""
    if family == "butterfly":
        return butterfly_columns(n, depth)
    if family == "clements":
        return clements_columns(n, depth)
    raise ValueError(f"mesh family {family!r} not built")


def family_depths(cfg: dict, family: str) -> list[int]:
    spec = cfg["meshes"][family]
    return list(spec["depths_in_layers_of_log2N_stages"] if family == "butterfly" else spec["depths"])


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


def registered_run(cfg: dict) -> dict:
    """Score every built family/depth/task at the primary noise point; select per loss budget; compare."""
    pp, opt = cfg["primary_point"], cfg["optimizer"]
    scores: dict = {}  # scores[task][modes][family][depth] = per-init-seed mean over fabrication seeds
    budgets: dict = {}
    for n in cfg["modes"]:
        budgets[n] = {f: {d: budget(f, n, family_columns(f, n, d)) for d in family_depths(cfg, f)}
                      for f in BUILT_FAMILIES}
    for task in cfg["tasks"]:
        if task["name"] not in BUILT_TASKS:
            continue
        for n in cfg["modes"]:
            for f in BUILT_FAMILIES:
                for d in family_depths(cfg, f):
                    cols, per_seed = family_columns(f, n, d), []
                    for s in cfg["seeds"]:
                        tg = task_targets(task, n, s)
                        r = fit(n, cols, tg, opt["steps"], opt["restarts"], s, opt["lr"])
                        target = torch.as_tensor(tg, dtype=torch.complex128)
                        fab = [float(fidelity(target, fabricated_unitary(
                            n, cols, r["theta"], r["phi"], r["out"], pp["phase_sigma_rad"],
                            pp["splitter_imbalance"], fs * 1000 + s)).mean()) for fs in cfg["fabrication_noise_seeds"]]
                        per_seed.append(float(np.mean(fab)))
                    scores.setdefault(task["name"], {}).setdefault(n, {}).setdefault(f, {})[d] = per_seed
    sel = {f: deepest_within_budget(budgets[pp["modes"]][f], pp["il_mzi_db"], pp["il_crossing_db"], pp["budget_db"])
           for f in BUILT_FAMILIES}
    comparisons = {}
    for name, by_n in scores.items():
        fam = by_n[pp["modes"]]
        if sel["butterfly"] is None or sel["clements"] is None:
            comparisons[name] = {"status": "no configuration within budget", "selected": sel}
            continue
        diffs = np.subtract(fam["butterfly"][sel["butterfly"]], fam["clements"][sel["clements"]])
        comparisons[name] = {"selected": sel, "butterfly_minus_clements": paired_t(diffs)}
    return {
        "study": "awen.fractal-h1-registered.v1", "id": "H1", "primary_point": pp,
        "budgets": {n: {f: {d: {"mzis": b.mzis, "max_mzis_per_path": b.max_mzis_per_path,
                                "max_crossings_per_path": b.max_crossings_per_path,
                                "loss_db_primary": b.path_loss_db(pp["il_mzi_db"], pp["il_crossing_db"])}
                            for d, b in fb.items()} for f, fb in fam.items()} for n, fam in budgets.items()},
        "scores_per_init_seed": scores, "comparisons_at_primary_point": comparisons,
        "not_built": {"families": [f for f in cfg["meshes"] if f not in BUILT_FAMILIES],
                      "tasks": [t["name"] for t in cfg["tasks"] if t["name"] not in BUILT_TASKS]},
        "verdict": "not decided: the low-rank baseline and the MNIST task are not built, so the registered criterion cannot be applied",
        "evidence": "L1 simulation (float64); loss is bookkeeping with free parameters, not a device prediction",
    }


def main(argv=None):
    ap = argparse.ArgumentParser(description="Butterfly vs Clements MZI mesh study")
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
