import math

import numpy as np
import pytest

from awen_physics import shg
from awen_physics.fdfd_adjoint import Grid, probe_intensity_gradient, slab_reflectance, solve
from awen_physics.fidelity import Fidelity
from awen_physics.tmm import transfer_matrix


# --- χ(2) second-harmonic generation -----------------------------------------

def test_low_depletion_phase_matched_matches_quadratic_growth():
    res = shg.integrate(1.0, 1e-6, 1000.0)  # conversion ~1e-6
    assert res.fidelity is Fidelity.NUMERICAL_PROPAGATION
    assert res.efficiency == pytest.approx(shg.undepleted(1.0, 1e-6, 1000.0), rel=1e-5)
    # P₂ ∝ z² along the crystal.
    z = res.z_um[1:]
    np.testing.assert_allclose(res.harmonic_power[1:] / z**2, 1e-12, rtol=1e-5)


@pytest.mark.parametrize("delta_k", [0.002, 0.01, 0.031])
def test_low_depletion_mismatch_follows_sinc_squared(delta_k):
    res = shg.integrate(1.0, 1e-6, 1000.0, delta_k)
    assert res.efficiency == pytest.approx(shg.undepleted(1.0, 1e-6, 1000.0, delta_k), rel=1e-5)


def test_mismatched_harmonic_oscillates_with_coherence_length():
    dk = 0.05
    res = shg.integrate(1.0, 1e-6, 4 * math.pi / dk, dk, points=4001)
    peak = res.z_um[np.argmax(res.harmonic_power)]
    assert peak == pytest.approx(math.pi / dk, rel=2e-3)  # maximum at one coherence length
    assert res.harmonic_power[-1] < 1e-20  # back to zero after two


def test_depleted_phase_matched_matches_tanh_squared_and_conserves_power():
    res = shg.integrate(1.0, 1e-2, 200.0)
    assert res.efficiency == pytest.approx(shg.phase_matched_depleted(1.0, 1e-2, 200.0), rel=1e-9)
    assert res.efficiency > 0.9
    np.testing.assert_allclose(res.fundamental_power + res.harmonic_power, 1.0, atol=1e-10)


def test_first_order_qpm_recovers_two_over_pi_squared():
    dk = 0.05
    g, period = shg.periodic_poling(dk, 1e-6)
    length = 20 * period
    res = shg.integrate(1.0, 1e-6, length, dk, g, np.arange(0, length, period / 2))
    assert res.efficiency == pytest.approx(shg.qpm_first_order_undepleted(1.0, 1e-6, length), rel=1e-5)
    unpoled = shg.integrate(1.0, 1e-6, length, dk).efficiency
    assert res.efficiency > 1e3 * unpoled


def test_shg_validation():
    with pytest.raises(ValueError):
        shg.integrate(-1.0, 1e-3, 10.0)
    with pytest.raises(ValueError):
        shg.periodic_poling(0.0, 1e-3)


# --- frequency-domain FD and adjoint gradients -------------------------------

def test_fdfd_reflectance_converges_second_order_to_transfer_matrix():
    lams = np.linspace(0.8, 1.4, 13)
    want = np.array([transfer_matrix([1.0, 2.0, 1.5, 1.45], [0.4, 0.3], lam).R for lam in lams])
    errs = []
    for h in (0.02, 0.01, 0.005):
        got = np.array([slab_reflectance([(4.0, 0.4), (2.25, 0.3)], 1.45**2, lam, h) for lam in lams])
        errs.append(np.abs(got - want).max())
    orders = [math.log2(errs[i] / errs[i + 1]) for i in range(2)]
    assert all(1.8 < p < 2.2 for p in orders), orders
    assert errs[-1] < 1e-3


def test_radiation_boundaries_do_not_reflect_in_vacuum():
    h, n = 0.005, 2001
    src = np.zeros(n)
    src[1000] = 1 / h
    e = solve(Grid(np.ones(n, dtype=complex), h, 1.0), src)
    # Outgoing waves only: |E| is constant on each side of the source.
    left, right = np.abs(e[50:950]), np.abs(e[1050:1950])
    assert np.ptp(left) / left.mean() < 1e-3 and np.ptp(right) / right.mean() < 1e-3


def random_design():
    rng = np.random.default_rng(0)
    n, h = 400, 0.01
    eps = np.ones(n, dtype=complex)
    eps[150:250] = 1 + rng.uniform(0, 3, 100) + 1j * rng.uniform(0, 0.05, 100)
    eps[-1] = 2.1  # substrate pixel at the boundary
    src = np.zeros(n)
    src[50] = 1 / h
    return Grid(eps, h, 1.0), src


def test_adjoint_gradient_matches_central_finite_differences():
    grid, src = random_design()
    res = probe_intensity_gradient(grid, src, 350)
    assert res.fidelity is Fidelity.FULL_WAVE
    d = 1e-6
    for i in [0, 120, 150, 173, 200, 249, 300, 399]:
        ep, em = grid.eps.copy(), grid.eps.copy()
        ep[i] += d
        em[i] -= d
        fd = (abs(solve(Grid(ep, grid.h_um, 1.0), src)[350]) ** 2 - abs(solve(Grid(em, grid.h_um, 1.0), src)[350]) ** 2) / (2 * d)
        assert res.gradient[i] == pytest.approx(fd, rel=1e-5, abs=1e-12), i


def test_adjoint_gradient_step_increases_objective():
    grid, src = random_design()
    res = probe_intensity_gradient(grid, src, 350)
    mask = np.zeros(grid.eps.size)
    mask[150:250] = 1
    step = 1e-2 * mask * res.gradient / np.abs(res.gradient[150:250]).max()
    after = probe_intensity_gradient(Grid(grid.eps + step, grid.h_um, 1.0), src, 350).objective
    predicted = res.objective + float(np.dot(res.gradient, step))
    assert after > res.objective
    assert after == pytest.approx(predicted, rel=1e-3)


def test_fdfd_validation():
    with pytest.raises(ValueError):
        solve(Grid(np.ones(10, dtype=complex), 0.1, 1.0), np.zeros(9))
    with pytest.raises(ValueError):
        solve(Grid(np.full(10, 1 - 0.1j), 0.1, 1.0), np.zeros(10))
