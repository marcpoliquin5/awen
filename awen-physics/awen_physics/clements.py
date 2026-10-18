"""Rectangular MZI-mesh decomposition of unitaries (Clements et al., 2016).

W. R. Clements, P. C. Humphreys, B. J. Metcalf, W. S. Kolthammer and
I. A. Walmsley, "Optimal design for universal multiport interferometers,"
Optica 3, 1460–1465 (2016), doi:10.1364/OPTICA.3.001460.

Each 2×2 unit acts on adjacent modes ``(m, m+1)`` as
``T(θ, φ) = [[e^{iφ} cos θ, −sin θ], [e^{iφ} sin θ, cos θ]]`` (the paper's
Eq. 1). An N-mode unitary decomposes into ``N(N−1)/2`` units in a mesh of
depth ``N`` plus output phases ``D``: ``U = D ∏ T``. Fidelity: analytic (exact
linear algebra).
"""

from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np

from .fidelity import Fidelity


def mzi(theta: float, phi: float) -> np.ndarray:
    return np.array(
        [[np.exp(1j * phi) * math.cos(theta), -math.sin(theta)],
         [np.exp(1j * phi) * math.sin(theta), math.cos(theta)]],
        dtype=complex,
    )


def embed(n: int, mode: int, block: np.ndarray) -> np.ndarray:
    out = np.eye(n, dtype=complex)
    out[mode:mode + 2, mode:mode + 2] = block
    return out


@dataclass(frozen=True)
class MziUnit:
    mode: int  # acts on modes (mode, mode + 1)
    theta: float
    phi: float


@dataclass(frozen=True)
class MeshDecomposition:
    size: int
    phases: np.ndarray  # output phases, D = diag(exp(i·phases))
    units: tuple[MziUnit, ...]  # applied first to last (rightmost factor first)
    fidelity: Fidelity = Fidelity.ANALYTIC

    def depth(self) -> int:
        """Number of columns when each unit is placed as early as its modes allow."""
        free = [0] * self.size
        for unit in self.units:
            column = max(free[unit.mode], free[unit.mode + 1])
            free[unit.mode] = free[unit.mode + 1] = column + 1
        return max(free) if self.units else 0

    def matrix(self, phase_errors: np.ndarray | None = None) -> np.ndarray:
        """Rebuild the unitary; optional per-unit (Δθ, Δφ) errors of shape [units, 2]."""
        u = np.eye(self.size, dtype=complex)
        for index, unit in enumerate(self.units):
            theta, phi = unit.theta, unit.phi
            if phase_errors is not None:
                theta += phase_errors[index, 0]
                phi += phase_errors[index, 1]
            u = embed(self.size, unit.mode, mzi(theta, phi)) @ u
        return np.diag(np.exp(1j * self.phases)) @ u


def _null_right(u: np.ndarray, row: int, mode: int) -> MziUnit:
    """Choose T on columns (mode, mode+1) so (U T⁻¹)[row, mode] = 0."""
    a, b = u[row, mode], u[row, mode + 1]
    theta = math.atan2(abs(a), abs(b))
    phi = float(np.angle(a) - np.angle(b))
    return MziUnit(mode, theta, phi)


def _null_left(u: np.ndarray, col: int, mode: int) -> MziUnit:
    """Choose T on rows (mode, mode+1) so (T U)[mode+1, col] = 0."""
    a, b = u[mode, col], u[mode + 1, col]
    theta = math.atan2(abs(b), abs(a))
    phi = float(math.pi + np.angle(b) - np.angle(a))
    return MziUnit(mode, theta, phi)


def decompose(unitary: np.ndarray, atol: float = 1e-10) -> MeshDecomposition:
    """Clements decomposition of an N×N unitary into a rectangular MZI mesh."""
    u = np.array(unitary, dtype=complex)
    n = u.shape[0]
    if u.shape != (n, n) or not np.allclose(u.conj().T @ u, np.eye(n), atol=atol):
        raise ValueError("input must be a square unitary matrix")
    right: list[MziUnit] = []
    left: list[MziUnit] = []
    for i in range(n - 1):
        if i % 2 == 0:
            for j in range(i + 1):
                unit = _null_right(u, n - 1 - j, i - j)
                u = u @ embed(n, unit.mode, mzi(unit.theta, unit.phi)).conj().T
                right.append(unit)
        else:
            for j in range(i + 1):
                unit = _null_left(u, j, n - 2 - i + j)
                u = embed(n, unit.mode, mzi(unit.theta, unit.phi)) @ u
                left.append(unit)
    # Now L_k…L_1 · U · R_1†…R_r† = D, so U = L_1†…L_k† · D · R_r…R_1.
    d = np.diag(u).copy()
    moved: list[MziUnit] = []
    for unit in reversed(left):
        # T(θ,φ)† · diag(d1, d2) = diag(−e^{−iφ} d2, d2) · T(θ, φ') with e^{iφ'} = −d1/d2.
        d1, d2 = d[unit.mode], d[unit.mode + 1]
        phi_new = float(np.angle(-d1 / d2))
        d[unit.mode] = -np.exp(-1j * unit.phi) * d2
        moved.append(MziUnit(unit.mode, unit.theta, phi_new))
    # U = D' · T'_1 … T'_k · R_r … R_1 ; applied order (rightmost first): R_1..R_r, then T'_k..T'_1.
    units = tuple(right) + tuple(moved)
    return MeshDecomposition(n, np.angle(d), units)


def haar_unitary(n: int, rng: np.random.Generator) -> np.ndarray:
    """Haar-random unitary via QR of a complex Gaussian matrix (Mezzadri 2007)."""
    z = (rng.normal(size=(n, n)) + 1j * rng.normal(size=(n, n))) / math.sqrt(2)
    q, r = np.linalg.qr(z)
    return q * (np.diag(r) / np.abs(np.diag(r)))


def fidelity(target: np.ndarray, achieved: np.ndarray) -> float:
    """Normalized overlap ``|Tr(U† V)|² / N²`` (1 means identical up to a global phase)."""
    n = target.shape[0]
    return float(abs(np.trace(target.conj().T @ achieved)) ** 2 / n**2)
