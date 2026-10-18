import math

import numpy as np
import pytest

from awen_qphotonic import fock
from awen_qphotonic.detection import (
    apply_uniform_loss,
    lossy_output_bruteforce,
    postselect_n,
    threshold_detection,
)
from awen_qphotonic.permanent import permanent, permanent_bruteforce


@pytest.mark.parametrize("n", [1, 2, 3, 4, 5, 6])
def test_ryser_matches_bruteforce(n):
    rng = np.random.default_rng(100 + n)
    for _ in range(5):
        a = rng.standard_normal((n, n)) + 1j * rng.standard_normal((n, n))
        assert permanent(a) == pytest.approx(permanent_bruteforce(a), rel=1e-10, abs=1e-10)


def test_permanent_known_values():
    assert permanent(np.ones((4, 4))) == pytest.approx(math.factorial(4))
    assert permanent(np.eye(5)) == pytest.approx(1.0)
    assert permanent(np.zeros((0, 0))) == 1.0
    with pytest.raises(ValueError):
        permanent(np.ones((2, 3)))


def test_fock_pattern_count():
    for n, m in [(1, 4), (3, 3), (3, 5), (4, 6)]:
        pats = fock.fock_patterns(n, m)
        assert len(pats) == math.comb(n + m - 1, n) == len(set(pats))
        assert all(sum(p) == n and len(p) == m for p in pats)


@pytest.mark.parametrize("s", [(1, 1, 0), (1, 1, 1, 0), (2, 1, 0, 0), (1, 0, 1, 0, 1), (2, 0, 2, 0)])
def test_permanent_amplitudes_match_polynomial_oracle(s):
    rng = np.random.default_rng(sum(s) * 7 + len(s))
    u = fock.haar_unitary(len(s), rng)
    oracle = fock.evolve_product_state(u, fock.fock_input_creators(s))
    for t in fock.fock_patterns(sum(s), len(s)):
        assert fock.transition_amplitude(u, s, t) == pytest.approx(oracle.get(t, 0.0), abs=1e-10)


def test_distributions_normalized_and_distinguishable_matches_enumeration():
    rng = np.random.default_rng(3)
    u = fock.haar_unitary(4, rng)
    s = (1, 1, 1, 0)
    assert sum(fock.output_distribution(u, s).values()) == pytest.approx(1.0, abs=1e-12)
    dist = fock.distinguishable_distribution(u, s)
    # enumerate independent single-photon trajectories
    p = np.abs(u) ** 2
    ref = {}
    for outs in np.ndindex(4, 4, 4):
        t = [0] * 4
        prob = 1.0
        for photon, (inp, o) in enumerate(zip([0, 1, 2], outs)):
            t[o] += 1
            prob *= p[o, inp]
        ref[tuple(t)] = ref.get(tuple(t), 0.0) + prob
    for t, v in ref.items():
        assert dist[t] == pytest.approx(v, abs=1e-12)


def test_non_unitary_rejected():
    with pytest.raises(ValueError):
        fock.output_distribution(0.9 * np.eye(2), (1, 1))


def test_boson_sampling_samples_converge_to_exact():
    rng = np.random.default_rng(11)
    u = fock.haar_unitary(5, rng)
    s = (1, 1, 1, 0, 0)
    exact = fock.output_distribution(u, s)
    samples = fock.boson_sample(u, s, 40_000, rng)
    tvd = fock.total_variation(exact, fock.empirical_distribution(samples))
    # 35 outcomes; E[TVD] ~ 0.5*sum sqrt(2 p(1-p)/(pi N)) < 0.02 here
    assert tvd < 0.03
    # indistinguishable and distinguishable statistics differ measurably on a Haar unitary
    assert fock.total_variation(exact, fock.distinguishable_distribution(u, s)) > 0.03


def test_hom_dip_balanced_beam_splitter():
    assert fock.hom_coincidence_probability(0.5, 1.0) == pytest.approx(0.0, abs=1e-14)
    assert fock.hom_coincidence_probability(0.5, 0.0) == pytest.approx(0.5, abs=1e-14)
    assert fock.hom_visibility(0.5) == pytest.approx(1.0, abs=1e-12)


@pytest.mark.parametrize("r", [0.1, 0.3, 0.5, 0.8])
@pytest.mark.parametrize("xi", [0.0, 0.25, 0.7, 1.0])
def test_hom_matches_closed_form(r, xi):
    t = 1 - r
    analytic = r * r + t * t - 2 * r * t * xi
    assert fock.hom_coincidence_probability(r, xi) == pytest.approx(analytic, abs=1e-12)
    assert fock.hom_visibility(r) == pytest.approx(2 * r * t / (r * r + t * t), abs=1e-12)


def test_hom_dip_shape_vs_delay():
    sigma = 1.0
    delays = np.linspace(-6, 6, 25)
    dip = fock.hom_dip(delays, sigma)
    assert dip.min() == pytest.approx(0.0, abs=1e-12)
    assert dip[0] == pytest.approx(0.5, abs=1e-3)
    np.testing.assert_allclose(dip, 0.5 * (1 - np.exp(-delays ** 2 / (4 * sigma ** 2))), atol=1e-12)
    # wavepacket overlap formula vs numerical quadrature
    t = np.linspace(-20, 20, 40001)
    psi = (2 * np.pi * sigma ** 2) ** -0.25 * np.exp(-t ** 2 / (4 * sigma ** 2))
    for d in (0.5, 1.3, 2.0):
        psi_d = (2 * np.pi * sigma ** 2) ** -0.25 * np.exp(-(t - d) ** 2 / (4 * sigma ** 2))
        ov = (np.sum(psi * psi_d) * (t[1] - t[0])) ** 2
        assert fock.gaussian_wavepacket_overlap(d, sigma) == pytest.approx(ov, rel=1e-8)


@pytest.mark.parametrize("eta", [0.3, 0.75, 1.0])
def test_uniform_loss_binomial_matches_environment_mode_oracle(eta):
    rng = np.random.default_rng(5)
    u = fock.haar_unitary(3, rng)
    s = (1, 1, 1)
    thinned = apply_uniform_loss(fock.output_distribution(u, s), eta)
    oracle = lossy_output_bruteforce(u, s, eta)
    for k in set(thinned) | set(oracle):
        assert thinned.get(k, 0.0) == pytest.approx(oracle.get(k, 0.0), abs=1e-12)
    _, p_success = postselect_n(thinned, 3)
    assert p_success == pytest.approx(eta ** 3, abs=1e-12)


def test_uniform_loss_commutes_with_interferometer():
    rng = np.random.default_rng(8)
    u = fock.haar_unitary(3, rng)
    s, eta = (1, 1, 0), 0.6
    after = apply_uniform_loss(fock.output_distribution(u, s), eta)
    before = {}
    for kept, p in apply_uniform_loss({s: 1.0}, eta).items():
        if sum(kept) == 0:
            before[(0, 0, 0)] = before.get((0, 0, 0), 0.0) + p
            continue
        for t, q in fock.output_distribution(u, kept).items():
            before[t] = before.get(t, 0.0) + p * q
    for k in set(after) | set(before):
        assert after.get(k, 0.0) == pytest.approx(before.get(k, 0.0), abs=1e-12)


def test_threshold_detection():
    d = threshold_detection({(2, 0): 1.0}, eta=0.5, dark_count=0.0)
    assert d[(1, 0)] == pytest.approx(0.75)
    assert d[(0, 0)] == pytest.approx(0.25)
    d = threshold_detection({(0, 0): 1.0}, eta=0.9, dark_count=0.01)
    assert d[(1, 1)] == pytest.approx(1e-4)
    assert sum(d.values()) == pytest.approx(1.0)
