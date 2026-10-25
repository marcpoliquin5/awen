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
    cfg = yaml.safe_load(_tiny(tmp_path).read_text())
    cfg["meshes"]["low_rank_svd"]["ranks"] = [1, 2]
    p = tmp_path / "h1_tiny_lr.yaml"
    p.write_text(yaml.safe_dump(cfg))
    meshes.main(["--config", str(p), "--out", str(out)])
    rep = json.loads((out / "h1_meshes_registered.json").read_text())
    assert rep["not_built"] == {"families": [], "tasks": []}
    assert set(rep["scores_per_init_seed"]) == {"haar_unitary", "dft_matrix"}
    assert rep["skipped"] == {"mnist_pca16_linear": {"4": "not applicable: 16 features need 16 modes"}}
    per_seed = rep["scores_per_init_seed"]["dft_matrix"]["4"]["clements"]["4"]
    assert len(per_seed) == 3 and all(0 <= x <= 1 for x in per_seed)
    cmp = rep["comparisons_at_primary_point"]["haar_unitary"]
    assert cmp["selected"] == {"butterfly": 2, "clements": 4, "low_rank_svd": 2}
    assert cmp["butterfly_minus_clements"]["n"] == 3 and cmp["butterfly_minus_low_rank_svd"]["n"] == 3
    # MNIST registered but not run at the primary modes, so no verdict
    assert rep["verdict"].startswith("not decided")


def test_low_rank_stage_and_budget():
    for n, r in ((4, 2), (8, 3), (16, 8)):
        cols = meshes.low_rank_stage_columns(n, r)
        assert meshes.mzi_count(cols) == r * n - r * (r + 1) // 2
        assert all(b == a + 1 for c in cols for a, b in c)
    with pytest.raises(ValueError, match="rank"):
        meshes.low_rank_stage_columns(4, 4)
    # n=4, r=2: per-mode MZIs per stage 1, 3, 4, 2; two stages + attenuators on modes 0, 1 -> 3, 7, 8, 4
    b = meshes.low_rank_budget(4, 2)
    assert (b.mzis, b.max_mzis_per_path, b.max_crossings_per_path) == (12, 8, 0)
    assert b.path_loss_db(0.25, 0.05) == pytest.approx(2.0)


def test_low_rank_matrix_rank_and_fabrication():
    n, r = 6, 2
    g = torch.Generator().manual_seed(5)
    params = [p.detach() for p in meshes.init_params("low_rank_svd", n, r, 3, g)]
    m = meshes.implemented("low_rank_svd", n, r, params)
    sv = torch.linalg.svdvals(m)
    assert torch.all(sv[:, r:] < 1e-10) and torch.all(sv[:, 0] <= 1 + 1e-12)
    assert torch.allclose(meshes.implemented("low_rank_svd", n, r, params, 0.0, 0.0, 201), m, atol=1e-12)
    noisy = meshes.implemented("low_rank_svd", n, r, params, 0.02, 0.02, 201)
    assert torch.equal(noisy, meshes.implemented("low_rank_svd", n, r, params, 0.02, 0.02, 201))
    assert not torch.allclose(noisy, m)
    # mesh families keep the pilot draw order: same phases as fit() for the same seed
    g1, g2 = torch.Generator().manual_seed(9), torch.Generator().manual_seed(9)
    a = meshes.init_params("clements", 4, 2, 2, g1)
    th = torch.rand(2, 3, generator=g2, dtype=torch.float64) * math.pi
    assert torch.equal(a[0].detach(), th)


def test_low_rank_fidelity_bounded_on_unitary_target():
    n, r = 4, 2
    tg = torch.as_tensor(meshes.haar_unitary(n, np.random.default_rng(3)))[None]
    opt = {"lr": 0.05, "steps": 200}
    _, _, final = meshes.fit_family("low_rank_svd", n, r, lambda m, e: 1 - meshes.fidelity(tg, m), 1, 0, opt, 1)
    f = 1 - float(final[0])
    # |Tr(V^dagger M)| <= sum of singular values of M <= r, so F <= (r / N)^2
    assert 0.1 < f <= (r / n) ** 2 + 1e-9


def _synthetic_mnist(train=200, test=100, seed=0):
    g = torch.Generator().manual_seed(seed)
    means = torch.rand(10, 784, generator=g)
    def split(k):
        y = torch.arange(k) % 10
        return (means[y] + 0.1 * torch.randn(k, 784, generator=g)).clamp(0, 1).float(), y
    tx, ty = split(train)
    vx, vy = split(test)
    return {"train_x": tx, "train_y": ty, "test_x": vx, "test_y": vy}


def test_mnist_pca_and_logits():
    d = meshes.mnist_pca(16, _synthetic_mnist())
    assert d["train_x"].shape == (200, 16) and d["test_x"].shape == (100, 16)
    assert torch.allclose(d["train_x"].norm(dim=1), torch.ones(200, dtype=torch.float64))
    assert torch.equal(d["train_x"], meshes.mnist_pca(16, _synthetic_mnist())["train_x"])
    m = torch.eye(16, dtype=torch.complex128)[None].repeat(2, 1, 1)
    z = meshes.mnist_logits(m, d["test_x"], torch.zeros(2, 1, dtype=torch.float64))
    assert z.shape == (2, 100, 10)
    assert torch.allclose(z[0], 10 * d["test_x"][:, :10] ** 2)


def test_tiny_run_at_16_modes_gives_verdict(tmp_path):
    cfg = yaml.safe_load(H1_YAML.read_text())
    cfg["modes"] = [16]
    cfg["meshes"] = {"butterfly": {"depths_in_layers_of_log2N_stages": [1]}, "clements": {"depths": [2]},
                     "low_rank_svd": {"ranks": [2]}}
    cfg["tasks"] = [{"name": "haar_unitary", "targets": 1, "score": "fidelity"},
                    {"name": "mnist_pca16_linear", "features": 16, "score": "test_accuracy"},
                    {"name": "dft_matrix", "score": "fidelity"}]
    cfg["optimizer"] = {"name": "adam", "lr": 0.05, "steps": 15, "restarts": 2}
    cfg["seeds"] = [101, 102, 103]
    cfg["fabrication_noise_seeds"] = [201, 202]
    rep = meshes.registered_run(cfg, _synthetic_mnist())
    acc = rep["scores_per_init_seed"]["mnist_pca16_linear"][16]
    assert set(acc) == {"butterfly", "clements", "low_rank_svd"}
    assert all(0 <= a <= 1 for fam in acc.values() for xs in fam.values() for a in xs)
    assert rep["skipped"] == {}
    assert set(rep["comparisons_at_primary_point"]) == {"haar_unitary", "mnist_pca16_linear", "dft_matrix"}
    assert rep["verdict"].startswith(("kept", "killed"))


def test_low_rank_ranks_at_or_above_n_are_excluded_and_reported(tmp_path):
    cfg = meshes.load_config(H1_YAML)
    # registered ranks [2, 4, 8]: rank 8 is full rank at N=8, so it is excluded there but kept at N=16
    keep8, ex8 = meshes.applicable_depths(cfg, "low_rank_svd", 8)
    assert keep8 == [2, 4] and list(ex8) == [8] and "not low rank" in ex8[8]
    assert meshes.applicable_depths(cfg, "low_rank_svd", 16) == ([2, 4, 8], {})
    assert meshes.applicable_depths(cfg, "clements", 8) == ([2, 4, 8, 16], {})
    out = tmp_path / "out"
    tiny = yaml.safe_load(_tiny(tmp_path).read_text())
    tiny["meshes"]["low_rank_svd"]["ranks"] = [2, 4]
    tiny["tasks"] = [{"name": "dft_matrix", "score": "fidelity"}]
    p = tmp_path / "h1_tiny_ex.yaml"
    p.write_text(yaml.safe_dump(tiny))
    meshes.main(["--config", str(p), "--out", str(out)])
    rep = json.loads((out / "h1_meshes_registered.json").read_text())
    assert list(rep["excluded_depths"]["4"]["low_rank_svd"]) == ["4"]
    assert set(rep["scores_per_init_seed"]["dft_matrix"]["4"]["low_rank_svd"]) == {"2"}
    assert set(rep["budgets"]["4"]["low_rank_svd"]) == {"2"}
