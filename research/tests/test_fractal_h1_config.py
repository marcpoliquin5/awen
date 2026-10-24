"""H1 registered-run entry point (--config): config checks, loss-budget selection, fabrication model, tiny run."""

import json
import math
import sys
from pathlib import Path

import numpy as np
import pytest
import torch

yaml = pytest.importorskip("yaml")

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
H1_YAML = FRACTAL / "configs" / "h1.yaml"
sys.path.insert(0, str(FRACTAL))
import h1_meshes as meshes  # noqa: E402


def _tiny(tmp_path, **over):
    cfg = yaml.safe_load(H1_YAML.read_text())
    cfg["modes"] = [4]
    cfg["primary_point"]["modes"] = 4
    cfg["meshes"]["butterfly"]["depths_in_layers_of_log2N_stages"] = [1, 2]
    cfg["meshes"]["clements"]["depths"] = [2, 4]
    cfg["tasks"] = [{"name": "haar_unitary", "targets": 2, "score": "fidelity"},
                    {"name": "mnist_pca16_linear", "features": 16, "score": "test_accuracy"},
                    {"name": "dft_matrix", "score": "fidelity"}]
    cfg["optimizer"] = {"name": "adam", "lr": 0.05, "steps": 30, "restarts": 1}
    cfg["seeds"] = [101, 102, 103]
    cfg["fabrication_noise_seeds"] = [201, 202]
    cfg.update(over)
    p = tmp_path / "h1_tiny.yaml"
    p.write_text(yaml.safe_dump(cfg))
    return p


def test_registered_config_loads():
    cfg = meshes.load_config(H1_YAML)
    assert cfg["id"] == "H1" and cfg["seeds"] == [101, 102, 103, 104, 105]
    assert cfg["fabrication_noise_seeds"] == [201, 202, 203, 204, 205]


def test_config_rejections(tmp_path):
    with pytest.raises(ValueError, match="pilot seeds"):
        meshes.load_config(_tiny(tmp_path, seeds=[2, 101]))
    with pytest.raises(ValueError, match="not an H1"):
        meshes.load_config(_tiny(tmp_path, id="H3"))
    with pytest.raises(ValueError, match="overlap"):
        meshes.load_config(_tiny(tmp_path, seeds=[201, 101]))
    with pytest.raises(ValueError, match="powers of 2"):
        meshes.load_config(_tiny(tmp_path, modes=[4, 6]))


def test_primary_point_must_be_on_grid(tmp_path):
    cfg = yaml.safe_load(_tiny(tmp_path).read_text())
    cfg["primary_point"]["budget_db"] = 7.0
    p = tmp_path / "bad.yaml"
    p.write_text(yaml.safe_dump(cfg))
    with pytest.raises(ValueError, match="budget_db"):
        meshes.load_config(p)


def test_deepest_within_budget():
    n = 16
    b = {d: meshes.budget("clements", n, meshes.clements_columns(n, d)) for d in (2, 4, 8, 16)}
    # Clements: no crossings; worst path passes one MZI per column it touches
    assert all(x.max_crossings_per_path == 0 for x in b.values())
    loss16 = b[16].path_loss_db(0.25, 0.05)
    assert meshes.deepest_within_budget(b, 0.25, 0.05, loss16) == 16
    assert meshes.deepest_within_budget(b, 0.25, 0.05, loss16 - 1e-6) == 8
    assert meshes.deepest_within_budget(b, 0.25, 0.05, 0.0) is None


def test_physical_mzi_matches_convention_at_zero_imbalance():
    g = torch.Generator().manual_seed(7)
    theta = torch.rand(5, generator=g, dtype=torch.float64) * math.pi
    phi = torch.rand(5, generator=g, dtype=torch.float64) * 2 * math.pi
    zero = torch.zeros(5, dtype=torch.float64)
    u = meshes.physical_mzi(theta, phi, zero, zero)
    c, s = torch.cos(theta), torch.sin(theta)
    e = torch.exp(1j * phi)
    ref = torch.stack([torch.stack([e * c, -s.to(torch.complex128)], -1),
                       torch.stack([e * s, c.to(torch.complex128)], -1)], -2)
    assert torch.allclose(u, ref, atol=1e-12)
    # imbalanced couplers stay unitary (lossless)
    eta = torch.full((5,), 0.05, dtype=torch.float64)
    v = meshes.physical_mzi(theta, phi, eta, -eta)
    eye = torch.eye(2, dtype=torch.complex128).expand(5, 2, 2)
    assert torch.allclose(v @ v.conj().transpose(-1, -2), eye, atol=1e-12)


def test_fabricated_unitary_noise_free_equals_mesh():
    n = 8
    cols = meshes.butterfly_columns(n, 1)
    g = torch.Generator().manual_seed(3)
    m = meshes.mzi_count(cols)
    th = torch.rand(2, m, generator=g, dtype=torch.float64) * math.pi
    ph = torch.rand(2, m, generator=g, dtype=torch.float64) * 2 * math.pi
    out = torch.rand(2, n, generator=g, dtype=torch.float64) * 2 * math.pi
    ideal = meshes.mesh_unitary(n, cols, th, ph, out)
    assert torch.allclose(meshes.fabricated_unitary(n, cols, th, ph, out, 0.0, 0.0, 201), ideal, atol=1e-12)
    noisy = meshes.fabricated_unitary(n, cols, th, ph, out, 0.02, 0.02, 201)
    assert torch.equal(noisy, meshes.fabricated_unitary(n, cols, th, ph, out, 0.02, 0.02, 201))
    f = meshes.fidelity(ideal, noisy)
    assert torch.all((f < 1) & (f > 0.9))


def test_dft_unitary_and_t_quantile():
    f = meshes.dft_unitary(8)
    assert np.allclose(f @ f.conj().T, np.eye(8), atol=1e-12)
    # textbook Student-t 0.975 quantiles: df 4 -> 2.776, df 2 -> 4.303
    assert meshes.t_quantile(0.975, 4) == pytest.approx(2.776, abs=1e-3)
    assert meshes.t_quantile(0.975, 2) == pytest.approx(4.303, abs=1e-3)
    r = meshes.paired_t([0.1, 0.12, 0.11, 0.09, 0.1])
    assert r["n"] == 5 and r["beats"] and r["ci95"][0] > 0
    assert not meshes.paired_t([0.1, -0.1, 0.05, -0.05, 0.0])["beats"]


def test_tiny_run_writes_report(tmp_path):
    out = tmp_path / "out"
    meshes.main(["--config", str(_tiny(tmp_path)), "--out", str(out)])
    rep = json.loads((out / "h1_meshes_registered.json").read_text())
    assert rep["not_built"] == {"families": ["low_rank_svd"], "tasks": ["mnist_pca16_linear"]}
    assert set(rep["scores_per_init_seed"]) == {"haar_unitary", "dft_matrix"}
    per_seed = rep["scores_per_init_seed"]["dft_matrix"]["4"]["clements"]["4"]
    assert len(per_seed) == 3 and all(0 <= x <= 1 for x in per_seed)
    cmp = rep["comparisons_at_primary_point"]["haar_unitary"]
    assert cmp["selected"] == {"butterfly": 2, "clements": 4}
    assert cmp["butterfly_minus_clements"]["n"] == 3
    assert rep["verdict"].startswith("not decided")
