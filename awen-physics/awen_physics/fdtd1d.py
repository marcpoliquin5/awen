"""1-D finite-difference time-domain (Yee) solver for normal-incidence layers.

Units: lengths in µm, time in µm/c (``c = 1``), fields normalized so the
vacuum impedance is 1. ``E`` lives on integer nodes ``x_i = i·dx`` and ``H``
on half nodes; the update is the standard leapfrog (Yee, *IEEE Trans.
Antennas Propag.* 14, 302 (1966); Taflove & Hagness, *Computational
Electrodynamics*). It is second-order accurate in ``dx`` and ``dt`` and
stable for Courant number ``S = dt/dx ≤ 1``.

:func:`reflectance_spectrum` launches a Gaussian pulse from a soft source,
records the backward wave left of the source with and without the structure,
and divides their spectra. It shares no code with :mod:`awen_physics.tmm`, so
comparing the two is an independent check (fidelity: full_wave, 1-D).
Material interfaces must fall on ``E`` nodes; the interface node uses the mean
permittivity of its neighbours.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Sequence

import numpy as np

from .fidelity import Fidelity


@dataclass(frozen=True)
class Spectrum:
    wavelength_um: np.ndarray
    reflectance: np.ndarray
    dx_um: float
    steps: int
    fidelity: Fidelity = Fidelity.FULL_WAVE


def _permittivity(x: np.ndarray, layers: Sequence[tuple[float, float]], x0: float, n_sub: float, dx: float) -> np.ndarray:
    eps = np.ones_like(x)
    edges = [x0]
    for _, d in layers:
        edges.append(edges[-1] + d)
    for (n, _), a, b in zip(layers, edges[:-1], edges[1:]):
        eps[(x > a + dx / 2) & (x < b - dx / 2)] = n * n
    eps[x > edges[-1] + dx / 2] = n_sub * n_sub
    # Interface nodes: mean of the two sides.
    values = [1.0] + [n * n for n, _ in layers] + [n_sub * n_sub]
    for e, left, right in zip(edges, values[:-1], values[1:]):
        i = int(round(e / dx))
        if abs(i * dx - e) > 1e-9 * max(1.0, abs(e)):
            raise ValueError(f"interface at {e} µm is not on the grid (dx={dx})")
        eps[i] = 0.5 * (left + right)
    return eps


def run(eps: np.ndarray, dx: float, steps: int, source_index: int, probe_index: int,
        courant: float, t0: float, spread: float) -> np.ndarray:
    """Leapfrog update; returns ``E`` at the probe for every step."""
    if not 0 < courant <= 1:
        raise ValueError("Courant number must be in (0, 1]")
    dt = courant * dx
    e = np.zeros(eps.size)
    h = np.zeros(eps.size - 1)
    coef = courant / eps[1:-1]
    trace = np.empty(steps)
    for n in range(steps):
        h += courant * (e[1:] - e[:-1])
        e[1:-1] += coef * (h[1:] - h[:-1])
        t = (n + 1) * dt
        e[source_index] += np.exp(-(((t - t0) / spread) ** 2))
        trace[n] = e[probe_index]
    return trace


def reflectance_spectrum(
    layers: Sequence[tuple[float, float]],
    n_substrate: float,
    wavelengths_um: np.ndarray,
    dx_um: float,
    courant: float = 0.5,
    centre_um: float = 1.0,
) -> Spectrum:
    """Normal-incidence reflectance of ``vacuum | layers | substrate``.

    ``layers`` is a list of ``(index, thickness_um)``; thicknesses must be
    multiples of ``dx_um``. The domain is long enough that the edges (fixed
    ``E = 0``) do not contaminate the recorded window.
    """
    if any(n < 1 for n, _ in layers) or n_substrate < 1:
        raise ValueError("indices must be >= 1 (lossless dielectrics)")
    total = sum(d for _, d in layers)
    spread = centre_um / 2.0  # pulse width in time; covers roughly 0.6-1.8 µm
    t0 = 5 * spread
    # Ringing: allow 40 round trips through the stack.
    n_max = max([n for n, _ in layers] + [n_substrate, 1.0])
    record = 2 * t0 + 2 * 20.0 + 80 * n_max * total + 20.0
    gap = 20.0
    x0 = round((gap + record / 2) / dx_um) * dx_um  # structure start
    probe_x = x0 - gap
    source_x = probe_x + 5.0
    length = x0 + total + record  # substrate long enough that its far edge is never reached
    cells = int(round(length / dx_um)) + 1
    x = np.arange(cells) * dx_um
    steps = int(record / (courant * dx_um))
    src, probe = int(round(source_x / dx_um)), int(round(probe_x / dx_um))
    vac = run(np.ones(cells), dx_um, steps, src, probe, courant, t0, spread)
    full = run(_permittivity(x, layers, x0, n_substrate, dx_um), dx_um, steps, src, probe, courant, t0, spread)
    dt = courant * dx_um
    freqs = 1.0 / np.asarray(wavelengths_um, dtype=float)  # c = 1
    t = (np.arange(steps) + 1) * dt
    phase = np.exp(-2j * np.pi * np.outer(freqs, t))
    inc = phase @ vac
    ref = phase @ (full - vac)
    return Spectrum(np.asarray(wavelengths_um, dtype=float), np.abs(ref / inc) ** 2, dx_um, steps)
