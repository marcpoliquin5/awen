"""Python bindings for the AWEN compiler and photonic GEMM simulator.

Every result from this package is simulated. ``photonic_gemm`` reports carry
``fidelity: "compact_model"`` and are not hardware measurements.
"""

from __future__ import annotations

import json
from typing import Any, Mapping, Optional, Tuple

import numpy as np

from . import _native

__version__ = _native.__version__
NOISE_CONTRACT_VERSION = _native.NOISE_CONTRACT_VERSION

__all__ = [
    "NOISE_CONTRACT_VERSION",
    "compile_program",
    "default_contract",
    "effective_bits_from_sigma",
    "ideal_contract",
    "photonic_gemm",
    "op_table",
    "predicted_noise",
    "run_op",
    "shot_limited_effective_bits",
    "validate_contract",
]


def _contract_json(contract: Optional[Mapping[str, Any]]) -> Optional[str]:
    return None if contract is None else json.dumps(dict(contract))


def default_contract() -> dict:
    """Default noise contract: an illustrative operating point, not a measured device."""
    return json.loads(_native.default_contract_json())


def ideal_contract() -> dict:
    """Noise-free contract (all optional terms disabled)."""
    return json.loads(_native.ideal_contract_json())


def validate_contract(contract: Mapping[str, Any]) -> dict:
    """Validate ``contract`` and return it normalized; raises ``ValueError``."""
    return json.loads(_native.validate_contract(json.dumps(dict(contract))))


def predicted_noise(k: int, signal: float = 1.0, contract: Optional[Mapping[str, Any]] = None) -> dict:
    """Analytic per-term noise standard deviations for one tile of length ``k``."""
    return json.loads(_native.predicted_noise(k, signal, _contract_json(contract)))


def effective_bits_from_sigma(sigma: float) -> float:
    return _native.effective_bits_from_sigma(sigma)


def shot_limited_effective_bits(full_scale_photoelectrons: float) -> float:
    return _native.shot_limited_effective_bits(full_scale_photoelectrons)


def photonic_gemm(
    x: Any,
    w: Any,
    contract: Optional[Mapping[str, Any]] = None,
    tile_k: int = 64,
    seed: int = 0,
) -> Tuple[np.ndarray, dict]:
    """Simulate ``x @ w.T`` (``x: [m, k]``, ``w: [n, k]``) with the photonic noise model."""
    xa = np.ascontiguousarray(x, dtype=np.float64)
    wa = np.ascontiguousarray(w, dtype=np.float64)
    if xa.ndim != 2 or wa.ndim != 2:
        raise ValueError("photonic_gemm expects 2-D operands")
    y, report = _native.photonic_gemm(xa, wa, _contract_json(contract), int(tile_k), int(seed))
    return y, json.loads(report)


def compile_program(
    program: Mapping[str, Any],
    capabilities: Mapping[str, Any],
    options: Optional[Mapping[str, Any]] = None,
) -> dict:
    """Compile a TensorProgram for DeviceCapabilities with the Rust compiler."""
    return json.loads(
        _native.compile_program(
            json.dumps(dict(program)),
            json.dumps(dict(capabilities)),
            None if options is None else json.dumps(dict(options)),
        )
    )


def op_table() -> list:
    """Operator support table with each operator's highest verified level.

    Levels: ``math_reference`` < ``photonic_sim`` < ``hardware_lowerable`` <
    ``hardware_measured``. No operator is ``hardware_measured``.
    """
    return json.loads(_native.op_table_json())


def run_op(
    op: str,
    inputs: Mapping[str, Any],
    params: Optional[Mapping[str, Any]] = None,
    contract: Optional[Mapping[str, Any]] = None,
    tile_k: int = 64,
    seed: int = 0,
) -> Tuple[dict, dict]:
    """Run a table operator; returns ``(outputs, report)``.

    ``contract=None`` runs the f64 math reference. With a contract every matrix
    product runs on the Rust compact noise model, so outputs are simulated.
    """
    request = {
        "op": op,
        "inputs": {
            name: {"shape": list(a.shape), "data": a.ravel().tolist()}
            for name, a in ((k, np.asarray(v, dtype=np.float64)) for k, v in inputs.items())
        },
        "params": dict(params or {}),
        "contract": None if contract is None else dict(contract),
        "tile_k": int(tile_k),
        "seed": int(seed),
    }
    response = json.loads(_native.run_op_json(json.dumps(request)))
    outputs = {
        name: np.asarray(a["data"], dtype=np.float64).reshape(a["shape"])
        for name, a in response.pop("outputs").items()
    }
    return outputs, response
