"""Configuration for AWEN-Mini decoder language models."""

from __future__ import annotations

import json
from dataclasses import asdict, dataclass, field
from pathlib import Path
from typing import Any

FFN_KINDS = ("gelu", "gated", "lowrank")


@dataclass(frozen=True)
class MiniConfig:
    """Hyperparameters of an AWEN-Mini decoder-only transformer.

    ``ffn`` selects the feed-forward block:

    * ``gelu``: standard two-layer MLP with GELU (baseline).
    * ``gated``: gated multiplicative unit, ``W_o(silu(W_a x) * (W_b x))``.
    * ``lowrank``: two-layer GELU MLP whose matrices are rank-``ffn_rank``
      factorizations.

    ``shared_layers`` reuses one block's weights ``n_layer`` times (recursive
    weight sharing).
    """

    vocab_size: int = 65
    block_size: int = 128
    n_layer: int = 5
    n_head: int = 4
    n_embd: int = 128
    ffn: str = "gelu"
    ffn_hidden: int = 512
    ffn_rank: int = 32
    shared_layers: bool = False
    dropout: float = 0.0
    bias: bool = True
    extra: dict[str, Any] = field(default_factory=dict)

    def __post_init__(self) -> None:
        if self.ffn not in FFN_KINDS:
            raise ValueError(f"ffn must be one of {FFN_KINDS}, got {self.ffn!r}")
        if self.n_embd % self.n_head:
            raise ValueError("n_embd must be divisible by n_head")
        for name in ("vocab_size", "block_size", "n_layer", "n_head", "n_embd", "ffn_hidden"):
            if getattr(self, name) <= 0:
                raise ValueError(f"{name} must be positive")

    def to_dict(self) -> dict[str, Any]:
        return asdict(self)

    @classmethod
    def from_dict(cls, data: dict[str, Any]) -> "MiniConfig":
        known = {f for f in cls.__dataclass_fields__}
        return cls(**{k: v for k, v in data.items() if k in known})

    def save(self, path: str | Path) -> None:
        Path(path).write_text(json.dumps(self.to_dict(), indent=2, sort_keys=True) + "\n")

    @classmethod
    def load(cls, path: str | Path) -> "MiniConfig":
        return cls.from_dict(json.loads(Path(path).read_text()))
