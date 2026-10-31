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
eager reference. Everything is simulated (L1); no energy, latency or speedup
figure is produced.
"""

from __future__ import annotations

from math import ceil, prod
from typing import Any, Dict, List, Mapping, Optional, Sequence

from .torch_backend import TORCH_BACKEND_VERSION, awen_backend, get_last_execution_traces

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


def fallback_reason(target: str) -> str:
    for token, reason in FALLBACK_REASONS:
        if token in target:
            return reason
    if any(target == token or target.endswith("." + token) for token in LAYOUT_TARGETS):
        return "view or layout change; no arithmetic"
    return "no AWEN lowering for this operation in " + TORCH_BACKEND_VERSION


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
    fallback: Dict[str, Dict[str, Any]] = {}
    for diagnostic in report.diagnostics:
        row = fallback.setdefault(diagnostic.target, {"target": diagnostic.target, "count": 0, "nodes": [],
                                                      "reason": fallback_reason(diagnostic.target)})
        row["count"] += 1
        row["nodes"].append(diagnostic.node)
    contract = dict(options.photonic_noise or awen_native.default_contract())
    return {
        "report": REPORT_VERSION,
        "backend": {"version": report.version, "torch_version": report.torch_version,
                    "graph_fingerprint": report.fingerprint, "graphs": 1},
        "execution": {"target": "simulated:photonic", "fidelity": "simulated (compact model)", "physical": False,
                      "evidence_level": "L1", "seed": options.seed, "tile_k": options.photonic_tile_k,
                      "seed_note": "torch-backend v1 passes the same seed to every lowered GEMM of a call"},
        "lowered": lowered,
        "fallback": sorted(fallback.values(), key=lambda row: (-row["count"], row["target"])),
        "counts": {"lowered_nodes": len(lowered), "fallback_nodes": len(report.fallback_nodes),
                   "regions": len(report.regions), "lowered_macs": sum(row["macs"] for row in lowered),
                   "host_macs": "not counted (fallback nodes are not shape-analysed in v1)"},
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
