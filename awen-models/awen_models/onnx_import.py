"""Minimal ONNX import: map an ONNX graph onto the AWEN operator table, report
what could run on a photonic matrix unit, and execute supported graphs.

Mapping rules (``awen.onnx-report.v1``):

* ``MatMul`` / ``Gemm`` with a constant weight and ``Conv`` (2-D, group 1,
  dilation 1) with constant kernels map to the AWEN ``gemm`` / ``conv2d``
  operators. Their matrix products are the photonic part.
* ``MatMul`` between two activations stays electronic: it would require the
  optical weights to be reprogrammed for every input.
* Elementwise, shape and normalization-free nonlinear ops run electronically
  after detection.
* Anything else is ``unsupported``; such a graph is reported but not run.

``run`` executes the graph. With ``contract=None`` the AWEN f64 reference runs
(fidelity ``exact``). With a noise contract every photonic product runs on the
Rust compact noise model (fidelity ``simulated``). Neither is a hardware
measurement. Levels come from ``awen_native.op_table()``.

Example::

    awen-onnx-report model.onnx --json report.json
"""

from __future__ import annotations

import argparse
import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any, Mapping, Optional, Union

import numpy as np

REPORT_VERSION = "awen.onnx-report.v1"

ELECTRONIC = {
    "Add": np.add,
    "Sub": np.subtract,
    "Mul": np.multiply,
    "Div": np.divide,
    "Relu": lambda x: np.maximum(x, 0),
    "Sigmoid": lambda x: 1 / (1 + np.exp(-x)),
    "Tanh": np.tanh,
    "Identity": lambda x: x,
}
SHAPE_OPS = {"Reshape", "Flatten", "Transpose", "Softmax", "Constant", "Gelu"}


def _onnx():
    try:
        import onnx
        from onnx import numpy_helper
    except ImportError as exc:  # pragma: no cover - exercised only without the extra
        raise ImportError("ONNX import needs `pip install awen-models[onnx]`") from exc
    return onnx, numpy_helper


@dataclass
class NodeDecision:
    index: int
    name: str
    op_type: str
    placement: str  # photonic | electronic | unsupported
    awen_op: Optional[str]
    level: Optional[str]
    reason: str
    macs: int = 0


@dataclass
class OnnxReport:
    model: str
    opset: int
    nodes: list[NodeDecision] = field(default_factory=list)

    @property
    def executable(self) -> bool:
        return all(n.placement != "unsupported" for n in self.nodes)

    def summary(self) -> dict[str, Any]:
        total = sum(n.macs for n in self.nodes)
        photonic = sum(n.macs for n in self.nodes if n.placement == "photonic")
        return {
            "nodes": len(self.nodes),
            "photonic_nodes": sum(n.placement == "photonic" for n in self.nodes),
            "electronic_nodes": sum(n.placement == "electronic" for n in self.nodes),
            "unsupported_ops": sorted({n.op_type for n in self.nodes if n.placement == "unsupported"}),
            "total_macs": total,
            "photonic_macs": photonic,
            "photonic_mac_fraction": photonic / total if total else 0.0,
            "executable": self.executable,
        }

    def to_dict(self) -> dict[str, Any]:
        return {
            "version": REPORT_VERSION,
            "model": self.model,
            "opset": self.opset,
            "evidence": "static analysis against the AWEN operator table; not a hardware measurement",
            "summary": self.summary(),
            "nodes": [asdict(n) for n in self.nodes],
        }


def load(model: Union[str, Path, Any]):
    """Load an ONNX model (path or ModelProto), check it and infer shapes."""
    onnx, _ = _onnx()
    proto = onnx.load(str(model)) if isinstance(model, (str, Path)) else model
    onnx.checker.check_model(proto)
    return onnx.shape_inference.infer_shapes(proto)


def _attrs(node) -> dict[str, Any]:
    onnx, _ = _onnx()
    return {a.name: onnx.helper.get_attribute_value(a) for a in node.attribute}


def _shapes(proto) -> dict[str, list[Optional[int]]]:
    out = {}
    g = proto.graph
    for vi in list(g.input) + list(g.value_info) + list(g.output):
        dims = vi.type.tensor_type.shape.dim
        out[vi.name] = [d.dim_value if d.HasField("dim_value") else None for d in dims]
    for init in g.initializer:
        out[init.name] = list(init.dims)
    return out


def _levels() -> dict[str, str]:
    import awen_native

    return {o["name"]: o["level"] for o in awen_native.op_table()}


def _prod(dims) -> int:
    p = 1
    for d in dims:
        if d is None:
            return 0
        p *= d
    return p


def analyze(model: Union[str, Path, Any], name: Optional[str] = None) -> OnnxReport:
    proto = load(model)
    g = proto.graph
    consts = {i.name for i in g.initializer}
    consts |= {o for n in g.node if n.op_type == "Constant" for o in n.output}
    shapes = _shapes(proto)
    levels = _levels()
    opset = max((o.version for o in proto.opset_import if o.domain in ("", "ai.onnx")), default=0)
    report = OnnxReport(model=name or (str(model) if isinstance(model, (str, Path)) else g.name), opset=opset)
    for i, node in enumerate(g.node):
        op, a = node.op_type, _attrs(node)
        dec = NodeDecision(i, node.name or f"{op}_{i}", op, "unsupported", None, None, f"{op} is not in the AWEN ONNX mapping")
        if op in ("MatMul", "Gemm"):
            x, w = node.input[0], node.input[1]
            ws = shapes.get(w, [])
            xs = shapes.get(x, [])
            if op == "Gemm" and a.get("transA", 0):
                dec.reason = "Gemm with transA=1 is not mapped"
            elif w in consts and len(ws) == 2:
                k, n = (ws[1], ws[0]) if a.get("transB", 0) else (ws[0], ws[1])
                rows = _prod(xs[:-1]) if xs else 0
                dec.placement, dec.awen_op = "photonic", "gemm"
                dec.reason = f"constant [{k}x{n}] weight; activation rows x weightᵀ"
                dec.macs = rows * k * n
            else:
                rows = _prod(xs[:-1]) if xs else 0
                dec.placement, dec.awen_op = "electronic", None
                dec.reason = "both operands are activations; weights would be reprogrammed per input"
                if len(ws) >= 2:
                    dec.macs = rows * ws[-1] * ws[-2] if xs else 0
        elif op == "Conv":
            ws = shapes.get(node.input[1], [])
            ys = shapes.get(node.output[0], [])
            if a.get("group", 1) != 1:
                dec.reason = "grouped/depthwise Conv is not mapped"
            elif any(d != 1 for d in a.get("dilations", [1, 1])):
                dec.reason = "dilated Conv is not mapped"
            elif len(ws) != 4 or node.input[1] not in consts:
                dec.reason = "only 2-D Conv with a constant kernel is mapped"
            elif len(set(a.get("strides", [1, 1]))) != 1 or len(set(a.get("pads", [0, 0, 0, 0]))) != 1:
                dec.reason = "only equal strides and symmetric pads are mapped"
            elif a.get("auto_pad", b"NOTSET") not in (b"NOTSET", "NOTSET"):
                dec.reason = "auto_pad is not mapped"
            else:
                dec.placement, dec.awen_op = "photonic", "conv2d"
                dec.reason = "im2col patches x kernelsᵀ"
                dec.macs = _prod(ys) * ws[1] * ws[2] * ws[3] if ys else 0
        elif op in ELECTRONIC or op in SHAPE_OPS:
            dec.placement = "electronic"
            dec.reason = "electronic post-detection step" if op in ELECTRONIC or op in ("Softmax", "Gelu") else "shape/constant op"
        if dec.awen_op:
            dec.level = levels.get(dec.awen_op)
        report.nodes.append(dec)
    return report


def _softmax(x: np.ndarray, axis: int) -> np.ndarray:
    e = np.exp(x - x.max(axis=axis, keepdims=True))
    return e / e.sum(axis=axis, keepdims=True)


def run(
    model: Union[str, Path, Any],
    inputs: Mapping[str, np.ndarray],
    contract: Optional[Mapping[str, Any]] = None,
    tile_k: int = 64,
    seed: int = 0,
) -> tuple[dict[str, np.ndarray], dict[str, Any]]:
    """Execute the graph; returns ``(outputs, run_report)``."""
    import awen_native

    _, numpy_helper = _onnx()
    proto = load(model)
    report = analyze(proto)
    if not report.executable:
        raise ValueError(f"graph has unsupported ops: {report.summary()['unsupported_ops']}")
    g = proto.graph
    env: dict[str, np.ndarray] = {i.name: numpy_helper.to_array(i).astype(np.float64) for i in g.initializer}
    for k, v in inputs.items():
        env[k] = np.asarray(v, dtype=np.float64)
    missing = [i.name for i in g.input if i.name not in env]
    if missing:
        raise ValueError(f"missing graph inputs: {missing}")
    photonic_macs = 0
    gemm_calls = 0

    def gemm(x2: np.ndarray, w_nk: np.ndarray, node_seed: int) -> np.ndarray:
        nonlocal photonic_macs, gemm_calls
        out, rep = awen_native.run_op("gemm", {"x": x2, "w": w_nk}, contract=contract, tile_k=tile_k, seed=node_seed)
        photonic_macs += rep["photonic_macs"]
        gemm_calls += rep["gemm_calls"]
        return out["y"]

    for dec, node in zip(report.nodes, g.node):
        op, a = node.op_type, _attrs(node)
        ins = [env[n] if n else None for n in node.input]
        s = seed + 1000 * dec.index
        if op == "MatMul" and dec.placement == "photonic":
            x, w = ins
            y = gemm(x.reshape(-1, x.shape[-1]), w.T, s).reshape(*x.shape[:-1], w.shape[1])
        elif op == "MatMul":
            y = np.matmul(ins[0], ins[1])
        elif op == "Gemm":
            x, w = ins[0], ins[1]
            w_nk = w if a.get("transB", 0) else w.T
            y = a.get("alpha", 1.0) * gemm(x, w_nk, s)
            if len(ins) > 2 and ins[2] is not None:
                y = y + a.get("beta", 1.0) * ins[2]
        elif op == "Conv":
            x, w = ins[0], ins[1]
            b = ins[2] if len(ins) > 2 else None
            params = {"stride": a.get("strides", [1, 1])[0], "padding": a.get("pads", [0, 0, 0, 0])[0]}
            ys = []
            for n in range(x.shape[0]):
                feed = {"x": x[n], "w": w}
                if b is not None:
                    feed["bias"] = b
                out, rep = awen_native.run_op("conv2d", feed, params, contract=contract, tile_k=tile_k, seed=s + n)
                photonic_macs += rep["photonic_macs"]
                gemm_calls += rep["gemm_calls"]
                ys.append(out["y"])
            y = np.stack(ys)
        elif op in ELECTRONIC:
            y = ELECTRONIC[op](*[i for i in ins if i is not None])
        elif op == "Reshape":
            shape = [int(d) for d in ins[1]]
            shape = [ins[0].shape[i] if d == 0 and not a.get("allowzero", 0) else d for i, d in enumerate(shape)]
            y = ins[0].reshape(shape)
        elif op == "Flatten":
            ax = a.get("axis", 1) % max(ins[0].ndim, 1)
            y = ins[0].reshape(int(np.prod(ins[0].shape[:ax])), -1)
        elif op == "Transpose":
            y = np.transpose(ins[0], a.get("perm"))
        elif op == "Softmax":
            y = _softmax(ins[0], a.get("axis", -1))
        elif op == "Gelu":
            from math import erf

            x = ins[0]
            if a.get("approximate", "none") in ("tanh", b"tanh"):
                y = 0.5 * x * (1 + np.tanh(np.sqrt(2 / np.pi) * (x + 0.044715 * x**3)))
            else:
                y = 0.5 * x * (1 + np.vectorize(erf)(x / np.sqrt(2)))
        elif op == "Constant":
            y = numpy_helper.to_array(a["value"]).astype(np.float64)
        else:  # pragma: no cover - analyze() rejects these
            raise ValueError(op)
        env[node.output[0]] = np.asarray(y, dtype=np.float64)

    outputs = {o.name: env[o.name] for o in g.output}
    run_report = {
        "version": REPORT_VERSION,
        "fidelity": "exact (f64 reference)" if contract is None else "simulated (compact noise model)",
        "photonic_macs": photonic_macs,
        "gemm_calls": gemm_calls,
        "summary": report.summary(),
    }
    return outputs, run_report


def format_report(report: OnnxReport) -> str:
    s = report.summary()
    lines = [
        f"{report.model} (opset {report.opset}): {s['nodes']} nodes, "
        f"{s['photonic_nodes']} photonic, {s['electronic_nodes']} electronic, "
        f"unsupported {s['unsupported_ops'] or 'none'}",
        f"photonic MAC fraction {s['photonic_mac_fraction']:.3f} of {s['total_macs']:,} MACs "
        "(static analysis, not a measurement)",
    ]
    for n in report.nodes:
        lines.append(f"  [{n.index}] {n.op_type:<10} {n.placement:<11} {n.awen_op or '-':<8} {n.level or '-':<18} {n.reason}")
    return "\n".join(lines)


def main(argv: Optional[list[str]] = None) -> None:
    p = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    p.add_argument("model", help="path to an .onnx file")
    p.add_argument("--json", help="write the report as JSON here")
    args = p.parse_args(argv)
    report = analyze(args.model)
    print(format_report(report))
    if args.json:
        Path(args.json).write_text(json.dumps(report.to_dict(), indent=2) + "\n")


if __name__ == "__main__":
    main()
