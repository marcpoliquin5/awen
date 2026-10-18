"""Fock-space linear optics for small photon numbers (exact, simulation only).

Convention: a passive interferometer U (m x m, unitary) maps creation
operators as a_j^dagger -> sum_i U[i, j] a_i^dagger. For an input occupation
pattern s and output pattern t with the same photon number n,

    <t| U |s> = Perm(U[t_rows, s_cols]) / sqrt(prod_i s_i! * prod_i t_i!)

where s_cols repeats column j s_j times and t_rows repeats row i t_i times.
``evolve_product_state`` computes the same amplitudes by explicit polynomial
expansion of the creation operators and is used as the brute-force oracle.
"""
from __future__ import annotations

import itertools
import math
from collections import defaultdict
from typing import Dict, Iterable, Sequence, Tuple

import numpy as np

from .permanent import permanent

Pattern = Tuple[int, ...]


def haar_unitary(m: int, rng: np.random.Generator) -> np.ndarray:
    """Haar-random m x m unitary (QR of a complex Ginibre matrix with phase fix)."""
    z = (rng.standard_normal((m, m)) + 1j * rng.standard_normal((m, m))) / math.sqrt(2.0)
    q, r = np.linalg.qr(z)
    d = np.diag(r)
    return q * (d / np.abs(d))


def beam_splitter(reflectivity: float) -> np.ndarray:
    """Lossless 2x2 beam splitter with power reflectivity R (symmetric phase convention)."""
    if not 0.0 <= reflectivity <= 1.0:
        raise ValueError("reflectivity must be in [0, 1]")
    t = math.sqrt(1.0 - reflectivity)
    r = math.sqrt(reflectivity)
    return np.array([[t, 1j * r], [1j * r, t]], dtype=complex)


def fock_patterns(n: int, m: int) -> list[Pattern]:
    """All occupation patterns of n photons in m modes (C(n+m-1, n) of them)."""
    out = []
    for bars in itertools.combinations(range(n + m - 1), m - 1):
        prev = -1
        pat = []
        for b in bars:
            pat.append(b - prev - 1)
            prev = b
        pat.append(n + m - 1 - prev - 1)
        out.append(tuple(pat))
    return out


def _expand(pattern: Sequence[int]) -> list[int]:
    return [i for i, k in enumerate(pattern) for _ in range(k)]


def _check_unitary(u: np.ndarray, atol: float = 1e-9) -> np.ndarray:
    u = np.asarray(u, dtype=complex)
    if u.ndim != 2 or u.shape[0] != u.shape[1]:
        raise ValueError("interferometer must be square")
    if not np.allclose(u.conj().T @ u, np.eye(u.shape[0]), atol=atol):
        raise ValueError("interferometer is not unitary; model loss explicitly (see detection.py)")
    return u


def transition_amplitude(u: np.ndarray, s: Sequence[int], t: Sequence[int]) -> complex:
    """<t|U|s> via the permanent (Ryser)."""
    u = np.asarray(u, dtype=complex)
    if sum(s) != sum(t):
        return 0.0 + 0.0j
    rows, cols = _expand(t), _expand(s)
    norm = math.sqrt(math.prod(math.factorial(k) for k in s) * math.prod(math.factorial(k) for k in t))
    return permanent(u[np.ix_(rows, cols)]) / norm


def output_distribution(u: np.ndarray, s: Sequence[int]) -> Dict[Pattern, float]:
    """Exact output probabilities for indistinguishable photons with input pattern s."""
    u = _check_unitary(u)
    n, m = sum(s), u.shape[0]
    return {t: abs(transition_amplitude(u, s, t)) ** 2 for t in fock_patterns(n, m)}


def distinguishable_distribution(u: np.ndarray, s: Sequence[int]) -> Dict[Pattern, float]:
    """Output probabilities for fully distinguishable photons: Perm(|U_st|^2) / prod t_i!."""
    u = _check_unitary(u)
    n, m = sum(s), u.shape[0]
    p = np.abs(u) ** 2
    cols = _expand(s)
    out = {}
    for t in fock_patterns(n, m):
        rows = _expand(t)
        out[t] = permanent(p[np.ix_(rows, cols)]).real / math.prod(math.factorial(k) for k in t)
    return out


def evolve_product_state(u: np.ndarray, creators: Iterable[Sequence[complex]]) -> Dict[Pattern, complex]:
    """Brute-force oracle: expand prod_k (sum_j c_kj a_j^dagger)|0> after U, normalized.

    Each creator is a length-m coefficient vector. Returns normalized Fock-basis
    amplitudes. Cost grows as m^n; intended for n <= ~6.
    """
    u = np.asarray(u, dtype=complex)
    m = u.shape[0]
    poly: Dict[Pattern, complex] = {tuple([0] * m): 1.0 + 0.0j}
    for c in creators:
        v = u @ np.asarray(c, dtype=complex)
        nxt: Dict[Pattern, complex] = defaultdict(complex)
        for pat, coef in poly.items():
            for i in range(m):
                if v[i] != 0:
                    q = list(pat)
                    q[i] += 1
                    nxt[tuple(q)] += coef * v[i]
        poly = dict(nxt)
    # monomial prod a_i^dagger^{t_i} |0> = sqrt(prod t_i!) |t>
    amps = {t: c * math.sqrt(math.prod(math.factorial(k) for k in t)) for t, c in poly.items()}
    norm = math.sqrt(sum(abs(a) ** 2 for a in amps.values()))
    if norm == 0:
        raise ValueError("input state has zero norm")
    return {t: a / norm for t, a in amps.items()}


def fock_input_creators(s: Sequence[int]) -> list[np.ndarray]:
    """Creation-operator vectors for a Fock input pattern s."""
    m = len(s)
    return [np.eye(m, dtype=complex)[j] for j in _expand(s)]


def boson_sample(u: np.ndarray, s: Sequence[int], shots: int, rng: np.random.Generator) -> list[Pattern]:
    """Draw exact boson-sampling outcomes (small n; enumerates the full distribution)."""
    dist = output_distribution(u, s)
    pats = list(dist)
    p = np.array([dist[t] for t in pats])
    p = p / p.sum()
    idx = rng.choice(len(pats), size=shots, p=p)
    return [pats[i] for i in idx]


def empirical_distribution(samples: Sequence[Pattern]) -> Dict[Pattern, float]:
    counts: Dict[Pattern, float] = defaultdict(float)
    for x in samples:
        counts[x] += 1.0
    n = float(len(samples))
    return {k: v / n for k, v in counts.items()}


def total_variation(p: Dict[Pattern, float], q: Dict[Pattern, float]) -> float:
    keys = set(p) | set(q)
    return 0.5 * sum(abs(p.get(k, 0.0) - q.get(k, 0.0)) for k in keys)


def hom_coincidence_probability(reflectivity: float, overlap: float) -> float:
    """Two-photon coincidence probability at a beam splitter, by Fock simulation.

    Photon A enters spatial mode 0 in internal state |e0>; photon B enters
    spatial mode 1 in sqrt(overlap)|e0> + sqrt(1 - overlap)|e1>, where
    ``overlap`` = |<psi_A|psi_B>|^2 is the internal-state (spectral/temporal/
    polarization) overlap. Modes are ordered (spatial, internal) -> 2*s + i.
    """
    if not 0.0 <= overlap <= 1.0:
        raise ValueError("overlap must be in [0, 1]")
    bs = beam_splitter(reflectivity)
    u = np.kron(bs, np.eye(2))
    a = np.zeros(4, dtype=complex)
    a[0] = 1.0
    b = np.zeros(4, dtype=complex)
    b[2] = math.sqrt(overlap)
    b[3] = math.sqrt(1.0 - overlap)
    state = evolve_product_state(u, [a, b])
    p = 0.0
    for t, amp in state.items():
        if t[0] + t[1] == 1 and t[2] + t[3] == 1:
            p += abs(amp) ** 2
    return p


def gaussian_wavepacket_overlap(delay: float, rms_width: float) -> float:
    """|<psi(0)|psi(delay)>|^2 for Gaussian temporal intensity profiles of rms width sigma.

    With |psi(t)|^2 a normal density of standard deviation sigma, the overlap is
    exp(-delay^2 / (4 sigma^2)).
    """
    return math.exp(-(delay ** 2) / (4.0 * rms_width ** 2))


def hom_dip(delays: Sequence[float], rms_width: float, reflectivity: float = 0.5) -> np.ndarray:
    """Coincidence probability versus relative delay (simulated HOM dip)."""
    return np.array([hom_coincidence_probability(reflectivity, gaussian_wavepacket_overlap(d, rms_width)) for d in delays])


def hom_visibility(reflectivity: float, overlap: float = 1.0) -> float:
    """(P_distinguishable - P(overlap)) / P_distinguishable from the Fock simulation."""
    p_dist = hom_coincidence_probability(reflectivity, 0.0)
    return (p_dist - hom_coincidence_probability(reflectivity, overlap)) / p_dist
