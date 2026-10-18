import math

import numpy as np
import pytest
from scipy.linalg import expm

from awen_qphotonic import fock
from awen_qphotonic import gaussian as g


@pytest.mark.parametrize("r", [0.0, 0.3, 1.0])
def test_squeezed_vacuum_quadratures_and_photons(r):
    st = g.apply(g.vacuum(1), g.squeezer(r), [0])
    assert st.cov[0, 0] == pytest.approx(math.exp(-2 * r))
    assert st.cov[1, 1] == pytest.approx(math.exp(2 * r))
    assert g.mean_photon_number(st, 0) == pytest.approx(math.sinh(r) ** 2)
    assert g.purity(st) == pytest.approx(1.0)
    assert g.vacuum_probability(st) == pytest.approx(1.0 / math.cosh(r))


def test_symplectic_matrices():
    rng = np.random.default_rng(0)
    for s in [g.squeezer(0.7, 0.4), g.rotation(1.1), g.two_mode_squeezer(0.5), g.passive(fock.haar_unitary(4, rng))]:
        assert g.is_symplectic(s)


def test_squeezing_against_truncated_fock_space():
    """Covariance-formalism moments vs the squeeze operator built in a truncated Fock basis."""
    r, cutoff = 0.6, 60
    a = np.diag(np.sqrt(np.arange(1, cutoff)), 1)
    sq = expm(0.5 * r * (a @ a - a.T @ a.T))
    psi = sq[:, 0]
    x = a + a.T
    p = -1j * (a - a.T)
    st = g.apply(g.vacuum(1), g.squeezer(r), [0])
    assert np.real(psi.conj() @ x @ x @ psi) == pytest.approx(st.cov[0, 0], rel=1e-9)
    assert np.real(psi.conj() @ p @ p @ psi) == pytest.approx(st.cov[1, 1], rel=1e-9)
    # photon-number distribution: P(2n) = tanh(r)^{2n} (2n)! / (4^n n!^2 cosh r)
    for n in range(5):
        analytic = math.tanh(r) ** (2 * n) * math.factorial(2 * n) / (4 ** n * math.factorial(n) ** 2 * math.cosh(r))
        assert abs(psi[2 * n]) ** 2 == pytest.approx(analytic, rel=1e-9)
        assert abs(psi[2 * n + 1]) ** 2 < 1e-20


def test_two_mode_squeezing_epr_and_thermal_marginal():
    r = 0.8
    st = g.apply(g.vacuum(2), g.two_mode_squeezer(r), [0, 1])
    v = st.cov
    var_xminus = v[0, 0] + v[1, 1] - 2 * v[0, 1]
    var_pplus = v[2, 2] + v[3, 3] + 2 * v[2, 3]
    assert var_xminus == pytest.approx(2 * math.exp(-2 * r))
    assert var_pplus == pytest.approx(2 * math.exp(-2 * r))
    marg = g.reduced(st, [0])
    np.testing.assert_allclose(marg.cov, math.cosh(2 * r) * np.eye(2), atol=1e-12)
    assert g.symplectic_eigenvalues(marg)[0] == pytest.approx(math.cosh(2 * r))
    assert g.purity(st) == pytest.approx(1.0)
    # same state from two opposite single-mode squeezers on a 50:50 beam splitter
    s2 = g.apply(g.apply(g.vacuum(2), g.squeezer(r), [0]), g.squeezer(-r), [1])
    s2 = g.apply(s2, g.passive(np.array([[1, 1], [-1, 1]]) / math.sqrt(2)), [0, 1])
    np.testing.assert_allclose(np.linalg.eigvalsh(s2.cov), np.linalg.eigvalsh(v), atol=1e-12)
    assert s2.cov[0, 0] + s2.cov[1, 1] - 2 * s2.cov[0, 1] == pytest.approx(2 * math.exp(-2 * r))


@pytest.mark.parametrize("eta", [0.2, 0.5, 0.9])
def test_loss_on_squeezed_vacuum(eta):
    r = 1.0
    st = g.loss(g.apply(g.vacuum(1), g.squeezer(r), [0]), eta, 0)
    assert st.cov[0, 0] == pytest.approx(eta * math.exp(-2 * r) + 1 - eta)
    assert g.mean_photon_number(st, 0) == pytest.approx(eta * math.sinh(r) ** 2)
    assert g.purity(st) < 1.0


def test_thermal_and_coherent():
    st = g.thermal(1.5)
    assert g.mean_photon_number(st, 0) == pytest.approx(1.5)
    assert g.purity(st) == pytest.approx(1 / 4.0)
    assert g.vacuum_probability(st) == pytest.approx(1 / 2.5)
    c = g.coherent(0.6 - 0.3j)
    assert g.mean_photon_number(c, 0) == pytest.approx(0.45)
    assert g.vacuum_probability(c) == pytest.approx(math.exp(-0.45))


def test_passive_interferometer_maps_coherent_amplitudes():
    rng = np.random.default_rng(4)
    u = fock.haar_unitary(3, rng)
    alpha = np.array([0.5 + 0.1j, -0.2j, 0.3])
    st = g.tensor(*[g.coherent(a) for a in alpha])
    out = g.apply(st, g.passive(u), [0, 1, 2])
    beta = u @ alpha
    np.testing.assert_allclose(out.mean[:3] + 1j * out.mean[3:], 2 * beta, atol=1e-12)
    np.testing.assert_allclose(out.cov, np.eye(6), atol=1e-12)


def test_homodyne_sampling_and_conditioning():
    rng = np.random.default_rng(9)
    r = 0.5
    st = g.apply(g.vacuum(1), g.squeezer(r), [0])
    for angle, var in [(0.0, math.exp(-2 * r)), (math.pi / 2, math.exp(2 * r))]:
        xs = g.homodyne_sample(st, 0, angle, 200_000, rng)
        assert abs(xs.mean()) < 5 * math.sqrt(var / 200_000)
        assert xs.var() == pytest.approx(var, rel=0.02)
    tmsv = g.apply(g.vacuum(2), g.two_mode_squeezer(0.7), [0, 1])
    cond = g.homodyne_condition(tmsv, 0, outcome=1.3)
    c2 = math.cosh(1.4)
    s2 = math.sinh(1.4)
    assert cond.cov[0, 0] == pytest.approx(1 / c2)
    assert cond.cov[1, 1] == pytest.approx(c2)
    assert cond.mean[0] == pytest.approx(s2 / c2 * 1.3)
    assert cond.mean[1] == pytest.approx(0.0)
