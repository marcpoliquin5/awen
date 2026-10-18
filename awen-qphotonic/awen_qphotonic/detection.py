"""Photon counting with detection efficiency, loss and dark counts (simulation only).

Uniform loss with transmissivity eta acts on each mode as binomial thinning
P(k | n) = C(n, k) eta^k (1 - eta)^(n - k). ``lossy_output_bruteforce`` checks
this by embedding the lossy interferometer in a 2m-mode unitary that couples
each mode to an environment mode and tracing the environment out.
"""
from __future__ import annotations

import itertools
import math
from collections import defaultdict
from typing import Dict, Sequence

import numpy as np

from .fock import Pattern, fock_patterns, output_distribution


def binomial_loss(n: int, eta: float) -> np.ndarray:
    """Vector P(k | n) for k = 0..n."""
    if not 0.0 <= eta <= 1.0:
        raise ValueError("eta must be in [0, 1]")
    return np.array([math.comb(n, k) * eta ** k * (1 - eta) ** (n - k) for k in range(n + 1)])


def apply_uniform_loss(dist: Dict[Pattern, float], eta: float) -> Dict[Pattern, float]:
    """Photon-number-resolving statistics after per-mode transmissivity eta."""
    out: Dict[Pattern, float] = defaultdict(float)
    for pat, p in dist.items():
        per_mode = [binomial_loss(k, eta) for k in pat]
        for kept in itertools.product(*[range(k + 1) for k in pat]):
            q = p
            for mode, k in enumerate(kept):
                q *= per_mode[mode][k]
            out[tuple(kept)] += q
    return dict(out)


def threshold_detection(dist: Dict[Pattern, float], eta: float, dark_count: float = 0.0) -> Dict[Pattern, float]:
    """Click/no-click statistics for threshold detectors with efficiency eta and dark-count probability per gate.

    P(no click | n photons) = (1 - dark_count) * (1 - eta)^n, independently per mode.
    """
    if not 0.0 <= dark_count <= 1.0:
        raise ValueError("dark_count must be in [0, 1]")
    out: Dict[Pattern, float] = defaultdict(float)
    for pat, p in dist.items():
        p0 = [(1 - dark_count) * (1 - eta) ** k for k in pat]
        for clicks in itertools.product((0, 1), repeat=len(pat)):
            q = p
            for mode, c in enumerate(clicks):
                q *= (1 - p0[mode]) if c else p0[mode]
            out[clicks] += q
    return dict(out)


def lossy_output_bruteforce(u: np.ndarray, s: Sequence[int], eta: float) -> Dict[Pattern, float]:
    """Oracle: U followed by per-mode loss, as a 2m-mode unitary with environment modes traced out."""
    u = np.asarray(u, dtype=complex)
    m = u.shape[0]
    t, r = math.sqrt(eta), math.sqrt(1.0 - eta)
    loss = np.block([[t * np.eye(m), r * np.eye(m)], [-r * np.eye(m), t * np.eye(m)]])
    big = loss @ np.block([[u, np.zeros((m, m))], [np.zeros((m, m)), np.eye(m)]])
    full = output_distribution(big, list(s) + [0] * m)
    out: Dict[Pattern, float] = defaultdict(float)
    for pat, p in full.items():
        out[pat[:m]] += p
    return dict(out)


def postselect_n(dist: Dict[Pattern, float], n: int) -> tuple[Dict[Pattern, float], float]:
    """Condition on exactly n detected photons; returns (normalized distribution, success probability)."""
    sel = {k: v for k, v in dist.items() if sum(k) == n}
    p = sum(sel.values())
    if p == 0:
        return {}, 0.0
    return {k: v / p for k, v in sel.items()}, p


__all__ = [
    "binomial_loss",
    "apply_uniform_loss",
    "threshold_detection",
    "lossy_output_bruteforce",
    "postselect_n",
    "fock_patterns",
]
