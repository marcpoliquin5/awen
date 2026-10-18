import math

import numpy as np
import pytest

from awen_physics.clements import decompose, haar_unitary
from awen_physics.slab import SlabWaveguide, analytic_te_modes, finite_difference_te_modes


def test_slab_validation_rejects_bad_geometry():
    with pytest.raises(ValueError):
        SlabWaveguide(1.4, 1.45, 1.0)
    with pytest.raises(ValueError):
        SlabWaveguide(2.0, 1.45, 0.0)


def test_single_mode_cutoff():
    guide = SlabWaveguide(1.50, 1.45, 1.0)
    # V < π/2 means exactly one TE mode.
    assert guide.v_number(1.55) < math.pi / 2
    assert len(analytic_te_modes(guide, 1.55).effective_indices) == 1
    assert len(finite_difference_te_modes(guide, 1.55, 200, window_um=30).effective_indices) == 1


def test_effective_indices_lie_between_cladding_and_core():
    guide = SlabWaveguide(3.48, 1.444, 0.5)  # silicon in silica
    for n in analytic_te_modes(guide, 1.55).effective_indices:
        assert 1.444 < n < 3.48


def test_decompose_rejects_non_unitary():
    with pytest.raises(ValueError):
        decompose(np.array([[1.0, 1.0], [0.0, 1.0]]))


def test_decompose_permutation_and_identity():
    for u in (np.eye(5), np.eye(4)[[1, 0, 3, 2]]):
        mesh = decompose(u)
        assert np.abs(mesh.matrix() - u).max() < 1e-12


def test_haar_unitary_is_unitary():
    u = haar_unitary(10, np.random.default_rng(0))
    assert np.allclose(u.conj().T @ u, np.eye(10), atol=1e-12)
