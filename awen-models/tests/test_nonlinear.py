import json

from awen_models.model import AwenMini
from awen_models.nonlinear import VARIANTS, check_parameter_budget, pareto_front, run, variant_config


def test_variants_share_a_parameter_budget():
    counts = check_parameter_budget(65)
    reference = counts["baseline-gelu-mlp"]
    assert set(counts) == set(VARIANTS)
    assert all(abs(c - reference) / reference <= 0.03 for c in counts.values())
    assert AwenMini(variant_config("recursive-shared-block", 65)).config.shared_layers


def test_pareto_front():
    points = [(1.0, 2.0, "a"), (2.0, 1.0, "b"), (2.0, 2.0, "c"), (0.5, 3.0, "d")]
    assert set(pareto_front(points)) == {"a", "b", "d"}


def test_harness_runs_end_to_end(tmp_path, corpus):
    report = run(steps=4, seeds=[1, 2], out=tmp_path, batch_size=2,
                 variants=["baseline-gelu-mlp", "gated-multiplicative"], text=corpus)
    assert len(report["runs"]) == 4
    saved = json.loads((tmp_path / "results.json").read_text())
    assert saved["summary"]["gated-multiplicative"]["val_loss_stdev"] is not None
    assert (tmp_path / "pareto.png").stat().st_size > 1000
