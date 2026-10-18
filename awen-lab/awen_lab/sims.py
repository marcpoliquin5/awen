"""Adapters over the existing AWEN simulators. No hardware path exists here."""

from __future__ import annotations

import hashlib
import math
from typing import Any, Mapping

import numpy as np


def array_hash(a: np.ndarray) -> str:
    data = np.ascontiguousarray(a)
    return "sha256:" + hashlib.sha256(data.tobytes() + str(data.shape).encode() + str(data.dtype).encode()).hexdigest()


def _contract(target: str, overrides: Mapping[str, Any]) -> dict[str, Any]:
    import awen_native

    base = awen_native.ideal_contract() if target == "awen-reference" else awen_native.default_contract()
    base.update(dict(overrides))
    return awen_native.validate_contract(base)


def gemm_inputs(m: int, k: int, n: int, input_seed: int) -> tuple[np.ndarray, np.ndarray]:
    """Full-scale inputs: uniform in [-1, 1], the regime the noise budget is defined for."""
    rng = np.random.default_rng(input_seed)
    return rng.uniform(-1.0, 1.0, (m, k)), rng.uniform(-1.0, 1.0, (n, k))


def run_photonic_gemm(exp: Mapping[str, Any], seed: int) -> dict[str, Any]:
    import awen_native

    m, k, n = exp["shape_mkn"]
    tile_k = int(exp.get("tile_k", 64))
    x, w = gemm_inputs(m, k, n, int(exp.get("input_seed", 0)))
    contract = _contract(exp["target"], exp.get("contract", {}))
    y, info = awen_native.photonic_gemm(x, w, contract, tile_k=tile_k, seed=seed)
    oracle = x @ w.T
    err = y - oracle
    rms = float(np.sqrt(np.mean(err ** 2)))
    tiles = math.ceil(k / tile_k)
    budget = info.get("tile_budget", {})
    predicted = float(budget.get("total", 0.0)) * tile_k * math.sqrt(tiles)
    return {
        "rms_error": rms,
        "relative_rms_error": rms / float(np.max(np.abs(oracle))),
        "max_abs_error": float(np.max(np.abs(err))),
        "predicted_rms_error": predicted,
        "effective_bits_per_tile": budget.get("effective_bits"),
        "output_hash": array_hash(y),
        "fidelity": info.get("fidelity", "compact_model"),
    }


def run_boson_sampling(exp: Mapping[str, Any], seed: int) -> dict[str, Any]:
    from awen_qphotonic.fock import boson_sample, empirical_distribution, haar_unitary, output_distribution, total_variation

    u = haar_unitary(int(exp["modes"]), np.random.default_rng(int(exp.get("unitary_seed", 0))))
    s = [int(v) for v in exp["input"]]
    shots = int(exp["shots"])
    exact = output_distribution(u, s)
    samples = boson_sample(u, s, shots, np.random.default_rng(seed))
    empirical = empirical_distribution(samples)
    tv = float(total_variation(exact, empirical))
    canon = repr(sorted((tuple(k), round(v, 12)) for k, v in empirical.items())).encode()
    return {
        "total_variation_vs_exact": tv,
        "support_size": len(exact),
        "shots": shots,
        "tv_bound": 3.0 * math.sqrt(len(exact) / shots),
        "output_hash": "sha256:" + hashlib.sha256(canon).hexdigest(),
        "fidelity": "exact reference simulation (sampling from exact distribution)",
    }


def hom_visibility_check() -> dict[str, Any]:
    from awen_qphotonic.fock import hom_coincidence_probability

    return {"coincidence_at_zero_delay_50_50": float(hom_coincidence_probability(0.5, 1.0)),
            "coincidence_distinguishable_50_50": float(hom_coincidence_probability(0.5, 0.0))}


RUNNERS = {"photonic_gemm": run_photonic_gemm, "boson_sampling": run_boson_sampling}


def run(exp: Mapping[str, Any], seed: int) -> dict[str, Any]:
    return RUNNERS[exp["kind"]](exp, seed)


def versions() -> dict[str, Any]:
    from importlib.metadata import PackageNotFoundError, version

    out: dict[str, Any] = {"numpy": np.__version__}
    for name in ("awen-native", "awen-qphotonic", "awen-lab"):
        try:
            out[name] = version(name)
        except PackageNotFoundError:
            out[name] = None
    return out
