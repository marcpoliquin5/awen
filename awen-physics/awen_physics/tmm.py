"""Planar multilayer optics: transfer (characteristic) matrices and S-matrices.

Two independent methods for a stack ``ambient | layer_1 … layer_N | substrate``:

* :func:`transfer_matrix` multiplies 2×2 characteristic matrices
  (Born & Wolf, *Principles of Optics*, 7th ed., §1.6; Macleod, *Thin-Film
  Optical Filters*). Time dependence ``exp(-iωt)``; a complex index
  ``n + iκ`` with ``κ > 0`` absorbs.
* :func:`scattering_matrix` composes interface Fresnel coefficients and
  layer propagation phases with the Redheffer star product. It is
  numerically stable for thick or absorbing stacks and shares no code with
  the transfer-matrix path.

Both are exact solutions of Maxwell's equations for laterally infinite
planar layers (fidelity: analytic).
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Sequence

import numpy as np

from .fidelity import Fidelity


@dataclass(frozen=True)
class StackResult:
    r: complex
    t: complex
    R: float
    T: float
    fidelity: Fidelity = Fidelity.ANALYTIC

    @property
    def A(self) -> float:
        """Absorbed fraction ``1 − R − T``."""
        return 1.0 - self.R - self.T


def _cos_theta(n: complex, n0_sin: complex) -> complex:
    c = np.sqrt(1 - (n0_sin / n) ** 2 + 0j)
    # Forward-propagating / decaying branch.
    if c.imag < 0 or (abs(c.imag) < 1e-15 and c.real < 0):
        c = -c
    return complex(c)


def _admittance(n: complex, cos_t: complex, pol: str) -> complex:
    if pol == "s":
        return n * cos_t
    if pol == "p":
        return n / cos_t
    raise ValueError("pol must be 's' or 'p'")


def _check(indices: Sequence[complex], thicknesses: Sequence[float]) -> None:
    if len(indices) != len(thicknesses) + 2:
        raise ValueError("need ambient, one index per layer, and substrate")
    if any(d < 0 for d in thicknesses):
        raise ValueError("thicknesses must be non-negative")
    if np.imag(indices[0]) != 0:
        raise ValueError("ambient must be lossless")


def transfer_matrix(
    indices: Sequence[complex],
    thicknesses_um: Sequence[float],
    wavelength_um: float,
    theta0_rad: float = 0.0,
    pol: str = "s",
) -> StackResult:
    """Reflection and transmission of a planar stack by characteristic matrices."""
    _check(indices, thicknesses_um)
    k0 = 2 * np.pi / wavelength_um
    s = indices[0] * np.sin(theta0_rad)
    etas = [_admittance(n, _cos_theta(n, s), pol) for n in indices]
    m = np.eye(2, dtype=complex)
    for n, d, eta in zip(indices[1:-1], thicknesses_um, etas[1:-1]):
        delta = k0 * n * _cos_theta(n, s) * d
        layer = np.array(
            [[np.cos(delta), -1j * np.sin(delta) / eta], [-1j * eta * np.sin(delta), np.cos(delta)]]
        )
        m = m @ layer
    b, c = m @ np.array([1.0, etas[-1]])
    eta0 = etas[0]
    r = (eta0 * b - c) / (eta0 * b + c)
    t = 2 * eta0 / (eta0 * b + c)
    R = abs(r) ** 2
    T = (etas[-1].real / eta0.real) * abs(t) ** 2
    return StackResult(complex(r), complex(t), float(R), float(T))


@dataclass(frozen=True)
class SMatrix:
    """Two-port scattering block: left-incident ``(r, t)``, right-incident ``(rp, tp)``."""

    r: complex
    t: complex
    rp: complex
    tp: complex

    def star(self, other: "SMatrix") -> "SMatrix":
        """Redheffer star product: ``self`` on the left, ``other`` on the right."""
        d = 1 - self.rp * other.r
        return SMatrix(
            r=self.r + self.tp * other.r * self.t / d,
            t=other.t * self.t / d,
            rp=other.rp + other.t * self.rp * other.tp / d,
            tp=self.tp * other.tp / d,
        )


def interface(eta_i: complex, eta_j: complex) -> SMatrix:
    """Fresnel interface from admittance ``eta_i`` into ``eta_j``."""
    s = eta_i + eta_j
    return SMatrix(r=(eta_i - eta_j) / s, t=2 * eta_i / s, rp=(eta_j - eta_i) / s, tp=2 * eta_j / s)


def propagation(phase: complex) -> SMatrix:
    p = np.exp(1j * phase)
    return SMatrix(0j, p, 0j, p)


def scattering_matrix(
    indices: Sequence[complex],
    thicknesses_um: Sequence[float],
    wavelength_um: float,
    theta0_rad: float = 0.0,
    pol: str = "s",
) -> StackResult:
    """Same quantities as :func:`transfer_matrix`, by star-product composition."""
    _check(indices, thicknesses_um)
    k0 = 2 * np.pi / wavelength_um
    s = indices[0] * np.sin(theta0_rad)
    cos = [_cos_theta(n, s) for n in indices]
    etas = [_admittance(n, c, pol) for n, c in zip(indices, cos)]
    total = SMatrix(0j, 1 + 0j, 0j, 1 + 0j)
    for j, d in enumerate(thicknesses_um, start=1):
        total = total.star(interface(etas[j - 1], etas[j])).star(propagation(k0 * indices[j] * cos[j] * d))
    total = total.star(interface(etas[-2], etas[-1]))
    R = abs(total.r) ** 2
    T = (etas[-1].real / etas[0].real) * abs(total.t) ** 2
    return StackResult(complex(total.r), complex(total.t), float(R), float(T))


def quarter_wave_mirror_reflectance(n0: float, n_high: float, n_low: float, n_sub: float, pairs: int) -> float:
    """Closed-form reflectance at the design wavelength of ``ambient | (HL)^N | substrate``
    (Macleod, *Thin-Film Optical Filters*)."""
    a = n0 * n_low ** (2 * pairs)
    b = n_sub * n_high ** (2 * pairs)
    return ((a - b) / (a + b)) ** 2
