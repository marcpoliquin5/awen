"""Conversions per token for the tiny Llama and Qwen3 models through ``torch.compile(backend=awen)`` (E-024).

    python -m awen.conversions_curve --out results/conversions-per-token/results.json

For each architecture in ``ARCHS``, simulator tile size in ``TILE_KS`` and sequence length in
``POSITIONS``, the model (``awen.compile_llama``, random init, built locally, nothing downloaded)
is compiled with the ideal (noise-free) contract, run once on one sequence without a KV cache,
and the DAC and ADC conversion counts are read from the capability report
(``awen_py.compile_report``). Counts per token are the report totals divided by the number of
positions. Every count is also derived in closed form from the model config
(``expected_counts``); the registered check is that the two agree exactly.

Counts only. The counting rule is the simulator's (rows*inner input DAC conversions, weights
programmed once per compiled call, rows*outputs*ceil(inner/tile_k) ADC reads); it is not a
statement about any device. No energy, latency or speedup figure is produced: energy per MAC
needs the full system accounting in ``awen-spec/hardware_assumptions.yaml`` and is a later slice.
Simulated (L1). See ``research/experiments/awen-photon-conversions/PREREGISTRATION.md``.
"""

from __future__ import annotations

import argparse
import json
from math import ceil
from pathlib import Path
from typing import Any, Mapping, Sequence

from awen.compile_llama import ARCHITECTURES, build

CURVE_VERSION = "awen.conversions-per-token-curve.v1"
ARCHS = ("llama", "qwen3")
TILE_KS = (8, 16, 32, 64, 128)
POSITIONS = (1, 8, 32, 128)


def projections(config: Mapping[str, int]) -> list[tuple[int, int]]:
    """``(inner, outputs)`` of every lowered linear node, in graph order (no bias terms in these configs)."""
    hidden, mlp = config["hidden_size"], config["intermediate_size"]
    head_dim = config.get("head_dim", hidden // config["num_attention_heads"])
    q, kv = config["num_attention_heads"] * head_dim, config["num_key_value_heads"] * head_dim
    layer = [(hidden, q), (hidden, kv), (hidden, kv), (q, hidden), (hidden, mlp), (hidden, mlp), (mlp, hidden)]
    return layer * config["num_hidden_layers"] + [(hidden, config["vocab_size"])]


def expected_counts(config: Mapping[str, int], positions: int, tile_k: int) -> dict[str, int]:
    """Closed-form totals under the simulator counting rule for one call over ``positions`` tokens."""
    shapes = projections(config)
    return {"dac_input": sum(positions * inner for inner, _ in shapes),
            "dac_weight": sum(inner * outputs for inner, outputs in shapes),
            "adc": sum(positions * outputs * ceil(inner / tile_k) for inner, outputs in shapes),
            "lowered_macs": sum(positions * inner * outputs for inner, outputs in shapes)}


def point(arch: str, tile_k: int, positions: int) -> dict[str, Any]:
    """One curve point: report counts, closed-form counts and per-token values."""
    import awen_native

    payload = build(positions=positions, noise=awen_native.ideal_contract(), arch=arch, tile_k=tile_k)
    compile_report = payload["compile"]
    conversions = compile_report["conversions"]
    measured = {"dac_input": conversions["dac_input"], "dac_weight": conversions["dac_weight"],
                "adc": conversions["adc"], "lowered_macs": compile_report["counts"]["lowered_macs"]}
    expected = expected_counts(ARCHITECTURES[arch][2], positions, tile_k)
    per_token = {key: measured[key] / positions for key in ("dac_input", "dac_weight", "adc")}
    per_token["total"] = sum(per_token.values())
    return {"arch": arch, "tile_k": tile_k, "positions": positions, "report_tile_k": compile_report["execution"]["tile_k"],
            "graph_fingerprint": compile_report["backend"]["graph_fingerprint"], "counts": measured,
            "expected": expected, "matches_closed_form": measured == expected, "per_token": per_token,
            "conversions_per_mac": sum(measured[key] for key in ("dac_input", "dac_weight", "adc"))
            / measured["lowered_macs"]}


def run(archs: Sequence[str] = ARCHS, tile_ks: Sequence[int] = TILE_KS,
        positions: Sequence[int] = POSITIONS) -> dict[str, Any]:
    points = [point(arch, tile_k, length) for arch in archs for tile_k in tile_ks for length in positions]
    return {"curve": CURVE_VERSION, "evidence_level": "L1", "source": "measured by us (counted by the report code)",
            "grid": {"archs": list(archs), "tile_ks": list(tile_ks), "positions": list(positions)},
            "contract": "ideal (counts do not depend on noise)", "use_cache": False,
            "per_token_rule": "report totals for one call over P positions, divided by P (prefill, no KV cache)",
            "energy": "not estimated: needs the full system accounting in awen-spec/hardware_assumptions.yaml",
            "verdict": "pass" if all(row["matches_closed_form"] for row in points) else "fail",
            "points": points}


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--archs", nargs="+", choices=sorted(ARCHITECTURES), default=list(ARCHS))
    parser.add_argument("--tile-ks", nargs="+", type=int, default=list(TILE_KS))
    parser.add_argument("--positions", nargs="+", type=int, default=list(POSITIONS))
    parser.add_argument("--out", help="write the JSON result here (default: stdout)")
    args = parser.parse_args(argv)
    text = json.dumps(run(args.archs, args.tile_ks, args.positions), indent=2, sort_keys=True) + "\n"
    if args.out:
        Path(args.out).parent.mkdir(parents=True, exist_ok=True)
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
