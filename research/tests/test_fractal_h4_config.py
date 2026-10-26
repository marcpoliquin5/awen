"""H4 registered-run entry point (--config): config checks, data generators, Haar, FLOPs, verdict, tiny run."""

import copy
import math
import sys
from pathlib import Path

import pytest

yaml = pytest.importorskip("yaml")
torch = pytest.importorskip("torch")
np = pytest.importorskip("numpy")

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
H4_YAML = FRACTAL / "configs" / "h4.yaml"
sys.path.insert(0, str(FRACTAL))
import h4_multiscale as h4  # noqa: E402


def _raw():
    return yaml.safe_load(H4_YAML.read_text())


def _write(tmp_path, cfg):
    p = tmp_path / "h4.yaml"
    p.write_text(yaml.safe_dump(cfg))
    return p


def test_registered_config_loads_and_widths_match_within_tolerance():
    cfg = h4.load_config(H4_YAML)
    assert cfg["seeds"] == [101, 102, 103] and cfg["models"] == h4.MODELS
    for t in cfg["tasks"]:
        m = h4.match_widths(t, cfg["compute_match"]["tolerance"])
        assert all(abs(v["rel_diff"]) <= 0.05 and v["width"] >= 8 for v in m.values())


def test_config_rejects_wrong_id_models_and_shapes(tmp_path):
    with pytest.raises(ValueError, match="not an H4"):
        h4.load_config(_write(tmp_path, {**_raw(), "id": "H2"}))
    with pytest.raises(ValueError, match="models must be"):
        h4.load_config(_write(tmp_path, {**_raw(), "models": ["dense_mlp", "fno"]}))
    bad = _raw()
    bad["tasks"][0]["haar_levels"] = 9
    with pytest.raises(ValueError, match="2\\^haar_levels"):
        h4.load_config(_write(tmp_path, bad))
    bad = _raw()
    bad["tasks"][1]["fno_modes"] = 300
    with pytest.raises(ValueError, match="Nyquist"):
        h4.load_config(_write(tmp_path, bad))


@pytest.mark.parametrize("shape,levels", [((16,), 3), ((8, 12), 2), ((28, 28), 2)])
def test_haar_is_orthonormal_and_invertible(shape, levels):
    x = torch.randn(3, 2, *shape, dtype=torch.float64)
    c = h4.haar(x, levels)
    assert torch.allclose(c.norm(), x.norm())
    assert torch.allclose(h4.haar(c, levels, inverse=True), x)


def test_haar_matches_hand_computed_length4():
    c = h4.haar(torch.tensor([[[1.0, 3.0, 5.0, 7.0]]], dtype=torch.float64), 2)[0, 0]
    r = math.sqrt(2)
    want = torch.tensor([16 / 2, (4 / r - 12 / r) / r, -2 / r, -2 / r], dtype=torch.float64)
    assert torch.allclose(c, want)


def test_haar_bands_sizes():
    b = h4.haar_bands((256,), 4)
    assert [(b == k).sum().item() for k in range(5)] == [16, 128, 64, 32, 16]
    b2 = h4.haar_bands((28, 28), 2)
    assert [(b2 == k).sum().item() for k in range(3)] == [49, 784 - 196, 196 - 49]


def test_dense_flops_match_torch_counter_and_hand_count():
    from torch.utils.flop_counter import FlopCounterMode

    t = h4.load_config(H4_YAML)["tasks"][2]
    model = h4.Net("dense_mlp", 64, t)
    with FlopCounterMode(display=False) as fc:
        model(torch.zeros(1, 1, 28, 28))
    assert fc.get_total_flops() == h4.flops("dense_mlp", 64, t) == 2 * (784 * 64 + 64 * 64 + 64 * 10)


def test_fno_and_haar_flop_terms():
    t = {"shape": [16], "layers": 1, "fno_modes": 4, "haar_levels": 2}
    lift_head = 2 * 16 * 3 * 2
    fft = 2 * 3 * 2.5 * 16 * 4
    assert h4.flops("fno", 3, t) == round(lift_head + 2 * 16 * 9 + fft + 8 * 4 * 9)
    assert h4.flops("haar_wavelet", 3, t) == round(lift_head + 4 * 16 * 9 + 2 * 3 * (2 * 16 + 2 * 8))


def test_burgers_linear_limit_and_mean_conservation():
    t = {"solver_grid": 64, "grid": 32, "ic_modes": 1, "ic_amplitude": 1e-6, "dt": 1e-3, "t_final": 0.5,
         "viscosity": 0.1}
    u0, u1 = h4.burgers(4, t, np.random.default_rng(0))
    assert u0.shape == u1.shape == (4, 32)
    assert np.allclose(u1, u0 * math.exp(-0.1 * 0.5), atol=1e-12)  # single mode k=1 decays as exp(-nu t)
    t.update(ic_modes=8, ic_amplitude=1.0, viscosity=0.01)
    u0, u1 = h4.burgers(4, t, np.random.default_rng(0))
    assert np.allclose(u0.mean(1), u1.mean(1)) and np.all((u1 ** 2).sum(1) < (u0 ** 2).sum(1))


def test_chirp_noise_level_is_registered_snr():
    t = h4.load_config(H4_YAML)["tasks"][1]
    noisy, clean = h4.chirps(400, t, np.random.default_rng(1))
    snr = 10 * np.log10((clean ** 2).sum() / ((noisy - clean) ** 2).sum())
    assert noisy.shape == (400, 512) and abs(snr - 10) < 0.1


@pytest.mark.parametrize("ti", [0, 1, 2])
@pytest.mark.parametrize("kind", h4.MODELS)
def test_model_shapes(kind, ti):
    t = h4.load_config(H4_YAML)["tasks"][ti]
    x = torch.randn(2, 1, *t["shape"])
    want = (2, 10) if t.get("classes") else x.shape
    assert h4.Net(kind, 6, t)(x).shape == want


def test_verdict_rule():
    tasks = [{"name": "a", "score": "relative_l2_error"}, {"name": "b", "score": "test_accuracy"}]
    rows = []
    for s, base in zip([1, 2, 3], [0.50, 0.52, 0.48]):
        rows += [{"task": "a", "model": "dense_mlp", "seed": s, "score": base},
                 {"task": "a", "model": "fno", "seed": s, "score": base - 0.1 - 0.001 * s},
                 {"task": "a", "model": "haar_wavelet", "seed": s, "score": base + 0.01},
                 {"task": "b", "model": "dense_mlp", "seed": s, "score": 0.9},
                 {"task": "b", "model": "fno", "seed": s, "score": 0.9 + 0.01 * (s - 2)},
                 {"task": "b", "model": "haar_wavelet", "seed": s, "score": 0.89}]
    v = h4.verdict(rows, tasks)
    assert v["tasks"]["a"]["candidates"]["fno"]["beats_dense"] and not v["tasks"]["a"]["killed"]
    assert v["tasks"]["b"]["killed"] and v["verdict"] == "killed"
    assert h4.t975(2) == pytest.approx(4.302653, abs=1e-4)


def test_tiny_run_config_end_to_end():
    pytest.importorskip("awen_models.train")
    cfg = copy.deepcopy(h4.load_config(H4_YAML))
    cfg["tasks"][0].update(solver_grid=256, dt=1e-3, t_final=0.1, n_train=16, n_test=8)
    cfg["tasks"][1].update(n_train=16, n_test=8)
    cfg["training"].update(epochs=1, batch_size=8)
    cfg["threads"] = 2
    g = torch.Generator().manual_seed(0)
    mnist = {"train_x": torch.rand(16, 784, generator=g), "train_y": torch.randint(0, 10, (16,), generator=g),
             "test_x": torch.rand(8, 784, generator=g), "test_y": torch.randint(0, 10, (8,), generator=g)}
    rep = h4.run_config(cfg, mnist=mnist)
    assert len(rep["runs"]) == 3 * 3 * 3 and rep["verdict"]["verdict"] in {"kept", "killed"}
    assert all(math.isfinite(r["score"]) for r in rep["runs"])
