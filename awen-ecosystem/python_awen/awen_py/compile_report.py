"""Capability report for a ``torch.compile(backend=awen)`` run on the photonic simulator.

    from awen_py.compile_report import CompileCapture, build_compile_capability_report
    capture = CompileCapture({"target": "photonic"})
    compiled = torch.compile(model, backend=capture)
    with torch.no_grad():
        reference, output = model(x), compiled(x)
    report = build_compile_capability_report(capture, output, reference)

The report follows the entry-point checklist: which FX nodes lowered, which
fell back and why, the precision and calibration the noise contract assumes,
simulated vs physical execution, conversion counts, and the deviation from an
eager reference. Host fallback nodes are shape-analysed from the FX
``example_value`` metadata to count the multiply-accumulates they leave on the
host (``HOST_MAC_RULE``). Everything is simulated (L1); no energy, latency or
speedup figure is produced.
"""

from __future__ import annotations

from math import ceil, prod
from typing import Any, Dict, List, Mapping, Optional, Sequence

from .torch_backend import (GEMM_SEED_DERIVATION, TORCH_BACKEND_VERSION, awen_backend,
                            get_last_execution_traces)

REPORT_VERSION = "awen.torch-compile-capability-report.v1"
CONTRACT_PRECISION_KEYS = ("dac_bits", "adc_bits", "photons_per_mac", "phase_sigma_rad",
                           "drift_rad_per_sqrt_s", "seconds_since_calibration")

# Why each kind of FX node stays on the host. Matching is by substring of the
# node's target name; the first match wins.
FALLBACK_REASONS = (
    ("scaled_dot_product_attention", "attention core (QK^T, softmax, AV): both operands are activations, so no "
     "weight stays programmed; torch-backend v1 lowers only matmul/mm/bmm and linear nodes"),
    ("embedding", "table gather, not a matrix product"),
    ("layer_norm", "per-row mean, variance and division; not a linear map"),
    ("gelu", "elementwise nonlinearity; stays in host electronics after detection"),
    ("relu", "elementwise nonlinearity; stays in host electronics after detection"),
    ("silu", "elementwise nonlinearity; stays in host electronics after detection"),
    ("softmax", "elementwise exponent and normalisation; host electronics"),
    ("dropout", "identity at inference; host"),
    ("operator.add", "elementwise add (residual or bias); host electronics"),
    ("operator.mul", "elementwise product; host electronics"),
    ("arange", "index generation; host"),
)
HOST_MAC_RULE = ("contraction MACs only: scaled_dot_product_attention counts QK^T plus AV densely "
                 "(batch*L*S*(E+Ev); causal masking is not subtracted), addmm/baddbmm count rows*inner*outputs; "
                 "elementwise, normalisation, gather, index and layout nodes count 0 by definition (their adds, "
                 "squares and reductions are not matrix MACs); RMS-norm (pow, mean, rsqrt), rotary (cos, sin, neg, "
                 "cat), dtype casts (to, float) and nodes whose output is an integer or boolean tensor (index and "
                 "mask construction) count 0; any other node is listed as uncounted")
# Fallback targets that perform no matrix contraction (substring of the target name).
ZERO_MAC_TOKENS = ("embedding", "layer_norm", "gelu", "relu", "silu", "softmax", "dropout", "operator.add",
                   "operator.mul", "operator.sub", "operator.truediv", "arange")
# Fallback targets matched by exact name (or a ``.name`` suffix), so short names such as ``to`` never match by
# substring. Each counts 0 contraction MACs (seen in transformers' Llama: RMS norm, rotary embedding, casts).
EXACT_REASONS = (
    (("pow", "mean", "rsqrt"), "RMS normalisation step (square, mean, reciprocal square root); not a linear map"),
    (("cos", "sin"), "rotary position table (cosine, sine); host elementwise"),
    (("neg",), "elementwise negation (rotary half-rotation); host electronics"),
    (("cat",), "concatenation; layout, no arithmetic"),
    (("to", "float"), "dtype or device cast; no arithmetic"),
    (("eq", "ne", "le", "lt", "ge", "gt", "and_", "or_"), "comparison or logical op (mask construction); host"),
    (("_log_api_usage_once",), "PyTorch API-usage logging call; no tensor operation"),
)
INTEGER_DTYPES = ("torch.bool", "torch.int8", "torch.uint8", "torch.int16", "torch.int32", "torch.int64")
LAYOUT_TARGETS = ("view", "reshape", "transpose", "permute", "contiguous", "split", "getitem", "size",
                  "unsqueeze", "squeeze", "expand", "flatten", "numpy_t")


class CompileCapture:
    """A ``torch.compile`` backend that forwards to ``awen_backend`` and keeps every compiled graph."""

    def __init__(self, options: Optional[Mapping[str, Any]] = None):
        self.options = dict(options or {})
        self.compiled: List[Any] = []

    def __call__(self, graph_module: Any, example_inputs: Sequence[Any]):
        compiled = awen_backend(graph_module, example_inputs, options=self.options)
        self.compiled.append(compiled)
        return compiled


_SEED_NOTES = {
    "shared": "seed_policy 'shared': every lowered GEMM of a call gets the same seed, so their noise draws are "
              "correlated",
    "per_gemm": f"seed_policy 'per_gemm': lowered GEMM i (graph order) gets its own seed, derived by "
                f"{GEMM_SEED_DERIVATION} (SHA-256 of '{GEMM_SEED_DERIVATION}:<seed>:<i>', first 8 bytes >> 2); "
                f"each row lists its seed",
}


def _exact(target: str, tokens: Sequence[str]) -> bool:
    return any(target == token or target.endswith("." + token) for token in tokens)


def fallback_reason(target: str) -> str:
    for token, reason in FALLBACK_REASONS:
        if token in target:
            return reason
    if _exact(target, LAYOUT_TARGETS):
        return "view or layout change; no arithmetic"
    for tokens, reason in EXACT_REASONS:
        if _exact(target, tokens):
            return reason
    return "no AWEN lowering for this operation in " + TORCH_BACKEND_VERSION


def _shape(value: Any) -> Optional[List[int]]:
    meta = getattr(value, "meta", None)
    if not isinstance(meta, dict):
        return None
    tensor = meta.get("example_value")
    if tensor is None:
        tensor = meta.get("tensor_meta")
    shape = getattr(tensor, "shape", None)
    return None if shape is None else [int(size) for size in shape]


def _integer_output(node: Any) -> bool:
    meta = getattr(node, "meta", None)
    tensor = meta.get("example_value") if isinstance(meta, dict) else None
    return str(getattr(tensor, "dtype", "")) in INTEGER_DTYPES


def _argument(node: Any, index: int, name: str) -> Any:
    return node.args[index] if len(node.args) > index else node.kwargs.get(name)


def host_macs(node: Any, target: str) -> Optional[int]:
    """Contraction MACs a fallback FX node runs on the host, by ``HOST_MAC_RULE``; ``None`` if not countable."""
    if "scaled_dot_product_attention" in target:
        query, key, value = (_shape(_argument(node, index, name))
                             for index, name in enumerate(("query", "key", "value")))
        output = _shape(node)
        if not (query and key and value and output) or min(len(query), len(key), len(value)) < 2:
            return None
        return prod(output[:-2]) * query[-2] * key[-2] * (query[-1] + value[-1])
    if target.endswith("addmm") or target.endswith("baddbmm"):
        lhs, rhs = _shape(_argument(node, 1, "mat1")), _shape(_argument(node, 2, "mat2"))
        if target.endswith("baddbmm"):
            lhs, rhs = _shape(_argument(node, 1, "batch1")), _shape(_argument(node, 2, "batch2"))
        if not (lhs and rhs) or len(lhs) < 2 or len(rhs) < 2:
            return None
        return prod(lhs[:-1]) * lhs[-1] * rhs[-1]
    if any(token in target for token in ZERO_MAC_TOKENS) or fallback_reason(target).startswith("view or layout"):
        return 0
    if any(_exact(target, tokens) for tokens, _ in EXACT_REASONS) or _integer_output(node):
        return 0
    return None


def _gemm_row(node: str, operation: str, shapes: Sequence[Sequence[int]], tile_k: int) -> Dict[str, Any]:
    lhs, rhs = list(shapes[0]), list(shapes[1])
    inner, outputs = int(lhs[-1]), int(rhs[-1])
    batch = prod(rhs[:-2]) if len(rhs) > 2 else 1
    rows = prod(lhs[:-1]) if len(rhs) == 2 else batch * int(lhs[-2])
    weights = batch * inner * outputs
    tiles = ceil(inner / tile_k)
    return {"node": node, "operation": operation, "input_shapes": [lhs, rhs], "rows": rows, "inner": inner,
            "outputs": outputs, "macs": rows * inner * outputs, "dac_input_conversions": rows * inner,
            "dac_weight_conversions": weights, "adc_conversions": rows * outputs * tiles,
            "tiles_per_output": tiles}


def _deviation(output: Any, reference: Any) -> Dict[str, Any]:
    diff = (output.detach().double() - reference.detach().double())
    ref = reference.detach().double()
    result = {"max_abs_diff": float(diff.abs().max()), "rms_diff": float(diff.pow(2).mean().sqrt()),
              "relative_l2": float(diff.norm() / ref.norm()) if float(ref.norm()) > 0 else None}
    if ref.dim() >= 2:
        result["top1_agreement"] = float((output.argmax(-1) == reference.argmax(-1)).double().mean())
    return result


def build_compile_capability_report(capture: CompileCapture, output: Any, reference: Any,
                                    traces: Optional[Sequence[Any]] = None) -> Dict[str, Any]:
    """Build the report for the last call of a model compiled with ``capture``.

    ``traces`` defaults to the execution traces of that call. The function
    supports exactly one compiled graph (no graph breaks), so traces map
    one-to-one, in order, onto the lowered nodes.
    """
    import awen_native

    if len(capture.compiled) != 1:
        raise ValueError(f"expected one compiled graph (no graph breaks), got {len(capture.compiled)}")
    report = capture.compiled[0].awen_report
    options = report.options
    if options.target != "photonic":
        raise ValueError("the capability report covers target='photonic' runs only")
    traces = tuple(get_last_execution_traces() if traces is None else traces)
    if len(traces) != len(report.supported_nodes) or any(not trace.events for trace in traces):
        raise ValueError("need one profiled execution trace per lowered node; compile with profile=True and "
                         "call the compiled model once before building the report")
    lowered = [_gemm_row(node, trace.events[0].operation, trace.events[0].input_shapes, options.photonic_tile_k)
               for node, trace in zip(report.supported_nodes, traces)]
    for index, row in enumerate(lowered):
        row["seed"] = options.gemm_seed(index)
    graph_nodes = {node.name: node for node in capture.compiled[0].awen_graph_module.graph.nodes}
    fallback: Dict[str, Dict[str, Any]] = {}
    uncounted: List[str] = []
    for diagnostic in report.diagnostics:
        row = fallback.setdefault(diagnostic.target, {"target": diagnostic.target, "count": 0, "nodes": [],
                                                      "reason": fallback_reason(diagnostic.target), "macs": 0})
        row["count"] += 1
        row["nodes"].append(diagnostic.node)
        macs = host_macs(graph_nodes[diagnostic.node], diagnostic.target) if diagnostic.node in graph_nodes else None
        if macs is None:
            uncounted.append(diagnostic.node)
            row["macs"] = None
        elif row["macs"] is not None:
            row["macs"] += macs
    lowered_macs = sum(row["macs"] for row in lowered)
    host_total = sum(row["macs"] for row in fallback.values() if row["macs"] is not None)
    contract = dict(options.photonic_noise or awen_native.default_contract())
    return {
        "report": REPORT_VERSION,
        "backend": {"version": report.version, "torch_version": report.torch_version,
                    "graph_fingerprint": report.fingerprint, "graphs": 1},
        "execution": {"target": "simulated:photonic", "fidelity": "simulated (compact model)", "physical": False,
                      "evidence_level": "L1", "seed": options.seed, "tile_k": options.photonic_tile_k,
                      "seed_policy": options.seed_policy, "seed_note": _SEED_NOTES[options.seed_policy]},
        "lowered": lowered,
        "fallback": sorted(fallback.values(), key=lambda row: (-row["count"], row["target"])),
        "counts": {"lowered_nodes": len(lowered), "fallback_nodes": len(report.fallback_nodes),
                   "regions": len(report.regions), "lowered_macs": lowered_macs,
                   "host_macs": "see host_fallback_macs (fallback nodes are shape-analysed since P2-3)",
                   "host_fallback_macs": host_total, "host_macs_uncounted_nodes": uncounted,
                   "host_mac_rule": HOST_MAC_RULE,
                   "lowered_mac_fraction": (lowered_macs / (lowered_macs + host_total)
                                            if not uncounted and lowered_macs + host_total else None)},
        "precision_and_calibration": {
            "contract_version": contract.get("version"),
            **{key: contract.get(key) for key in CONTRACT_PRECISION_KEYS},
            "calibration": "assumed by the noise contract; no calibration was performed (simulation only)",
        },
        "conversions": {
            "dac_input": sum(row["dac_input_conversions"] for row in lowered),
            "dac_weight": sum(row["dac_weight_conversions"] for row in lowered),
            "adc": sum(row["adc_conversions"] for row in lowered),
            "counting_rule": "simulator tiling: rows*inner input DAC, weights programmed once per call, "
                             "rows*outputs*ceil(inner/tile_k) ADC reads; partial sums accumulate on the host",
        },
        "energy": "not estimated: needs the full system accounting in awen-spec/hardware_assumptions.yaml",
        "deviation_vs_reference": _deviation(output, reference),
    }
