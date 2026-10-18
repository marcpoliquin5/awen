"""Gaussian states in the covariance formalism (simulation only).

Conventions: hbar = 2, so the vacuum covariance is the identity and
x = a + a^dagger, p = -i(a - a^dagger). Quadratures are ordered
(x_1, ..., x_m, p_1, ..., p_m). A Gaussian state is (mean, cov); Gaussian
unitaries act as mean -> S mean, cov -> S cov S^T with S symplectic,
S Omega S^T = Omega, Omega = [[0, I], [-I, 0]].
"""
from __future__ import annotations

import math
from dataclasses import dataclass

import numpy as np


def omega(m: int) -> np.ndarray:
    z, i = np.zeros((m, m)), np.eye(m)
    return np.block([[z, i], [-i, z]])


@dataclass
class GaussianState:
    mean: np.ndarray
    cov: np.ndarray

    @property
    def modes(self) -> int:
        return self.mean.shape[0] // 2

    def copy(self) -> "GaussianState":
        return GaussianState(self.mean.copy(), self.cov.copy())


def vacuum(m: int) -> GaussianState:
    return GaussianState(np.zeros(2 * m), np.eye(2 * m))


def thermal(nbar: float) -> GaussianState:
    return GaussianState(np.zeros(2), (2 * nbar + 1) * np.eye(2))


def coherent(alpha: complex) -> GaussianState:
    return GaussianState(np.array([2 * alpha.real, 2 * alpha.imag]), np.eye(2))


def tensor(*states: GaussianState) -> GaussianState:
    m = sum(s.modes for s in states)
    mean = np.zeros(2 * m)
    cov = np.zeros((2 * m, 2 * m))
    off = 0
    for s in states:
        k = s.modes
        idx = np.r_[off:off + k, m + off:m + off + k]
        sidx = np.r_[0:k, k:2 * k]
        mean[idx] = s.mean[sidx]
        cov[np.ix_(idx, idx)] = s.cov[np.ix_(sidx, sidx)]
        off += k
    return GaussianState(mean, cov)


def _indices(m: int, modes) -> np.ndarray:
    modes = list(modes)
    return np.array(modes + [m + j for j in modes])


def embed(s_local: np.ndarray, m: int, modes) -> np.ndarray:
    """Embed a 2k x 2k symplectic acting on ``modes`` into the m-mode space."""
    s = np.eye(2 * m)
    idx = _indices(m, modes)
    s[np.ix_(idx, idx)] = s_local
    return s


def squeezer(r: float, phi: float = 0.0) -> np.ndarray:
    """Single-mode squeezing; for phi = 0, x -> e^{-r} x and p -> e^{r} p."""
    c, s = math.cos(phi), math.sin(phi)
    rot = np.array([[c, -s], [s, c]])
    return rot @ np.diag([math.exp(-r), math.exp(r)]) @ rot.T


def rotation(theta: float) -> np.ndarray:
    """Phase rotation a -> e^{i theta} a."""
    c, s = math.cos(theta), math.sin(theta)
    return np.array([[c, -s], [s, c]])


def passive(u: np.ndarray) -> np.ndarray:
    """Symplectic matrix of a passive interferometer with unitary U (a -> U a)."""
    u = np.asarray(u, dtype=complex)
    return np.block([[u.real, -u.imag], [u.imag, u.real]])


def two_mode_squeezer(r: float) -> np.ndarray:
    """Two-mode squeezing S2(r) on modes (0, 1), ordering (x0, x1, p0, p1)."""
    ch, sh = math.cosh(r), math.sinh(r)
    zx = np.array([[ch, sh], [sh, ch]])
    zp = np.array([[ch, -sh], [-sh, ch]])
    out = np.zeros((4, 4))
    out[:2, :2] = zx
    out[2:, 2:] = zp
    return out


def is_symplectic(s: np.ndarray, atol: float = 1e-10) -> bool:
    m = s.shape[0] // 2
    return bool(np.allclose(s @ omega(m) @ s.T, omega(m), atol=atol))


def apply(state: GaussianState, s_local: np.ndarray, modes) -> GaussianState:
    s = embed(s_local, state.modes, modes)
    return GaussianState(s @ state.mean, s @ state.cov @ s.T)


def displace(state: GaussianState, alpha: complex, mode: int) -> GaussianState:
    out = state.copy()
    out.mean[mode] += 2 * alpha.real
    out.mean[state.modes + mode] += 2 * alpha.imag
    return out


def loss(state: GaussianState, eta: float, mode: int) -> GaussianState:
    """Pure-loss channel with transmissivity eta on one mode."""
    if not 0.0 <= eta <= 1.0:
        raise ValueError("eta must be in [0, 1]")
    m = state.modes
    idx = _indices(m, [mode])
    g = np.eye(2 * m)
    g[idx, idx] = math.sqrt(eta)
    noise = np.zeros((2 * m, 2 * m))
    noise[idx, idx] = 1.0 - eta
    return GaussianState(g @ state.mean, g @ state.cov @ g.T + noise)


def reduced(state: GaussianState, modes) -> GaussianState:
    idx = _indices(state.modes, modes)
    return GaussianState(state.mean[idx], state.cov[np.ix_(idx, idx)])


def mean_photon_number(state: GaussianState, mode: int) -> float:
    r = reduced(state, [mode])
    return float((np.trace(r.cov) + r.mean @ r.mean - 2.0) / 4.0)


def purity(state: GaussianState) -> float:
    """Tr(rho^2) = 1 / sqrt(det V) for hbar = 2."""
    return float(1.0 / math.sqrt(np.linalg.det(state.cov)))


def symplectic_eigenvalues(state: GaussianState) -> np.ndarray:
    ev = np.linalg.eigvals(1j * omega(state.modes) @ state.cov)
    return np.sort(np.abs(ev))[::2]


def vacuum_probability(state: GaussianState) -> float:
    """Probability of detecting no photons in any mode."""
    m = state.modes
    q = state.cov + np.eye(2 * m)
    d = state.mean
    return float(2 ** m / math.sqrt(np.linalg.det(q)) * math.exp(-0.5 * d @ np.linalg.solve(q, d)))


def homodyne_sample(state: GaussianState, mode: int, angle: float, shots: int, rng: np.random.Generator) -> np.ndarray:
    """Ideal homodyne outcomes of x_theta = x cos(theta) + p sin(theta) on one mode."""
    r = reduced(state, [mode])
    v = np.array([math.cos(angle), math.sin(angle)])
    return rng.normal(v @ r.mean, math.sqrt(v @ r.cov @ v), size=shots)


def homodyne_condition(state: GaussianState, mode: int, outcome: float) -> GaussianState:
    """State of the remaining modes after an ideal x-homodyne on ``mode`` returned ``outcome``."""
    m = state.modes
    rest = [j for j in range(m) if j != mode]
    a = _indices(m, rest)
    xb = mode
    vab = state.cov[np.ix_(a, [xb])][:, 0]
    vbb = state.cov[xb, xb]
    cov = state.cov[np.ix_(a, a)] - np.outer(vab, vab) / vbb
    mean = state.mean[a] + vab * (outcome - state.mean[xb]) / vbb
    return GaussianState(mean, cov)
