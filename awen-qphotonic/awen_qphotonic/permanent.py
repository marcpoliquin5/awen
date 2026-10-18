"""Matrix permanents.

``permanent`` uses Ryser's inclusion-exclusion formula with Gray-code
ordering, O(2^n * n) operations. ``permanent_bruteforce`` sums over all n!
permutations and exists only as an independent oracle for small n.
"""
from __future__ import annotations

import itertools

import numpy as np


def permanent(a: np.ndarray) -> complex:
    """Permanent of a square matrix by Ryser's formula (Gray-code order)."""
    a = np.asarray(a, dtype=complex)
    if a.ndim != 2 or a.shape[0] != a.shape[1]:
        raise ValueError("permanent requires a square matrix")
    n = a.shape[0]
    if n == 0:
        return 1.0 + 0.0j
    # Row sums over the currently selected column subset, updated one column per step.
    row_sums = np.zeros(n, dtype=complex)
    total = 0.0 + 0.0j
    sign = -1.0
    gray_prev = 0
    for k in range(1, 1 << n):
        gray = k ^ (k >> 1)
        changed = gray ^ gray_prev
        col = changed.bit_length() - 1
        if gray & changed:
            row_sums += a[:, col]
        else:
            row_sums -= a[:, col]
        gray_prev = gray
        # sign is (-1)^(n - |S|)
        size = bin(gray).count("1")
        sign = -1.0 if (n - size) % 2 else 1.0
        total += sign * np.prod(row_sums)
    return complex(total)


def permanent_bruteforce(a: np.ndarray) -> complex:
    """Permanent by explicit sum over permutations (oracle, n <= ~8)."""
    a = np.asarray(a, dtype=complex)
    n = a.shape[0]
    total = 0.0 + 0.0j
    for perm in itertools.permutations(range(n)):
        p = 1.0 + 0.0j
        for i, j in enumerate(perm):
            p *= a[i, j]
        total += p
    return complex(total)
