"""TE modes of a symmetric dielectric slab waveguide.

Two independent methods:

* :func:`analytic_te_modes` solves the transcendental dispersion relation
  (fidelity: analytic). For a core of index ``n1``, thickness ``d`` and
  cladding ``n2``, with ``κ = k0 √(n1² − n_eff²)`` and ``γ = k0 √(n_eff² − n2²)``:
  even modes satisfy ``tan(κ d/2) = γ/κ`` and odd modes ``−cot(κ d/2) = γ/κ``
  (Okamoto, *Fundamentals of Optical Waveguides*, 2nd ed., ch. 2; Saleh & Teich,
  *Fundamentals of Photonics*, ch. 9).
* :func:`finite_difference_te_modes` discretizes the scalar Helmholtz equation
  ``E'' + k0² n(x)² E = β² E`` (exact Maxwell for TE in 1-D) with second-order
  central differences and solves the tridiagonal eigenproblem (fidelity:
  full-wave, 1-D). It shares no code with the analytic solver.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
from scipy.linalg import eigh_tridiagonal
from scipy.optimize import brentq

from .fidelity import Fidelity


@dataclass(frozen=True)
class SlabWaveguide:
    core_index: float
    cladding_index: float
    thickness_um: float

    def __post_init__(self) -> None:
        if not self.core_index > self.cladding_index >= 1.0:
            raise ValueError("need core_index > cladding_index >= 1")
        if self.thickness_um <= 0:
            raise ValueError("thickness must be positive")

    def v_number(self, wavelength_um: float) -> float:
        """Normalized frequency ``V = k0 (d/2) √(n1² − n2²)``."""
        k0 = 2 * math.pi / wavelength_um
        return k0 * self.thickness_um / 2 * math.sqrt(self.core_index**2 - self.cladding_index**2)

    def te_mode_count(self, wavelength_um: float) -> int:
        """Number of guided TE modes, ``⌈2V/π⌉``."""
        return math.ceil(2 * self.v_number(wavelength_um) / math.pi)


@dataclass(frozen=True)
class ModeResult:
    effective_indices: tuple[float, ...]
    fidelity: Fidelity
    method: str


def analytic_te_modes(guide: SlabWaveguide, wavelength_um: float) -> ModeResult:
    """Effective indices of all guided TE modes, highest first."""
    n1, n2, d = guide.core_index, guide.cladding_index, guide.thickness_um
    k0 = 2 * math.pi / wavelength_um
    v = guide.v_number(wavelength_um)

    # Work in u = κ d/2 ∈ (0, V); w = √(V² − u²). Mode m lives in u ∈ (mπ/2, (m+1)π/2).
    def even(u: float) -> float:
        return u * math.tan(u) - math.sqrt(max(v * v - u * u, 0.0))

    def odd(u: float) -> float:
        return -u / math.tan(u) - math.sqrt(max(v * v - u * u, 0.0))

    indices = []
    m = 0
    eps = 1e-12
    while m * math.pi / 2 < v:
        lo = m * math.pi / 2 + eps
        hi = min((m + 1) * math.pi / 2, v) - eps
        f = even if m % 2 == 0 else odd
        if hi > lo and f(lo) * f(hi) < 0:
            u = brentq(f, lo, hi, xtol=1e-15, rtol=1e-15, maxiter=500)
            kappa = 2 * u / d
            indices.append(math.sqrt(n1 * n1 - (kappa / k0) ** 2))
        m += 1
    return ModeResult(tuple(indices), Fidelity.ANALYTIC, "transcendental dispersion relation")


def finite_difference_te_modes(
    guide: SlabWaveguide,
    wavelength_um: float,
    points_per_um: float = 200.0,
    window_um: float | None = None,
) -> ModeResult:
    """Guided TE effective indices from a finite-difference eigen-solve, highest first.

    Grid points sit at cell centres so the core/cladding interfaces fall midway
    between points; the field is zero at the window edges (the window defaults
    to the core plus 4 µm of cladding on each side).
    """
    n1, n2, d = guide.core_index, guide.cladding_index, guide.thickness_um
    k0 = 2 * math.pi / wavelength_um
    window = window_um if window_um is not None else d + 8.0
    count = int(round(window * points_per_um))
    h = window / count
    x = (np.arange(count) + 0.5) * h - window / 2
    n_sq = np.where(np.abs(x) < d / 2, n1 * n1, n2 * n2)
    # Points exactly one half-step from an interface are fine; the step is resolved at h.
    diagonal = -2.0 / h**2 + k0**2 * n_sq
    off = np.full(count - 1, 1.0 / h**2)
    beta_sq = eigh_tridiagonal(diagonal, off, eigvals_only=True,
                               select="v", select_range=((k0 * n2) ** 2, (k0 * n1) ** 2))
    neff = np.sqrt(np.sort(beta_sq)[::-1]) / k0
    return ModeResult(tuple(float(v) for v in neff), Fidelity.FULL_WAVE,
                      f"1-D finite differences, h={h * 1000:.3f} nm, window={window} µm")
