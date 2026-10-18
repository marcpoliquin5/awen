"""Device selection shared by training, inference and the runtime facade."""

from __future__ import annotations

import torch


def resolve_device(device: str | torch.device | None = "auto") -> torch.device:
    """Resolve ``auto`` to CUDA when available, otherwise CPU.

    Explicit ``cpu``/``cuda``/``cuda:N`` values are honored; asking for CUDA on
    a machine without it raises instead of silently falling back.
    """
    if device is None or str(device) == "auto":
        return torch.device("cuda" if torch.cuda.is_available() else "cpu")
    resolved = torch.device(device)
    if resolved.type == "cuda" and not torch.cuda.is_available():
        raise RuntimeError("CUDA was requested but torch.cuda.is_available() is False")
    return resolved
