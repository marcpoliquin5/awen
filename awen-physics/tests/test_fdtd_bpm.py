import math

import numpy as np
import pytest

from awen_physics.bpm import (
    BpmGrid, gaussian_width_analytic, imaginary_distance_mode, propagate, rms_width,
)
from awen_physics.fdtd1d import reflectance_spectrum
from awen_physics.fidelity import Fidelity
from awen_physics.slab import SlabWaveguide, analytic_te_modes
from awen_physics.tmm import transfer_matrix

LAM = np.linspace(0.8, 1.4, 25)


def test_fdtd_half_space_matches_fresnel():
    coarse = reflectance_spectrum([], 2.0, LAM, 0.01)
    fine = reflectance_spectrum([], 2.0, LAM, 0.005)
    assert coarse.fidelity is Fidelity.FULL_WAVE
    e1 = np.abs(coarse.reflectance - (1 / 3) ** 2).max()
    e2 = np.abs(fine.reflectance - (1 / 3) ** 2).max()
    assert e1 < 1e-3 and e2 < e1 / 3


def test_fdtd_converges_second_order_to_transfer_matrix():
    layers = [(2.0, 0.4), (1.5, 0.3)]
    want = np.array([transfer_matrix([1.0, 2.0, 1.5, 1.45], [0.4, 0.3], lam).R for lam in LAM])
    errs = [np.abs(reflectance_spectrum(layers, 1.45, LAM, dx).reflectance - want).max() for dx in (0.02, 0.01, 0.005)]
    orders = [math.log2(errs[i] / errs[i + 1]) for i in range(2)]
    assert all(1.8 < p < 2.2 for p in orders), orders
    assert errs[-1] < 5e-4


def test_fdtd_rejects_bad_inputs():
    with pytest.raises(ValueError):
        reflectance_spectrum([(2.0, 0.403)], 1.0, LAM, 0.01)  # interface off the grid
    with pytest.raises(ValueError):
        reflectance_spectrum([(0.5, 0.4)], 1.0, LAM, 0.01)
    with pytest.raises(ValueError):
        reflectance_spectrum([(2.0, 0.4)], 1.0, LAM, 0.01, courant=1.5)


def free_space(lam=1.0, half=40.0, points=4001):
    x = np.linspace(-half, half, points)
    return x, BpmGrid(x, np.ones_like(x), lam, 1.0)


def test_bpm_gaussian_beam_matches_analytic_and_converges_in_dz():
    x, grid = free_space()
    u0 = np.exp(-(x / 2.0) ** 2)
    want = gaussian_width_analytic(2.0, 60.0, 1.0, 1.0)
    errs = []
    for dz in (2.0, 1.0, 0.5):
        p = propagate(grid, u0, 60.0, dz)
        assert p.fidelity is Fidelity.NUMERICAL_PROPAGATION
        errs.append(abs(rms_width(x, p.fields[-1]) - want) / want)
    assert errs[0] > errs[1] > errs[2] and errs[2] < 1e-3


def test_bpm_preserves_power():
    x, grid = free_space(points=1001)
    u0 = np.exp(-((x - 3) / 1.5) ** 2) * np.exp(0.3j * x)
    p = propagate(grid, u0, 40.0, 0.25, save_every=40)
    norms = np.linalg.norm(p.fields, axis=1)
    np.testing.assert_allclose(norms, norms[0], rtol=1e-12)


def slab_grid(ppu):
    x = np.arange(-10, 10, 1 / ppu) + 0.5 / ppu
    return BpmGrid(x, np.where(np.abs(x) < 1.0, 1.50, 1.45), 1.55, 1.45)


def test_imaginary_distance_mode_matches_analytic_slab_second_order():
    want = analytic_te_modes(SlabWaveguide(1.50, 1.45, 2.0), 1.55).effective_indices[0]
    errs = [abs(imaginary_distance_mode(slab_grid(p)).effective_index - want) for p in (20, 40, 80)]
    assert errs[1] < 1e-5
    orders = [math.log2(errs[i] / errs[i + 1]) for i in range(2)]
    assert all(1.8 < o < 2.2 for o in orders), orders


def test_guided_mode_keeps_its_shape_and_phase_velocity():
    grid = slab_grid(40)
    mode = imaginary_distance_mode(grid)
    u0 = mode.profile / np.linalg.norm(mode.profile)
    length = 500.0
    p = propagate(grid, u0, length, 0.5)
    overlap = np.vdot(u0, p.fields[-1])
    assert abs(overlap) > 1 - 1e-6
    # The paraxial envelope of an eigenmode advances at λ = (β² − k₀²n₀²)/(2k₀n₀)
    # per µm (not β − k₀n₀: that difference is the paraxial error), and
    # Crank–Nicolson turns λ·dz into 2·atan(λ·dz/2) per step.
    k0, n0 = grid.k0, grid.reference_index
    lam = ((k0 * mode.effective_index) ** 2 - (k0 * n0) ** 2) / (2 * k0 * n0)
    expected = 2 * math.atan(lam * 0.5 / 2) * (length / 0.5)
    diff = (np.angle(overlap) - expected + np.pi) % (2 * np.pi) - np.pi
    assert abs(diff) < 1e-6
