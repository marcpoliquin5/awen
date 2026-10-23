"""F2: Fibonacci (quasi-periodic) and triadic-Cantor multilayer stacks.

Layers A (high index) and B (low index) are quarter-wave at the reference
frequency ω0, so the phase of every layer is δ = (π/2)(ω/ω0). Each layer's
2×2 characteristic matrix (det = 1 for lossless layers) is the one used in
``awen_physics.tmm.transfer_matrix``; transmission here is cross-checked against
that function.

Established results reproduced:

* Kohmoto, Kadanoff & Tang, PRL 50, 1870 (1983), doi:10.1103/PhysRevLett.50.1870, and
  Kohmoto, Sutherland & Iguchi, PRL 58, 2436 (1987), doi:10.1103/PhysRevLett.58.2436:
  for S_{l+1} = S_l S_{l-1}, the half-traces x_l = Tr(M_l)/2 obey the trace map
  x_{l+1} = 2 x_l x_{l-1} − x_{l-2}, which conserves
  I = x_{l+1}² + x_l² + x_{l-1}² − 2 x_{l+1} x_l x_{l-1} − 1.
* Lavrinenko et al., PRE 65, 036621 (2002), doi:10.1103/PhysRevE.65.036621: scaling
  (self-similarity) of the transmission spectrum of an optical Cantor filter.
* Gellermann et al., PRL 72, 633 (1994), doi:10.1103/PhysRevLett.72.633: measured
  transmission of Fibonacci dielectric multilayers.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path

import numpy as np

from awen_physics.tmm import transfer_matrix


def fibonacci_word(generation: int) -> str:
    """S_0 = 'B', S_1 = 'A', S_{l+1} = S_l + S_{l-1} (lengths are Fibonacci numbers)."""
    a, b = "B", "A"
    if generation == 0:
        return a
    for _ in range(generation - 1):
        a, b = b, b + a
    return b


def cantor_word(generation: int) -> str:
    """Triadic Cantor stack: C_0 = 'A', C_{k+1} = C_k + 'B'*3^k + C_k."""
    w = "A"
    for k in range(generation):
        w = w + "B" * 3 ** k + w
    return w


def layer_matrices(n: float, omega: np.ndarray) -> np.ndarray:
    """Quarter-wave-at-ω0 layer characteristic matrices ``[F, 2, 2]`` (normal incidence)."""
    d = 0.5 * np.pi * omega
    m = np.empty((omega.size, 2, 2), dtype=complex)
    m[:, 0, 0] = np.cos(d)
    m[:, 0, 1] = -1j * np.sin(d) / n
    m[:, 1, 0] = -1j * n * np.sin(d)
    m[:, 1, 1] = np.cos(d)
    return m


def stack_matrix(word: str, n_a: float, n_b: float, omega: np.ndarray) -> np.ndarray:
    ma, mb = layer_matrices(n_a, omega), layer_matrices(n_b, omega)
    m = np.broadcast_to(np.eye(2, dtype=complex), (omega.size, 2, 2)).copy()
    for ch in word:
        m = m @ (ma if ch == "A" else mb)
    return m


def transmission(m: np.ndarray, n0: float = 1.0, ns: float = 1.0) -> np.ndarray:
    b = m[:, 0, 0] + m[:, 0, 1] * ns
    c = m[:, 1, 0] + m[:, 1, 1] * ns
    t = 2 * n0 / (n0 * b + c)
    return (ns / n0) * np.abs(t) ** 2


def spectrum(word: str, n_a: float, n_b: float, omega: np.ndarray) -> np.ndarray:
    return transmission(stack_matrix(word, n_a, n_b, omega))


def tmm_crosscheck(word: str, n_a: float, n_b: float, omega: float) -> tuple[float, float]:
    """(this module, awen_physics.tmm) transmission at one frequency; λ0 = 1 µm."""
    idx = [1.0] + [n_a if ch == "A" else n_b for ch in word] + [1.0]
    th = [0.25 / (n_a if ch == "A" else n_b) for ch in word]
    ours = float(spectrum(word, n_a, n_b, np.array([omega]))[0])
    ref = transfer_matrix(idx, th, 1.0 / omega).T
    return ours, float(ref)


def trace_map(n_a: float, n_b: float, omega: np.ndarray, generations: int) -> np.ndarray:
    """Half-traces x_l of the Fibonacci stack matrices, computed by direct products ``[L, F]``."""
    return np.stack([0.5 * np.trace(stack_matrix(fibonacci_word(g), n_a, n_b, omega), axis1=1, axis2=2).real
                     for g in range(generations)])


def trace_invariant(x: np.ndarray) -> np.ndarray:
    """I_l for l = 2…L−1 from consecutive half-traces (should be constant in l)."""
    return x[2:] ** 2 + x[1:-1] ** 2 + x[:-2] ** 2 - 2 * x[2:] * x[1:-1] * x[:-2] - 1


def band_measure(t: np.ndarray, threshold: float = 0.5) -> float:
    """Fraction of the sampled frequency window with T above ``threshold``."""
    return float((t > threshold).mean())


def scaling_scan(n_a: float, n_b: float, generation: int, window: float, factors: np.ndarray,
                 samples: int = 4001) -> list[tuple[float, float]]:
    """Correlation of log T_{G+1}(1 + Δ/s) with log T_G(1 + Δ) over |Δ| ≤ window, for each s."""
    delta = np.linspace(-window, window, samples)
    base = np.log10(np.maximum(spectrum(cantor_word(generation), n_a, n_b, 1 + delta), 1e-12))
    out = []
    for s in factors:
        nxt = np.log10(np.maximum(spectrum(cantor_word(generation + 1), n_a, n_b, 1 + delta / s), 1e-12))
        out.append((float(s), float(np.corrcoef(base, nxt)[0, 1])))
    return out


def study(n_a: float = 2.3, n_b: float = 1.38) -> dict:
    omega = np.linspace(0.01, 1.99, 20001)
    x = trace_map(n_a, n_b, omega, 14)
    inv = trace_invariant(x)
    q = trace_map(n_a, n_b, np.array([1.0]), 20)[:, 0]
    t_qw = [float(spectrum(fibonacci_word(g), n_a, n_b, np.array([1.0]))[0]) for g in range(20)]
    fib_bands = [{"generation": g, "layers": len(fibonacci_word(g)),
                  "fraction_T_gt_0.5": round(band_measure(spectrum(fibonacci_word(g), n_a, n_b, omega)), 5)}
                 for g in range(3, 16)]
    periodic_bands = [{"layers": 2 * k, "fraction_T_gt_0.5": round(band_measure(spectrum("AB" * k, n_a, n_b, omega)), 5)}
                      for k in (2, 4, 8, 16, 32, 64, 128, 256)]
    factors = np.round(np.arange(1.5, 5.01, 0.1), 2)
    scans = {f"G{g}->G{g + 1}": scaling_scan(n_a, n_b, g, 0.5, factors) for g in (1, 2, 3, 4)}
    bounded = np.max(np.abs(x), axis=0) < 1e3  # in band gaps |x_l| grows exponentially and float64 loses I
    best = {k: max(v, key=lambda p: p[1]) for k, v in scans.items()}
    crosscheck = [tmm_crosscheck(fibonacci_word(7), n_a, n_b, w) for w in (0.37, 1.0, 1.61)]
    return {"study": "awen.fractal-stacks.v1", "n_a": n_a, "n_b": n_b, "ambient_and_substrate": 1.0,
            "note": "indices are free parameters of the study, not a specific fabricated material pair",
            "tmm_crosscheck_fibonacci_g7": crosscheck,
            "trace_invariant": {"max_abs_variation_over_generations_bounded_freqs":
                                float(np.max(np.abs(inv - inv[0])[:, bounded])),
                                "bounded_frequency_fraction": round(float(bounded.mean()), 4),
                                "closed_form_at_quarter_wave": float(0.25 * (n_a / n_b - n_b / n_a) ** 2),
                                "measured_at_quarter_wave": float(inv[0][np.argmin(abs(omega - 1.0))])},
            "quarter_wave_half_traces": [round(float(v), 10) for v in q],
            "quarter_wave_transmission_by_generation": [round(v, 10) for v in t_qw],
            "fibonacci_band_measure": fib_bands, "periodic_band_measure": periodic_bands,
            "cantor_scaling_scan_window": 0.5, "cantor_naive_prediction_factor": 3.0,
            "cantor_scaling_scan_best": {k: {"factor": v[0], "correlation": round(v[1], 5)} for k, v in best.items()},
            "cantor_scaling_scan": {k: [[s, round(c, 5)] for s, c in v] for k, v in scans.items()},
            "evidence": "exact transfer-matrix calculation (lossless, normal incidence)"}


def main(argv=None):
    ap = argparse.ArgumentParser(description="Fibonacci and Cantor multilayer study")
    ap.add_argument("--out", default=str(Path(__file__).parent / "results" / "pilot"))
    a = ap.parse_args(argv)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    rep = study()
    (out / "stacks.json").write_text(json.dumps(rep, indent=2) + "\n")
    print(json.dumps({k: v for k, v in rep.items() if k != "cantor_scaling_scan"}, indent=1))


if __name__ == "__main__":
    main()
