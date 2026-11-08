"""Accuracy axis for the energy-per-MAC curve on the bundled AWEN-Mini (E-038).

    python -m awen_models.accuracy_energy_mac --registration e038 \
        --out results/accuracy-energy-per-mac/results.json

One model with both an accuracy metric and a lowering count: the bundled, trained AWEN-Mini character model
on ``Runtime(device="photonic-sim")``. For each registered noise contract (ideal, and the simulator default
contract with ``dac_bits = adc_bits`` set to 4, 6 and 8) and each noise seed, it evaluates held-out
next-character NLL on fixed validation windows of Tiny Shakespeare, next to the eager CPU reference, and reads
the simulator's conversion and lowered-MAC counts from the capability report (``awen.capability``).

The energy axis reuses the registered rules unchanged: the E-036 accounting (``scenario_envelope_v1``, same 11
terms and published points from ``awen-spec/hardware_assumptions.yaml``) applied to the AWEN-Mini counts, then
the E-037 per-lowered-MAC division. The counts do not depend on the contract, so the system envelope is one
envelope for every bits key; only the optics-only detection floor is read per bits key. Host-fallback MACs
(attention cores, the tied output head) are reported as a fraction, not charged. Simulated (L1); not a
measured energy, not one physical device, not a speedup or savings figure (D-039).
"""
from __future__ import annotations

import argparse
import hashlib
import json
import math
from pathlib import Path
from typing import Any, Mapping, Sequence

from awen_models import energy_accounting as ea
from awen_models import energy_per_mac as epm
from awen_models.sim2real import T_975

CURVE = "awen.accuracy-energy-per-mac-curve.v1"
ARCH = "awen-mini"
BITS = (4, 6, 8)  # contract dac_bits = adc_bits; the same keys as the E-036 detection-floor sweep
SEEDS = (0, 1, 2)
WINDOWS = 16  # validation windows of block_size characters, one forward pass (one compiled call) each
WINDOW_SEED = 2024  # the generator seed of the stored awen-mini evaluation
VAL_FRACTION = 0.1  # the training split rule of the bundled checkpoint
IDEAL_NLL_TOL = 1e-3  # nats; the ideal contract must reproduce the CPU reference
COUNT_KEYS = ("dac_input", "dac_weight", "adc", "lowered_macs")
# SHA-256 of the inputs this registration names (main c3877f1).
REGISTERED_INPUTS = {
    "e038": {"checkpoint": "3a4e2158c139e86a0905cfc40d063001f09565f09af3e04253840f063a04b003",
             "assumptions": "e0965ba697e23559d0638fdbf31a405f497f898215ec75a3349a0330651d246f",
             "corpus": "86c4e6aa9db7c042ec79f339dcb96d42b0075e16b8fc2e86bf0ca57e2dc565ed"},
}
DISCLOSURE = ("accuracy is held-out next-character NLL of one small trained model on the photonic simulator (L1); "
              "energy is the E-036 scenario envelope on this model's counts divided by its lowered MACs per token "
              "(E-037 rule); sums of labelled published points within one transmitter model; not one physical device; "
              "host compute is not a term; no speedup or savings claim")


def contract_labels() -> list[str]:
    return ["ideal"] + [f"bits{b}" for b in BITS]


def contracts() -> dict[str, dict[str, Any]]:
    import awen_native

    base = dict(awen_native.default_contract())
    out = {"ideal": dict(awen_native.ideal_contract())}
    out.update({f"bits{b}": {**base, "dac_bits": b, "adc_bits": b} for b in BITS})
    return out


def closed_form(shapes: Sequence[tuple[int, int]], positions: int, tile_k: int) -> dict[str, int]:
    """Counts for one call over ``positions`` tokens from ``(inner, outputs)`` of every lowered linear layer."""
    return {"dac_input": sum(positions * i for i, _ in shapes),
            "dac_weight": sum(i * o for i, o in shapes),
            "adc": sum(positions * o * math.ceil(i / tile_k) for i, o in shapes),
            "lowered_macs": sum(positions * i * o for i, o in shapes)}


def count_point(counts: Mapping[str, int], positions: int, tile_k: int) -> dict[str, Any]:
    """An E-024-shaped point, so the E-036 accounting and the E-037 division apply unchanged."""
    per_token = {k: counts[k] / positions for k in ("dac_input", "dac_weight", "adc")}
    per_token["total"] = sum(per_token.values())
    return {"arch": ARCH, "tile_k": tile_k, "positions": positions, "counts": dict(counts), "per_token": per_token}


def ci95(values: Sequence[float]) -> dict[str, Any]:
    n = len(values)
    mean = sum(values) / n
    sd = math.sqrt(sum((v - mean) ** 2 for v in values) / (n - 1)) if n > 1 else 0.0
    half = T_975[n] * sd / math.sqrt(n) if n in T_975 else None
    return {"n": n, "mean": mean, "sd": sd, "ci95_half_width": half}


def assemble(evals: Mapping[str, Mapping[int, Mapping[str, Any]]], expected: Mapping[str, int], positions: int,
             tile_k: int, doc: Mapping[str, Any], registration: str = "e038",
             input_sha256: Mapping[str, str] | None = None, host_fraction: float | None = None) -> dict[str, Any]:
    """Pure assembly and registered checks. ``evals[label][seed]`` has ``sim_nll``, ``cpu_nll`` (lists, one per
    window), ``counts`` (one dict per window) and ``min_tile_effective_bits``."""
    failures: dict[str, list[str]] = {}
    if input_sha256 is not None and dict(input_sha256) != REGISTERED_INPUTS[registration]:
        failures["inputs"] = ["input SHA-256 differs from the registered inputs"]
    if set(evals) != set(contract_labels()) or any(set(v) != set(SEEDS) for v in evals.values()):
        failures.setdefault("inputs", []).append("contracts or seeds are not the registered set")
    curve = {}
    for label, by_seed in evals.items():
        found = []
        for seed, ev in by_seed.items():
            if len(ev["sim_nll"]) != WINDOWS or any(c != dict(expected) for c in ev["counts"]):
                found.append(f"seed {seed}: windows or counts differ from the closed form")
            if not all(math.isfinite(x) for x in ev["sim_nll"] + ev["cpu_nll"]):
                found.append(f"seed {seed}: non-finite NLL")
        sim = [sum(ev["sim_nll"]) / len(ev["sim_nll"]) for _, ev in sorted(by_seed.items())]
        cpu = [sum(ev["cpu_nll"]) / len(ev["cpu_nll"]) for _, ev in sorted(by_seed.items())]
        if label == "ideal" and any(abs(s - c) > IDEAL_NLL_TOL for s, c in zip(sim, cpu)):
            found.append("ideal contract does not reproduce the CPU reference NLL")
        curve[label] = {"sim_nll_nats": ci95(sim), "cpu_nll_nats": cpu[0] if cpu else None,
                        "nll_diff_nats": ci95([s - c for s, c in zip(sim, cpu)]),
                        "min_tile_effective_bits": [ev["min_tile_effective_bits"] for _, ev in sorted(by_seed.items())]}
        if found:
            failures[label] = found
    cp = count_point(expected, positions, tile_k)
    total_point = ea.account(doc, cp, ea.TERMS_E036, total_rule=ea.TOTAL_RULE_E036)
    if found := ea.check(total_point) + [f"term not sourced: {t}" for t in total_point["not_sourced"]]:
        failures["energy"] = found
    energy = None
    try:
        energy = epm.per_mac_point(total_point, cp)
        if found := epm.check(energy):
            failures.setdefault("energy", []).extend(found)
    except (ValueError, KeyError, StopIteration) as exc:
        failures.setdefault("energy", []).append(str(exc))
    floor = (energy or {}).get("optical_detection_floor_J_per_mac", {}).get("by_effective_bits", {})
    for b in BITS:
        if f"bits{b}" in curve:
            curve[f"bits{b}"]["optical_detection_floor_J_per_mac"] = floor.get(str(b))
    return {"curve": CURVE, "registration": registration, "evidence_level": "L1", "model": ARCH,
            "accuracy_metric": "held-out next-character NLL, nats per character (lower is better)",
            "grid": {"contracts": contract_labels(), "seeds": list(SEEDS), "windows": WINDOWS,
                     "window_seed": WINDOW_SEED, "positions": positions, "tile_k": tile_k},
            "counts": dict(expected), "host_mac_fraction": host_fraction,
            "energy_per_lowered_mac": energy, "assumption_set": doc.get("version"),
            "accounting": ea.REGISTRATIONS["e036"][1], "per_mac_rule": epm.MAC_RULE,
            "input_sha256": dict(input_sha256) if input_sha256 is not None else None,
            "disclosure": DISCLOSURE, "claims": "none: an L1 curve, no speedup or savings figure",
            "verdict": "fail" if failures else "pass", "failures": failures, "accuracy": curve}


def evaluate(noise: Mapping[str, Any], seed: int, texts: Sequence[str]) -> dict[str, Any]:
    """Runs the model: one capability report per window (simulator vs CPU on the same characters)."""
    from awen.capability import build_report
    from awen.runtime import Runtime

    loaded = Runtime(device="photonic-sim", noise=dict(noise), seed=seed).load_model(ARCH)
    out: dict[str, Any] = {"sim_nll": [], "cpu_nll": [], "counts": [], "min_tile_effective_bits": None}
    for text in texts:
        report = build_report(loaded, text)
        dev = report["deviation_vs_cpu"]
        out["sim_nll"].append(dev["sim_mean_nll_nats"])
        out["cpu_nll"].append(dev["cpu_mean_nll_nats"])
        out["counts"].append({**{k: report["conversions"][k] for k in COUNT_KEYS[:3]},
                              "lowered_macs": report["mac_split"]["photonic_sim_macs"]})
        out["min_tile_effective_bits"] = report["precision_and_calibration"]["min_tile_effective_bits"]
        out["host_fraction"] = 1.0 - report["mac_split"]["photonic_sim_fraction"]
    return out


def run(registration: str = "e038", assumptions: str | None = None) -> dict[str, Any]:
    import torch
    import yaml

    from awen_models.checkpoint import bundled_checkpoint_dir, load_checkpoint
    from awen_models.data import download_tiny_shakespeare, sample_batch, split_tokens

    directory = bundled_checkpoint_dir(ARCH)
    model, tokenizer, _ = load_checkpoint(directory, torch.device("cpu"))
    corpus = download_tiny_shakespeare()
    apath = Path(assumptions) if assumptions else ea.find_assumptions()
    sha = {"checkpoint": hashlib.sha256((directory / "model.safetensors").read_bytes()).hexdigest(),
           "assumptions": hashlib.sha256(apath.read_bytes()).hexdigest(),
           "corpus": hashlib.sha256(corpus.encode("utf-8")).hexdigest()}
    positions = model.config.block_size
    val = split_tokens(tokenizer.encode(corpus), VAL_FRACTION).val
    x, _ = sample_batch(val, WINDOWS, positions, torch.Generator().manual_seed(WINDOW_SEED))
    texts = [tokenizer.decode(row.tolist()) for row in x]
    tile_k = _runtime_tile_k()
    shapes = [(layer.weight.shape[1], layer.weight.shape[0]) for _, layer in model.linear_layers()]
    expected = closed_form(shapes, positions, tile_k)
    evals = {label: {seed: evaluate(noise, seed, texts) for seed in SEEDS} for label, noise in contracts().items()}
    host = evals["ideal"][SEEDS[0]]["host_fraction"]
    return assemble(evals, expected, positions, tile_k, yaml.safe_load(apath.read_text()), registration, sha, host)


def _runtime_tile_k() -> int:
    from awen.photonic import PhotonicGemmExecutor

    return PhotonicGemmExecutor().tile_k  # the runtime's executor default; read, not chosen here


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--registration", choices=sorted(REGISTERED_INPUTS), default="e038")
    parser.add_argument("--assumptions", default=None, help="path to awen-spec/hardware_assumptions.yaml")
    parser.add_argument("--out", help="write the JSON result here (default: stdout)")
    args = parser.parse_args(argv)
    text = json.dumps(run(args.registration, args.assumptions), indent=2, sort_keys=True) + "\n"
    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
