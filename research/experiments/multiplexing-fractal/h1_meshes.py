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


def main(argv=None):
    ap = argparse.ArgumentParser(description="Butterfly vs Clements MZI mesh study")
    ap.add_argument("--modes", type=int, nargs="+", default=[8])
    ap.add_argument("--targets", type=int, default=16)
    ap.add_argument("--steps", type=int, default=800)
    ap.add_argument("--restarts", type=int, default=3)
    ap.add_argument("--out", default=str(Path(__file__).parent / "results"))
    a = ap.parse_args(argv)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    for n in a.modes:
        rep = study(n, a.targets, a.steps, a.restarts)
        (out / f"meshes_n{n}.json").write_text(json.dumps(rep, indent=2) + "\n")


if __name__ == "__main__":
    main()
