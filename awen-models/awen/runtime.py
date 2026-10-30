"""High-level AWEN runtime facade.

    from awen import Runtime
    rt = Runtime(device="auto")            # CUDA when available, else CPU
    model = rt.load_model("awen-mini")     # bundled, trained checkpoint
    print(model.generate("ROMEO:", max_new_tokens=200, seed=0))

Devices:

* ``auto``/``cpu``/``cuda``: standard PyTorch execution (measured on the host).
* ``photonic-sim``: every dense GEMM of the model runs through the AWEN Rust
  photonic simulator with its physical noise model (``awen_native``). Results
  are labeled ``simulated``; they are not hardware measurements.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from pathlib import Path
from typing import Any, Optional

import torch

from awen_models.checkpoint import bundled_checkpoint_dir, load_checkpoint
from awen_models.device import resolve_device
from awen_models.model import AwenMini
from awen_models.tokenizer import CharTokenizer

DEVICES = ("auto", "cpu", "cuda", "photonic-sim")


@dataclass
class ExecutionInfo:
    device: str
    fidelity: str
    gemm_backend: str
    notes: list[str] = field(default_factory=list)


class LoadedModel:
    def __init__(self, model: AwenMini, tokenizer: CharTokenizer, metadata: dict[str, Any], info: ExecutionInfo,
                 device: torch.device, checkpoint_dir: Optional[Path] = None):
        self.model = model
        self.tokenizer = tokenizer
        self.metadata = metadata
        self.execution = info
        self._device = device
        self.checkpoint_dir = checkpoint_dir

    def generate(
        self,
        prompt: str,
        max_new_tokens: int = 200,
        temperature: float = 0.8,
        top_k: Optional[int] = 40,
        seed: Optional[int] = 0,
    ) -> str:
        """Return ``prompt`` followed by ``max_new_tokens`` generated characters."""
        if not prompt:
            raise ValueError("prompt must contain at least one character")
        ids = torch.tensor([self.tokenizer.encode(prompt)], dtype=torch.long, device=self._device)
        generator = None
        if seed is not None and temperature != 0:
            generator = torch.Generator(device=self._device).manual_seed(seed)
        out = self.model.generate(ids, max_new_tokens, temperature=temperature, top_k=top_k, generator=generator)
        return self.tokenizer.decode(out[0].tolist())

    @torch.no_grad()
    def log_likelihood(self, text: str) -> float:
        """Mean next-character cross-entropy (nats) of ``text`` under the model."""
        ids = torch.tensor([self.tokenizer.encode(text)], dtype=torch.long, device=self._device)
        block = self.model.config.block_size
        ids = ids[:, : block + 1]
        _, loss = self.model(ids[:, :-1], ids[:, 1:])
        return float(loss)

    def capability_report(self, text: str) -> dict[str, Any]:
        """Capability report for ``device="photonic-sim"`` over one forward pass of ``text``.

        Lists the GEMMs that ran on the photonic simulator and the operations
        that stayed on the host (with the reason), the precision and
        calibration the noise contract assumes, conversion counts, and the
        output deviation from the CPU reference loaded from the same
        checkpoint. The executor's counters are reset first (its seed is
        kept), so the counts cover exactly this pass. Everything is simulated;
        no energy, latency or speedup figure is produced.
        """
        if self.execution.device != "photonic-sim":
            raise RuntimeError("capability_report() needs a model loaded with device='photonic-sim'")
        if self.checkpoint_dir is None:
            raise RuntimeError("capability_report() needs the checkpoint directory to load the CPU reference")
        from awen.capability import build_report

        return build_report(self, text)


class Runtime:
    """Load and run AWEN models on a selected execution target."""

    def __init__(self, device: str = "auto", noise: Optional[dict[str, Any]] = None, seed: int = 0):
        if device not in DEVICES and not str(device).startswith("cuda:"):
            raise ValueError(f"device must be one of {DEVICES}, got {device!r}")
        self.requested_device = device
        self.noise = noise
        self.seed = seed

    def load_model(self, name_or_path: str = "awen-mini") -> LoadedModel:
        path = Path(name_or_path)
        directory = path if path.is_dir() else bundled_checkpoint_dir(name_or_path)
        if self.requested_device == "photonic-sim":
            host = torch.device("cpu")
            model, tokenizer, metadata = load_checkpoint(directory, host)
            info = _attach_photonic_simulator(model, self.noise, self.seed)
            return LoadedModel(model, tokenizer, metadata, info, host, checkpoint_dir=directory)
        device = resolve_device(self.requested_device)
        model, tokenizer, metadata = load_checkpoint(directory, device)
        info = ExecutionInfo(device=str(device), fidelity="measured", gemm_backend=f"torch-{device.type}")
        return LoadedModel(model, tokenizer, metadata, info, device, checkpoint_dir=directory)


def _attach_photonic_simulator(model: AwenMini, noise: Optional[dict[str, Any]], seed: int) -> ExecutionInfo:
    try:
        from awen.photonic import PhotonicGemmExecutor
    except ImportError as error:  # pragma: no cover - exercised when the extension is absent
        raise RuntimeError(
            "device='photonic-sim' needs the AWEN native extension (awen_native). "
            "Build it with `pip install ./awen-native` (requires a Rust toolchain)."
        ) from error
    executor = PhotonicGemmExecutor(noise=noise, seed=seed)
    for name, layer in model.linear_layers():
        layer.matmul_executor = executor.for_layer(name)
    return ExecutionInfo(
        device="photonic-sim",
        fidelity="simulated",
        gemm_backend="awen_native photonic GEMM simulator",
        notes=[executor.describe()],
    )
