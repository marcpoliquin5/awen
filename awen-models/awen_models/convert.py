"""Model conversion utility (directive item C10).

Two conversions, both verified numerically rather than assumed:

* ``to-onnx``: an AWEN-Mini safetensors checkpoint -> ONNX (opset 17, legacy
  TorchScript exporter, fixed sequence length). The exported graph is executed
  with ``onnx.reference.ReferenceEvaluator`` and compared with the PyTorch
  model on the same tokens; the maximum absolute logit difference is part of
  the report. The report also includes the AWEN ONNX compatibility analysis
  (which nodes are photonic candidates and which stay electronic).
* ``to-program``: an ONNX model -> an ``awen.tensor.v1`` program with one
  ``gemm`` op per MatMul/Gemm node whose weight is a constant, so
  ``awenctl compile`` can plan it. Activation x activation products (attention
  scores) stay electronic and are listed as skipped, with the reason.

Nothing here measures hardware.
"""

from __future__ import annotations

import argparse
import json
from pathlib import Path
from typing import Any, Optional, Union

import numpy as np

from . import onnx_import

TENSOR_IR_VERSION = "awen.tensor.v1"
REPORT_VERSION = "awen.conversion-report.v1"


def _onnx():
    return onnx_import._onnx()[0]


def mini_to_onnx(
    checkpoint_dir: Union[str, Path],
    output: Union[str, Path],
    seq_len: int = 32,
    seed: int = 0,
) -> dict[str, Any]:
    """Export an AWEN-Mini checkpoint to ONNX and verify it against PyTorch."""
    import torch

    from .checkpoint import load_checkpoint

    onnx = _onnx()
    from onnx.reference import ReferenceEvaluator

    model, tokenizer, _ = load_checkpoint(checkpoint_dir)
    if not 1 <= seq_len <= model.config.block_size:
        raise ValueError(f"seq_len must be in 1..{model.config.block_size}")

    class LogitsOnly(torch.nn.Module):
        def __init__(self, inner):
            super().__init__()
            self.inner = inner

        def forward(self, tokens):
            out = self.inner(tokens)
            return out[0] if isinstance(out, (tuple, list)) else out

    wrapper = LogitsOnly(model).eval()
    generator = torch.Generator().manual_seed(seed)
    tokens = torch.randint(0, tokenizer.vocab_size, (1, seq_len), generator=generator)
    with torch.no_grad():
        expected = wrapper(tokens).numpy()
    output = Path(output)
    output.parent.mkdir(parents=True, exist_ok=True)
    import warnings

    with warnings.catch_warnings():
        warnings.simplefilter("ignore")
        torch.onnx.export(
            wrapper,
            (tokens,),
            str(output),
            dynamo=False,
            input_names=["tokens"],
            output_names=["logits"],
            opset_version=17,
        )
    proto = onnx.load(str(output))
    onnx.checker.check_model(proto)
    got = ReferenceEvaluator(proto).run(None, {"tokens": tokens.numpy()})[0]
    report = onnx_import.analyze(proto, name=output.name)
    return {
        "version": REPORT_VERSION,
        "conversion": "awen-mini-checkpoint->onnx",
        "source": str(checkpoint_dir),
        "output": str(output),
        "opset": 17,
        "sequence_length": seq_len,
        "seed": seed,
        "verification": {
            "oracle": "torch forward vs onnx.reference.ReferenceEvaluator",
            "max_abs_logit_diff": float(np.abs(got - expected).max()),
            "logits_shape": list(got.shape),
        },
        "op_histogram": _histogram(proto),
        "compatibility": report.summary(),
    }


def _histogram(proto) -> dict[str, int]:
    counts: dict[str, int] = {}
    for node in proto.graph.node:
        counts[node.op_type] = counts.get(node.op_type, 0) + 1
    return dict(sorted(counts.items()))


def onnx_to_tensor_program(model: Union[str, Path, Any], dtype: str = "f32") -> dict[str, Any]:
    """Map constant-weight MatMul/Gemm nodes to an ``awen.tensor.v1`` program."""
    proto = onnx_import.load(model)
    shapes = onnx_import._shapes(proto)
    report = onnx_import.analyze(proto)
    tensors: list[dict[str, Any]] = []
    ops: list[dict[str, Any]] = []
    skipped: list[dict[str, str]] = []
    epilogues: list[dict[str, Any]] = []
    for decision, node in zip(report.nodes, proto.graph.node):
        if node.op_type not in ("MatMul", "Gemm"):
            continue
        if decision.awen_op != "gemm":
            skipped.append({"node": decision.name, "reason": decision.reason})
            continue
        x_shape = shapes.get(node.input[0], [])
        w_shape = shapes.get(node.input[1], [])
        if not x_shape or any(d is None or d <= 0 for d in x_shape):
            skipped.append({"node": decision.name, "reason": "activation shape is not static"})
            continue
        attrs = onnx_import._attrs(node)
        trans_b = bool(attrs.get("transB", 0)) if node.op_type == "Gemm" else False
        rows = int(np.prod(x_shape[:-1]))
        k = int(x_shape[-1])
        n = int(w_shape[0] if trans_b else w_shape[1])
        index = len(ops)
        lhs, rhs, out = f"op{index}.lhs", f"op{index}.rhs", f"op{index}.out"
        tensors += [
            {"id": lhs, "shape": [rows, k], "dtype": dtype, "layout": "row_major"},
            {"id": rhs, "shape": list(map(int, w_shape)), "dtype": dtype, "layout": "row_major"},
            {"id": out, "shape": [rows, n], "dtype": dtype, "layout": "row_major"},
        ]
        ops.append(
            {
                "op": "gemm",
                "id": decision.name.replace("/", "_") or f"gemm{index}",
                "lhs": lhs,
                "rhs": rhs,
                "output": out,
                "transpose_rhs": trans_b,
            }
        )
        if node.op_type == "Gemm" and (attrs.get("alpha", 1.0) != 1.0 or len(node.input) > 2):
            # The tensor program carries the GEMM only; scale and bias run
            # electronically after detection.
            epilogues.append(
                {
                    "op": ops[-1]["id"],
                    "alpha": float(attrs.get("alpha", 1.0)),
                    "bias": node.input[2] if len(node.input) > 2 else None,
                }
            )
    program = {
        "ir_version": TENSOR_IR_VERSION,
        "tensors": tensors,
        "ops": ops,
        "metadata": {
            "source": "onnx",
            "graph": proto.graph.name or "onnx-graph",
            "converted_by": "awen_models.convert",
            "skipped_matmuls": str(len(skipped)),
        },
    }
    return {"program": program, "skipped": skipped, "electronic_epilogues": epilogues}


def main(argv: Optional[list[str]] = None) -> None:
    parser = argparse.ArgumentParser(prog="awen-convert", description=__doc__.split("\n\n")[0])
    sub = parser.add_subparsers(dest="command", required=True)
    to_onnx = sub.add_parser("to-onnx", help="AWEN-Mini checkpoint -> ONNX, verified")
    to_onnx.add_argument("checkpoint", nargs="?", help="checkpoint directory (default: bundled AWEN-Mini)")
    to_onnx.add_argument("--output", required=True)
    to_onnx.add_argument("--seq-len", type=int, default=32)
    to_onnx.add_argument("--seed", type=int, default=0)
    to_onnx.add_argument("--report", help="write the conversion report JSON here")
    to_program = sub.add_parser("to-program", help="ONNX -> awen.tensor.v1 program for awenctl compile")
    to_program.add_argument("model")
    to_program.add_argument("--output", required=True)
    to_program.add_argument("--dtype", default="f32")
    args = parser.parse_args(argv)
    if args.command == "to-onnx":
        from .checkpoint import bundled_checkpoint_dir

        report = mini_to_onnx(args.checkpoint or bundled_checkpoint_dir(), args.output, args.seq_len, args.seed)
        text = json.dumps(report, indent=2)
        if args.report:
            Path(args.report).write_text(text + "\n")
        print(text)
    else:
        result = onnx_to_tensor_program(args.model, args.dtype)
        Path(args.output).write_text(json.dumps(result["program"], indent=2) + "\n")
        summary = {
            "ops": len(result["program"]["ops"]),
            "skipped": result["skipped"],
            "electronic_epilogues": result["electronic_epilogues"],
        }
        print(json.dumps(summary, indent=2))


if __name__ == "__main__":
    main()
