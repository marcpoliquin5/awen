import importlib.util

import pytest

from awen import Runtime
from awen_models.checkpoint import bundled_checkpoint_dir


def test_bundled_checkpoint_loads_and_generates():
    model = Runtime(device="cpu").load_model("awen-mini")
    assert model.execution.fidelity == "measured"
    assert model.metadata["parameters"] == sum(p.numel() for p in model.model.parameters())
    first = model.generate("ROMEO:", max_new_tokens=40, seed=3)
    second = model.generate("ROMEO:", max_new_tokens=40, seed=3)
    assert first == second
    assert first.startswith("ROMEO:") and len(first) == len("ROMEO:") + 40


def test_bundled_checkpoint_beats_uniform_on_held_out_text():
    import math

    model = Runtime(device="auto").load_model("awen-mini")
    # Text that does not appear verbatim in Tiny Shakespeare.
    loss = model.log_likelihood("And now, good friends, let us go hence and speak of these things.\n")
    assert loss < 0.75 * math.log(model.tokenizer.vocab_size)


def test_unknown_device_is_rejected():
    with pytest.raises(ValueError):
        Runtime(device="tpu")


def test_bundled_checkpoint_directory_exists():
    assert (bundled_checkpoint_dir("awen-mini") / "model.safetensors").is_file()


@pytest.mark.skipif(importlib.util.find_spec("awen_native") is not None, reason="native extension installed")
def test_photonic_sim_reports_missing_extension():
    with pytest.raises(RuntimeError, match="awen_native"):
        Runtime(device="photonic-sim").load_model("awen-mini")


def test_evaluate_reports_float32_and_int8(corpus):
    from awen_models.evaluate import evaluate

    report = evaluate(batches=2, batch_size=4, generate_tokens=8, corpus=corpus)
    fp32 = report["variants"]["float32"]
    int8 = report["variants"]["int8_weights"]
    assert report["evidence"].startswith("measured")
    assert int8["linear_weight_bytes"] < fp32["linear_weight_bytes"] / 3
    assert abs(int8["val_loss_nats"] - fp32["val_loss_nats"]) < 0.05
