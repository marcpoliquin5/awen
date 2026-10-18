"""AWEN-Mini with every transformer linear layer on the Rust photonic simulator."""

import pytest

awen_native = pytest.importorskip("awen_native")

from awen import Runtime  # noqa: E402

TEXT = "And now, good friends, let us go hence and speak of these things.\n"


def test_photonic_sim_with_ideal_contract_matches_cpu():
    cpu = Runtime(device="cpu").load_model("awen-mini")
    sim = Runtime(device="photonic-sim", noise=awen_native.ideal_contract()).load_model("awen-mini")
    assert sim.execution.fidelity == "simulated"
    assert abs(sim.log_likelihood(TEXT) - cpu.log_likelihood(TEXT)) < 1e-4


def test_photonic_sim_generates_and_counts_gemms():
    sim = Runtime(device="photonic-sim", seed=1).load_model("awen-mini")
    text = sim.generate("ROMEO:", max_new_tokens=4, seed=0)
    assert text.startswith("ROMEO:") and len(text) == len("ROMEO:") + 4
    executor = next(layer.matmul_executor for _, layer in sim.model.linear_layers())
    assert executor.calls == 4 * 20  # 4 forward passes x 20 linear layers


def test_noise_degrades_likelihood_and_more_photons_recover_it():
    cpu = Runtime(device="cpu").load_model("awen-mini").log_likelihood(TEXT)
    losses = {}
    for photons in (100.0, 100000.0):
        contract = awen_native.default_contract()
        contract["photons_per_mac"] = photons
        losses[photons] = Runtime(device="photonic-sim", noise=contract).load_model("awen-mini").log_likelihood(TEXT)
    assert losses[100.0] > losses[100000.0] > cpu - 0.05
    assert losses[100000.0] - cpu < 0.3
