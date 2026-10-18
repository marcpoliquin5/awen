"""A small photonic quantum-kernel experiment with a classical RBF baseline.

Feature map: x in R^d sets phase shifters between fixed, seeded Haar-random
mixing layers on m modes; the input is a Fock state s (default |1,1,0,0>).
The kernel is k(x, y) = |<s| U(y)^dagger U(x) |s>|^2, the probability that an
inversion test returns the input pattern. ``kernel_matrix`` computes it
exactly from Fock amplitudes; ``kernel_matrix_shots`` adds binomial shot noise
from a finite number of post-selected inversion-test shots.

This is a simulation for comparing kernels on the same data. It makes no
claim of quantum advantage: the exact kernel is computed classically here, at
a cost that is trivial for these sizes.
"""
from __future__ import annotations

import math
from dataclasses import dataclass, field
from typing import Sequence

import numpy as np

from .fock import fock_patterns, haar_unitary, transition_amplitude


@dataclass
class PhotonicFeatureMap:
    features: int
    modes: int = 4
    input_pattern: Sequence[int] = (1, 1, 0, 0)
    layers: int = 2
    scale: float = 1.0
    seed: int = 0
    _mixers: list = field(default_factory=list, init=False, repr=False)
    _patterns: list = field(default_factory=list, init=False, repr=False)

    def __post_init__(self):
        if len(self.input_pattern) != self.modes:
            raise ValueError("input_pattern length must equal modes")
        rng = np.random.default_rng(self.seed)
        self._mixers = [haar_unitary(self.modes, rng) for _ in range(self.layers + 1)]
        self._patterns = fock_patterns(sum(self.input_pattern), self.modes)

    def unitary(self, x: np.ndarray) -> np.ndarray:
        u = self._mixers[0]
        for layer in range(self.layers):
            phases = np.zeros(self.modes)
            for j in range(self.modes):
                phases[j] = self.scale * x[(layer * self.modes + j) % self.features]
            u = self._mixers[layer + 1] @ np.diag(np.exp(1j * phases)) @ u
        return u

    def state(self, x: np.ndarray) -> np.ndarray:
        """Fock-basis amplitudes of U(x)|s>."""
        u = self.unitary(np.asarray(x, dtype=float))
        return np.array([transition_amplitude(u, self.input_pattern, t) for t in self._patterns])


def kernel_matrix(fmap: PhotonicFeatureMap, xa: np.ndarray, xb: np.ndarray | None = None) -> np.ndarray:
    sa = np.array([fmap.state(x) for x in xa])
    sb = sa if xb is None else np.array([fmap.state(x) for x in xb])
    return np.abs(sa.conj() @ sb.T) ** 2


def kernel_matrix_shots(k_exact: np.ndarray, shots: int, rng: np.random.Generator, symmetric: bool = False) -> np.ndarray:
    """Binomial estimate of each kernel entry from ``shots`` post-selected inversion tests."""
    k = rng.binomial(shots, np.clip(k_exact, 0.0, 1.0)) / shots
    if symmetric:
        k = np.triu(k, 1)
        k = k + k.T + np.eye(k.shape[0])
    return k


def rbf_kernel(xa: np.ndarray, xb: np.ndarray, gamma: float) -> np.ndarray:
    d2 = ((xa[:, None, :] - xb[None, :, :]) ** 2).sum(-1)
    return np.exp(-gamma * d2)


def coherent_state_kernel(xa: np.ndarray, xb: np.ndarray, c: float, cutoff: int = 40) -> np.ndarray:
    """|<alpha(x)|alpha(y)>|^2 with alpha_j = c x_j, from truncated Fock vectors (one mode per feature)."""
    n = np.arange(cutoff)
    logfact = np.array([math.lgamma(k + 1) for k in n])

    def vec(a):
        return np.exp(-abs(a) ** 2 / 2 + n * np.log(abs(a) + 1e-300) - logfact / 2) * np.exp(1j * n * np.angle(a))

    out = np.ones((len(xa), len(xb)))
    for j in range(xa.shape[1]):
        va = np.array([vec(c * x) for x in xa[:, j]])
        vb = np.array([vec(c * x) for x in xb[:, j]])
        out *= np.abs(va.conj() @ vb.T) ** 2
    return out


def make_moons(n: int, noise: float, rng: np.random.Generator) -> tuple[np.ndarray, np.ndarray]:
    """Two interleaved half circles (labels +/-1), standardized to zero mean and unit variance."""
    n0 = n // 2
    t0 = rng.uniform(0, math.pi, n0)
    t1 = rng.uniform(0, math.pi, n - n0)
    x = np.vstack([np.c_[np.cos(t0), np.sin(t0)], np.c_[1 - np.cos(t1), 0.5 - np.sin(t1)]])
    x += noise * rng.standard_normal(x.shape)
    y = np.r_[np.ones(n0), -np.ones(n - n0)]
    x = (x - x.mean(0)) / x.std(0)
    perm = rng.permutation(n)
    return x[perm], y[perm]


def make_circles(n: int, noise: float, rng: np.random.Generator) -> tuple[np.ndarray, np.ndarray]:
    n0 = n // 2
    t = rng.uniform(0, 2 * math.pi, n)
    r = np.r_[np.ones(n0), 0.5 * np.ones(n - n0)]
    x = np.c_[r * np.cos(t), r * np.sin(t)] + noise * rng.standard_normal((n, 2))
    y = np.r_[np.ones(n0), -np.ones(n - n0)]
    x = (x - x.mean(0)) / x.std(0)
    perm = rng.permutation(n)
    return x[perm], y[perm]


def ridge_fit_predict(k_train: np.ndarray, y_train: np.ndarray, k_test_train: np.ndarray, lam: float) -> np.ndarray:
    """Kernel ridge classifier: sign(K_* (K + lam I)^-1 y)."""
    alpha = np.linalg.solve(k_train + lam * np.eye(len(y_train)), y_train)
    return np.sign(k_test_train @ alpha)


def _cv_select(kfun, params, lams, x, y, folds: int, rng: np.random.Generator):
    idx = rng.permutation(len(y))
    chunks = np.array_split(idx, folds)
    best = (-1.0, None, None)
    for p in params:
        k_full = kfun(p, x, x)
        for lam in lams:
            accs = []
            for f in range(folds):
                te = chunks[f]
                tr = np.concatenate([chunks[g] for g in range(folds) if g != f])
                pred = ridge_fit_predict(k_full[np.ix_(tr, tr)], y[tr], k_full[np.ix_(te, tr)], lam)
                accs.append(np.mean(pred == y[te]))
            acc = float(np.mean(accs))
            if acc > best[0]:
                best = (acc, p, lam)
    return best


def run_experiment(dataset: str = "moons", n_train: int = 120, n_test: int = 200, noise: float = 0.15,
                   seed: int = 0, shots: int = 1000, folds: int = 4) -> dict:
    """Train/test both kernels on the same split; hyperparameters chosen by CV on the training set only."""
    rng = np.random.default_rng(seed)
    gen = {"moons": make_moons, "circles": make_circles}[dataset]
    x, y = gen(n_train + n_test, noise, rng)
    xtr, ytr, xte, yte = x[:n_train], y[:n_train], x[n_train:], y[n_train:]
    lams = [1e-3, 1e-2, 1e-1, 1.0]

    gammas = [0.1, 0.3, 1.0, 3.0, 10.0]
    cv_rbf, g_best, lam_rbf = _cv_select(lambda g, a, b: rbf_kernel(a, b, g), gammas, lams, xtr, ytr, folds, rng)
    acc_rbf = float(np.mean(ridge_fit_predict(rbf_kernel(xtr, xtr, g_best), ytr, rbf_kernel(xte, xtr, g_best), lam_rbf) == yte))

    scales = [0.3, 0.6, 1.0, 1.5, 2.5]
    fmaps = {s: PhotonicFeatureMap(features=x.shape[1], scale=s, seed=seed) for s in scales}
    cache = {}

    def qk(s, a, b):
        key = (s, id(a), id(b))
        if key not in cache:
            cache[key] = kernel_matrix(fmaps[s], a, None if a is b else b)
        return cache[key]

    cv_q, s_best, lam_q = _cv_select(qk, scales, lams, xtr, ytr, folds, rng)
    fm = fmaps[s_best]
    ktr = kernel_matrix(fm, xtr)
    kte = kernel_matrix(fm, xte, xtr)
    acc_q = float(np.mean(ridge_fit_predict(ktr, ytr, kte, lam_q) == yte))
    ktr_s = kernel_matrix_shots(ktr, shots, rng, symmetric=True)
    kte_s = kernel_matrix_shots(kte, shots, rng)
    # regularization re-selected by CV on the noisy training kernel only
    cv_qs, _, lam_qs = _cv_select(lambda _p, a, b: ktr_s, [None], lams + [3.0, 10.0], xtr, ytr, folds, rng)
    acc_q_shots = float(np.mean(ridge_fit_predict(ktr_s, ytr, kte_s, lam_qs) == yte))

    lin = xtr @ xtr.T
    acc_lin = float(np.mean(ridge_fit_predict(lin, ytr, xte @ xtr.T, 1.0) == yte))
    return {
        "label": "simulation (exact Fock-amplitude kernel and binomial shot-noise model); not hardware",
        "dataset": dataset, "noise": noise, "seed": seed, "n_train": n_train, "n_test": n_test,
        "feature_map": {"modes": fm.modes, "input_pattern": list(fm.input_pattern), "layers": fm.layers},
        "rbf": {"test_acc": acc_rbf, "cv_acc": cv_rbf, "gamma": g_best, "lambda": lam_rbf},
        "photonic_exact": {"test_acc": acc_q, "cv_acc": cv_q, "scale": s_best, "lambda": lam_q},
        "photonic_shots": {"test_acc": acc_q_shots, "cv_acc": cv_qs, "lambda": lam_qs, "shots_per_entry": shots},
        "linear": {"test_acc": acc_lin},
    }


def main(argv: Sequence[str] | None = None) -> None:
    import argparse
    import json

    ap = argparse.ArgumentParser(description="Photonic quantum kernel vs RBF baseline (simulation).")
    ap.add_argument("--datasets", nargs="+", default=["moons", "circles"])
    ap.add_argument("--seeds", nargs="+", type=int, default=[0, 1, 2])
    ap.add_argument("--shots", type=int, default=1000)
    ap.add_argument("--out", default=None)
    a = ap.parse_args(argv)
    runs = [run_experiment(d, seed=s, shots=a.shots) for d in a.datasets for s in a.seeds]
    text = json.dumps({"runs": runs}, indent=2)
    if a.out:
        open(a.out, "w").write(text + "\n")
    print(text)


if __name__ == "__main__":
    main()
