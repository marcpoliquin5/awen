"""Unit tests for the E-038 accuracy axis on synthetic evaluations (the registered run is not run here)."""
import copy
import math

import pytest

yaml = pytest.importorskip("yaml")

from awen_models import accuracy_energy_mac as aem
from awen_models import energy_accounting as ea

SHAPES = [(128, 384), (128, 128), (128, 512), (512, 128)]  # synthetic (inner, outputs)
P, TILE = 128, 64


@pytest.fixture(scope="module")
def doc():
    return yaml.safe_load(ea.find_assumptions().read_text())


def _evals(expected, shift=0.0):
    out = {}
    for label in aem.contract_labels():
        noise = 0.0 if label == "ideal" else 0.01 * (9 - int(label[4:]))
        out[label] = {seed: {"sim_nll": [1.5 + noise + 0.0004 * seed + (shift if label == "ideal" else 0.0)] * aem.WINDOWS,
                             "cpu_nll": [1.5] * aem.WINDOWS, "counts": [dict(expected)] * aem.WINDOWS,
                             "min_tile_effective_bits": None if label == "ideal" else 5.0}
                      for seed in aem.SEEDS}
    return out


def test_closed_form_counts():
    got = aem.closed_form(SHAPES, P, TILE)
    assert got["lowered_macs"] == P * (128 * 384 + 128 * 128 + 128 * 512 + 512 * 128)
    assert got["dac_input"] == P * (128 * 3 + 512)
    assert got["dac_weight"] == 128 * 384 + 128 * 128 + 128 * 512 + 512 * 128
    assert got["adc"] == P * (384 * 2 + 128 * 2 + 512 * 2 + 128 * 8)


def test_pass_pairs_accuracy_with_energy_per_lowered_mac(doc):
    expected = aem.closed_form(SHAPES, P, TILE)
    out = aem.assemble(_evals(expected), expected, P, TILE, doc, host_fraction=0.2)
    assert out["verdict"] == "pass", out["failures"]
    assert out["curve"] == aem.CURVE and out["evidence_level"] == "L1"
    energy = out["energy_per_lowered_mac"]
    assert energy["lowered_macs_per_token"] == expected["lowered_macs"] // P
    for name in ("dml_laser", "modulator_eo"):
        env = energy["by_transmitter"][name]
        assert 0 < env["min_J_per_mac"] <= env["max_J_per_mac"]
    for b in aem.BITS:
        point = out["accuracy"][f"bits{b}"]
        assert point["optical_detection_floor_J_per_mac"] == pytest.approx(
            energy["optical_detection_floor_J_per_mac"]["by_effective_bits"][str(b)])
        assert point["sim_nll_nats"]["n"] == 3 and point["sim_nll_nats"]["ci95_half_width"] > 0
    assert out["accuracy"]["ideal"]["nll_diff_nats"]["mean"] == pytest.approx(0.0004)
    assert "no speedup or savings" in out["claims"]


def test_ci95_uses_student_t():
    got = aem.ci95([1.0, 2.0, 3.0])
    assert got["mean"] == 2.0 and got["sd"] == 1.0
    assert got["ci95_half_width"] == pytest.approx(4.303 / math.sqrt(3))


def test_count_mismatch_fails(doc):
    expected = aem.closed_form(SHAPES, P, TILE)
    evals = _evals(expected)
    evals["bits6"][1]["counts"] = [dict(expected, adc=expected["adc"] + 1)] * aem.WINDOWS
    out = aem.assemble(evals, expected, P, TILE, doc)
    assert out["verdict"] == "fail" and "bits6" in out["failures"]


def test_ideal_must_reproduce_cpu(doc):
    expected = aem.closed_form(SHAPES, P, TILE)
    out = aem.assemble(_evals(expected, shift=0.01), expected, P, TILE, doc)
    assert out["verdict"] == "fail" and "ideal" in out["failures"]


def test_unregistered_inputs_or_grid_fail(doc):
    expected = aem.closed_form(SHAPES, P, TILE)
    out = aem.assemble(_evals(expected), expected, P, TILE, doc,
                       input_sha256={"checkpoint": "0", "assumptions": "0", "corpus": "0"})
    assert out["verdict"] == "fail" and "inputs" in out["failures"]
    evals = _evals(expected)
    del evals["bits4"][2]
    assert "inputs" in aem.assemble(evals, expected, P, TILE, doc)["failures"]


def test_non_finite_nll_fails(doc):
    expected = aem.closed_form(SHAPES, P, TILE)
    evals = copy.deepcopy(_evals(expected))
    evals["bits4"][0]["sim_nll"] = [float("nan")] * aem.WINDOWS
    assert "bits4" in aem.assemble(evals, expected, P, TILE, doc)["failures"]
