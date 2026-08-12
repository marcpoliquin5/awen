"""Stable in-process tensor runtime used by every AWEN framework frontend.

The v1 normal path accepts live NumPy, PyTorch, or JAX values and returns live
values from the same framework. It never writes a tensor to disk and never
launches ``awenctl``. The implementation is a semantic reference and dispatch
boundary; a GPU or photonic hardware plugin can implement the same contract.
"""

from __future__ import annotations

from concurrent.futures import Future, ThreadPoolExecutor
from contextlib import nullcontext
from dataclasses import asdict, dataclass, field
from enum import Enum
import hashlib
import json
import math
import threading
import time
from typing import Any, Dict, Mapping, Optional, Sequence, Tuple

from .errors import ContractError, DeviceError, ExecutionError, SerializationError


FRAMEWORK_RUNTIME_VERSION = "awen.framework-runtime.v1"
FRAMEWORK_PLAN_VERSION = "awen.framework-plan.v1"
FRAMEWORK_TRACE_VERSION = "awen.framework-trace.v1"
SUPPORTED_OPERATIONS = frozenset(
    {
        "gemm",
        "batched_gemm",
        "complex_gemm",
        "linear",
        "attention_scores",
        "attention_value",
        "mlp_projection",
        "fft",
        "ifft",
    }
)


class BufferOwner(str, Enum):
    BORROWED = "borrowed"
    RUNTIME = "runtime"
    FRAMEWORK = "framework"


@dataclass(frozen=True)
class NumericalContract:
    max_abs_error: float = 1.0e-5
    max_rel_error: float = 1.0e-5
    minimum_effective_bits: Optional[int] = None

    def validate(self) -> None:
        if (
            not math.isfinite(self.max_abs_error)
            or self.max_abs_error < 0
            or not math.isfinite(self.max_rel_error)
            or self.max_rel_error < 0
            or self.minimum_effective_bits is not None
            and self.minimum_effective_bits <= 0
        ):
            raise ContractError(
                "numerical tolerances must be non-negative and effective bits must be positive"
            )


@dataclass(frozen=True)
class Device:
    kind: str
    index: Optional[int] = None

    @classmethod
    def parse(cls, value: str) -> "Device":
        parts = value.split(":", 1)
        kind = parts[0].lower()
        if kind not in {"cpu", "cuda", "mps", "tpu", "photonic"}:
            raise DeviceError("device must be cpu, cuda, mps, tpu, or photonic")
        index = None
        if len(parts) == 2:
            try:
                index = int(parts[1])
            except ValueError as error:
                raise DeviceError("device index must be an integer") from error
            if index < 0:
                raise DeviceError("device index must be non-negative")
        return cls(kind, index)

    def __str__(self) -> str:
        return self.kind if self.index is None else f"{self.kind}:{self.index}"


@dataclass(frozen=True)
class Stream:
    device: Device
    identifier: str = "default"
    native: Any = field(default=None, repr=False, compare=False)


@dataclass(frozen=True)
class ExecutionOptions:
    target: str = "auto"
    stream: Optional[Stream] = None
    asynchronous: bool = False
    deterministic: bool = True
    seed: int = 0
    profile: bool = True
    allow_fallback: bool = True

    def validate(self) -> None:
        if self.target not in {"auto", "cpu", "gpu", "photonic", "framework"}:
            raise ContractError("target must be auto, cpu, gpu, photonic, or framework")
        if self.seed < 0:
            raise ContractError("deterministic replay seed must be non-negative")


@dataclass(frozen=True)
class Diagnostic:
    code: str
    message: str
    node: Optional[str] = None
    action: str = "fallback"


@dataclass(frozen=True)
class ProfileEvent:
    operation_id: str
    operation: str
    target: str
    framework: str
    device: str
    stream: str
    input_shapes: Tuple[Tuple[int, ...], ...]
    output_shapes: Tuple[Tuple[int, ...], ...]
    started_ns: int
    duration_ns: int
    asynchronous: bool


@dataclass(frozen=True)
class OperationPlan:
    version: str
    operation_id: str
    operation: str
    target: str
    framework: str
    input_shapes: Tuple[Tuple[int, ...], ...]
    input_dtypes: Tuple[str, ...]
    attributes: Mapping[str, Any]
    contract: NumericalContract
    seed: int
    fingerprint: str = ""

    def __post_init__(self) -> None:
        if self.version != FRAMEWORK_PLAN_VERSION:
            raise ContractError(f"expected {FRAMEWORK_PLAN_VERSION}, got {self.version}")
        if not self.operation_id or self.operation not in SUPPORTED_OPERATIONS:
            raise ContractError("operation plan has an invalid id or unsupported operation")
        self.contract.validate()
        expected = _fingerprint(self._fingerprint_payload())
        if self.fingerprint and self.fingerprint != expected:
            raise SerializationError("framework plan fingerprint does not match its contents")
        if not self.fingerprint:
            object.__setattr__(self, "fingerprint", expected)

    def _fingerprint_payload(self) -> Dict[str, Any]:
        return {
            "version": self.version,
            "operation_id": self.operation_id,
            "operation": self.operation,
            "target": self.target,
            "framework": self.framework,
            "input_shapes": self.input_shapes,
            "input_dtypes": self.input_dtypes,
            "attributes": dict(self.attributes),
            "contract": asdict(self.contract),
            "seed": self.seed,
        }

    def to_dict(self) -> Dict[str, Any]:
        value = self._fingerprint_payload()
        value["fingerprint"] = self.fingerprint
        return value

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), sort_keys=True, separators=(",", ":"))

    @classmethod
    def from_json(cls, value: str) -> "OperationPlan":
        try:
            data = json.loads(value)
            expected = {
                "version",
                "operation_id",
                "operation",
                "target",
                "framework",
                "input_shapes",
                "input_dtypes",
                "attributes",
                "contract",
                "seed",
                "fingerprint",
            }
            if set(data) != expected:
                raise SerializationError("framework plan fields do not match v1")
            return cls(
                version=data["version"],
                operation_id=data["operation_id"],
                operation=data["operation"],
                target=data["target"],
                framework=data["framework"],
                input_shapes=tuple(tuple(shape) for shape in data["input_shapes"]),
                input_dtypes=tuple(data["input_dtypes"]),
                attributes=data["attributes"],
                contract=NumericalContract(**data["contract"]),
                seed=data["seed"],
                fingerprint=data["fingerprint"],
            )
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
            raise SerializationError("invalid serialized AWEN framework plan") from error


@dataclass(frozen=True)
class ExecutionTrace:
    version: str
    plan_fingerprint: str
    events: Tuple[ProfileEvent, ...]
    diagnostics: Tuple[Diagnostic, ...]
    replay_fingerprint: str

    def to_dict(self) -> Dict[str, Any]:
        return {
            "version": self.version,
            "plan_fingerprint": self.plan_fingerprint,
            "events": [asdict(event) for event in self.events],
            "diagnostics": [asdict(diagnostic) for diagnostic in self.diagnostics],
            "replay_fingerprint": self.replay_fingerprint,
        }

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), sort_keys=True, separators=(",", ":"))

    @classmethod
    def from_json(cls, value: str) -> "ExecutionTrace":
        try:
            data = json.loads(value)
            expected = {
                "version",
                "plan_fingerprint",
                "events",
                "diagnostics",
                "replay_fingerprint",
            }
            if set(data) != expected or data["version"] != FRAMEWORK_TRACE_VERSION:
                raise SerializationError("framework trace fields or version do not match v1")
            _validate_sha256(data["plan_fingerprint"], "plan fingerprint")
            _validate_sha256(data["replay_fingerprint"], "replay fingerprint")
            event_fields = set(ProfileEvent.__dataclass_fields__)
            events = []
            for event in data["events"]:
                if set(event) != event_fields:
                    raise SerializationError("framework trace event fields do not match v1")
                event = dict(event)
                event["input_shapes"] = tuple(tuple(shape) for shape in event["input_shapes"])
                event["output_shapes"] = tuple(tuple(shape) for shape in event["output_shapes"])
                events.append(ProfileEvent(**event))
            diagnostic_fields = set(Diagnostic.__dataclass_fields__)
            diagnostics = []
            for diagnostic in data["diagnostics"]:
                if set(diagnostic) != diagnostic_fields:
                    raise SerializationError("framework trace diagnostic fields do not match v1")
                diagnostics.append(Diagnostic(**diagnostic))
            return cls(
                version=data["version"],
                plan_fingerprint=data["plan_fingerprint"],
                events=tuple(events),
                diagnostics=tuple(diagnostics),
                replay_fingerprint=data["replay_fingerprint"],
            )
        except (KeyError, TypeError, ValueError, json.JSONDecodeError) as error:
            raise SerializationError("invalid serialized AWEN framework trace") from error


@dataclass(frozen=True)
class ExecutionResult:
    outputs: Any
    trace: ExecutionTrace
    plan: OperationPlan


@dataclass(frozen=True)
class Buffer:
    value: Any
    owner: BufferOwner
    framework: str
    device: Device
    shape: Tuple[int, ...]
    strides: Tuple[int, ...]
    dtype: str

    @classmethod
    def wrap(cls, value: Any, owner: BufferOwner = BufferOwner.BORROWED) -> "Buffer":
        framework = _framework(value)
        shape = _shape(value)
        dtype = _dtype(value)
        device = _device(value, framework)
        strides = _strides(value, framework, shape)
        return cls(value, owner, framework, device, shape, strides, dtype)


class AwenFuture:
    """Framework-neutral future whose wait operation returns ExecutionResult."""

    def __init__(self, future: Future):
        self._future = future

    def done(self) -> bool:
        return self._future.done()

    def wait(self, timeout: Optional[float] = None) -> ExecutionResult:
        return self._future.result(timeout=timeout)

    result = wait


class InProcessRuntime:
    """Reference implementation of ``awen.framework-runtime.v1``."""

    abi_version = FRAMEWORK_RUNTIME_VERSION

    def __init__(self, max_workers: int = 4):
        if max_workers <= 0:
            raise ContractError("max_workers must be positive")
        self._executor = ThreadPoolExecutor(max_workers=max_workers, thread_name_prefix="awen")
        self._lock = threading.Lock()
        self._last_trace: Optional[ExecutionTrace] = None

    def close(self, wait: bool = True) -> None:
        """Release asynchronous workers; submitted work may finish when requested."""

        self._executor.shutdown(wait=wait, cancel_futures=not wait)

    def __enter__(self) -> "InProcessRuntime":
        return self

    def __exit__(self, exc_type: Any, exc_value: Any, traceback: Any) -> None:
        self.close()

    @property
    def last_trace(self) -> Optional[ExecutionTrace]:
        with self._lock:
            return self._last_trace

    def transfer(self, value: Any, device: str, non_blocking: bool = True) -> Any:
        target = Device.parse(device)
        framework = _framework(value)
        try:
            if framework == "torch":
                return value.to(str(target), non_blocking=non_blocking)
            if framework == "jax":
                import jax

                candidates = [candidate for candidate in jax.devices() if candidate.platform == target.kind]
                if target.index is not None:
                    candidates = [candidate for candidate in candidates if candidate.id == target.index]
                if not candidates:
                    raise DeviceError(f"JAX has no device matching {target}")
                return jax.device_put(value, candidates[0])
            if framework == "numpy" and target.kind == "cpu":
                return value
        except DeviceError:
            raise
        except Exception as error:
            raise DeviceError(f"failed to transfer {framework} value to {target}") from error
        raise DeviceError(f"{framework} values cannot be transferred to {target}")

    def plan(
        self,
        operation: str,
        inputs: Sequence[Any],
        *,
        attributes: Optional[Mapping[str, Any]] = None,
        contract: NumericalContract = NumericalContract(),
        options: ExecutionOptions = ExecutionOptions(),
    ) -> OperationPlan:
        options.validate()
        contract.validate()
        if operation not in SUPPORTED_OPERATIONS:
            raise ContractError(f"unsupported in-process operation '{operation}'")
        buffers = tuple(Buffer.wrap(value) for value in inputs)
        if not buffers:
            raise ContractError("an operation requires at least one input buffer")
        frameworks = {buffer.framework for buffer in buffers}
        if len(frameworks) != 1:
            raise ContractError("all inputs must belong to the same framework")
        target = _selected_target(options.target, buffers[0].device)
        operation_identity = {
            "operation": operation,
            "target": target,
            "framework": buffers[0].framework,
            "input_shapes": tuple(buffer.shape for buffer in buffers),
            "input_dtypes": tuple(buffer.dtype for buffer in buffers),
            "attributes": dict(attributes or {}),
            "contract": asdict(contract),
            "seed": options.seed,
        }
        operation_id = f"{operation}.{_fingerprint(operation_identity)[7:19]}"
        return OperationPlan(
            version=FRAMEWORK_PLAN_VERSION,
            operation_id=operation_id,
            operation=operation,
            target=target,
            framework=buffers[0].framework,
            input_shapes=tuple(buffer.shape for buffer in buffers),
            input_dtypes=tuple(buffer.dtype for buffer in buffers),
            attributes=dict(attributes or {}),
            contract=contract,
            seed=options.seed,
        )

    def execute(
        self,
        operation: str,
        *inputs: Any,
        attributes: Optional[Mapping[str, Any]] = None,
        contract: NumericalContract = NumericalContract(),
        options: ExecutionOptions = ExecutionOptions(),
    ) -> Any:
        if options.asynchronous:
            return self.submit(
                operation,
                *inputs,
                attributes=attributes,
                contract=contract,
                options=options,
            )
        return self.execute_with_trace(
            operation,
            *inputs,
            attributes=attributes,
            contract=contract,
            options=options,
        ).outputs

    def submit(
        self,
        operation: str,
        *inputs: Any,
        attributes: Optional[Mapping[str, Any]] = None,
        contract: NumericalContract = NumericalContract(),
        options: ExecutionOptions = ExecutionOptions(asynchronous=True),
    ) -> AwenFuture:
        options.validate()
        synchronous = ExecutionOptions(
            target=options.target,
            stream=options.stream,
            asynchronous=False,
            deterministic=options.deterministic,
            seed=options.seed,
            profile=options.profile,
            allow_fallback=options.allow_fallback,
        )
        future = self._executor.submit(
            self.execute_with_trace,
            operation,
            *inputs,
            attributes=attributes,
            contract=contract,
            options=synchronous,
            _reported_asynchronous=True,
        )
        return AwenFuture(future)

    def execute_with_trace(
        self,
        operation: str,
        *inputs: Any,
        attributes: Optional[Mapping[str, Any]] = None,
        contract: NumericalContract = NumericalContract(),
        options: ExecutionOptions = ExecutionOptions(),
        _reported_asynchronous: bool = False,
    ) -> ExecutionResult:
        plan = self.plan(
            operation,
            inputs,
            attributes=attributes,
            contract=contract,
            options=options,
        )
        buffers = tuple(Buffer.wrap(value) for value in inputs)
        if options.stream is not None and options.stream.device != buffers[0].device:
            raise DeviceError(
                f"stream device {options.stream.device} does not match input device {buffers[0].device}"
            )
        if (
            options.stream is not None
            and options.stream.native is not None
            and buffers[0].framework != "torch"
        ):
            raise DeviceError("native stream objects are supported only for PyTorch in v1")
        _validate_operation(plan, buffers)
        started = time.perf_counter_ns()
        try:
            with _stream_context(buffers[0].framework, options.stream):
                output = _dispatch(plan.operation, tuple(buffer.value for buffer in buffers), plan.attributes)
        except Exception as error:
            if isinstance(error, (ContractError, DeviceError, ExecutionError)):
                raise
            shapes = ", ".join(str(buffer.shape) for buffer in buffers)
            raise ExecutionError(
                f"{plan.operation_id} failed for {plan.framework} inputs {shapes}: {error}"
            ) from error
        duration = time.perf_counter_ns() - started
        outputs = output if isinstance(output, tuple) else (output,)
        output_buffers = tuple(Buffer.wrap(value, BufferOwner.FRAMEWORK) for value in outputs)
        stream_id = options.stream.identifier if options.stream else "default"
        event = ProfileEvent(
            operation_id=plan.operation_id,
            operation=plan.operation,
            target=plan.target,
            framework=plan.framework,
            device=str(output_buffers[0].device),
            stream=stream_id,
            input_shapes=plan.input_shapes,
            output_shapes=tuple(buffer.shape for buffer in output_buffers),
            started_ns=started,
            duration_ns=duration,
            asynchronous=_reported_asynchronous,
        )
        replay_fingerprint = _fingerprint(
            {
                "plan": plan.fingerprint,
                "outputs": [
                    {"shape": buffer.shape, "dtype": buffer.dtype, "device": str(buffer.device)}
                    for buffer in output_buffers
                ],
            }
        )
        trace = ExecutionTrace(
            version=FRAMEWORK_TRACE_VERSION,
            plan_fingerprint=plan.fingerprint,
            events=(event,) if options.profile else (),
            diagnostics=(),
            replay_fingerprint=replay_fingerprint,
        )
        with self._lock:
            self._last_trace = trace
        return ExecutionResult(output, trace, plan)

    def replay(self, plan: OperationPlan, *inputs: Any) -> ExecutionResult:
        buffers = tuple(Buffer.wrap(value) for value in inputs)
        if tuple(buffer.shape for buffer in buffers) != plan.input_shapes:
            raise ContractError("replay input shapes do not match the serialized plan")
        if tuple(buffer.dtype for buffer in buffers) != plan.input_dtypes:
            raise ContractError("replay input dtypes do not match the serialized plan")
        if any(buffer.framework != plan.framework for buffer in buffers):
            raise ContractError("replay input framework does not match the serialized plan")
        return self.execute_with_trace(
            plan.operation,
            *inputs,
            attributes=plan.attributes,
            contract=plan.contract,
            options=ExecutionOptions(target=_requested_target(plan.target), seed=plan.seed),
        )


def debug_finite_difference(
    function: Any,
    inputs: Sequence[Any],
    *,
    epsilon: float = 1.0e-4,
    enabled: bool = False,
) -> Tuple[Any, ...]:
    """Explicit NumPy-only finite-difference diagnostic; never used by normal execution."""

    if not enabled:
        raise ContractError("finite differences are disabled; pass enabled=True for debug use")
    if not math.isfinite(epsilon) or epsilon <= 0:
        raise ContractError("finite-difference epsilon must be positive and finite")
    import numpy as np

    arrays = [np.asarray(value, dtype=float).copy() for value in inputs]
    gradients = []
    for input_index, value in enumerate(arrays):
        gradient = np.zeros_like(value)
        for index in np.ndindex(value.shape):
            positive = [candidate.copy() for candidate in arrays]
            negative = [candidate.copy() for candidate in arrays]
            positive[input_index][index] += epsilon
            negative[input_index][index] -= epsilon
            positive_value = np.asarray(function(*positive)).sum()
            negative_value = np.asarray(function(*negative)).sum()
            gradient[index] = (positive_value - negative_value) / (2 * epsilon)
        gradients.append(gradient)
    return tuple(gradients)


def _validate_operation(plan: OperationPlan, buffers: Sequence[Buffer]) -> None:
    minimum_bits = plan.contract.minimum_effective_bits
    if minimum_bits is not None and any(_dtype_bits(buffer.dtype) < minimum_bits for buffer in buffers):
        raise ContractError("input dtype cannot satisfy the minimum effective-bit contract")
    ranks = [len(buffer.shape) for buffer in buffers]
    if plan.operation in {"gemm", "complex_gemm", "linear", "mlp_projection"}:
        if len(buffers) not in ({2, 3} if plan.operation in {"linear", "mlp_projection"} else {2}):
            suffix = " and optional bias" if plan.operation in {"linear", "mlp_projection"} else ""
            raise ContractError(f"{plan.operation} requires two inputs{suffix}")
        if ranks[0] < 2 or ranks[1] != 2:
            raise ContractError(f"{plan.operation} requires matrix operands")
    elif plan.operation in {"batched_gemm", "attention_value"}:
        if len(buffers) != 2 or ranks[0] < 2 or ranks[1] < 2:
            raise ContractError(f"{plan.operation} requires two rank-two-or-higher operands")
    elif plan.operation == "attention_scores":
        if len(buffers) != 2 or ranks[0] < 2 or ranks[1] < 2:
            raise ContractError("attention_scores requires Q and K matrices")
    elif plan.operation in {"fft", "ifft"} and len(buffers) != 1:
        raise ContractError(f"{plan.operation} requires one input")


def _dispatch(operation: str, inputs: Tuple[Any, ...], attributes: Mapping[str, Any]) -> Any:
    if operation in {"gemm", "batched_gemm", "complex_gemm", "attention_value"}:
        return inputs[0] @ inputs[1]
    if operation in {"linear", "mlp_projection"}:
        output = inputs[0] @ inputs[1]
        return output + inputs[2] if len(inputs) == 3 and inputs[2] is not None else output
    if operation == "attention_scores":
        scale = attributes.get("scale", 1.0)
        return (inputs[0] @ _transpose_last_two(inputs[1])) * scale
    if operation in {"fft", "ifft"}:
        return _fft(inputs[0], inverse=operation == "ifft")
    raise ContractError(f"operation '{operation}' has no in-process implementation")


def _transpose_last_two(value: Any) -> Any:
    framework = _framework(value)
    if framework == "torch":
        return value.transpose(-2, -1)
    if framework == "jax":
        import jax.numpy as jnp

        return jnp.swapaxes(value, -2, -1)
    import numpy as np

    return np.swapaxes(value, -2, -1)


def _fft(value: Any, inverse: bool) -> Any:
    framework = _framework(value)
    if framework == "torch":
        import torch

        return torch.fft.ifft(value) if inverse else torch.fft.fft(value)
    if framework == "jax":
        import jax.numpy as jnp

        return jnp.fft.ifft(value) if inverse else jnp.fft.fft(value)
    import numpy as np

    return np.fft.ifft(value) if inverse else np.fft.fft(value)


def _stream_context(framework: str, stream: Optional[Stream]):
    if stream is None or stream.native is None:
        return nullcontext()
    if framework == "torch":
        import torch

        return torch.cuda.stream(stream.native)
    return nullcontext()


def _selected_target(requested: str, device: Device) -> str:
    if requested in {"auto", "framework"}:
        return f"framework:{device}"
    if requested == "gpu":
        if device.kind not in {"cuda", "mps"}:
            raise DeviceError("gpu execution requires GPU-resident framework tensors")
        return f"framework:{device}"
    if requested == "cpu":
        if device.kind != "cpu":
            raise DeviceError("cpu execution requires CPU-resident framework tensors")
        return "framework:cpu"
    return "simulated:photonic"


def _requested_target(selected: str) -> str:
    if selected == "simulated:photonic":
        return "photonic"
    if selected.startswith("framework:cuda") or selected.startswith("framework:mps"):
        return "framework"
    return "framework"


def _dtype_bits(dtype: str) -> int:
    normalized = dtype.lower()
    for name, bits in (
        ("complex128", 53),
        ("complex64", 24),
        ("float64", 53),
        ("float32", 24),
        ("bfloat16", 8),
        ("float16", 11),
        ("int64", 64),
        ("int32", 32),
        ("int16", 16),
        ("int8", 8),
    ):
        if name in normalized:
            return bits
    raise ContractError(f"unsupported framework dtype '{dtype}'")


def _framework(value: Any) -> str:
    module = type(value).__module__.split(".", 1)[0]
    if module == "torch":
        return "torch"
    if module in {"jax", "jaxlib"} or hasattr(value, "__jax_array__"):
        return "jax"
    if module == "numpy" or hasattr(value, "__array_interface__"):
        return "numpy"
    raise ContractError(f"unsupported tensor type {type(value).__name__}")


def _shape(value: Any) -> Tuple[int, ...]:
    try:
        return tuple(int(dimension) for dimension in value.shape)
    except (AttributeError, TypeError, ValueError) as error:
        raise ContractError("tensor shape must be concrete at execution") from error


def _dtype(value: Any) -> str:
    dtype = getattr(value, "dtype", None)
    if dtype is None:
        raise ContractError("tensor dtype is unavailable")
    return str(dtype).replace("torch.", "")


def _device(value: Any, framework: str) -> Device:
    if framework == "numpy":
        return Device("cpu")
    if framework == "torch":
        return Device.parse(str(value.device))
    candidate = getattr(value, "device", None)
    candidate = candidate() if callable(candidate) else candidate
    platform = getattr(candidate, "platform", "cpu")
    index = getattr(candidate, "id", None)
    return Device(platform, index)


def _strides(value: Any, framework: str, shape: Tuple[int, ...]) -> Tuple[int, ...]:
    if framework == "torch":
        return tuple(int(stride) for stride in value.stride())
    strides = getattr(value, "strides", None)
    if strides is None:
        stride = 1
        result = []
        for dimension in reversed(shape):
            result.append(stride)
            stride *= dimension
        return tuple(reversed(result))
    itemsize = int(getattr(getattr(value, "dtype", None), "itemsize", 1))
    return tuple(int(stride) // itemsize for stride in strides)


def _fingerprint(value: Mapping[str, Any]) -> str:
    payload = json.dumps(value, sort_keys=True, separators=(",", ":"), default=str).encode()
    return "sha256:" + hashlib.sha256(payload).hexdigest()


def _validate_sha256(value: str, name: str) -> None:
    prefix = "sha256:"
    digest = value[len(prefix) :] if isinstance(value, str) and value.startswith(prefix) else ""
    if len(digest) != 64 or any(character not in "0123456789abcdef" for character in digest):
        raise SerializationError(f"framework trace {name} is not a SHA-256 fingerprint")


_DEFAULT_RUNTIME = InProcessRuntime()


def get_runtime() -> InProcessRuntime:
    return _DEFAULT_RUNTIME
