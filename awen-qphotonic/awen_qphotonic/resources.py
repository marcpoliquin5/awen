"""Resource model for small linear-optical experiments (estimates, not measurements).

Every quantity is a formula evaluated on user-supplied parameters. The only
physics assumption is uniform, independent loss: each photon reaches a
detector and registers with probability eta = source * circuit * detector,
which ``detection.lossy_output_bruteforce`` shows is exact for uniform loss
regardless of the interferometer.
"""
from __future__ import annotations

import math
from dataclasses import asdict, dataclass


@dataclass(frozen=True)
class ExperimentSpec:
    photons: int
    modes: int
    source_efficiency: float = 1.0
    circuit_transmission: float = 1.0
    detector_efficiency: float = 1.0
    dark_count_per_gate: float = 0.0
    trigger_rate_hz: float = 1e6
    target_samples: int = 1000

    def __post_init__(self):
        for name in ("source_efficiency", "circuit_transmission", "detector_efficiency", "dark_count_per_gate"):
            v = getattr(self, name)
            if not 0.0 <= v <= 1.0:
                raise ValueError(f"{name} must be in [0, 1]")
        if self.photons < 1 or self.modes < 1:
            raise ValueError("photons and modes must be >= 1")


def per_photon_efficiency(spec: ExperimentSpec) -> float:
    return spec.source_efficiency * spec.circuit_transmission * spec.detector_efficiency


def n_fold_success_probability(spec: ExperimentSpec) -> float:
    """Probability that all n photons are detected in one trigger (uniform loss)."""
    return per_photon_efficiency(spec) ** spec.photons


def no_spurious_click_probability(spec: ExperimentSpec) -> float:
    """Probability that none of the m modes fires from a dark count in one gate (upper bound on clean events)."""
    return (1.0 - spec.dark_count_per_gate) ** spec.modes


def num_outcomes(photons: int, modes: int) -> int:
    """Number of n-photon output patterns in m modes, C(n + m - 1, n)."""
    return math.comb(photons + modes - 1, photons)


def shots_for_probability(p: float, relative_error: float) -> float:
    """Shots N such that the standard error of an estimated probability p is relative_error * p: (1 - p) / (p eps^2)."""
    if not 0 < p <= 1:
        raise ValueError("p must be in (0, 1]")
    return (1.0 - p) / (p * relative_error ** 2)


def shots_for_total_variation(outcomes: int, epsilon: float, delta: float) -> float:
    """Samples so that P(||p_hat - p||_1 >= epsilon) <= delta (Bretagnolle-Huber-Carol inequality).

    P(||p_hat - p||_1 >= eps) <= 2^K exp(-N eps^2 / 2)  =>  N >= 2 (K ln 2 + ln(1/delta)) / eps^2.
    Note the L1 distance is twice the total variation distance.
    """
    return 2.0 * (outcomes * math.log(2.0) + math.log(1.0 / delta)) / epsilon ** 2


def ryser_operations_per_amplitude(photons: int) -> int:
    """Operation-count estimate for one permanent by Ryser with Gray code, about n * 2^n."""
    return photons * 2 ** photons


def estimate(spec: ExperimentSpec, tvd_epsilon: float = 0.05, delta: float = 0.05) -> dict:
    """Summary dictionary (all fields are model estimates)."""
    p = n_fold_success_probability(spec)
    k = num_outcomes(spec.photons, spec.modes)
    triggers = spec.target_samples / p if p > 0 else math.inf
    tvd_samples = shots_for_total_variation(k, 2 * tvd_epsilon, delta)
    return {
        "label": "estimate (formula on supplied parameters, not a measurement)",
        "spec": asdict(spec),
        "per_photon_efficiency": per_photon_efficiency(spec),
        "n_fold_success_probability": p,
        "no_spurious_click_probability": no_spurious_click_probability(spec),
        "expected_triggers": triggers,
        "expected_wall_time_s": triggers / spec.trigger_rate_hz,
        "num_outcomes": k,
        "samples_for_tvd": tvd_samples,
        "triggers_for_tvd": tvd_samples / p if p > 0 else math.inf,
        "classical_exact_amplitudes": k,
        "classical_ryser_operations_total": k * ryser_operations_per_amplitude(spec.photons),
    }
