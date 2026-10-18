"""Fractal track F1-F4: fast checks of the code paths behind the committed results."""

import json
import math
import sys
from pathlib import Path

import numpy as np
import pytest
import torch

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
sys.path.insert(0, str(FRACTAL))
import h1_meshes as meshes  # noqa: E402
import h3_stacks as stacks  # noqa: E402
import h2_structured_weights as sw  # noqa: E402

from awen_physics.clements import decompose  # noqa: E402


# ------------------------------------------------------------------ F1 meshes
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
def test_words():
    assert [len(stacks.fibonacci_word(g)) for g in range(8)] == [1, 1, 2, 3, 5, 8, 13, 21]
    assert stacks.fibonacci_word(4) == "ABAAB"
    assert stacks.cantor_word(1) == "ABA"
    assert stacks.cantor_word(2) == "ABABBBABA"


def test_spectrum_matches_awen_physics_tmm():
    for w in (0.4, 1.0, 1.7):
        ours, ref = stacks.tmm_crosscheck(stacks.fibonacci_word(6), 2.3, 1.38, w)
        assert ours == pytest.approx(ref, abs=1e-12)


def test_trace_map_recursion_and_invariant():
    omega = np.linspace(0.05, 1.95, 401)
    x = stacks.trace_map(2.3, 1.38, omega, 10)
    assert np.allclose(x[3:], 2 * x[2:-1] * x[1:-2] - x[:-3], atol=1e-8 * (1 + np.abs(x[3:])))
    inv = stacks.trace_invariant(x)
    ok = np.max(np.abs(x), axis=0) < 1e3
    assert np.max(np.abs(inv - inv[0])[:, ok]) < 1e-7
    q = stacks.trace_invariant(stacks.trace_map(2.3, 1.38, np.array([1.0]), 6))[0, 0]
    assert q == pytest.approx(0.25 * (2.3 / 1.38 - 1.38 / 2.3) ** 2)


def test_quarter_wave_transmission_six_cycle():
    t = [float(stacks.spectrum(stacks.fibonacci_word(g), 2.3, 1.38, np.array([1.0]))[0]) for g in range(2, 16)]
    assert np.allclose(t[:-6], t[6:], atol=1e-10)


# ------------------------------------------------------------------ F3 structured weights
def test_structured_layers_match_dense_equivalents():
    x = torch.randn(4, 3, 16)
    ml = sw.MonarchLinear(16, 32, 4, 8)
    assert torch.allclose(ml(x), ml.forward_factored(x), atol=1e-5)
    kl = sw.Kron3Linear((2, 2, 4), (2, 4, 4), 3)
    assert torch.allclose(kl(x), x @ kl.dense().T + kl.bias, atol=1e-5)


def test_structured_variants_are_within_budget():
    counts = sw.budget(65)
    assert set(counts) == set(sw.VARIANTS)
    for v in sw.VARIANTS:
        logits, loss = sw.build(v, 65)(torch.zeros(1, 8, dtype=torch.long), torch.zeros(1, 8, dtype=torch.long))
        assert logits.shape == (1, 8, 65) and torch.isfinite(loss)


# ------------------------------------------------------------------ F4 box counting
def test_graph_dimension_of_smooth_curve_is_one():
    import x_boxcount as boxcount
    a = np.linspace(-1, 1, 1025)
    assert boxcount.graph_dimension(a, np.sin(3 * a) + a ** 2, [4, 8, 16, 32, 64, 128]) == pytest.approx(1.0, abs=0.08)


def test_hill_estimator_recovers_pareto_tail():
    import x_boxcount as boxcount
    rng = np.random.default_rng(0)
    # density ~ x^-alpha with alpha = 3  <=>  Pareto shape a = alpha - 1 = 2
    sample = rng.pareto(2.0, 200000) + 1
    assert boxcount.hill_tail_exponent(sample, 0.1) == pytest.approx(3.0, rel=0.05)


# ------------------------------------------------------------------ committed results
@pytest.mark.parametrize("name", ["meshes_n8.json", "meshes_n16.json", "stacks.json"])
def test_committed_results_parse(name):
    data = json.loads((FRACTAL / "results" / name).read_text())
    assert data["study"].startswith("awen.fractal-")


def test_pilot_metadata_lists_every_result_file():
    meta = json.loads((FRACTAL / "results" / "PILOT_RUNS.json").read_text())
    stored = {p.name for p in (FRACTAL / "results").iterdir()} - {"PILOT_RUNS.json"}
    assert stored <= set(meta["commands"])
