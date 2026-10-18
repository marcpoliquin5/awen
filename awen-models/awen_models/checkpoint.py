"""Checkpoint save/load using safetensors (no pickle, no code execution on load)."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Any

import torch
from safetensors.torch import load_file, save_file

from .config import MiniConfig
from .model import AwenMini
from .tokenizer import CharTokenizer

WEIGHTS = "model.safetensors"
CONFIG = "config.json"
TOKENIZER = "tokenizer.json"
METADATA = "metadata.json"


def save_checkpoint(
    directory: str | Path,
    model: AwenMini,
    tokenizer: CharTokenizer,
    metadata: dict[str, Any] | None = None,
) -> Path:
    out = Path(directory)
    out.mkdir(parents=True, exist_ok=True)
    state = {k: v.detach().cpu().contiguous() for k, v in model.state_dict().items()}
    save_file(state, str(out / WEIGHTS), metadata={"format": "pt", "model": "awen-mini"})
    model.config.save(out / CONFIG)
    tokenizer.save(out / TOKENIZER)
    if metadata is not None:
        (out / METADATA).write_text(json.dumps(metadata, indent=2, sort_keys=True) + "\n")
    return out


def load_checkpoint(
    directory: str | Path, device: str | torch.device = "cpu"
) -> tuple[AwenMini, CharTokenizer, dict[str, Any]]:
    path = Path(directory)
    config = MiniConfig.load(path / CONFIG)
    tokenizer = CharTokenizer.load(path / TOKENIZER)
    if tokenizer.vocab_size != config.vocab_size:
        raise ValueError("tokenizer and config vocabulary sizes differ")
    model = AwenMini(config)
    state = load_file(str(path / WEIGHTS))
    model.load_state_dict(state, strict=True)
    model.to(device).eval()
    metadata_path = path / METADATA
    metadata = json.loads(metadata_path.read_text()) if metadata_path.exists() else {}
    return model, tokenizer, metadata


def bundled_checkpoint_dir(name: str = "awen-mini") -> Path:
    """Directory of a checkpoint shipped inside the package."""
    path = Path(__file__).resolve().parent / "checkpoints" / name
    if not (path / WEIGHTS).exists():
        raise FileNotFoundError(f"no bundled checkpoint named {name!r} at {path}")
    return path
