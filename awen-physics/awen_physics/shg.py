"""Second-harmonic generation in a χ⁽²⁾ medium: plane-wave coupled-wave equations.

Power-normalized amplitudes (``|A₁|² + |A₂|²`` is the total power, arbitrary
units) under the slowly varying envelope approximation, type-0 interaction,
lossless medium and phase mismatch ``Δk = k₂ − 2k₁``::

    dA₁/dz = i Γ(z) A₂ A₁* exp(−iΔk z)
    dA₂/dz = i Γ(z) A₁²   exp(+iΔk z)

``Γ`` (1/(µm·√power)) collects d_eff, indices and frequency. Closed forms
(Boyd, *Nonlinear Optics*, 3rd ed.; Armstrong et al., *Phys. Rev.* 127, 1918
(1962)):

* undepleted pump: ``P₂ = Γ² P₁² L² sinc²(ΔkL/2)``;
* phase matched with depletion: ``P₂ = P₁ tanh²(Γ √P₁ L)``;
* first-order quasi-phase matching (sign of Γ flipped every coherence length
  ``π/Δk``): ``P₂ = (2/π)² Γ² P₁² L²`` in the undepleted limit.

:func:`integrate` solves the coupled equations numerically (fidelity:
numerical_propagation); the closed forms are fidelity analytic.
"""

from __future__ import annotations

import math
from dataclasses import dataclass
from typing import Callable, Optional

import numpy as np
from scipy.integrate import solve_ivp

from .fidelity import Fidelity


@dataclass(frozen=True)
class ShgResult:
    z_um: np.ndarray
    fundamental_power: np.ndarray
    harmonic_power: np.ndarray
    fidelity: Fidelity

    @property
    def efficiency(self) -> float:
        return float(self.harmonic_power[-1] / (self.fundamental_power[0] + self.harmonic_power[0]))


def undepleted(power: float, gamma: float, length_um: float, delta_k: float = 0.0) -> float:
    x = delta_k * length_um / 2
    sinc = 1.0 if x == 0 else math.sin(x) / x
    return gamma**2 * power**2 * length_um**2 * sinc**2


def phase_matched_depleted(power: float, gamma: float, length_um: float) -> float:
    return power * math.tanh(gamma * math.sqrt(power) * length_um) ** 2


def qpm_first_order_undepleted(power: float, gamma: float, length_um: float) -> float:
    return (2 / math.pi) ** 2 * gamma**2 * power**2 * length_um**2


def integrate(
    power: float,
    gamma: float,
    length_um: float,
    delta_k: float = 0.0,
    gamma_of_z: Optional[Callable[[float], float]] = None,
    breakpoints_um: Optional[np.ndarray] = None,
    rtol: float = 1e-11,
    atol: float = 1e-14,
    points: int = 201,
) -> ShgResult:
    """Integrate the coupled equations from ``A₁ = √P, A₂ = 0``.

    ``gamma_of_z`` overrides ``gamma`` (e.g. periodic poling); pass the domain
    walls as ``breakpoints_um`` so the integrator never steps across one.
    """
    if power < 0 or length_um <= 0:
        raise ValueError("power must be >= 0 and length > 0")
    g = gamma_of_z or (lambda z: gamma)

    def rhs(z, y):
        a1 = y[0] + 1j * y[1]
        a2 = y[2] + 1j * y[3]
        gz = g(z)
        d1 = 1j * gz * a2 * np.conj(a1) * np.exp(-1j * delta_k * z)
        d2 = 1j * gz * a1 * a1 * np.exp(1j * delta_k * z)
        return [d1.real, d1.imag, d2.real, d2.imag]

    edges = [0.0] + [float(b) for b in (breakpoints_um if breakpoints_um is not None else []) if 0 < b < length_um] + [length_um]
    y = np.array([math.sqrt(power), 0.0, 0.0, 0.0])
    zs, p1, p2 = [0.0], [power], [0.0]
    for a, b in zip(edges[:-1], edges[1:]):
        n = max(2, int(points * (b - a) / length_um) + 1)
        sol = solve_ivp(rhs, (a, b), y, method="DOP853", rtol=rtol, atol=atol, t_eval=np.linspace(a, b, n))
        if not sol.success:  # pragma: no cover
            raise RuntimeError(sol.message)
        zs.extend(sol.t[1:])
        p1.extend(sol.y[0, 1:] ** 2 + sol.y[1, 1:] ** 2)
        p2.extend(sol.y[2, 1:] ** 2 + sol.y[3, 1:] ** 2)
        y = sol.y[:, -1]
    return ShgResult(np.array(zs), np.array(p1), np.array(p2), Fidelity.NUMERICAL_PROPAGATION)


def periodic_poling(delta_k: float, gamma: float) -> tuple[Callable[[float], float], float]:
    """First-order QPM: Γ flips sign every coherence length ``π/|Δk|``; returns ``(Γ(z), period)``."""
    if delta_k == 0:
        raise ValueError("poling needs a nonzero mismatch")
    lc = math.pi / abs(delta_k)

    def g(z: float) -> float:
        return gamma if int(z // lc) % 2 == 0 else -gamma

    return g, 2 * lc
