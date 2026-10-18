"""Frontier-physics checks reproduce established results."""

import math
import sys
from pathlib import Path

import numpy as np
import pytest

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "frontier-physics"))
import frontier_checks as f  # noqa: E402


def test_casimir_pressure_matches_casimir_1948_value():
    # Casimir 1948: F/A = 0.013 dyn/cm^2 / a_um^4  ->  1.3e-3 Pa at 1 um.
    assert f.casimir_pressure(1e-6) == pytest.approx(1.30e-3, rel=1e-3)
    assert f.casimir_pressure(100e-9) == pytest.approx(f.casimir_pressure(1e-6) * 1e4)


def test_pressure_is_minus_energy_gradient():
    a, h = 500e-9, 1e-12
    grad = (f.casimir_energy_per_area(a + h) - f.casimir_energy_per_area(a - h)) / (2 * h)
    # Force per area -dE/da is negative (attractive); its magnitude is the pressure.
    assert grad == pytest.approx(f.casimir_pressure(a), rel=1e-6)


def test_closed_cycle_does_no_net_work():
    path = np.array([1e-6, 200e-9, 50e-9, 400e-9, 1e-6])
    assert abs(f.cycle_work_per_area(path)) < 1e-18
    with pytest.raises(ValueError):
        f.cycle_work_per_area(np.array([1e-6, 2e-6]))
    with pytest.raises(ValueError):
        f.casimir_pressure(0.0)


def test_shannon_and_shot_noise_capacity():
    assert f.shannon_capacity_bits(1.0, 1e9) == pytest.approx(1e9)
    assert f.shot_noise_bits_per_sample(3.0) == pytest.approx(1.0)


@pytest.mark.parametrize("lt,lr,z,wl", [(50e-6, 50e-6, 500e-6, 1e-6), (100e-6, 40e-6, 400e-6, 1.55e-6),
                                         (30e-6, 30e-6, 100e-6, 1e-6)])
def test_mode_count_matches_paraxial_estimate(lt, lr, z, wl):
    count = f.well_coupled_modes(lt, lr, z, wl)
    assert abs(count - f.paraxial_mode_estimate(lt, lr, z, wl)) <= 1.5


def test_mode_count_grows_with_aperture():
    assert f.well_coupled_modes(80e-6, 80e-6, 400e-6, 1e-6) > f.well_coupled_modes(40e-6, 40e-6, 400e-6, 1e-6)


def test_sierpinski_box_dimension():
    dim = f.box_counting_dimension(f.sierpinski_points(8), [2, 4, 8, 16, 32, 64])
    assert dim == pytest.approx(math.log(3) / math.log(2), abs=1e-6)


def test_anapole_cancels_far_field():
    p, k = 1.0 + 0.5j, 2 * math.pi / 1.55e-6
    t = f.anapole_toroidal_moment(p, k)
    assert f.relative_radiated_power(p, t, k) == pytest.approx(0.0, abs=1e-20)
    assert f.relative_radiated_power(p, 0.0, k) == pytest.approx(abs(p) ** 2)


def test_simulator_effective_bits_respect_capacity_bound():
    awen_native = pytest.importorskip("awen_native")
    for photons in (10.0, 100.0, 1e3, 1e4, 1e5):
        assert awen_native.shot_limited_effective_bits(photons) <= f.shot_noise_bits_per_sample(photons)
