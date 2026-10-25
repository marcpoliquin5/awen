"""H2 registered-run entry point: dimension-factorized FFN layers vs dense (configs/h2.yaml).

Trains the registered variants with the ``awen_models.efficiency`` harness (Tiny Shakespeare
character LM, stored parameters matched within the config tolerance), measures batch-1 forward
latency, counts conversions per useful MAC analytically, and applies the three registered kill
rules. Evidence level: L0 (CPU training) plus an analytic conversion count; no device, no energy.

Conversion count (definitions fixed in PREREGISTRATION.md, section H2, before any H2 full run):

* Weights stay resident (v2 section 6), so weight-programming conversions are not counted.
* **Useful MACs** of a layer are those of the dense-equivalent matrix-vector product,
  ``d_in * d_out`` per token, whatever path executes it.
* **Conversions** follow the registered mapping literally: one DAC per modulated input element and
  one ADC per detected output, *per matrix-vector product*. A rank-R factorized layer is R
  structured products (one per rank term) summed electronically, so the **primary** count is
  ``R * (d_in + d_out)``. The registered mapping describes no optical fan-out of the input or
  photocurrent summation across rank terms; the count with both, ``d_in + d_out``, is reported as
  ``lower_bound`` only, never used by the verdict.
* **Legality** (v2 section 10 physics rules): polarization supports at most 2x2 mixing, so a factor
  mapped to polarization must have both sides <= 2. Wavelength factors use broadcast-and-weight
  (diagonal weight + detector sum), which never moves energy between wavelengths. Channel counts
  are reported as requirements, never assumed device capabilities. A variant whose registered
  mapping is illegal gets conversions verdict ``not evaluable`` (no kill or keep on that rule).
* **Amendment A1** (2026-10-25, before any full run): ``kron3`` is built from the config's
  ``kron3_shape`` (legal: polarization factor 2 -> 2) instead of the harness's original
  ``(4,4,8) -> (8,16,16)`` shape, with the opt-in ``rank_chunk`` for memory. The harness default
  build is unchanged; the swap is scoped to this script (:func:`registered_build`).

Example::

    python research/experiments/multiplexing-fractal/h2_structured_weights.py \
        --config research/experiments/multiplexing-fractal/configs/h2.yaml --out results/h2
"""

from __future__ import annotations

import argparse
import contextlib
import json
import math
import statistics
import sys
from pathlib import Path

import numpy as np
import torch
import torch.nn as nn

from awen_models import efficiency as eff
from awen_models.nonlinear import measure_latency_ms
from awen_models.train import hardware_record

PILOT_SEEDS = {1, 2}  # H2 pilot seeds in draft #128 @ 0974f38
REGISTERED_VARIANTS = ["baseline", "kron3", "tensor-train"]
POLARIZATION_MAX = 2  # v2 section 10: polarization gives 2x2 mixing (physics, not a device count)


def load_config(path) -> dict:
    import yaml

    cfg = yaml.safe_load(Path(path).read_text())
    if cfg.get("id") != "H2" or cfg.get("schema") != "awen.fractal-prereg-config.v1":
        raise ValueError(f"{path} is not an H2 config")
    if set(cfg["seeds"]) & PILOT_SEEDS:
        raise ValueError(f"seeds {sorted(set(cfg['seeds']) & PILOT_SEEDS)} reuse H2 pilot seeds")
    if cfg["variants"] != REGISTERED_VARIANTS:
        raise ValueError(f"variants must be {REGISTERED_VARIANTS}, got {cfg['variants']}")
    unknown = [v for v in cfg["conversion_mapping"] if v != "dense" and v not in cfg["variants"]]
    if unknown or not all(v in cfg["conversion_mapping"] for v in ("dense", "kron3", "tensor-train")):
        raise ValueError("conversion_mapping must cover dense, kron3 and tensor-train only")
    if "conversions_per_useful_mac" not in cfg["metrics"]:
        raise ValueError("conversions_per_useful_mac missing from metrics")
    if not 0 < cfg["parameter_match_tolerance"] <= eff.PARAMETER_TOLERANCE:
        raise ValueError("parameter_match_tolerance must be in (0, harness tolerance]")
    check_kron3_shape(cfg)
    return cfg


def check_kron3_shape(cfg: dict) -> dict:
    """Validate amendment A1 (``kron3_shape``): three factors, d_model kept, polarization factor legal."""
    ks = cfg.get("kron3_shape")
    if ks is None:
        raise ValueError("kron3_shape (amendment A1) missing: the original kron3 shape is illegal")
    ins, hid = tuple(ks["in"]), tuple(ks["hidden"])
    if len(ins) != 3 or len(hid) != 3 or int(ks["rank"]) < 1:
        raise ValueError("kron3_shape needs three in factors, three hidden factors and rank >= 1")
    if math.prod(ins) != eff.BASE["n_embd"]:
        raise ValueError(f"kron3_shape in factors multiply to {math.prod(ins)}, not d_model {eff.BASE['n_embd']}")
    if max(ins[1], hid[1]) > POLARIZATION_MAX:
        raise ValueError(f"kron3_shape polarization factor {ins[1]}->{hid[1]} exceeds 2x2")
    chunk = ks.get("rank_chunk")
    if chunk is not None and int(chunk) < 1:
        raise ValueError("kron3_shape rank_chunk must be None or >= 1")
    return ks


@contextlib.contextmanager
def registered_build(cfg: dict):
    """Within this block, ``eff.build("kron3", ...)`` (also as called by ``check_budget`` and
    ``train_variant``) returns the amendment-A1 shape with its ``rank_chunk``. Restored on exit."""
    ks = check_kron3_shape(cfg)
    original = eff.build
    ins, hid, rank = tuple(ks["in"]), tuple(ks["hidden"]), int(ks["rank"])
    chunk = None if ks.get("rank_chunk") is None else int(ks["rank_chunk"])

    def build(name: str, vocab: int):
        if name != "kron3":
            return original(name, vocab)
        return eff._replace_ffn(eff._mini(vocab), lambda: eff.Kron3FeedForward(ins, hid, rank, rank_chunk=chunk))

    eff.build = build
    try:
        yield build
    finally:
        eff.build = original


# --------------------------------------------------------------------------- conversion counter
def layer_conversions(layer: nn.Module) -> dict:
    """Analytic DAC/ADC count for one FFN matrix under the registered mapping (module docstring)."""
    if isinstance(layer, eff.Kron3Linear):
        kind, rank = "kron3", layer.rank
        factors = [("spatial", layer.ins[0], layer.outs[0]), ("polarization", layer.ins[1], layer.outs[1]),
                   ("wavelength", layer.ins[2], layer.outs[2])]
    elif isinstance(layer, eff.TTLinear):
        kind, rank = "tensor-train", layer.g1.shape[-1]
        factors = [("spatial", layer.in_shape[0], layer.out_shape[0]),
                   ("wavelength", layer.in_shape[1], layer.out_shape[1])]
    elif isinstance(layer, nn.Linear):
        kind, rank = "dense", 1
        factors = [("spatial", layer.in_features, layer.out_features)]
    else:
        raise TypeError(f"no conversion mapping for {type(layer).__name__}")
    d_in, d_out = layer.in_features, layer.out_features
    reasons = [f"{axis} factor {i}->{o} exceeds 2x2 polarization mixing" for axis, i, o in factors
               if axis == "polarization" and max(i, o) > POLARIZATION_MAX]
    req = {"spatial_modes": max(max(i, o) for a, i, o in factors if a == "spatial")}
    for a, i, o in factors:
        if a == "wavelength":
            req["wavelengths"], req["weight_banks"] = i, o
        if a == "polarization":
            req["polarization_states"] = max(i, o)
    return {"kind": kind, "d_in": d_in, "d_out": d_out, "rank_terms": rank,
            "factors": [{"axis": a, "in": i, "out": o} for a, i, o in factors],
            "useful_macs": d_in * d_out, "conversions": rank * (d_in + d_out),
            "conversions_lower_bound": d_in + d_out, "legal": not reasons, "illegal_reasons": reasons,
            "channel_requirements": req}


def ffn_matrices(model) -> list[nn.Module]:
    out = []
    for block in model.blocks:
        f = block.ffn
        if isinstance(f, (eff.Kron3FeedForward, eff.TTFeedForward)):
            out += [f.fc, f.proj]
        else:
            out += [m for m in f.modules() if isinstance(m, nn.Linear)]
    return out


def conversion_report(model) -> dict:
    """Per-token FFN totals over all blocks (attention and head are dense in every variant, so excluded)."""
    layers = [layer_conversions(m) for m in ffn_matrices(model)]
    useful = sum(x["useful_macs"] for x in layers)
    conv = sum(x["conversions"] for x in layers)
    low = sum(x["conversions_lower_bound"] for x in layers)
    reasons = sorted({r for x in layers for r in x["illegal_reasons"]})
    return {"scope": "FFN matrices, all blocks, per token", "useful_macs": useful, "conversions": conv,
            "conversions_per_useful_mac": conv / useful, "conversions_per_useful_mac_lower_bound": low / useful,
            "legal": not reasons, "illegal_reasons": reasons, "layers": layers[:2]}


# --------------------------------------------------------------------------- statistics and verdict
def t_quantile(p: float, df: int) -> float:
    """Student-t quantile by bisection on a numerically integrated CDF (no scipy on the build box)."""
    c = math.gamma((df + 1) / 2) / (math.sqrt(df * math.pi) * math.gamma(df / 2))

    def cdf(x: float) -> float:
        xs = np.linspace(0.0, x, 20001)
        ys = c * (1 + xs ** 2 / df) ** (-(df + 1) / 2)
        return 0.5 + float(np.sum((ys[1:] + ys[:-1]) * np.diff(xs)) / 2)

    lo, hi = 0.0, 100.0
    for _ in range(80):
        mid = (lo + hi) / 2
        lo, hi = (mid, hi) if cdf(mid) < p else (lo, mid)
    return (lo + hi) / 2


def paired_t(diffs) -> dict:
    d = np.asarray(diffs, dtype=float)
    n, mean = len(d), float(d.mean())
    if n < 2:
        return {"n": n, "mean": mean, "ci95": None}
    half = t_quantile(0.975, n - 1) * float(d.std(ddof=1)) / math.sqrt(n)
    return {"n": n, "mean": mean, "ci95": [mean - half, mean + half]}


def verdict(rows: list[dict], conv: dict, params: dict) -> dict:
    """Registered H2 kill rules (configs/h2.yaml ``falsification``), per candidate vs baseline."""
    base = {r["seed"]: r for r in rows if r["variant"] == "baseline"}
    out = {}
    for cand in [v for v in params if v != "baseline"]:
        mine = {r["seed"]: r for r in rows if r["variant"] == cand}
        seeds = sorted(set(base) & set(mine))
        acc = paired_t([mine[s]["val_loss"] - base[s]["val_loss"] for s in seeds])
        acc_killed = acc["ci95"] is not None and acc["ci95"][0] > 0
        if not conv[cand]["legal"]:
            conv_rule = "not evaluable: registered mapping illegal"
        else:
            conv_rule = ("killed" if conv[cand]["conversions_per_useful_mac"]
                         >= conv["baseline"]["conversions_per_useful_mac"] else "passed")
        ratio = (statistics.mean(mine[s]["forward_latency_ms_b1"] for s in seeds)
                 / statistics.mean(base[s]["forward_latency_ms_b1"] for s in seeds))
        wins_params = params[cand] < params["baseline"]
        lat_killed = wins_params and ratio > 1.2
        rules = {"accuracy": {"paired_t_val_loss_minus_baseline": acc, "result": "killed" if acc_killed else "passed"},
                 "conversions": {"result": conv_rule},
                 "latency": {"ratio_to_baseline": ratio, "wins_on_parameters": wins_params,
                             "result": "killed" if lat_killed else "passed"}}
        results = [x["result"] for x in rules.values()]
        overall = ("killed" if "killed" in results else "not evaluable" if any(r.startswith("not") for r in results)
                   else "kept")
        out[cand] = {"rules": rules, "verdict": overall}
    return out


# --------------------------------------------------------------------------- run
def run_config(cfg: dict, text: str | None = None, batch_size: int = 32) -> dict:
    """``batch_size`` 32 is the harness default and the registered value; only tests lower it."""
    with registered_build(cfg):
        return _run_config(cfg, text, batch_size)


def _run_config(cfg: dict, text: str | None, batch_size: int) -> dict:
    corpus = text if text is not None else eff.download_tiny_shakespeare()
    vocab = len(set(corpus))
    torch.set_num_threads(int(cfg["threads"]))
    counts = eff.check_budget(vocab)
    params = {v: counts[v] for v in cfg["variants"]}
    ref = params["baseline"]
    for v, c in params.items():
        if abs(c - ref) / ref > cfg["parameter_match_tolerance"]:
            raise ValueError(f"{v}: {c} stored parameters outside the registered tolerance of {ref}")
    conv = {v: conversion_report(eff.build(v, vocab)) for v in cfg["variants"]}
    rows = []
    for name in cfg["variants"]:
        for seed in cfg["seeds"]:
            model, info = eff.train_variant(name, corpus, int(cfg["steps"]), int(seed), batch_size=batch_size)
            row = {"variant": name, "seed": seed, "stored_parameters": params[name], **info,
                   "forward_latency_ms_b1": measure_latency_ms(model, repeats=int(cfg["latency"]["repeats"]))}
            rows.append(row)
            print(json.dumps(row), flush=True)
    return {"id": "H2", "config": cfg, "dataset": "tiny_shakespeare" if text is None else
            {"name": "custom", "characters": len(corpus)},
            "batch_size": batch_size, "kron3_shape": cfg["kron3_shape"], "runs": rows, "conversions": conv, "verdict": verdict(rows, conv, params),
            "hardware": hardware_record(torch.device("cpu")),
            "evidence": "val loss and latency measured on the CPU above (L0); conversions counted analytically"}


def main(argv=None) -> int:
    ap = argparse.ArgumentParser(description="H2 registered run (configs/h2.yaml)")
    ap.add_argument("--config", required=True)
    ap.add_argument("--out", default="research/experiments/multiplexing-fractal/results/h2")
    ap.add_argument("--conversions-only", action="store_true", help="print the conversion count, no training")
    a = ap.parse_args(argv)
    cfg = load_config(a.config)
    if a.conversions_only:
        vocab = 64  # any vocabulary: FFN matrices and their counts do not depend on it
        with registered_build(cfg) as build:
            rep = {v: conversion_report(build(v, vocab)) for v in cfg["variants"]}
        print(json.dumps(rep, indent=2))
        return 0
    rep = run_config(cfg)
    out = Path(a.out)
    out.mkdir(parents=True, exist_ok=True)
    (out / "h2_registered.json").write_text(json.dumps(rep, indent=2) + "\n")
    print(json.dumps({k: v["verdict"] for k, v in rep["verdict"].items()}))
    return 0


if __name__ == "__main__":
    sys.exit(main())
