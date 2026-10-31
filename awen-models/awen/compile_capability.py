"""AWEN-Mini through ``torch.compile(backend=awen)`` on the photonic simulator, with a capability report.

    python -m awen.compile_capability --out report.json          # default noise contract
    python -m awen.compile_capability --ideal --out ideal.json   # noise-free contract
    python -m awen.compile_capability --seed-policy shared ...   # one seed for every GEMM (backend default)

TorchDynamo captures one FX graph of the bundled model's forward pass. The AWEN
backend sends every ``linear`` and ``matmul`` node (including the tied output
head) to the Rust photonic GEMM simulator (``awen_native``) and leaves the rest
on the host. The report lists lowered and fallback nodes with reasons, the
contract's precision and calibration, conversion counts and the logit deviation
from the eager CPU model. Each lowered GEMM gets its own deterministic noise
seed by default (``--seed-policy per_gemm``), so layer noise is not correlated.
Simulated (L1); no energy, latency or speedup figure.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Optional

import torch

from awen.capability import DEFAULT_TEXT
from awen_models.checkpoint import bundled_checkpoint_dir, load_checkpoint

REPORT_VERSION = "awen.mini-compile-capability-report.v1"


@torch.no_grad()
def build(model_name: str = "awen-mini", text: str = DEFAULT_TEXT, seed: int = 0,
          noise: Optional[dict[str, Any]] = None, seed_policy: str = "per_gemm") -> dict[str, Any]:
    try:
        from awen_py.compile_report import CompileCapture, build_compile_capability_report
    except ImportError as error:  # pragma: no cover - exercised when awen_py is absent
        raise RuntimeError("needs the AWEN Python integrations: pip install ./awen-ecosystem/python_awen") from error
    if not text:
        raise ValueError("text must contain at least one character")
    path = Path(model_name)
    directory = path if path.is_dir() else bundled_checkpoint_dir(model_name)
    model, tokenizer, _ = load_checkpoint(directory, torch.device("cpu"))
    model.eval()
    ids = torch.tensor([tokenizer.encode(text)], dtype=torch.long)[:, : model.config.block_size]
    reference, _ = model(ids)
    torch._dynamo.reset()  # a fresh capture, so the report covers exactly one compiled graph
    capture = CompileCapture({"target": "photonic", "photonic_noise": noise, "seed": seed, "seed_policy": seed_policy})
    compiled = torch.compile(model, backend=capture, fullgraph=True)
    logits, _ = compiled(ids)
    report = build_compile_capability_report(capture, logits, reference)
    return {
        "report": REPORT_VERSION,
        "model": {"checkpoint": directory.name, "parameters": model.num_parameters()},
        "input": {"characters": len(text), "positions": int(ids.shape[1])},
        "contract": "ideal" if noise is not None else "default",
        "compile": report,
    }


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--model", default="awen-mini", help="bundled name or checkpoint directory")
    parser.add_argument("--text", default=DEFAULT_TEXT)
    parser.add_argument("--seed", type=int, default=0)
    parser.add_argument("--ideal", action="store_true", help="use the ideal (noise-free) contract")
    parser.add_argument("--seed-policy", choices=("per_gemm", "shared"), default="per_gemm",
                        help="per_gemm: one derived seed per lowered GEMM; shared: the same seed for all")
    parser.add_argument("--out", help="write the JSON report here (default: stdout)")
    args = parser.parse_args(argv)
    noise = None
    if args.ideal:
        import awen_native

        noise = awen_native.ideal_contract()
    payload = json.dumps(build(args.model, args.text, args.seed, noise, args.seed_policy), indent=2, sort_keys=True) + "\n"
    if args.out:
        Path(args.out).write_text(payload, encoding="utf-8")
    else:
        print(payload, end="")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
