import json

import pytest
import torch

from awen_models import efficiency as ef

TEXT = ("To be, or not to be, that is the question: Whether 'tis nobler in the mind to suffer "
        "the slings and arrows of outrageous fortune, or to take arms against a sea of troubles. ") * 40


def test_tt_forward_matches_core_contraction_and_rank():
    torch.manual_seed(0)
    tt = ef.TTLinear((8, 16), (32, 32), 5)
    x = torch.randn(3, 7, 128)
    assert torch.allclose(tt(x), tt.contract(x), atol=1e-5)
    assert torch.linalg.matrix_rank(tt.full_weight().T.reshape(8, 16, 32, 32).permute(0, 2, 1, 3)
                                    .reshape(8 * 32, 16 * 32)) <= 5


def test_moe_routes_each_token_to_one_expert_and_balance_loss():
    torch.manual_seed(0)
    moe = ef.MoEFeedForward(16, 8, 4)
    x = torch.randn(2, 5, 16)
    y = moe(x)
    assert y.shape == x.shape and moe.aux_loss.item() > 0
    moe.aux_loss.backward()
    assert moe.router.weight.grad is not None and torch.isfinite(moe.router.weight.grad).all()
    # each token's output equals gate * its chosen expert's output
    flat = x.reshape(-1, 16)
    p = torch.softmax(moe.router(flat), -1)
    g, i = p.max(-1)
    ref = torch.stack([moe.experts[int(e)](t) * gg for t, e, gg in zip(flat, i, g)])
    assert torch.allclose(y.reshape(-1, 16), ref, atol=1e-5)


def test_fake_quant_int4_has_at_most_15_levels_per_row_and_ste_gradient():
    w = torch.randn(6, 50, requires_grad=True)
    q = ef.fake_quant(w, 4)
    for row in q.detach():
        assert len(torch.unique(row)) <= 15
    q.sum().backward()
    assert torch.allclose(w.grad, torch.ones_like(w))


def test_budget_is_matched_on_tiny_shakespeare_vocab():
    counts = ef.check_budget(65)
    ref = counts["baseline"]
    assert all(abs(c - ref) / ref <= 0.03 for c in counts.values())
    assert set(counts) == set(ef.VARIANTS)


def test_macs_reflect_moe_and_recursion():
    base = ef.macs_per_token(ef.build("baseline", 65), 128)
    assert ef.macs_per_token(ef.build("moe-top1", 65), 128) < base
    assert ef.macs_per_token(ef.build("recursive", 65), 128) > base


@pytest.mark.parametrize("name", ef.VARIANTS)
def test_every_variant_trains(name):
    torch.set_num_threads(2)
    model, info = ef.train_variant(name, TEXT, steps=4, seed=0, batch_size=4, eval_batches=2)
    assert info["val_loss"] > 0
    if name == "sparse-50":
        assert info["mlp_weight_sparsity"] == pytest.approx(0.5, abs=0.01)
    if name in ("qat-int4", "ptq-int4"):
        lin = next(m for m in model.modules() if isinstance(m, ef.AwenLinear))
        assert len(torch.unique(lin.weight[0])) <= 15


def test_summarize_and_plot(tmp_path):
    runs = [{"variant": v, "seed": s, "stored_parameters": 100 + i, "val_loss": 2.0 - 0.1 * i + 0.01 * s,
             "forward_latency_ms_b1": 1.0 + i, "weight_memory_kib": 10.0 * (i + 1),
             "macs_per_token_at_full_context": 1000 * (i + 1)}
            for i, v in enumerate(["baseline", "low-rank"]) for s in (1, 2)]
    rep = ef.summarize(runs, 4, [1, 2], {"baseline": 100, "low-rank": 101}, tmp_path)
    assert rep["summary"]["low-rank"]["energy_per_token_estimate"]["relative_to_baseline"] == 2.0
    assert "estimated" in rep["summary"]["baseline"]["energy_per_token_estimate"]["label"]
    assert (tmp_path / "pareto.png").exists()
    assert json.loads((tmp_path / "results.json").read_text())["seeds"] == [1, 2]
