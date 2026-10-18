"""Small runnable checks for the frontier-physics hypotheses.

Kept apart from the AWEN core packages on purpose: nothing in awen-* imports
this file, and nothing here is used to make product claims. Each function
reproduces an established textbook result so a hypothesis built on it starts
from a checked baseline.

* Casimir: ideal parallel-plate pressure and energy (Casimir 1948), and a
  closed-cycle work check showing the force is conservative (no net work).
* Information capacity: Shannon capacity of a Gaussian channel and the
  number of well-coupled spatial modes between two apertures (Miller 2000),
  computed from the singular values of a scalar Green's function.
* Fractal geometry: box-counting dimension of a Sierpinski gasket.
* Toroidal multipoles: far-field superposition of electric and toroidal
  dipoles in an explicit convention (anapole cancellation).
"""

from __future__ import annotations

import math

import numpy as np

HBAR = 1.054571817e-34  # J s (CODATA 2018, exact-derived)
C = 299_792_458.0  # m/s (exact)


# ----------------------------------------------------------------- Casimir

def casimir_pressure(gap_m: float) -> float:
    """Attractive pressure (Pa) between ideal conductors: pi^2 hbar c / (240 a^4)."""
    if gap_m <= 0:
        raise ValueError("gap must be positive")
    return math.pi ** 2 * HBAR * C / (240.0 * gap_m ** 4)


def casimir_energy_per_area(gap_m: float) -> float:
    """Interaction energy per area (J/m^2): -pi^2 hbar c / (720 a^3)."""
    if gap_m <= 0:
        raise ValueError("gap must be positive")
    return -math.pi ** 2 * HBAR * C / (720.0 * gap_m ** 3)


def cycle_work_per_area(gaps_m: np.ndarray) -> float:
    """Net work per area (J/m^2) done by the Casimir force around a closed gap path.

    The ideal force is the gradient of a single-valued energy, so any closed
    cycle returns exactly the energy it took: the result is zero up to
    quadrature error. Extracting net energy would require a path-dependent
    (non-conservative) change, e.g. changing the materials during the cycle,
    which costs at least the energy it releases.
    """
    gaps = np.asarray(gaps_m, dtype=float)
    if gaps[0] != gaps[-1]:
        raise ValueError("path must be closed")
    work = 0.0
    for a, b in zip(gaps[:-1], gaps[1:]):
        # Exact integral of -P da (attractive force does positive work when the gap closes).
        work += casimir_energy_per_area(a) - casimir_energy_per_area(b)
    return work


# ----------------------------------------------------------------- information capacity

def shannon_capacity_bits(snr: float, bandwidth_hz: float = 1.0) -> float:
    """C = B log2(1 + SNR) for an additive white Gaussian noise channel."""
    if snr < 0 or bandwidth_hz <= 0:
        raise ValueError("snr must be >= 0 and bandwidth > 0")
    return bandwidth_hz * math.log2(1.0 + snr)


def shot_noise_bits_per_sample(photons: float) -> float:
    """Gaussian-approximation bits per real sample when SNR = mean photon count."""
    return 0.5 * math.log2(1.0 + photons)


def coupling_singular_values(lt: float, lr: float, z: float, wavelength: float, samples: int = 400) -> np.ndarray:
    """Singular values of the scalar 2-D Green's function between two parallel line apertures.

    G(r) ~ exp(i k r) / sqrt(r), apertures of lengths lt and lr centred on a
    common axis and separated by z. Values are normalised to the largest.
    """
    k = 2 * math.pi / wavelength
    xt = np.linspace(-lt / 2, lt / 2, samples)
    xr = np.linspace(-lr / 2, lr / 2, samples)
    r = np.sqrt(z ** 2 + (xr[:, None] - xt[None, :]) ** 2)
    g = np.exp(1j * k * r) / np.sqrt(r)
    s = np.linalg.svd(g, compute_uv=False)
    return s / s[0]


def well_coupled_modes(lt: float, lr: float, z: float, wavelength: float, threshold: float = 0.5) -> int:
    return int(np.sum(coupling_singular_values(lt, lr, z, wavelength) > threshold))


def paraxial_mode_estimate(lt: float, lr: float, z: float, wavelength: float) -> float:
    """Paraxial count of well-coupled modes for 1-D apertures: lt * lr / (wavelength * z)."""
    return lt * lr / (wavelength * z)


# ----------------------------------------------------------------- fractal geometry

def sierpinski_points(level: int) -> np.ndarray:
    """Lower-left corners of the filled cells of a Sierpinski gasket on a 2^level grid."""
    n = 2 ** level
    i, j = np.meshgrid(np.arange(n), np.arange(n), indexing="ij")
    mask = (i & j) == 0
    return np.stack([i[mask], j[mask]], axis=1) / n


def box_counting_dimension(points: np.ndarray, scales: list[int]) -> float:
    """Slope of log N(eps) vs log(1/eps) for boxes of side 1/s, s in scales."""
    counts = [len({(int(x * s), int(y * s)) for x, y in points}) for s in scales]
    slope, _ = np.polyfit(np.log(scales), np.log(counts), 1)
    return float(slope)


# ----------------------------------------------------------------- toroidal dipole

def effective_dipole(p: complex, t: complex, k: float) -> complex:
    """Effective radiating dipole p + i k T (convention: T has units of p times length).

    Electric and toroidal dipoles share the same far-field angular pattern, so
    their fields add through this single quantity. The sign of the i k T term
    depends on the time convention and the definition of T; this function fixes
    exp(-i omega t) and T defined so that the term enters with a plus sign.
    """
    return p + 1j * k * t


def relative_radiated_power(p: complex, t: complex, k: float) -> float:
    """Far-field power relative to a lone electric dipole of unit amplitude: |p + i k T|^2."""
    return abs(effective_dipole(p, t, k)) ** 2


def anapole_toroidal_moment(p: complex, k: float) -> complex:
    """Toroidal moment that cancels the electric-dipole far field: T = i p / k."""
    return 1j * p / k
