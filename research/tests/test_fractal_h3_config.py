"""H3 registered-run entry point (--config): config validation, perturbation, channel metrics, tiny run."""

import json
import sys
from pathlib import Path

import numpy as np
import pytest

yaml = pytest.importorskip("yaml")

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
H3_YAML = FRACTAL / "configs" / "h3.yaml"
sys.path.insert(0, str(FRACTAL))
import h3_stacks as stacks  # noqa: E402


def _tiny(tmp_path, **over):
    cfg = yaml.safe_load(H3_YAML.read_text())
    cfg["cantor_generations"] = [1, 2]
    cfg["free_parameters"]["thickness_sigma_fraction"] = [0.0, 0.02]
    cfg["seeds"] = [101, 102]
    cfg["frequency_grid"]["points"] = 801
    cfg.update(over)
    p = tmp_path / "h3_tiny.yaml"
    p.write_text(yaml.safe_dump(cfg))
    return p


def test_registered_config_loads():
    cfg = stacks.load_config(H3_YAML)
    assert cfg["id"] == "H3" and cfg["seeds"] == [101, 102, 103, 104, 105]


def test_config_rejects_pilot_seeds_and_wrong_id(tmp_path):
    with pytest.raises(ValueError, match="pilot seeds"):
        stacks.load_config(_tiny(tmp_path, seeds=[1, 101]))
    with pytest.raises(ValueError, match="not an H3"):
        stacks.load_config(_tiny(tmp_path, id="H1"))


def test_unperturbed_matches_pilot_spectrum():
    omega = np.linspace(0.5, 1.5, 301)
    w = stacks.cantor_word(3)
    ref = stacks.spectrum(w, 2.3, 1.38, omega)
    got = stacks.transmission(stacks.perturbed_stack_matrix(w, 2.3, 1.38, omega, np.ones(len(w))))
    assert np.allclose(got, ref, atol=1e-12)


def test_thickness_factors_seeded_and_lossless():
    a = stacks.thickness_factors(27, 0.02, 101)
    assert np.array_equal(a, stacks.thickness_factors(27, 0.02, 101))
    assert not np.array_equal(a, stacks.thickness_factors(27, 0.02, 102))
    assert np.array_equal(stacks.thickness_factors(5, 0.0, 101), np.ones(5))
    omega = np.linspace(0.5, 1.5, 101)
    m = stacks.perturbed_stack_matrix(stacks.cantor_word(2), 2.3, 1.38, omega, a[:9])
    assert np.allclose(np.linalg.det(m), 1.0, atol=1e-9)  # lossless layers keep det = 1
    t = stacks.transmission(m)
    assert np.all((t >= 0) & (t <= 1 + 1e-12))


def test_channel_metrics_on_synthetic_spectrum():
    omega = np.linspace(0.0, 1.0, 11)
    t = np.array([0.9, 0.9, 0.001, 0.8, 0.8, 0.5, 0.01, 0.01, 0.6, 0.6, 0.0])
    c = stacks.channels(t, omega, 0.5, 15.0)
    assert c["channels"] == 3
    # crosstalk 0-1: 10log10(0.001/0.8) = -29.03 dB; 1-2: 10log10(0.01/0.6) = -17.78 dB
    assert c["worst_adjacent_crosstalk_db"] == pytest.approx(10 * np.log10(0.01 / 0.6), abs=1e-6)
    assert c["usable_channels"] == 3
    assert stacks.channels(t, omega, 0.5, 20.0)["usable_channels"] == 1
    assert stacks.channels(np.full(5, 0.1), np.linspace(0, 1, 5), 0.5, 15.0)["channels"] == 0


def test_tiny_run_writes_report(tmp_path):
    out = tmp_path / "out"
    stacks.main(["--config", str(_tiny(tmp_path)), "--out", str(out)])
    rep = json.loads((out / "h3_channel_plans.json").read_text())
    assert len(rep["runs"]) == 2 * 2 * 2
    assert rep["uniform_grid"]["status"] == "not run"
    assert {r["footprint"]["layers"] for r in rep["runs"]} == {3, 9}
    assert all(r["usable_channels"] <= r["channels"] for r in rep["runs"])
