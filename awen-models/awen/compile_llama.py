"""A tiny randomly initialised ``transformers`` Llama through ``torch.compile(backend=awen)``, with a capability report.

    python -m awen.compile_llama --out report.json          # default noise contract
    python -m awen.compile_llama --ideal --out ideal.json   # noise-free contract

This is the first external open-model architecture through the AWEN torch.compile
backend. The model is built locally from a ``transformers`` ``LlamaConfig``
(``TINY_LLAMA``) with seeded random weights: nothing is downloaded and no remote
code runs. TorchDynamo captures one FX graph (``fullgraph=True``, ``use_cache=False``,
SDPA attention). Every ``linear`` node (q/k/v/o projections, gate/up/down MLP
projections and the untied output head) goes to the Rust photonic GEMM simulator
(``awen_native``); every other node stays on the host and is listed with its reason
and host contraction MACs (``awen_py.compile_report.HOST_MAC_RULE``). The logits are
compared with the eager CPU model. Random weights say nothing about model quality:
the run checks graph capture, lowering coverage and the simulator path only.
Simulated (L1); no energy, latency or speedup figure.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Optional

import torch

REPORT_VERSION = "awen.llama-compile-capability-report.v1"
TINY_LLAMA = {"vocab_size": 256, "hidden_size": 64, "intermediate_size": 128, "num_hidden_layers": 2,
              "num_attention_heads": 4, "num_key_value_heads": 2, "max_position_embeddings": 128,
              "tie_word_embeddings": False}


def tiny_llama(init_seed: int = 0) -> Any:
    """``LlamaForCausalLM`` at ``TINY_LLAMA`` with weights drawn from ``init_seed`` (no download)."""
    from transformers import LlamaConfig, LlamaForCausalLM

    config = LlamaConfig(**TINY_LLAMA, attn_implementation="sdpa")
    with torch.random.fork_rng(devices=[]):
        torch.manual_seed(init_seed)
        model = LlamaForCausalLM(config)
    return model.eval()


@torch.no_grad()
def build(positions: int = 32, seed: int = 0, init_seed: int = 0, input_seed: int = 0,
          noise: Optional[dict[str, Any]] = None, seed_policy: str = "per_gemm") -> dict[str, Any]:
    try:
        import transformers
        from awen_py.compile_report import CompileCapture, build_compile_capability_report
    except ImportError as error:  # pragma: no cover - exercised when an optional dependency is absent
        raise RuntimeError("needs transformers (pip install './awen-models[hf]') and the AWEN Python "
                           "integrations (pip install ./awen-ecosystem/python_awen)") from error
    if not 1 <= positions <= TINY_LLAMA["max_position_embeddings"]:
        raise ValueError(f"positions must be in 1..{TINY_LLAMA['max_position_embeddings']}")
    model = tiny_llama(init_seed)
    generator = torch.Generator().manual_seed(input_seed)
    ids = torch.randint(0, TINY_LLAMA["vocab_size"], (1, positions), generator=generator)
    reference = model(input_ids=ids, use_cache=False).logits
    torch._dynamo.reset()  # a fresh capture, so the report covers exactly one compiled graph
    capture = CompileCapture({"target": "photonic", "photonic_noise": noise, "seed": seed, "seed_policy": seed_policy})
    compiled = torch.compile(model, backend=capture, fullgraph=True)
    logits = compiled(input_ids=ids, use_cache=False).logits
    report = build_compile_capability_report(capture, logits, reference)
    return {
        "report": REPORT_VERSION,
        "model": {"architecture": "LlamaForCausalLM", "config": dict(TINY_LLAMA),
                  "attn_implementation": "sdpa", "weights": f"random init, torch.manual_seed({init_seed}); "
                  "built locally from transformers.LlamaConfig, nothing downloaded, no remote code",
                  "transformers_version": transformers.__version__,
                  "parameters": sum(parameter.numel() for parameter in model.parameters())},
        "input": {"positions": positions, "token_ids": f"torch.randint(0, {TINY_LLAMA['vocab_size']}) with "
                  f"torch.Generator().manual_seed({input_seed})", "use_cache": False},
        "contract": "ideal" if noise is not None else "default",
        "compile": report,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--positions", type=int, default=32)
    parser.add_argument("--seed", type=int, default=0, help="noise seed")
    parser.add_argument("--init-seed", type=int, default=0, help="weight initialisation seed")
    parser.add_argument("--input-seed", type=int, default=0, help="token id seed")
    parser.add_argument("--ideal", action="store_true", help="use the ideal (noise-free) contract")
    parser.add_argument("--seed-policy", choices=("per_gemm", "shared"), default="per_gemm",
                        help="per_gemm: one derived seed per lowered GEMM; shared: the same seed for all")
    parser.add_argument("--out", help="write the JSON report here (default: stdout)")
    args = parser.parse_args(argv)
    noise = None
    if args.ideal:
        import awen_native

        noise = awen_native.ideal_contract()
    payload = build(args.positions, args.seed, args.init_seed, args.input_seed, noise, args.seed_policy)
    text = json.dumps(payload, indent=2, sort_keys=True) + "\n"
    if args.out:
        Path(args.out).write_text(text, encoding="utf-8")
    else:
        print(text, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
