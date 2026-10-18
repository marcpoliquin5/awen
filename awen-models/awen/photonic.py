"""Photonic-simulator execution of AWEN model GEMMs.

:class:`PhotonicGemmExecutor` plugs into :class:`awen_models.model.AwenLinear`
and runs ``x @ W.T`` in the AWEN Rust photonic GEMM model (``awen_native``)
with its physical-unit noise contract. Everything else in the model stays on
the host. Results are labeled ``simulated`` (compact-model fidelity); they are
not hardware measurements.
"""

from __future__ import annotations

from typing import Any, Mapping, Optional

import numpy as np
import torch

try:
    import awen_native
except ImportError as error:  # pragma: no cover - import guard
    raise ImportError(
        "awen.photonic needs the AWEN native extension: pip install ./awen-native"
    ) from error


class PhotonicGemmExecutor:
    """Callable ``(x[rows, in], weight[out, in]) -> y[rows, out]`` on the photonic model.

    Each call uses ``seed + call_index`` so a sequence of calls is reproducible
    for a given seed while successive GEMMs see independent noise.
    """

    def __init__(self, noise: Optional[Mapping[str, Any]] = None, seed: int = 0, tile_k: int = 64):
        if tile_k <= 0:
            raise ValueError("tile_k must be positive")
        self.contract = awen_native.validate_contract(noise) if noise is not None else awen_native.default_contract()
        self.seed = int(seed)
        self.tile_k = int(tile_k)
        self.calls = 0
        self.macs = 0

    def reset(self, seed: Optional[int] = None) -> None:
        if seed is not None:
            self.seed = int(seed)
        self.calls = 0
        self.macs = 0

    def __call__(self, x: torch.Tensor, weight: torch.Tensor) -> torch.Tensor:
        xa = np.ascontiguousarray(x.detach().to("cpu", torch.float64).numpy())
        wa = np.ascontiguousarray(weight.detach().to("cpu", torch.float64).numpy())
        y, _ = awen_native.photonic_gemm(xa, wa, self.contract, tile_k=self.tile_k, seed=self.seed + self.calls)
        self.calls += 1
        self.macs += xa.shape[0] * xa.shape[1] * wa.shape[0]
        return torch.from_numpy(y).to(device=x.device, dtype=x.dtype)

    def describe(self) -> str:
        c = self.contract
        return (
            f"photonic GEMM model (compact model, simulated): tile_k={self.tile_k}, "
            f"photons_per_mac={c.get('photons_per_mac')}, insertion_loss_db={c.get('insertion_loss_db')}, "
            f"dac_bits={c.get('dac_bits')}, adc_bits={c.get('adc_bits')}"
        )
