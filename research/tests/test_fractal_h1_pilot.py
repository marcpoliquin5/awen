"""H1 pilot (butterfly vs Clements meshes): fast checks of the code behind the stored pilot result."""

import hashlib
import json
import math
import sys
from pathlib import Path

import numpy as np
import pytest
import torch

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
PILOT = FRACTAL / "results" / "pilot"
sys.path.insert(0, str(FRACTAL))
import h1_meshes as meshes  # noqa: E402

from awen_physics.clements import decompose  # noqa: E402


def test_butterfly_topology_counts():
    for n in (4, 8, 16):
        p = int(math.log2(n))
        cols = meshes.butterfly_columns(n, 1)
        assert len(cols) == p and meshes.mzi_count(cols) == n // 2 * p
        # every mode is touched exactly once per stage
        for col in cols:
            assert sorted(m for pair in col for m in pair) == list(range(n))
    with pytest.raises(ValueError):
        meshes.butterfly_columns(6)


def test_clements_topology_and_crossings():
    cols = meshes.clements_columns(8, 8)
    assert meshes.mzi_count(cols) == 8 * 7 // 2
    b = meshes.budget("clements", 8, cols)
    assert b.max_crossings_per_path == 0 and b.max_mzis_per_path == 8
    # stride-1 butterfly stage needs no crossings; stride-4 needs some
    assert max(meshes.crossings_per_column(8, meshes.butterfly_columns(8)[0])) == 0
    assert max(meshes.crossings_per_column(8, meshes.butterfly_columns(8)[2])) > 0
    assert b.path_loss_db(0.1, 0.05) == pytest.approx(0.8)


def test_mesh_unitary_is_unitary_and_matches_clements_convention():
    n = 6
    rng = np.random.default_rng(3)
    u = meshes.haar_unitary(n, rng)
    assert np.allclose(u.conj().T @ u, np.eye(n), atol=1e-12)
    dec = decompose(u)
    cols = [[(unit.mode, unit.mode + 1)] for unit in dec.units]
    th = torch.tensor([[unit.theta for unit in dec.units]], dtype=torch.float64)
    ph = torch.tensor([[unit.phi for unit in dec.units]], dtype=torch.float64)
    out = torch.as_tensor(np.asarray(dec.phases)[None], dtype=torch.float64)
    v = meshes.mesh_unitary(n, cols, th, ph, out)[0].numpy()
    assert np.allclose(v, u, atol=1e-9)


def test_fit_reaches_target_inside_and_not_outside_the_mesh_family():
    n = 4
    rng = np.random.default_rng(0)
    targets = np.stack([meshes.haar_unitary(n, rng) for _ in range(3)])
    full = meshes.fit(n, meshes.clements_columns(n, n), targets, steps=400, restarts=2)
    assert full["fidelity"].min() > 0.999
    shallow = meshes.fit(n, meshes.clements_columns(n, 1), targets, steps=200, restarts=2)
    assert shallow["fidelity"].max() < 0.99


def test_phase_noise_first_order_prediction_small_sigma():
    n = 8
    cols = meshes.butterfly_columns(n, 1)
    g = torch.Generator().manual_seed(1)
    m = meshes.mzi_count(cols)
    th, ph = torch.rand(1, m, generator=g, dtype=torch.float64), torch.rand(1, m, generator=g, dtype=torch.float64)
    out = torch.zeros(1, n, dtype=torch.float64)
    s = 1e-3
    meas = meshes.phase_noise_infidelity(n, cols, th, ph, out, s, draws=400)
    pred = meshes.first_order_infidelity(n, m, th, s)
    assert meas == pytest.approx(pred, rel=0.35)


# ------------------------------------------------------------------ F2 stacks


def test_pilot_results_are_stored_with_their_hashes():
    meta = json.loads((PILOT / "H1_PILOT.json").read_text())
    assert "not evidence" in meta["note"] and meta["source"]["commit"] == "0974f38"
    for name, digest in meta["sha256"].items():
        assert hashlib.sha256((PILOT / name).read_bytes()).hexdigest() == digest, name
