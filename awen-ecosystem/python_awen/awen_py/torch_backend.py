"""Native ``torch.compile`` backend for AWEN-supported linear regions."""

from __future__ import annotations

from contextlib import nullcontext
from contextvars import ContextVar
from dataclasses import asdict, dataclass, field, replace
import hashlib
import json
import operator
from typing import Any, Dict, Iterable, Mapping, Optional, Sequence, Tuple

from .errors import ContractError, UnsupportedGraphError
from .runtime import ExecutionOptions, NumericalContract, get_runtime


TORCH_BACKEND_VERSION = "awen.torch-backend.v1"
_SUPPORTED_TORCH_MINORS = (10, 15)  # PyTorch 2.x, inclusive lower and exclusive upper minor
SUPPORTED_TORCH_RANGE = f">=2.{_SUPPORTED_TORCH_MINORS[0]},<2.{_SUPPORTED_TORCH_MINORS[1]}"
# "shared": every lowered GEMM of a call gets ``seed`` (v1 behaviour, the default).
# "per_gemm": lowered GEMM number i (graph order) gets derive_gemm_seed(seed, i).
SEED_POLICIES = ("shared", "per_gemm")
GEMM_SEED_DERIVATION = "awen.torch-backend.gemm-seed.v1"
_GEMM_INDEX_KWARG = "awen_gemm_index"


def derive_gemm_seed(seed: int, gemm_index: int) -> int:
    """Deterministic noise seed for lowered GEMM ``gemm_index`` of a graph run with ``seed``.

    The first 8 bytes of SHA-256 over ``"awen.torch-backend.gemm-seed.v1:<seed>:<index>"``,
    shifted right by 2 bits so the result stays below 2**62: the photonic dispatcher adds
    the batch index to the seed for batched GEMMs, and the native seed is a u64.
    """
    if seed < 0 or gemm_index < 0:
        raise ContractError("seed and GEMM index must be non-negative")
    digest = hashlib.sha256(f"{GEMM_SEED_DERIVATION}:{seed}:{gemm_index}".encode()).digest()
    return int.from_bytes(digest[:8], "big") >> 2


@dataclass(frozen=True)
class TorchBackendOptions:
    target: str = "auto"
    allow_fallback: bool = True
    max_abs_error: float = 1.0e-5
    max_rel_error: float = 1.0e-5
    minimum_effective_bits: Optional[int] = None
    deterministic: bool = True
    seed: int = 0
    profile: bool = True
    photonic_noise: Optional[Mapping[str, Any]] = field(default=None, compare=False, hash=False)
    photonic_tile_k: int = 64
    seed_policy: str = "shared"

    @classmethod
    def from_mapping(cls, values: Optional[Mapping[str, Any]]) -> "TorchBackendOptions":
        values = dict(values or {})
        unknown = set(values) - set(cls.__dataclass_fields__)
        if unknown:
            raise ContractError(f"unknown torch backend options: {sorted(unknown)}")
        result = cls(**values)
        if result.seed_policy not in SEED_POLICIES:
            raise ContractError(f"seed_policy must be one of {list(SEED_POLICIES)}, got {result.seed_policy!r}")
        result.contract().validate()
        ExecutionOptions(
            target=result.target,
            allow_fallback=result.allow_fallback,
            deterministic=result.deterministic,
            seed=result.seed,
            profile=result.profile,
            photonic_tile_k=result.photonic_tile_k,
        ).validate()
        return result

    def contract(self) -> NumericalContract:
        return NumericalContract(
            max_abs_error=self.max_abs_error,
            max_rel_error=self.max_rel_error,
            minimum_effective_bits=self.minimum_effective_bits,
        )

    def gemm_seed(self, gemm_index: int) -> int:
        """Noise seed passed to lowered GEMM ``gemm_index`` under this seed policy."""
        return derive_gemm_seed(self.seed, gemm_index) if self.seed_policy == "per_gemm" else self.seed

    def execution(self) -> ExecutionOptions:
        return ExecutionOptions(
            target=self.target,
            allow_fallback=self.allow_fallback,
            deterministic=self.deterministic,
            seed=self.seed,
            profile=self.profile,
            photonic_noise=self.photonic_noise,
            photonic_tile_k=self.photonic_tile_k,
        )


@dataclass(frozen=True)
class TorchDiagnostic:
    code: str
    node: str
    target: str
    message: str
    action: str


@dataclass(frozen=True)
class TorchRegion:
    region_id: str
    nodes: Tuple[str, ...]
    operations: Tuple[str, ...]


@dataclass(frozen=True)
class TorchCompileReport:
    version: str
    graph_name: str
    torch_version: str
    supported_torch_range: str
    shape_guard_owner: str
    input_contracts: Tuple[Mapping[str, Any], ...]
    supported_nodes: Tuple[str, ...]
    fallback_nodes: Tuple[str, ...]
    regions: Tuple[TorchRegion, ...]
    diagnostics: Tuple[TorchDiagnostic, ...]
    options: TorchBackendOptions
    fingerprint: str

    def to_dict(self) -> Dict[str, Any]:
        value = asdict(self)
        value["regions"] = [asdict(region) for region in self.regions]
        value["diagnostics"] = [asdict(diagnostic) for diagnostic in self.diagnostics]
        return value

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), sort_keys=True, separators=(",", ":"))


_OPTIONS: ContextVar[TorchBackendOptions] = ContextVar(
    "awen_torch_options", default=TorchBackendOptions()
)
_EXECUTION_TRACES: ContextVar[Tuple[Any, ...]] = ContextVar("awen_torch_traces", default=())
_LAST_COMPILE_REPORT: Optional[TorchCompileReport] = None


def awen_backend(graph_module: Any, example_inputs: Sequence[Any], **kwargs: Any):
    """Compile an FX graph into mixed AWEN-supported and eager fallback regions.

    This function implements PyTorch's documented custom-backend contract:
    ``(GraphModule, List[Tensor]) -> Callable``. TorchDynamo remains responsible
    for capture, graph breaks, guards, and recompilation for dynamic dimensions.
    """

    import torch

    _require_supported_torch(torch.__version__)
    raw_options = kwargs.get("options")
    if raw_options is None:
        raw_options = {
            key: value
            for key, value in kwargs.items()
            if key in TorchBackendOptions.__dataclass_fields__
        }
    options = TorchBackendOptions.from_mapping(raw_options)
    supported = []
    fallback = []
    diagnostics = []
    node_operations = {}

    for node in graph_module.graph.nodes:
        operation = _classify_node(graph_module, node)
        if operation is not None:
            _rewrite_node(graph_module, node, operation, len(supported))
            supported.append(node.name)
            node_operations[node.name] = operation
        elif node.op in {"call_function", "call_method", "call_module"}:
            target = _target_name(node.target)
            fallback.append(node.name)
            diagnostics.append(
                TorchDiagnostic(
                    code="unsupported_torch_operation",
                    node=node.name,
                    target=target,
                    message=(
                        f"node '{node.name}' target '{target}' remains an eager PyTorch operation; "
                        "AWEN v1 lowers matmul/mm/bmm and linear"
                    ),
                    action="framework_fallback",
                )
            )

    if fallback and not options.allow_fallback:
        details = "; ".join(diagnostic.message for diagnostic in diagnostics)
        raise UnsupportedGraphError(f"AWEN fallback is disabled: {details}")
    if not supported and not options.allow_fallback:
        raise UnsupportedGraphError("the FX graph contains no AWEN-supported operation")

    graph_module.graph.lint()
    graph_module.recompile()
    regions = _regions(graph_module.graph.nodes, node_operations)
    report_payload = {
        "version": TORCH_BACKEND_VERSION,
        "graph_name": graph_module.__class__.__name__,
        "torch_version": torch.__version__.split("+", 1)[0],
        "supported_torch_range": SUPPORTED_TORCH_RANGE,
        "shape_guard_owner": "torch_dynamo",
        "input_contracts": [_input_contract(value) for value in example_inputs],
        "supported_nodes": supported,
        "fallback_nodes": fallback,
        "regions": [asdict(region) for region in regions],
        "diagnostics": [asdict(diagnostic) for diagnostic in diagnostics],
        "options": asdict(options),
    }
    report = TorchCompileReport(
        version=TORCH_BACKEND_VERSION,
        graph_name=graph_module.__class__.__name__,
        torch_version=torch.__version__.split("+", 1)[0],
        supported_torch_range=SUPPORTED_TORCH_RANGE,
        shape_guard_owner="torch_dynamo",
        input_contracts=tuple(report_payload["input_contracts"]),
        supported_nodes=tuple(supported),
        fallback_nodes=tuple(fallback),
        regions=regions,
        diagnostics=tuple(diagnostics),
        options=options,
        fingerprint=_fingerprint(report_payload),
    )
    global _LAST_COMPILE_REPORT
    _LAST_COMPILE_REPORT = report

    def compiled(*args: Any, **call_kwargs: Any):
        option_token = _OPTIONS.set(options)
        trace_token = _EXECUTION_TRACES.set(())
        try:
            return graph_module.forward(*args, **call_kwargs)
        finally:
            _OPTIONS.reset(option_token)
            # Keep the completed immutable trace tuple visible to this context.
            completed = _EXECUTION_TRACES.get()
            _EXECUTION_TRACES.reset(trace_token)
            _EXECUTION_TRACES.set(completed)

    compiled.awen_report = report
    compiled.awen_graph_module = graph_module
    return compiled


def get_last_compile_report() -> Optional[TorchCompileReport]:
    return _LAST_COMPILE_REPORT


def get_last_execution_traces() -> Tuple[Any, ...]:
    return _EXECUTION_TRACES.get()


def _awen_matmul(lhs: Any, rhs: Any, *, awen_gemm_index: int = 0):
    rank = max(lhs.dim(), rhs.dim())
    operation = "batched_gemm" if rank > 2 else "gemm"
    if lhs.is_complex() or rhs.is_complex():
        operation = "complex_gemm"
    return _execute_torch(operation, lhs, rhs, gemm_index=awen_gemm_index)


def _awen_linear(inputs: Any, weight: Any, bias: Optional[Any] = None, *, awen_gemm_index: int = 0):
    # torch.nn.functional.linear stores [out_features, in_features].
    transposed = weight.transpose(-2, -1)
    operands = (inputs, transposed) if bias is None else (inputs, transposed, bias)
    return _execute_torch("linear", *operands, gemm_index=awen_gemm_index)


def _execute_torch(operation: str, *inputs: Any, gemm_index: int = 0):
    import torch

    options = _OPTIONS.get()
    execution = options.execution()
    if options.seed_policy != "shared":
        execution = replace(execution, seed=options.gemm_seed(gemm_index))
    context = torch.profiler.record_function(f"awen::{operation}") if options.profile else nullcontext()
    with context:
        result = get_runtime().execute_with_trace(
            operation,
            *inputs,
            contract=options.contract(),
            options=execution,
        )
    _EXECUTION_TRACES.set(_EXECUTION_TRACES.get() + (result.trace,))
    return result.outputs


def _classify_node(graph_module: Any, node: Any) -> Optional[str]:
    if node.op == "call_method" and str(node.target) in {"matmul", "mm", "bmm", "__matmul__"}:
        return "matmul"
    if node.op == "call_module":
        module = graph_module.get_submodule(str(node.target))
        if module.__class__.__name__ == "Linear":
            return "linear_module"
        return None
    if node.op != "call_function":
        return None
    name = _target_name(node.target)
    leaf_name = getattr(node.target, "__name__", "").lower()
    if node.target is operator.matmul or leaf_name in {"matmul", "mm", "bmm"} or any(
        token in name for token in ("aten.matmul", "aten.mm", "aten.bmm")
    ):
        return "matmul"
    if leaf_name == "linear" or any(
        token in name for token in ("torch._c._nn.linear", "functional.linear", "aten.linear")
    ):
        return "linear"
    return None


def _rewrite_node(graph_module: Any, node: Any, operation: str, gemm_index: int) -> None:
    if operation == "linear_module":
        target = str(node.target)
        with graph_module.graph.inserting_before(node):
            weight = graph_module.graph.get_attr(f"{target}.weight")
            module = graph_module.get_submodule(target)
            bias = graph_module.graph.get_attr(f"{target}.bias") if module.bias is not None else None
        node.op = "call_function"
        node.target = _awen_linear
        node.args = (node.args[0], weight, bias)
        node.kwargs = {}
    elif operation == "linear":
        node.op = "call_function"
        node.target = _awen_linear
    else:
        node.op = "call_function"
        node.target = _awen_matmul
    # The graph-order index of this GEMM; it selects the noise seed under seed_policy="per_gemm".
    node.kwargs = {**node.kwargs, _GEMM_INDEX_KWARG: gemm_index}
    node.meta["awen.gemm_index"] = gemm_index
    node.meta["awen.supported"] = True
    node.meta["awen.operation"] = "linear" if operation.startswith("linear") else "gemm"


def _regions(nodes: Iterable[Any], operations: Mapping[str, str]) -> Tuple[TorchRegion, ...]:
    result = []
    current_nodes = []
    current_operations = []
    for node in nodes:
        operation = operations.get(node.name)
        if operation is None:
            if current_nodes:
                result.append(
                    TorchRegion(
                        region_id=f"awen.region.{len(result)}",
                        nodes=tuple(current_nodes),
                        operations=tuple(current_operations),
                    )
                )
                current_nodes = []
                current_operations = []
            continue
        current_nodes.append(node.name)
        current_operations.append("linear" if operation.startswith("linear") else "gemm")
    if current_nodes:
        result.append(
            TorchRegion(
                region_id=f"awen.region.{len(result)}",
                nodes=tuple(current_nodes),
                operations=tuple(current_operations),
            )
        )
    return tuple(result)


def _input_contract(value: Any) -> Mapping[str, Any]:
    if not hasattr(value, "dim"):
        return {
            "kind": "symbolic_scalar",
            "type": type(value).__name__,
            "value": str(value),
            "dynamic_dimensions": "guarded_by_torch_dynamo",
        }
    stride = tuple(str(item) for item in value.stride()) if hasattr(value, "stride") else ()
    return {
        "kind": "tensor",
        "rank": int(value.dim()),
        "dtype": str(value.dtype).replace("torch.", ""),
        "device": str(value.device),
        "layout": "contiguous" if value.is_contiguous() else "strided",
        "stride": stride,
        "dynamic_dimensions": "guarded_by_torch_dynamo",
    }


def _target_name(target: Any) -> str:
    module = getattr(target, "__module__", "")
    name = getattr(target, "__qualname__", getattr(target, "__name__", str(target)))
    return f"{module}.{name}".lower().strip(".")


def _require_supported_torch(version: str) -> None:
    numeric = version.split("+", 1)[0].split(".")
    try:
        major, minor = int(numeric[0]), int(numeric[1])
    except (ValueError, IndexError) as error:
        raise ContractError(f"cannot parse PyTorch version '{version}'") from error
    if major != 2 or not _SUPPORTED_TORCH_MINORS[0] <= minor < _SUPPORTED_TORCH_MINORS[1]:
        raise ContractError(
            f"PyTorch {version} is outside the tested range {SUPPORTED_TORCH_RANGE}"
        )


def _fingerprint(value: Mapping[str, Any]) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"), default=str).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()
