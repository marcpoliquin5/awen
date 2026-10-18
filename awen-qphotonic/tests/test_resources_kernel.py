import math

import numpy as np
import pytest

from awen_qphotonic import fock, kernel, resources
from awen_qphotonic.detection import lossy_output_bruteforce, postselect_n


def test_success_probability_matches_lossy_simulation():
    spec = resources.ExperimentSpec(photons=3, modes=4, source_efficiency=0.9, circuit_transmission=0.7, detector_efficiency=0.8)
    eta = resources.per_photon_efficiency(spec)
    u = fock.haar_unitary(4, np.random.default_rng(2))
    _, p = postselect_n(lossy_output_bruteforce(u, (1, 1, 1, 0), eta), 3)
    assert resources.n_fold_success_probability(spec) == pytest.approx(p, abs=1e-12)


def test_success_rate_monte_carlo_and_estimate_fields():
    spec = resources.ExperimentSpec(photons=4, modes=8, source_efficiency=0.8, circuit_transmission=0.9,
                                    detector_efficiency=0.9, trigger_rate_hz=1e6, target_samples=1000)
    rng = np.random.default_rng(0)
    n = 200_000
    detected = rng.binomial(spec.photons, resources.per_photon_efficiency(spec), size=n)
    rate = np.mean(detected == spec.photons)
    p = resources.n_fold_success_probability(spec)
    assert abs(rate - p) < 5 * math.sqrt(p * (1 - p) / n)
    est = resources.estimate(spec)
    assert est["num_outcomes"] == math.comb(11, 4)
    assert est["expected_triggers"] == pytest.approx(1000 / p)
    assert est["expected_wall_time_s"] == pytest.approx(1000 / p / 1e6)
    assert "not a measurement" in est["label"]


def test_sampling_variance_formula():
    rng = np.random.default_rng(1)
    p, shots = 0.07, 2000
    estimates = rng.binomial(shots, p, size=20_000) / shots
    assert estimates.var() == pytest.approx(p * (1 - p) / shots, rel=0.05)
    n = resources.shots_for_probability(p, 0.1)
    assert math.sqrt(p * (1 - p) / n) == pytest.approx(0.1 * p)


def test_total_variation_bound_holds_empirically():
    rng = np.random.default_rng(3)
    u = fock.haar_unitary(4, rng)
    dist = fock.output_distribution(u, (1, 1, 0, 0))
    k = len(dist)
    eps_l1, delta = 0.2, 0.05
    n = int(math.ceil(resources.shots_for_total_variation(k, eps_l1, delta)))
    probs = np.array(list(dist.values()))
    failures = 0
    trials = 300
    for _ in range(trials):
        counts = rng.multinomial(n, probs / probs.sum())
        failures += np.abs(counts / n - probs).sum() >= eps_l1
    assert failures / trials <= delta


def test_spec_validation_and_dark_counts():
    with pytest.raises(ValueError):
        resources.ExperimentSpec(photons=2, modes=2, detector_efficiency=1.5)
    spec = resources.ExperimentSpec(photons=2, modes=10, dark_count_per_gate=1e-3)
    assert resources.no_spurious_click_probability(spec) == pytest.approx(0.999 ** 10)
    assert resources.ryser_operations_per_amplitude(10) == 10 * 1024


def test_photonic_kernel_is_valid():
    rng = np.random.default_rng(0)
    x = rng.standard_normal((25, 2))
    fm = kernel.PhotonicFeatureMap(features=2, scale=1.0, seed=0)
    for xi in x[:3]:
        u = fm.unitary(xi)
        assert np.allclose(u.conj().T @ u, np.eye(4), atol=1e-12)
        assert np.linalg.norm(fm.state(xi)) == pytest.approx(1.0, abs=1e-12)
    k = kernel.kernel_matrix(fm, x)
    np.testing.assert_allclose(np.diag(k), 1.0, atol=1e-12)
    np.testing.assert_allclose(k, k.T, atol=1e-12)
    assert np.linalg.eigvalsh(k).min() > -1e-10
    assert ((k >= -1e-12) & (k <= 1 + 1e-12)).all()


def test_inversion_test_probability_equals_kernel():
    fm = kernel.PhotonicFeatureMap(features=2, seed=1)
    x, y = np.array([0.3, -1.0]), np.array([1.2, 0.4])
    u = fm.unitary(y).conj().T @ fm.unitary(x)
    p_return = abs(fock.transition_amplitude(u, fm.input_pattern, fm.input_pattern)) ** 2
    assert kernel.kernel_matrix(fm, x[None], y[None])[0, 0] == pytest.approx(p_return, abs=1e-12)


def test_coherent_state_kernel_equals_rbf():
    rng = np.random.default_rng(2)
    xa, xb = rng.standard_normal((6, 2)), rng.standard_normal((5, 2))
    c = 0.8
    np.testing.assert_allclose(kernel.coherent_state_kernel(xa, xb, c), kernel.rbf_kernel(xa, xb, c * c), atol=1e-10)


def test_shot_kernel_unbiased():
    rng = np.random.default_rng(4)
    k = np.full((50, 50), 0.3)
    est = np.mean([kernel.kernel_matrix_shots(k, 500, rng) for _ in range(40)], axis=0)
    assert abs(est.mean() - 0.3) < 0.005


def test_experiment_reproducible_and_labelled():
    a = kernel.run_experiment("moons", n_train=40, n_test=40, seed=5, shots=200)
    b = kernel.run_experiment("moons", n_train=40, n_test=40, seed=5, shots=200)
    assert a == b
    assert "not hardware" in a["label"]
    for key in ("rbf", "photonic_exact", "photonic_shots", "linear"):
        assert 0.0 <= a[key]["test_acc"] <= 1.0
