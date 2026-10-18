"""Datasets for AWEN-Mini.

Tiny Shakespeare is a 1,115,394-character excerpt of Shakespeare's plays
assembled by Andrej Karpathy for char-rnn. Shakespeare's text is in the public
domain; the file is distributed in the MIT-licensed ``karpathy/char-rnn``
repository. The download is pinned by SHA-256 so every run trains on identical
bytes.
"""

from __future__ import annotations

import hashlib
import urllib.request
from dataclasses import dataclass
from pathlib import Path

import torch

TINY_SHAKESPEARE = {
    "name": "tiny-shakespeare",
    "url": "https://raw.githubusercontent.com/karpathy/char-rnn/master/data/tinyshakespeare/input.txt",
    "sha256": "86c4e6aa9db7c042ec79f339dcb96d42b0075e16b8fc2e86bf0ca57e2dc565ed",
    "characters": 1115394,
    "license": "Public domain text (Shakespeare); file from the MIT-licensed karpathy/char-rnn repository",
}


def default_cache_dir() -> Path:
    return Path.home() / ".cache" / "awen-models"


def download_tiny_shakespeare(cache_dir: str | Path | None = None) -> str:
    """Return the Tiny Shakespeare text, downloading and verifying it once."""
    cache = Path(cache_dir) if cache_dir else default_cache_dir()
    cache.mkdir(parents=True, exist_ok=True)
    path = cache / "tinyshakespeare.txt"
    if not path.exists():
        with urllib.request.urlopen(TINY_SHAKESPEARE["url"], timeout=60) as response:
            payload = response.read()
        digest = hashlib.sha256(payload).hexdigest()
        if digest != TINY_SHAKESPEARE["sha256"]:
            raise ValueError(f"Tiny Shakespeare checksum mismatch: {digest}")
        path.write_bytes(payload)
    payload = path.read_bytes()
    if hashlib.sha256(payload).hexdigest() != TINY_SHAKESPEARE["sha256"]:
        raise ValueError(f"cached Tiny Shakespeare at {path} is corrupt; delete it and retry")
    return payload.decode("utf-8")


@dataclass
class TokenSplits:
    train: torch.Tensor
    val: torch.Tensor


def split_tokens(ids: list[int], val_fraction: float = 0.1) -> TokenSplits:
    """Contiguous split: the first ``1 - val_fraction`` for training, the rest for validation."""
    if not 0.0 < val_fraction < 1.0:
        raise ValueError("val_fraction must be in (0, 1)")
    data = torch.tensor(ids, dtype=torch.long)
    cut = int(len(data) * (1.0 - val_fraction))
    return TokenSplits(train=data[:cut], val=data[cut:])


def sample_batch(
    data: torch.Tensor,
    batch_size: int,
    block_size: int,
    generator: torch.Generator,
    device: torch.device | str = "cpu",
) -> tuple[torch.Tensor, torch.Tensor]:
    """Sample ``batch_size`` random windows of ``block_size`` tokens and their next-token targets."""
    if len(data) <= block_size + 1:
        raise ValueError("split is shorter than one training window")
    starts = torch.randint(len(data) - block_size - 1, (batch_size,), generator=generator)
    x = torch.stack([data[s : s + block_size] for s in starts.tolist()])
    y = torch.stack([data[s + 1 : s + 1 + block_size] for s in starts.tolist()])
    return x.to(device), y.to(device)
