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
    assert rep["uniform_grid"]["status"] == "run"
    # generations 1 and 2 are too thin for any registered spacing's cavity
    assert len(rep["uniform_grid"]["infeasible"]) == 2 * 3 and rep["comparison_cells"] == []
    assert rep["verdict"] == "not evaluable" and rep["verdict_detail"]["comparable_cells"] == 0
    assert {r["footprint"]["layers"] for r in rep["runs"]} == {3, 9}
    assert all(r["usable_channels"] <= r["channels"] for r in rep["runs"])


def test_layers_matrix_matches_quarter_wave_stack():
    omega = np.linspace(0.5, 1.5, 201)
    w = stacks.cantor_word(2)
    ns = np.array([2.3 if ch == "A" else 1.38 for ch in w])
    ref = stacks.stack_matrix(w, 2.3, 1.38, omega)
    assert np.allclose(stacks.layers_matrix(ns, 0.25 / ns, omega), ref, atol=1e-12)


@pytest.mark.parametrize("g,spacing", [(3, 0.1), (4, 0.1), (4, 0.05)])
def test_uniform_grid_stack_has_equal_footprint(g, spacing):
    base = stacks.footprint(stacks.cantor_word(g), 2.3, 1.38, np.ones(3 ** g))
    ns, ds, info = stacks.uniform_grid_stack(2.3, 1.38, spacing, base["layers"], base["total_thickness_lambda0"])
    assert len(ns) == base["layers"]
    assert ds.sum() == pytest.approx(base["total_thickness_lambda0"], abs=1e-12)
    assert np.all(ds > 0) and info["cavity_half_waves"] == round(1 / spacing)


def test_uniform_grid_infeasible_when_cavity_does_not_fit():
    base = stacks.footprint(stacks.cantor_word(2), 2.3, 1.38, np.ones(9))
    assert stacks.uniform_grid_stack(2.3, 1.38, 0.1, 9, base["total_thickness_lambda0"]) is None
    base4 = stacks.footprint(stacks.cantor_word(4), 2.3, 1.38, np.ones(81))
    assert stacks.uniform_grid_stack(2.3, 1.38, 0.02, 81, base4["total_thickness_lambda0"]) is None


def test_uniform_grid_comb_passes_design_frequency_on_a_near_uniform_grid():
    base = stacks.footprint(stacks.cantor_word(4), 2.3, 1.38, np.ones(81))
    grid = np.linspace(0.5, 1.5, 4001)
    ns, ds, info = stacks.uniform_grid_stack(2.3, 1.38, 0.1, 81, base["total_thickness_lambda0"], grid)
    assert info["mirror_rule"].startswith("fewest pairs")
    omega = np.linspace(0.85, 1.15, 6001)
    m = stacks.layers_matrix(ns, ds, omega)
    assert np.allclose(np.linalg.det(m), 1.0, atol=1e-8)
    t = stacks.transmission(m)
    # omega0 lies in a passband; the outer spacers (weak air/B etalons) keep it from reaching T = 1
    assert t[int(np.argmin(abs(omega - 1.0)))] > 0.5
    gaps = np.diff(stacks.channels(t, omega, 0.5, 0.0)["centers"])
    assert len(gaps) >= 2 and np.ptp(gaps) < 0.1 * gaps.mean()  # near-uniform grid around omega0
    # mirror penetration makes the real grid finer than the nominal spacing; it is recorded, not hidden
    assert 0.0 < info["measured_spacing_near_omega0"] < 0.1


def test_compare_cell_outcomes():
    def runs(d, x):
        return [{"channels_per_unit_normalized_frequency": d, "worst_adjacent_crosstalk_db": x}] * 2
    assert stacks.compare_cell(runs(5, -30), runs(3, -20))["outcome"] == "cantor better on both"
    assert stacks.compare_cell(runs(2, -10), runs(3, -20))["outcome"] == "cantor worse on both"
    assert stacks.compare_cell(runs(5, -10), runs(3, -20))["outcome"] == "mixed"
    assert stacks.compare_cell(runs(5, None), runs(3, -20))["outcome"] == "not comparable"


def test_tiny_feasible_run_compares_cells(tmp_path):
    out = tmp_path / "out"
    cfg = _tiny(tmp_path, cantor_generations=[3], uniform_grid={"channel_spacings_normalized": [0.1, 0.02]})
    stacks.main(["--config", str(cfg), "--out", str(out)])
    rep = json.loads((out / "h3_channel_plans.json").read_text())
    assert rep["uniform_grid"]["infeasible"] == [{"generation": 3, "spacing": 0.02,
                                                   "status": "infeasible at equal footprint"}]
    assert len(rep["uniform_grid"]["runs"]) == 2 * 2 and len(rep["comparison_cells"]) == 2
    assert {r["footprint"]["layers"] for r in rep["uniform_grid"]["runs"]} == {27}
    assert all(c["outcome"] in {"cantor better on both", "cantor worse on both", "mixed", "not comparable"}
               for c in rep["comparison_cells"])


def test_verdict_rule_is_registered_in_config_and_preregistration():
    cfg = yaml.safe_load(H3_YAML.read_text())
    assert cfg["verdict_rule"]["id"] == stacks.VERDICT_RULE
    assert {"kept", "killed", "not_kept", "not_evaluable"} <= set(cfg["verdict_rule"])
    assert stacks.VERDICT_RULE in (FRACTAL / "PREREGISTRATION.md").read_text()


def test_verdict_rule_outcomes():
    def cells(*outcomes):
        return [{"outcome": o} for o in outcomes]
    b, w, m, n = stacks.OUTCOMES
    assert stacks.verdict(cells(b, b, b))["verdict"] == "kept"
    assert stacks.verdict(cells(b, b, n))["verdict"] == "not kept"  # kept needs every cell comparable
    assert stacks.verdict(cells(w, w, b))["verdict"] == "killed"
    assert stacks.verdict(cells(w, m))["verdict"] == "not kept"  # exactly half is not a strict majority
    assert stacks.verdict(cells(w, w, n))["verdict"] == "killed"  # majority of the comparable cells
    assert stacks.verdict(cells(m, m, b))["verdict"] == "not kept"
    assert stacks.verdict(cells(n, n))["verdict"] == "not evaluable"
    assert stacks.verdict([])["verdict"] == "not evaluable"
    d = stacks.verdict(cells(w, w, b, n))
    assert d["comparable_cells"] == 3 and d["outcome_counts"] == {b: 1, w: 2, m: 0, n: 1}


def test_config_without_rule_is_not_evaluated(tmp_path):
    out = tmp_path / "out"
    cfg = yaml.safe_load(_tiny(tmp_path).read_text())
    del cfg["verdict_rule"]
    p = tmp_path / "norule.yaml"
    p.write_text(yaml.safe_dump(cfg))
    stacks.main(["--config", str(p), "--out", str(out)])
    rep = json.loads((out / "h3_channel_plans.json").read_text())
    assert rep["verdict"].startswith("not evaluated") and "verdict_detail" not in rep
