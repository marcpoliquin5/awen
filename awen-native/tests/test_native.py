"""End-to-end checks of the Rust photonic model through the Python binding."""

import json
import math
from pathlib import Path

import numpy as np
import pytest

import awen_native

REPO = Path(__file__).resolve().parents[2]


def shot_only(photons_per_mac: float) -> dict:
    contract = awen_native.ideal_contract()
    contract.update(photons_per_mac=photons_per_mac, quantum_efficiency=1.0, insertion_loss_db=0.0)
    return contract


def test_ideal_contract_reproduces_numpy():
    rng = np.random.default_rng(0)
    x = rng.uniform(-1, 1, (5, 70))
    w = rng.uniform(-1, 1, (9, 70))
    y, report = awen_native.photonic_gemm(x, w, awen_native.ideal_contract(), tile_k=32)
    np.testing.assert_allclose(y, x @ w.T, rtol=0, atol=1e-12)
    assert report["fidelity"] == "compact_model"
    assert report["tiles_per_output"] == 3


def test_shot_noise_rms_matches_theory():
    # One tile, full-scale operands of all ones so every output is at full scale.
    k, samples, photons = 16, 4000, 50.0
    x = np.ones((samples, k))
    w = np.ones((1, k))
    y, _ = awen_native.photonic_gemm(x, w, shot_only(photons), tile_k=k, seed=7)
    relative_sigma = np.std(y[:, 0] / k)
    expected = 1 / math.sqrt(photons * k)
    assert abs(relative_sigma / expected - 1) < 0.05


def test_measured_enob_matches_shot_limited_theory():
    k, samples, photons = 32, 4000, 200.0
    x = np.ones((samples, k))
    w = np.ones((1, k))
    y, _ = awen_native.photonic_gemm(x, w, shot_only(photons), tile_k=k, seed=3)
    sigma = np.std(y[:, 0] / k)
    measured = awen_native.effective_bits_from_sigma(sigma)
    theory = awen_native.shot_limited_effective_bits(photons * k)
    assert abs(measured - theory) < 0.1


def test_predicted_budget_matches_simulated_error():
    contract = awen_native.default_contract()
    contract.update(crosstalk_db=None, phase_sigma_rad=0.0, dac_bits=None)
    # Half-scale signal: half the inputs are on, so the ADC does not clip.
    k, samples = 64, 4000
    x = np.zeros((samples, k))
    x[:, : k // 2] = 1.0
    w = np.ones((1, k))
    y, _ = awen_native.photonic_gemm(x, w, contract, tile_k=k, seed=11)
    predicted = awen_native.predicted_noise(k, 0.5, contract)
    measured = np.sqrt(np.mean((y[:, 0] / k - 0.5) ** 2))
    assert abs(measured / predicted["total"] - 1) < 0.08


def test_seeded_runs_are_deterministic():
    rng = np.random.default_rng(1)
    x, w = rng.normal(size=(3, 40)), rng.normal(size=(4, 40))
    a, _ = awen_native.photonic_gemm(x, w, seed=5)
    b, _ = awen_native.photonic_gemm(x, w, seed=5)
    c, _ = awen_native.photonic_gemm(x, w, seed=6)
    np.testing.assert_array_equal(a, b)
    assert not np.array_equal(a, c)


def test_invalid_inputs_raise_value_error():
    bad = awen_native.default_contract()
    bad["quantum_efficiency"] = 1.5
    with pytest.raises(ValueError):
        awen_native.validate_contract(bad)
    with pytest.raises(ValueError):
        awen_native.photonic_gemm(np.ones((2, 3)), np.ones((2, 4)))
    with pytest.raises(ValueError):
        awen_native.photonic_gemm(np.ones((2, 3)), np.ones((2, 3)), tile_k=0)


def test_default_contract_matches_spec_fixture():
    fixture = json.loads((REPO / "awen-spec/fixtures/photonic_noise_contract.default.json").read_text())
    assert awen_native.default_contract() == fixture


def test_compile_program_uses_the_rust_compiler():
    program = json.loads((REPO / "awen-compiler/examples/gemm_4x4.json").read_text())
    capabilities = json.loads((REPO / "awen-compiler/capabilities/reference_2x2.json").read_text())
    artifact = awen_native.compile_program(program, capabilities)
    assert artifact["placement"][0]["op_id"] == "test_projection"
    assert artifact["backend_id"] == capabilities["backend_id"]
