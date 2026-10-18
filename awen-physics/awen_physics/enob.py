"""Effective number of bits of the AWEN photonic GEMM model, measured by sine test.

A coherently sampled sine ``s(t)`` drives ``k − 1`` inputs of a length-``k``
dot product with weights +1. One extra input is held at +1 with weight 0: it
does not change the result but pins the per-vector DAC scale of the model to a
fixed full scale, as a real converter has. The simulator
(``awen_native.photonic_gemm``) adds its noise. A least-squares fit of the
known-frequency sine leaves the residual noise ``σ`` in full-scale units. The
full-scale-referred ``ENOB = (10·log10((1/2)/σ²) − 1.76)/6.02`` follows IEEE Std
1241-2010 (doi:10.1109/IEEESTD.2011.5692956).

Theory for a shot-noise-limited detector with ``N_fs`` full-scale
photoelectrons: ``σ = 1/√N_fs`` of full scale, hence
``ENOB = (10·log10(N_fs/2) − 1.76)/6.02``. A ``b``-bit uniform quantizer with
step ``Δ = 1/(2^{b−1} − 1)`` adds ``Δ²/12`` of noise power.
"""

from __future__ import annotations

import math
from typing import Any, Mapping, Optional

import numpy as np

from .fidelity import Fidelity


def _native():
    import awen_native

    return awen_native


def shot_only_contract(photons_per_mac: float, adc_bits: Optional[int] = None) -> dict:
    contract = _native().ideal_contract()
    contract.update(photons_per_mac=photons_per_mac, quantum_efficiency=1.0,
                    insertion_loss_db=0.0, adc_bits=adc_bits)
    return contract


def theory_enob(full_scale_photoelectrons: Optional[float], adc_bits: Optional[int] = None) -> float:
    power = 0.0
    if full_scale_photoelectrons:
        power += 1.0 / full_scale_photoelectrons
    if adc_bits is not None:
        step = 1.0 / (2 ** (adc_bits - 1) - 1)
        power += step * step / 12
    sinad_db = 10 * math.log10(0.5 / power)
    return (sinad_db - 1.76) / 6.02


def sine_test(
    contract: Mapping[str, Any],
    k: int = 16,
    samples: int = 4096,
    cycles: int = 127,
    seed: int = 0,
) -> dict:
    """Measure ENOB of the simulated GEMM with a coherently sampled full-scale sine."""
    if math.gcd(cycles, samples) != 1:
        raise ValueError("cycles and samples must be coprime for coherent sampling")
    t = np.arange(samples)
    s = np.sin(2 * math.pi * cycles * t / samples)
    x = np.ones((samples, k))
    x[:, :-1] = s[:, None]
    w = np.ones((1, k))
    w[0, -1] = 0.0
    y, report = _native().photonic_gemm(x, w, dict(contract), tile_k=k, seed=seed)
    out = y[:, 0] / k  # normalized detector output; full scale is 1
    basis = np.column_stack([np.sin(2 * math.pi * cycles * t / samples),
                             np.cos(2 * math.pi * cycles * t / samples), np.ones(samples)])
    coeffs, *_ = np.linalg.lstsq(basis, out, rcond=None)
    residual = out - basis @ coeffs
    noise_power = float(np.mean(residual**2))
    sinad_db = 10 * math.log10(0.5 / noise_power)  # referred to a full-scale sine
    return {
        "enob": (sinad_db - 1.76) / 6.02,
        "sinad_db": sinad_db,
        "fitted_amplitude": float(math.hypot(coeffs[0], coeffs[1])),
        "full_scale_photoelectrons": report["tile_budget"]["full_scale_photoelectrons"],
        "fidelity": Fidelity.COMPACT_MODEL.value,
        "evidence": "simulated (AWEN photonic noise model); not a hardware measurement",
    }
