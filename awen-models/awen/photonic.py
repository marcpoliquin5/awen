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
        self.layer_stats: dict[str, dict[str, Any]] = {}

    def reset(self, seed: Optional[int] = None) -> None:
        if seed is not None:
            self.seed = int(seed)
        self.calls = 0
        self.macs = 0
        self.layer_stats = {}

    def __call__(self, x: torch.Tensor, weight: torch.Tensor, label: Optional[str] = None) -> torch.Tensor:
        xa = np.ascontiguousarray(x.detach().to("cpu", torch.float64).numpy())
        wa = np.ascontiguousarray(weight.detach().to("cpu", torch.float64).numpy())
        y, report = awen_native.photonic_gemm(xa, wa, self.contract, tile_k=self.tile_k, seed=self.seed + self.calls)
        self.calls += 1
        self.macs += xa.shape[0] * xa.shape[1] * wa.shape[0]
        self._record(label or "unlabeled", report)
        return torch.from_numpy(y).to(device=x.device, dtype=x.dtype)

    def _record(self, label: str, report: Mapping[str, Any]) -> None:
        """Accumulate per-layer counts from the simulator's own GEMM report.

        Conversion counts follow the simulator's tiling: every input element is
        converted once (DAC), every weight element is programmed once per call
        (DAC), and every output is read once per contraction tile (ADC); tile
        partial sums are accumulated on the host.
        """
        rows, inner, outputs = int(report["rows"]), int(report["inner"]), int(report["outputs"])
        tiles = int(report["tiles_per_output"])
        bits = report.get("tile_budget", {}).get("effective_bits")  # None for the ideal (noise-free) contract
        stats = self.layer_stats.setdefault(label, {
            "inner": inner, "outputs": outputs, "tile_k": int(report["tile_k"]), "tiles_per_output": tiles,
            "calls": 0, "rows": 0, "macs": 0, "dac_input_conversions": 0, "dac_weight_conversions": 0,
            "adc_conversions": 0, "tile_effective_bits": None if bits is None else float(bits),
            "max_abs_error_vs_exact": 0.0, "_sq_error": 0.0, "_outputs_seen": 0,
        })
        stats["calls"] += 1
        stats["rows"] += rows
        stats["macs"] += rows * inner * outputs
        stats["dac_input_conversions"] += rows * inner
        stats["dac_weight_conversions"] += outputs * inner
        stats["adc_conversions"] += rows * outputs * tiles
        stats["max_abs_error_vs_exact"] = max(stats["max_abs_error_vs_exact"], float(report["max_abs_error_vs_exact"]))
        stats["_sq_error"] += float(report["rms_error_vs_exact"]) ** 2 * rows * outputs
        stats["_outputs_seen"] += rows * outputs

    def layer_summary(self) -> list[dict[str, Any]]:
        """Per-layer counts with the GEMM error vs exact float64 aggregated as RMS."""
        summary = []
        for label, stats in self.layer_stats.items():
            row = {k: v for k, v in stats.items() if not k.startswith("_")}
            seen = stats["_outputs_seen"]
            row["rms_error_vs_exact"] = (stats["_sq_error"] / seen) ** 0.5 if seen else 0.0
            summary.append({"layer": label, **row})
        return summary

    def for_layer(self, label: str) -> "LabeledExecutor":
        return LabeledExecutor(self, label)

    def describe(self) -> str:
        c = self.contract
        return (
            f"photonic GEMM model (compact model, simulated): tile_k={self.tile_k}, "
            f"photons_per_mac={c.get('photons_per_mac')}, insertion_loss_db={c.get('insertion_loss_db')}, "
            f"dac_bits={c.get('dac_bits')}, adc_bits={c.get('adc_bits')}"
        )


class LabeledExecutor:
    """Per-layer view of a shared :class:`PhotonicGemmExecutor`.

    Calls are forwarded with the layer label so the capability report can
    attribute counts; attributes such as ``calls`` and ``macs`` read through
    to the shared executor.
    """

    def __init__(self, executor: PhotonicGemmExecutor, label: str):
        self.executor = executor
        self.label = label

    def __call__(self, x: torch.Tensor, weight: torch.Tensor) -> torch.Tensor:
        return self.executor(x, weight, label=self.label)

    def __getattr__(self, name: str) -> Any:
        return getattr(self.executor, name)
