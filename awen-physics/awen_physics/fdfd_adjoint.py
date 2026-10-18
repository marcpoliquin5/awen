"""1-D frequency-domain finite differences (Helmholtz) with adjoint gradients.

Solves ``E'' + k₀² ε(x) E = −s(x)`` on a uniform grid with outgoing-wave
(radiation) boundaries ``E' = ±i k₀ √ε E`` imposed through ghost points; the
boundary rows are halved so the system matrix ``A`` is complex symmetric.
Time dependence ``exp(−iωt)``. Second order in ``h``; fidelity full_wave (1-D).

For an objective ``J = |E_m|²`` at probe node ``m`` the adjoint method gives
the gradient with respect to every pixel permittivity from one extra solve
(Giles & Pierce, *Flow Turbul. Combust.* 65, 393 (2000); Molesky et al.,
*Nat. Photonics* 12, 659 (2018) for photonic inverse design)::

    A μ = e_m,   ∂J/∂ε_i = −2 Re(conj(E_m) μ_i (∂A_ii/∂ε_i) E_i)

because ``A = Aᵀ`` and ``∂A/∂ε_i`` is diagonal (``k₀²`` inside; the halved
boundary rows also carry the √ε of the radiation condition). The adjoint gradient is
checked against central finite differences in the tests.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np
from scipy.linalg import solve_banded

from .fidelity import Fidelity


@dataclass(frozen=True)
class Grid:
    eps: np.ndarray
    h_um: float
    wavelength_um: float

    @property
    def k0(self) -> float:
        return 2 * math.pi / self.wavelength_um


def _banded(grid: Grid) -> np.ndarray:
    n, h, k0 = grid.eps.size, grid.h_um, grid.k0
    eps = grid.eps.astype(complex)
    diag = -2.0 / h**2 + k0**2 * eps
    lower = np.full(n - 1, 1.0 / h**2, dtype=complex)
    upper = lower.copy()
    # Ghost points: E_{-1} = E_1 + 2ih k_L E_0 and E_n = E_{n-2} + 2ih k_R E_{n-1}.
    kl, kr = k0 * np.sqrt(eps[0]), k0 * np.sqrt(eps[-1])
    diag[0] += 2j * kl / h
    diag[-1] += 2j * kr / h
    upper[0] *= 2
    lower[-1] *= 2
    # Halve the boundary rows: symmetric matrix.
    diag[0] /= 2
    upper[0] /= 2
    diag[-1] /= 2
    lower[-1] /= 2
    ab = np.zeros((3, n), dtype=complex)
    ab[0, 1:] = upper
    ab[1] = diag
    ab[2, :-1] = lower
    return ab


def _rhs_scale(n: int) -> np.ndarray:
    s = np.ones(n)
    s[0] = s[-1] = 0.5
    return s


def solve(grid: Grid, source: np.ndarray) -> np.ndarray:
    """Field ``E`` for source density ``s`` (same length as ``eps``)."""
    if grid.eps.shape != source.shape:
        raise ValueError("source and eps must have the same shape")
    if np.any(grid.eps.real <= 0) or np.any(grid.eps.imag < -1e-12):
        raise ValueError("eps must have positive real part and non-negative imaginary part (passive dielectric)")
    return solve_banded((1, 1), _banded(grid), -source * _rhs_scale(source.size))


@dataclass(frozen=True)
class GradientResult:
    objective: float
    gradient: np.ndarray
    fidelity: Fidelity = Fidelity.FULL_WAVE


def probe_intensity_gradient(grid: Grid, source: np.ndarray, probe: int) -> GradientResult:
    """``J = |E_probe|²`` and ``∂J/∂ε`` for every node by the adjoint method."""
    e = solve(grid, source)
    unit = np.zeros(grid.eps.size, dtype=complex)
    unit[probe] = 1.0
    mu = solve_banded((1, 1), _banded(grid), unit)
    k0, h = grid.k0, grid.h_um
    d_a = np.full(grid.eps.size, k0**2, dtype=complex)
    # Boundary rows are halved and their radiation condition depends on √ε:
    # ∂A_bb/∂ε_b = k₀²/2 + i k₀ / (2 h √ε_b).
    for b in (0, -1):
        d_a[b] = k0**2 / 2 + 1j * k0 / (2 * h * np.sqrt(complex(grid.eps[b])))
    grad = -2 * np.real(np.conj(e[probe]) * mu * d_a * e)
    return GradientResult(float(abs(e[probe]) ** 2), grad)


def slab_reflectance(eps_layers: list[tuple[complex, float]], eps_sub: complex, wavelength_um: float, h_um: float, pad_um: float = 3.0) -> float:
    """Normal-incidence reflectance of ``vacuum | layers | substrate`` from two solves."""
    total = sum(d for _, d in eps_layers)
    length = 2 * pad_um + total
    n = int(round(length / h_um)) + 1
    x = np.arange(n) * h_um
    x0 = pad_um
    # Cell-averaged permittivity over [x - h/2, x + h/2]: interfaces that fall
    # on nodes get the mean of both sides, which keeps second-order accuracy.
    edges = [x0]
    for _, d in eps_layers:
        edges.append(edges[-1] + d)
    values = [1.0] + [e_l for e_l, _ in eps_layers] + [eps_sub]
    bounds = [-np.inf] + edges + [np.inf]
    lo, hi = x - h_um / 2, x + h_um / 2
    eps = np.zeros(n, dtype=complex)
    for v, a, b in zip(values, bounds[:-1], bounds[1:]):
        eps += v * np.clip(np.minimum(hi, b) - np.maximum(lo, a), 0, None) / h_um
    src = np.zeros(n)
    js = int(round(1.5 / h_um))
    src[js] = 1.0 / h_um
    probe = int(round(0.5 / h_um))
    vac = solve(Grid(np.ones(n, dtype=complex), h_um, wavelength_um), src)
    full = solve(Grid(eps, h_um, wavelength_um), src)
    return float(abs(full[probe] - vac[probe]) ** 2 / abs(vac[probe]) ** 2)
