import math

import numpy as np
import pytest

from awen_physics.fidelity import Fidelity
from awen_physics.rings import (
    add_drop_analytic, all_pass_analytic, all_pass_round_trips, amplitude_from_loss,
    free_spectral_range_um, fwhm_um,
)
from awen_physics.tmm import quarter_wave_mirror_reflectance, scattering_matrix, transfer_matrix


def fresnel(n1, n2, theta, pol):
    c1 = math.cos(theta)
    c2 = math.sqrt(1 - (n1 / n2 * math.sin(theta)) ** 2)
    if pol == "s":
        return (n1 * c1 - n2 * c2) / (n1 * c1 + n2 * c2)
    return (n2 * c1 - n1 * c2) / (n2 * c1 + n1 * c2)


@pytest.mark.parametrize("pol", ["s", "p"])
@pytest.mark.parametrize("theta", [0.0, 0.3, 0.7])
def test_bare_interface_matches_fresnel(pol, theta):
    res = transfer_matrix([1.0, 1.0, 1.5], [0.2], 1.55, theta, pol)  # ambient-index spacer layer
    want = fresnel(1.0, 1.5, theta, pol) ** 2
    assert res.R == pytest.approx(want, abs=1e-12)
    assert res.R + res.T == pytest.approx(1.0, abs=1e-12)
    assert res.fidelity is Fidelity.ANALYTIC


def test_brewster_angle_p_reflection_vanishes():
    res = transfer_matrix([1.0, 1.0, 1.5], [0.0], 0.8, math.atan(1.5), "p")
    assert res.R < 1e-25


@pytest.mark.parametrize("pairs", [1, 4, 10])
def test_quarter_wave_mirror_matches_closed_form(pairs):
    lam, nh, nl = 1.55, 2.2, 1.46
    idx = [1.0] + [nh, nl] * pairs + [1.52]
    d = [lam / (4 * nh), lam / (4 * nl)] * pairs
    want = quarter_wave_mirror_reflectance(1.0, nh, nl, 1.52, pairs)
    assert transfer_matrix(idx, d, lam).R == pytest.approx(want, abs=1e-12)
    assert scattering_matrix(idx, d, lam).R == pytest.approx(want, abs=1e-12)


def test_half_wave_layer_is_absentee():
    lam = 1.31
    res = transfer_matrix([1.0, 2.0, 1.5], [lam / (2 * 2.0)], lam)
    assert res.R == pytest.approx(fresnel(1.0, 1.5, 0, "s") ** 2, abs=1e-12)


@pytest.mark.parametrize("pol", ["s", "p"])
def test_smatrix_agrees_with_transfer_matrix_on_random_lossy_stacks(pol):
    rng = np.random.default_rng(0)
    for _ in range(20):
        n_layers = rng.integers(1, 8)
        idx = [1.0] + list(rng.uniform(1.3, 3.5, n_layers) + 1j * rng.uniform(0, 0.05, n_layers)) + [1.45]
        d = list(rng.uniform(0.05, 0.6, n_layers))
        theta = rng.uniform(0, 1.2)
        a = transfer_matrix(idx, d, 1.55, theta, pol)
        b = scattering_matrix(idx, d, 1.55, theta, pol)
        assert abs(a.r - b.r) < 1e-12 and abs(a.T - b.T) < 1e-12
        assert a.A >= -1e-12


def test_weak_absorber_follows_beer_lambert():
    lam, kappa, d = 1.0, 1e-3, 20.0
    res = transfer_matrix([1.5, 1.5 + 1j * kappa, 1.5], [d], lam)
    assert res.T == pytest.approx(math.exp(-4 * math.pi * kappa * d / lam), rel=1e-6)


def test_smatrix_stable_where_transfer_matrix_overflows():
    # Thick strongly absorbing layer: characteristic-matrix entries overflow.
    res = scattering_matrix([1.0, 3.0 + 1.0j, 1.5], [400.0], 1.0)
    assert np.isfinite(res.R) and res.T < 1e-300
    assert res.R == pytest.approx(abs((1 - (3 + 1j)) / (1 + 3 + 1j)) ** 2, rel=1e-12)


def test_stack_validation():
    with pytest.raises(ValueError):
        transfer_matrix([1.0, 1.5], [0.1], 1.0)
    with pytest.raises(ValueError):
        transfer_matrix([1.0, 1.5, 1.5], [-0.1], 1.0)
    with pytest.raises(ValueError):
        transfer_matrix([1.0, 1.5, 1.5], [0.1], 1.0, pol="x")


# --- rings -----------------------------------------------------------------

def test_round_trip_propagation_converges_to_analytic():
    phi = np.linspace(-np.pi, np.pi, 801)
    for r, a in [(0.9, 0.95), (0.98, 0.99), (0.7, 0.7)]:
        num = all_pass_round_trips(phi, r, a)
        assert num.fidelity is Fidelity.NUMERICAL_PROPAGATION
        np.testing.assert_allclose(num.transmission, all_pass_analytic(phi, r, a), atol=1e-11)


def test_critical_coupling_and_lossless_ring():
    assert all_pass_analytic(0.0, 0.95, 0.95) == pytest.approx(0.0, abs=1e-15)
    phi = np.linspace(0, 2 * np.pi, 50)
    np.testing.assert_allclose(all_pass_analytic(phi, 0.9, 1.0), 1.0, atol=1e-12)


def test_add_drop_conserves_power_without_loss_and_drops_fully_when_symmetric():
    phi = np.linspace(-np.pi, np.pi, 301)
    t, d = add_drop_analytic(phi, 0.93, 0.93, 1.0)
    np.testing.assert_allclose(t + d, 1.0, atol=1e-12)
    assert d[150] == pytest.approx(1.0, abs=1e-12)  # phi = 0
    t, d = add_drop_analytic(phi, 0.93, 0.93, 0.98)
    assert np.all(t + d < 1.0)


def test_fsr_and_fwhm_from_a_sampled_dispersive_spectrum():
    radius = 10.0
    length = 2 * math.pi * radius
    lam0, neff0, ng = 1.55, 2.4, 4.2
    a = amplitude_from_loss(3.0, length)
    r = 0.97
    lam = np.linspace(1.53, 1.57, 400_001)
    neff = neff0 - (ng - neff0) * (lam - lam0) / lam0  # first-order dispersion
    phi = 2 * math.pi * neff * length / lam
    t = all_pass_analytic(phi, r, a)
    mins = np.where((t[1:-1] < t[:-2]) & (t[1:-1] < t[2:]))[0] + 1
    centre = mins[np.argmin(abs(lam[mins] - lam0))]
    nxt = mins[np.searchsorted(mins, centre) + 1]
    # With linear n_eff(λ) the group index is exactly ng everywhere, so adjacent
    # resonances satisfy Δλ = λ1 λ2 / (n_g L): FSR evaluated at their geometric mean.
    lam_mid = math.sqrt(lam[nxt] * lam[centre])
    assert lam[nxt] - lam[centre] == pytest.approx(free_spectral_range_um(lam_mid, ng, length), rel=1e-4)
    # FWHM of the dip: width where 1 - T reaches half its peak.
    dip = 1 - t
    half = dip[centre] / 2
    lo = centre
    while dip[lo] > half:
        lo -= 1
    hi = centre
    while dip[hi] > half:
        hi += 1
    assert lam[hi] - lam[lo] == pytest.approx(fwhm_um(lam[centre], ng, length, r, a), rel=1e-2)


def test_ring_validation():
    with pytest.raises(ValueError):
        all_pass_analytic(0.0, 1.2, 0.9)
    with pytest.raises(ValueError):
        all_pass_round_trips(0.0, 1.0, 1.0)
