"""JAX export/import through JAX's portable StableHLO API."""

from __future__ import annotations

from dataclasses import asdict, dataclass
import hashlib
import json
import re
from typing import Any, Dict, Optional, Sequence, Tuple

from .errors import ContractError, SerializationError, UnsupportedGraphError
from .runtime import NumericalContract


JAX_INTEGRATION_VERSION = "awen.jax-stablehlo.v1"
SUPPORTED_JAX_RANGE = ">=0.9,<0.12"
_STABLEHLO_OPERATION = re.compile(r"\bstablehlo\.([a-zA-Z0-9_]+)\b")
_AWEN_SUPPORTED_STABLEHLO = frozenset(
    {
        "add",
        "broadcast_in_dim",
        "constant",
        "convert",
        "dot_general",
        "reshape",
        "return",
        "transpose",
    }
)


@dataclass(frozen=True)
class JaxDiagnostic:
    code: str
    operation: str
    message: str
    action: str


@dataclass(frozen=True)
class JaxImportReport:
    version: str
    jax_version: str
    supported_jax_range: str
    calling_convention_version: int
    platforms: Tuple[str, ...]
    input_avals: Tuple[str, ...]
    output_avals: Tuple[str, ...]
    supported_operations: Tuple[str, ...]
    fallback_operations: Tuple[str, ...]
    diagnostics: Tuple[JaxDiagnostic, ...]
    contract: NumericalContract
    portable_fingerprint: str

    def to_dict(self) -> Dict[str, Any]:
        value = asdict(self)
        value["diagnostics"] = [asdict(item) for item in self.diagnostics]
        return value

    def to_json(self) -> str:
        return json.dumps(self.to_dict(), sort_keys=True, separators=(",", ":"))


class JaxExecutable:
    """Serializable StableHLO program with JAX's compatibility guarantees."""

    def __init__(self, exported: Any, portable: bytes, report: JaxImportReport):
        self._exported = exported
        self._portable = bytes(portable)
        self.report = report

    @property
    def stablehlo_text(self) -> str:
        return self._exported.mlir_module()

    @property
    def portable_bytes(self) -> bytes:
        return self._portable

    def __call__(self, *args: Any, **kwargs: Any) -> Any:
        try:
            return self._exported.call(*args, **kwargs)
        except Exception as error:
            raise ContractError(f"JAX StableHLO execution failed: {error}") from error

    def serialize(self) -> bytes:
        return self._portable

    @classmethod
    def deserialize(
        cls,
        portable: bytes,
        *,
        contract: NumericalContract = NumericalContract(),
        allow_fallback: bool = True,
    ) -> "JaxExecutable":
        jax = _jax()
        try:
            exported = jax.export.deserialize(bytearray(portable))
        except Exception as error:
            raise SerializationError("invalid serialized JAX StableHLO executable") from error
        return _build_executable(
            jax,
            exported,
            bytes(portable),
            contract=contract,
            allow_fallback=allow_fallback,
        )


def export_jax(
    function: Any,
    *example_args: Any,
    polymorphic_shapes: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    allow_fallback: bool = True,
    vjp_order: int = 1,
) -> JaxExecutable:
    """Export a JAX function as portable StableHLO and import its AWEN regions.

    ``polymorphic_shapes`` follows ``jax.export.symbolic_args_specs`` and keeps
    symbolic dimensions in the exported program. Unsupported StableHLO remains
    executable through JAX when ``allow_fallback`` is true.
    """

    if not example_args:
        raise ContractError("JAX export requires at least one example argument")
    if vjp_order < 0:
        raise ContractError("vjp_order must be non-negative")
    contract.validate()
    jax = _jax()
    _validate_input_contract(example_args, contract)
    try:
        specifications = example_args
        if polymorphic_shapes is not None:
            specifications = jax.export.symbolic_args_specs(example_args, polymorphic_shapes)
        exported = jax.export.export(jax.jit(function))(*specifications)
        portable = bytes(exported.serialize(vjp_order=vjp_order))
    except Exception as error:
        raise ContractError(f"JAX StableHLO export failed: {error}") from error
    return _build_executable(
        jax,
        exported,
        portable,
        contract=contract,
        allow_fallback=allow_fallback,
    )


def export_jax_value_and_grad(
    function: Any,
    *example_args: Any,
    argnums: Any = 0,
    polymorphic_shapes: Optional[Any] = None,
    contract: NumericalContract = NumericalContract(),
    allow_fallback: bool = True,
) -> JaxExecutable:
    """Export an analytic JAX value-and-gradient program as StableHLO."""

    jax = _jax()
    differentiated = jax.value_and_grad(function, argnums=argnums)
    return export_jax(
        differentiated,
        *example_args,
        polymorphic_shapes=polymorphic_shapes,
        contract=contract,
        allow_fallback=allow_fallback,
        vjp_order=1,
    )


def assert_numerical_contract(
    reference: Any, candidate: Any, contract: NumericalContract
) -> None:
    """Raise when two framework values violate the declared error contract."""

    contract.validate()
    import numpy as np

    expected = np.asarray(reference)
    actual = np.asarray(candidate)
    if expected.shape != actual.shape:
        raise ContractError(
            f"output shape {actual.shape} does not match reference shape {expected.shape}"
        )
    if not np.allclose(
        expected,
        actual,
        atol=contract.max_abs_error,
        rtol=contract.max_rel_error,
        equal_nan=False,
    ):
        difference = np.abs(expected - actual)
        raise ContractError(
            "output violates the numerical contract: "
            f"max_abs_error={float(difference.max())}"
        )


def _build_executable(
    jax: Any,
    exported: Any,
    portable: bytes,
    *,
    contract: NumericalContract,
    allow_fallback: bool,
) -> JaxExecutable:
    stablehlo = exported.mlir_module()
    operations = tuple(sorted(set(_STABLEHLO_OPERATION.findall(stablehlo))))
    supported = tuple(item for item in operations if item in _AWEN_SUPPORTED_STABLEHLO)
    fallback = tuple(item for item in operations if item not in _AWEN_SUPPORTED_STABLEHLO)
    diagnostics = tuple(
        JaxDiagnostic(
            code="unsupported_stablehlo_operation",
            operation=operation,
            message=(
                f"StableHLO operation '{operation}' is not lowered by AWEN v1 and "
                "will execute through the JAX framework fallback"
            ),
            action="framework_fallback",
        )
        for operation in fallback
    )
    if fallback and not allow_fallback:
        raise UnsupportedGraphError(
            "AWEN fallback is disabled for StableHLO operations: " + ", ".join(fallback)
        )
    if "dot_general" not in supported and not allow_fallback:
        raise UnsupportedGraphError("StableHLO program has no AWEN-supported dot_general region")
    report = JaxImportReport(
        version=JAX_INTEGRATION_VERSION,
        jax_version=jax.__version__,
        supported_jax_range=SUPPORTED_JAX_RANGE,
        calling_convention_version=int(exported.calling_convention_version),
        platforms=tuple(str(item) for item in exported.platforms),
        input_avals=tuple(str(item) for item in exported.in_avals),
        output_avals=tuple(str(item) for item in exported.out_avals),
        supported_operations=supported,
        fallback_operations=fallback,
        diagnostics=diagnostics,
        contract=contract,
        portable_fingerprint="sha256:" + hashlib.sha256(portable).hexdigest(),
    )
    return JaxExecutable(exported, portable, report)


def _jax() -> Any:
    try:
        import jax
    except ImportError as error:
        raise ContractError("JAX integration requires the awen_py[jax] extra") from error
    _require_supported_jax(jax.__version__)
    return jax


def _require_supported_jax(version: str) -> None:
    numeric = version.split("+", 1)[0].split(".")
    try:
        major, minor = int(numeric[0]), int(numeric[1])
    except (ValueError, IndexError) as error:
        raise ContractError(f"cannot parse JAX version '{version}'") from error
    if major != 0 or not 9 <= minor < 12:
        raise ContractError(f"JAX {version} is outside the tested range {SUPPORTED_JAX_RANGE}")


def _validate_input_contract(inputs: Sequence[Any], contract: NumericalContract) -> None:
    minimum_bits = contract.minimum_effective_bits
    if minimum_bits is None:
        return
    for value in inputs:
        dtype = str(getattr(value, "dtype", ""))
        bits = _effective_bits(dtype)
        if bits < minimum_bits:
            raise ContractError(
                f"JAX input dtype {dtype} provides {bits} effective bits; "
                f"the contract requires {minimum_bits}"
            )


def _effective_bits(dtype: str) -> int:
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
        if name in dtype.lower():
            return bits
    raise ContractError(f"unsupported JAX dtype '{dtype}'")
