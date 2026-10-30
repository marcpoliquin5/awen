"""Capability report for AWEN-Mini on ``device="photonic-sim"``.

    python -m awen.capability --out report.json            # bundled awen-mini
    python -m awen.capability --model PATH --text "ROMEO:" --seed 1

The report covers which GEMMs lowered to the photonic simulator, which
operations fell back to the host and why, the precision and calibration the
noise contract assumes, simulated vs physical execution, conversion counts
(data movement), and the output deviation from the CPU reference. All values
come from the simulator and the model structure; nothing is measured on
hardware, and no energy, latency or speedup figure is produced.
"""

from __future__ import annotations

import argparse
import json
from typing import Any

import torch
import torch.nn as nn
import torch.nn.functional as F

from awen_models.checkpoint import load_checkpoint
from awen_models.model import CausalSelfAttention, FeedForward

REPORT_VERSION = "awen.runtime-capability-report.v1"
DEFAULT_TEXT = "ROMEO:\nWhat light through yonder window breaks?\n"
CONTRACT_PRECISION_KEYS = ("dac_bits", "adc_bits", "photons_per_mac", "phase_sigma_rad",
                           "drift_rad_per_sqrt_s", "seconds_since_calibration")


def _fallbacks(model: nn.Module, time: int) -> list[dict[str, Any]]:
    """Host operations of one forward pass, with the reason each stays on the host."""
    config = model.config
    width, vocab = config.n_embd, config.vocab_size
    applications = config.n_layer if config.shared_layers else 1
    rows: list[dict[str, Any]] = []
    for name, module in model.named_modules():
        if isinstance(module, nn.Embedding):
            rows.append({"op": name, "kind": "embedding lookup",
                         "reason": "gather from a table, not a GEMM", "macs": 0})
        elif isinstance(module, nn.LayerNorm):
            rows.append({"op": name, "kind": "layer norm",
                         "reason": "per-row mean/variance and division; not a linear map", "macs": 0})
        elif isinstance(module, CausalSelfAttention):
            rows.append({"op": f"{name}.scaled_dot_product_attention", "kind": "attention core (QK^T, softmax, AV)",
                         "reason": "activation-by-activation products: both operands change every token, so no "
                                   "weight stays programmed; only weight GEMMs (AwenLinear) are routed",
                         "macs": 2 * time * time * width * applications})
        elif isinstance(module, FeedForward):
            activation = "silu gate product" if module.kind == "gated" else "gelu"
            rows.append({"op": f"{name}.{activation.split()[0]}", "kind": f"activation ({activation})",
                         "reason": "elementwise nonlinearity; the simulator models a linear core with "
                                   "detection, nonlinearity stays in host electronics", "macs": 0})
    rows.append({"op": "head (x @ tok_emb.weight.T)", "kind": "tied output projection",
                 "reason": "computed as a plain matmul in AwenMini.forward, not through AwenLinear, so the "
                           "runtime cannot route it; changing that is a model change not made here",
                 "macs": time * width * vocab})
    rows.append({"op": "bias and residual additions", "kind": "elementwise add",
                 "reason": "AwenLinear adds bias on the host by design; residuals are host tensors", "macs": 0})
    return rows


@torch.no_grad()
def _deviation(sim_logits: torch.Tensor, ref_logits: torch.Tensor, ids: torch.Tensor) -> dict[str, Any]:
    diff = (sim_logits - ref_logits).double()
    ref = ref_logits.double()
    result: dict[str, Any] = {
        "positions": int(ids.shape[1]),
        "logits_max_abs_diff": float(diff.abs().max()),
        "logits_rms_diff": float(diff.pow(2).mean().sqrt()),
        "logits_relative_l2": float(diff.norm() / ref.norm()) if float(ref.norm()) > 0 else None,
        "top1_agreement": float((sim_logits.argmax(-1) == ref_logits.argmax(-1)).double().mean()),
    }
    if ids.shape[1] >= 2:
        targets = ids[:, 1:].reshape(-1)
        sim_nll = F.cross_entropy(sim_logits[:, :-1].reshape(-1, sim_logits.size(-1)).double(), targets)
        ref_nll = F.cross_entropy(ref_logits[:, :-1].reshape(-1, ref_logits.size(-1)).double(), targets)
        result.update({"sim_mean_nll_nats": float(sim_nll), "cpu_mean_nll_nats": float(ref_nll),
                       "nll_diff_nats": float(sim_nll - ref_nll)})
    return result


@torch.no_grad()
def build_report(loaded: Any, text: str) -> dict[str, Any]:
    if not text:
        raise ValueError("text must contain at least one character")
    model = loaded.model
    executors = [layer.matmul_executor for _, layer in model.linear_layers()]
    if not executors or any(e is None for e in executors):
        raise RuntimeError("every AwenLinear needs a photonic executor attached")
    executor = executors[0].executor if hasattr(executors[0], "executor") else executors[0]
    executor.reset()
    ids = torch.tensor([loaded.tokenizer.encode(text)], dtype=torch.long)[:, : model.config.block_size]
    was_training = model.training
    model.eval()
    try:
        sim_logits, _ = model(ids)
    finally:
        model.train(was_training)
    reference, _, _ = load_checkpoint(loaded.checkpoint_dir, torch.device("cpu"))
    reference.eval()
    ref_logits, _ = reference(ids)

    lowered = executor.layer_summary()
    lowered_macs = sum(row["macs"] for row in lowered)
    fallbacks = _fallbacks(model, int(ids.shape[1]))
    host_macs = sum(row["macs"] for row in fallbacks)
    contract = dict(executor.contract)
    return {
        "report": REPORT_VERSION,
        "model": {"checkpoint": str(loaded.checkpoint_dir), "parameters": model.num_parameters(),
                  "config": {k: getattr(model.config, k) for k in ("n_layer", "n_head", "n_embd", "block_size",
                                                                     "vocab_size", "ffn", "shared_layers")}},
        "execution": {"device": "photonic-sim", "fidelity": "simulated (compact model)", "physical": False,
                      "evidence_level": "L1", "backend": loaded.execution.gemm_backend,
                      "seed": executor.seed, "tile_k": executor.tile_k},
        "input": {"characters": len(text), "positions": int(ids.shape[1])},
        "lowered": lowered,
        "fallback": fallbacks,
        "mac_split": {"photonic_sim_macs": lowered_macs, "host_macs": host_macs,
                      "photonic_sim_fraction": lowered_macs / (lowered_macs + host_macs) if lowered_macs + host_macs else 0.0,
                      "note": "dense counts; causal masking in attention is not subtracted"},
        "precision_and_calibration": {
            "contract_version": contract.get("version"),
            **{key: contract.get(key) for key in CONTRACT_PRECISION_KEYS},
            "min_tile_effective_bits": min((row["tile_effective_bits"] for row in lowered
                                            if row["tile_effective_bits"] is not None), default=None),
            "calibration": "assumed by the noise contract; no calibration was performed (simulation only)",
        },
        "conversions": {
            "dac_input": sum(row["dac_input_conversions"] for row in lowered),
            "dac_weight": sum(row["dac_weight_conversions"] for row in lowered),
            "adc": sum(row["adc_conversions"] for row in lowered),
            "counting_rule": "simulator tiling: rows*inner input DAC, outputs*inner weight programming per call, "
                             "rows*outputs*tiles ADC reads; tile partial sums accumulate on the host",
        },
        "energy": "not estimated: needs the full system accounting in awen-spec/hardware_assumptions.yaml",
        "deviation_vs_cpu": _deviation(sim_logits, ref_logits, ids),
    }


def main(argv: list[str] | None = None) -> int:
    from awen.runtime import Runtime

    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--model", default="awen-mini", help="bundled name or checkpoint directory")
    parser.add_argument("--text", default=DEFAULT_TEXT)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--ideal", action="store_true", help="use the ideal (noise-free) contract")
    parser.add_argument("--out", help="write the JSON report here (default: stdout)")
    args = parser.parse_args(argv)
    noise = None
    if args.ideal:
        import awen_native

        noise = awen_native.ideal_contract()
    loaded = Runtime(device="photonic-sim", noise=noise, seed=args.seed).load_model(args.model)
    payload = json.dumps(build_report(loaded, args.text), indent=2, sort_keys=True) + "\n"
    if args.out:
        with open(args.out, "w", encoding="utf-8") as handle:
            handle.write(payload)
    else:
        print(payload, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
