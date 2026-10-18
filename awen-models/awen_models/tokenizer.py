"""Character-level tokenizer with a deterministic, serializable vocabulary."""

from __future__ import annotations

import json
from pathlib import Path
from typing import Iterable


class CharTokenizer:
    """Maps each distinct character of a corpus to an integer id.

    The vocabulary is the sorted set of characters, so the same corpus always
    produces the same ids. Unknown characters raise ``KeyError`` on encode.
    """

    version = "awen.char-tokenizer.v1"

    def __init__(self, chars: Iterable[str]):
        ordered = sorted(set(chars))
        if not ordered:
            raise ValueError("tokenizer vocabulary is empty")
        self.itos = ordered
        self.stoi = {ch: i for i, ch in enumerate(ordered)}

    @classmethod
    def from_text(cls, text: str) -> "CharTokenizer":
        return cls(text)

    @property
    def vocab_size(self) -> int:
        return len(self.itos)

    def encode(self, text: str) -> list[int]:
        return [self.stoi[ch] for ch in text]

    def decode(self, ids: Iterable[int]) -> str:
        return "".join(self.itos[i] for i in ids)

    def save(self, path: str | Path) -> None:
        Path(path).write_text(
            json.dumps({"version": self.version, "itos": self.itos}, ensure_ascii=False, indent=1)
            + "\n",
            encoding="utf-8",
        )

    @classmethod
    def load(cls, path: str | Path) -> "CharTokenizer":
        data = json.loads(Path(path).read_text(encoding="utf-8"))
        if data.get("version") != cls.version:
            raise ValueError(f"unsupported tokenizer version {data.get('version')!r}")
        tokenizer = cls(data["itos"])
        if tokenizer.itos != data["itos"]:
            raise ValueError("tokenizer vocabulary is not sorted and unique")
        return tokenizer
