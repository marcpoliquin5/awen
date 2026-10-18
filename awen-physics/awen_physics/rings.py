"""Microring resonator transmission.

* :func:`all_pass_analytic` and :func:`add_drop_analytic` are the closed-form
  intensity transmissions (Bogaerts et al., "Silicon microring resonators",
  *Laser Photonics Rev.* 6, 47 (2012)). Fidelity: analytic.
* :func:`all_pass_round_trips` propagates the field one round trip at a time
  through a lossless directional coupler ``[[r, iκ], [iκ, r]]`` until the
  ring field converges (fidelity: numerical_propagation). It shares no
  formula with the analytic path.

Parameters: self-coupling ``r`` (``κ² = 1 − r²``), single-pass amplitude
transmission ``a`` and round-trip phase ``φ = 2π n_eff L / λ``.
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np

from .fidelity import Fidelity


def _check(r: float, a: float) -> None:
    if not (0 <= r <= 1 and 0 <= a <= 1):
        raise ValueError("r and a must lie in [0, 1]")


def all_pass_analytic(phi, r: float, a: float):
    _check(r, a)
    c = np.cos(phi)
    return (a**2 - 2 * r * a * c + r**2) / (1 - 2 * a * r * c + (r * a) ** 2)


def add_drop_analytic(phi, r1: float, r2: float, a: float):
    """Returns ``(through, drop)`` intensity transmissions."""
    _check(r1, a)
    _check(r2, a)
    c = np.cos(phi)
    den = 1 - 2 * r1 * r2 * a * c + (r1 * r2 * a) ** 2
    through = (r2**2 * a**2 - 2 * r1 * r2 * a * c + r1**2) / den
    drop = (1 - r1**2) * (1 - r2**2) * a / den
    return through, drop


@dataclass(frozen=True)
class RoundTripResult:
    transmission: np.ndarray
    round_trips: int
    fidelity: Fidelity = Fidelity.NUMERICAL_PROPAGATION


def all_pass_round_trips(phi, r: float, a: float, tol: float = 1e-13, max_trips: int = 1_000_000) -> RoundTripResult:
    """Steady-state through-port intensity by explicit round-trip propagation."""
    _check(r, a)
    if r * a >= 1:
        raise ValueError("lossless critically closed ring never converges (need r·a < 1)")
    phi = np.atleast_1d(np.asarray(phi, dtype=float))
    k = math.sqrt(1 - r**2)
    ring = 1j * k * np.ones_like(phi, dtype=complex)  # field injected into the ring
    out = r * np.ones_like(phi, dtype=complex)  # direct through path
    prop = a * np.exp(1j * phi)
    trips = 0
    while trips < max_trips:
        returning = prop * ring
        out = out + 1j * k * returning
        ring = r * returning
        trips += 1
        if np.max(np.abs(ring)) < tol:
            break
    return RoundTripResult(np.abs(out) ** 2, trips)


def amplitude_from_loss(loss_db_per_cm: float, length_um: float) -> float:
    """Single-pass amplitude transmission for propagation loss in dB/cm."""
    return 10 ** (-loss_db_per_cm * length_um * 1e-4 / 20)


def free_spectral_range_um(wavelength_um: float, group_index: float, length_um: float) -> float:
    """``FSR = λ² / (n_g L)``."""
    return wavelength_um**2 / (group_index * length_um)


def fwhm_um(wavelength_um: float, group_index: float, length_um: float, r: float, a: float) -> float:
    """Resonance full width at half maximum, ``(1 − ra) λ² / (π n_g L √(ra))``."""
    ra = r * a
    return (1 - ra) * wavelength_um**2 / (math.pi * group_index * length_um * math.sqrt(ra))


def critical_coupling_r(a: float) -> float:
    """All-pass ring is critically coupled (zero on-resonance transmission) when ``r = a``."""
    return a
