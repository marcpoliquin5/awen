"""Consistency checks on the stored E-038 result (results/accuracy-energy-per-mac)."""
import hashlib
import json
import math
from pathlib import Path

import pytest

from awen_models import energy_accounting as ea
from awen_models import energy_per_mac as epm
from awen_models.accuracy_energy_mac import (BITS, CURVE, IDEAL_NLL_TOL, REGISTERED_INPUTS, SEEDS, WINDOWS,
                                             closed_form, contract_labels, count_point)
from awen_models.sim2real import T_975

ROOT = Path(__file__).resolve().parents[1]
RESULTS = ROOT / "results" / "accuracy-energy-per-mac"
CHECKPOINT = ROOT / "awen_models" / "checkpoints" / "awen-mini"
ASSUMPTIONS = ROOT.parent / "awen-spec" / "hardware_assumptions.yaml"
RUN_COMMIT = "926cef9fa8e3c64039d5c0820c8c75e6b8b064c7"


def _report():
    return json.loads((RESULTS / "results.json").read_text())


def _sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def test_checksums_match_stored_files():
    for line in (RESULTS / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split()
        assert _sha(RESULTS / name) == digest, name


def test_inputs_are_the_registered_pins():
    assert _report()["input_sha256"] == REGISTERED_INPUTS["e038"]
    assert _sha(CHECKPOINT / "model.safetensors") == REGISTERED_INPUTS["e038"]["checkpoint"]


def test_verdict_pass_with_registered_grid():
    report = _report()
    assert report["curve"] == CURVE and report["registration"] == "e038"
    assert report["evidence_level"] == "L1" and report["model"] == "awen-mini"
    assert report["claims"] == "none: an L1 curve, no speedup or savings figure"
    assert report["verdict"] == "pass" and report["failures"] == {}
    grid = report["grid"]
    assert grid["contracts"] == contract_labels() and grid["seeds"] == list(SEEDS)
    assert grid["windows"] == WINDOWS and grid["window_seed"] == 2024
    assert set(report["accuracy"]) == set(contract_labels())


def test_counts_equal_closed_form_from_checkpoint_shapes():
    from safetensors import safe_open

    report = _report()
    config = json.loads((CHECKPOINT / "config.json").read_text())
    with safe_open(str(CHECKPOINT / "model.safetensors"), framework="pt") as f:
        shapes = {k: tuple(f.get_slice(k).get_shape()) for k in f.keys()}
    # Lowered linear layers: rank-2 weights except the embeddings (the tied output head stays on the host).
    linear = [(s[1], s[0]) for k, s in shapes.items()
              if len(s) == 2 and not any(e in k for e in ("emb", "wte", "wpe"))]
    assert linear
    expected = closed_form(linear, config["block_size"], report["grid"]["tile_k"])
    assert report["counts"] == expected


def test_accuracy_statistics_recompute():
    acc = _report()["accuracy"]
    cpu = {v["cpu_nll_nats"] for v in acc.values()}
    assert len(cpu) == 1  # same windows, same CPU reference for every contract
    for label, v in acc.items():
        for key in ("sim_nll_nats", "nll_diff_nats"):
            s = v[key]
            assert s["n"] == len(SEEDS) and math.isfinite(s["mean"])
            assert s["ci95_half_width"] == pytest.approx(T_975[3] * s["sd"] / math.sqrt(3), rel=1e-12, abs=1e-15)
        assert v["nll_diff_nats"]["mean"] == pytest.approx(v["sim_nll_nats"]["mean"] - v["cpu_nll_nats"], abs=1e-12)
        assert len(v["min_tile_effective_bits"]) == len(SEEDS)
    assert abs(acc["ideal"]["nll_diff_nats"]["mean"]) <= IDEAL_NLL_TOL


def test_energy_point_reproduces_from_stored_counts():
    report = _report()
    if _sha(ASSUMPTIONS) != REGISTERED_INPUTS["e038"]["assumptions"]:
        pytest.skip("assumption set changed after the E-038 run; stored result stays pinned to its own hash")
    import yaml

    grid = report["grid"]
    cp = count_point(report["counts"], grid["positions"], grid["tile_k"])
    total = ea.account(yaml.safe_load(ASSUMPTIONS.read_text()), cp, ea.TERMS_E036, total_rule=ea.TOTAL_RULE_E036)
    again = json.loads(json.dumps(epm.per_mac_point(total, cp), sort_keys=True))
    assert again == report["energy_per_lowered_mac"]
    assert epm.check(again) == []
    floor = again["optical_detection_floor_J_per_mac"]["by_effective_bits"]
    for b in BITS:
        assert report["accuracy"][f"bits{b}"]["optical_detection_floor_J_per_mac"] == floor[str(b)]


def _fmt(x, nd=4):
    return f"{x:.{nd}g}"


def test_readme_figures_match_results():
    readme = (RESULTS / "README.md").read_text()
    report = _report()
    acc = report["accuracy"]
    for b in BITS:
        s = acc[f"bits{b}"]["sim_nll_nats"]
        assert f"| bits{b} | {s['mean']:.4g} ± {s['ci95_half_width']:.3f} |" in readme, b
    env = report["energy_per_lowered_mac"]["by_transmitter"]
    for name, v in env.items():
        cell = f"{_fmt(v['min_J_per_mac'] * 1e12)} pJ – {_fmt(v['max_J_per_mac'] * 1e12)} pJ"
        assert f"| {name} |" in readme and cell in readme, name
    assert f"{report['energy_per_lowered_mac']['lowered_macs_per_token']:,}" in readme
    assert f"{report['host_mac_fraction']:.4f}" in readme
    assert RUN_COMMIT in readme
    assert all(digest in readme for digest in REGISTERED_INPUTS["e038"].values())
