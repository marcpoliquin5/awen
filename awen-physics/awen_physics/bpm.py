"""2-D scalar paraxial beam propagation method (Crank–Nicolson finite differences).

Solves ``∂u/∂z = (i / 2k₀n₀) [∂²u/∂x² + k₀²(n(x)² − n₀²)] u`` for the slowly
varying envelope ``u`` with reference index ``n₀`` (Feit & Fleck, *Appl. Opt.*
17, 3990 (1978) for the method; Okamoto, *Fundamentals of Optical
Waveguides* for the finite-difference form). Crank–Nicolson is
unconditionally stable, second order in ``dz`` and exactly norm-preserving for
real ``n`` with the zero-field boundaries used here. Paraxial, scalar and
lossless: fidelity numerical_propagation.

:func:`imaginary_distance_mode` propagates along ``z → −iz`` so the
fundamental mode dominates and returns its effective index from the
eigenvalue of the discretized operator.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
from scipy.linalg import solve_banded

from .fidelity import Fidelity


@dataclass(frozen=True)
class BpmGrid:
    x_um: np.ndarray
    index: np.ndarray
    wavelength_um: float
    reference_index: float

    @property
    def dx(self) -> float:
        return float(self.x_um[1] - self.x_um[0])

    @property
    def k0(self) -> float:
        return 2 * math.pi / self.wavelength_um


def _operator(grid: BpmGrid) -> tuple[np.ndarray, np.ndarray]:
    """Tridiagonal ``L = (1/2k₀n₀)(D² + k₀²(n² − n₀²))``: diagonal and off-diagonal."""
    k0, n0, dx = grid.k0, grid.reference_index, grid.dx
    scale = 1.0 / (2 * k0 * n0)
    diag = scale * (-2.0 / dx**2 + k0**2 * (grid.index**2 - n0**2))
    off = np.full(grid.index.size - 1, scale / dx**2)
    return diag, off


def _cn_step(u: np.ndarray, diag: np.ndarray, off: np.ndarray, alpha: complex) -> np.ndarray:
    """One step of ``(1 − αL) u' = (1 + αL) u``."""
    rhs = (1 + alpha * diag) * u
    rhs[1:] += alpha * off * u[:-1]
    rhs[:-1] += alpha * off * u[1:]
    ab = np.zeros((3, u.size), dtype=complex)
    ab[0, 1:] = -alpha * off
    ab[1] = 1 - alpha * diag
    ab[2, :-1] = -alpha * off
    return solve_banded((1, 1), ab, rhs)


@dataclass(frozen=True)
class Propagation:
    z_um: np.ndarray
    fields: np.ndarray  # [len(z), len(x)]
    fidelity: Fidelity = Fidelity.NUMERICAL_PROPAGATION


def propagate(grid: BpmGrid, u0: np.ndarray, length_um: float, dz_um: float, save_every: int = 1) -> Propagation:
    if dz_um <= 0 or length_um <= 0:
        raise ValueError("dz and length must be positive")
    steps = int(round(length_um / dz_um))
    diag, off = _operator(grid)
    alpha = 0.5j * dz_um
    u = np.asarray(u0, dtype=complex).copy()
    zs, out = [0.0], [u.copy()]
    for s in range(1, steps + 1):
        u = _cn_step(u, diag, off, alpha)
        if s % save_every == 0 or s == steps:
            zs.append(s * dz_um)
            out.append(u.copy())
    return Propagation(np.array(zs), np.array(out))


def rms_width(x: np.ndarray, u: np.ndarray) -> float:
    """Second-moment beam radius ``w = 2 √⟨(x − x̄)²⟩`` of ``|u|²`` (equals the 1/e² radius of a Gaussian)."""
    p = np.abs(u) ** 2
    p = p / p.sum()
    mean = (x * p).sum()
    return 2 * math.sqrt(((x - mean) ** 2 * p).sum())


def gaussian_width_analytic(w0_um: float, z_um: float, wavelength_um: float, index: float) -> float:
    """``w(z) = w₀ √(1 + (z/z_R)²)`` with ``z_R = π w₀² n / λ``."""
    zr = math.pi * w0_um**2 * index / wavelength_um
    return w0_um * math.sqrt(1 + (z_um / zr) ** 2)


@dataclass(frozen=True)
class ModeEstimate:
    effective_index: float
    profile: np.ndarray
    fidelity: Fidelity = Fidelity.NUMERICAL_PROPAGATION


def imaginary_distance_mode(grid: BpmGrid, dz_um: float = 0.5, steps: int = 4000, tol: float = 1e-13) -> ModeEstimate:
    """Fundamental-mode effective index by imaginary-distance propagation."""
    diag, off = _operator(grid)
    alpha = 0.5 * dz_um  # z → −iz turns i·dz/2 into dz/2
    rng = np.random.default_rng(0)
    u = np.exp(-(grid.x_um / (0.25 * (grid.x_um[-1] - grid.x_um[0]))) ** 2) * (1 + 0.01 * rng.standard_normal(grid.x_um.size))
    u = u.astype(complex)
    lam_prev = None
    for _ in range(steps):
        u = _cn_step(u, diag, off, alpha)
        u /= np.linalg.norm(u)
        lu = diag * u
        lu[1:] += off * u[:-1]
        lu[:-1] += off * u[1:]
        lam = float(np.real(np.vdot(u, lu)))
        if lam_prev is not None and abs(lam - lam_prev) < tol * max(1.0, abs(lam)):
            break
        lam_prev = lam
    k0, n0 = grid.k0, grid.reference_index
    beta_sq = (k0 * n0) ** 2 + 2 * k0 * n0 * lam
    return ModeEstimate(math.sqrt(beta_sq) / k0, np.abs(u))
