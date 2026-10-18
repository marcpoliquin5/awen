"""Immutable, content-addressed lab states with branches.

A :class:`LabState` is a JSON document (experiment, results, provenance and
parent hashes). Its identity is ``sha256`` of its canonical JSON, so any
change produces a new state and a stored state cannot be altered without
detection. :class:`LabStore` keeps states under ``objects/<hash>.json``
(write-once) and named branches under ``refs/<name>``.
"""

from __future__ import annotations

import hashlib
import json
import re
from dataclasses import dataclass
from pathlib import Path
from types import MappingProxyType
from typing import Any, Mapping, Optional

BRANCH_RE = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._-]{0,63}$")


class StateIntegrityError(RuntimeError):
    """A stored state no longer matches its content hash."""


def canonical(obj: Any) -> bytes:
    return json.dumps(obj, sort_keys=True, separators=(",", ":"), allow_nan=False).encode()


def content_hash(obj: Any) -> str:
    return "sha256:" + hashlib.sha256(canonical(obj)).hexdigest()


def _freeze(obj: Any) -> Any:
    if isinstance(obj, Mapping):
        return MappingProxyType({k: _freeze(v) for k, v in obj.items()})
    if isinstance(obj, (list, tuple)):
        return tuple(_freeze(v) for v in obj)
    return obj


def _thaw(obj: Any) -> Any:
    if isinstance(obj, Mapping):
        return {k: _thaw(v) for k, v in obj.items()}
    if isinstance(obj, tuple):
        return [_thaw(v) for v in obj]
    return obj


@dataclass(frozen=True)
class LabState:
    """Read-only view of one lab state."""

    hash: str
    document: Mapping[str, Any]

    @classmethod
    def create(cls, document: Mapping[str, Any]) -> "LabState":
        plain = json.loads(canonical(document))
        return cls(content_hash(plain), _freeze(plain))

    def to_dict(self) -> dict[str, Any]:
        return _thaw(self.document)

    @property
    def parents(self) -> list[str]:
        return list(self.document.get("parents", ()))

    @property
    def workflow(self) -> str:
        return self.document["workflow"]

    @property
    def metrics(self) -> dict[str, Any]:
        return _thaw(self.document.get("metrics", {}))


class LabStore:
    def __init__(self, root: str | Path):
        self.root = Path(root)
        (self.root / "objects").mkdir(parents=True, exist_ok=True)
        (self.root / "refs").mkdir(parents=True, exist_ok=True)

    # objects ---------------------------------------------------------------
    def _path(self, digest: str) -> Path:
        if not re.fullmatch(r"sha256:[0-9a-f]{64}", digest):
            raise ValueError(f"not a state hash: {digest!r}")
        return self.root / "objects" / (digest.split(":", 1)[1] + ".json")

    def put(self, state: LabState) -> str:
        path = self._path(state.hash)
        if path.exists():
            self.get(state.hash)  # verifies integrity of the existing copy
            return state.hash
        with open(path, "x", encoding="utf-8") as handle:
            handle.write(json.dumps(state.to_dict(), indent=2, sort_keys=True) + "\n")
        return state.hash

    def get(self, digest: str) -> LabState:
        path = self._path(digest)
        if not path.exists():
            raise KeyError(digest)
        document = json.loads(path.read_text(encoding="utf-8"))
        if content_hash(document) != digest:
            raise StateIntegrityError(f"state {digest} was modified on disk")
        return LabState(digest, _freeze(document))

    def resolve(self, ref: str) -> str:
        """A state hash, a unique hash prefix (``sha256:abcd`` or ``abcd``) or a branch name."""
        if (self.root / "refs" / ref).is_file() and BRANCH_RE.match(ref):
            return (self.root / "refs" / ref).read_text().strip()
        prefix = ref.split(":", 1)[-1]
        if re.fullmatch(r"[0-9a-f]{6,64}", prefix):
            hits = [p.stem for p in (self.root / "objects").glob(prefix + "*.json")]
            if len(hits) == 1:
                return "sha256:" + hits[0]
            if len(hits) > 1:
                raise KeyError(f"ambiguous state prefix {ref!r}")
        raise KeyError(ref)

    # branches --------------------------------------------------------------
    def branches(self) -> dict[str, str]:
        return {p.name: p.read_text().strip() for p in sorted((self.root / "refs").iterdir())}

    def set_branch(self, name: str, digest: str) -> None:
        if not BRANCH_RE.match(name):
            raise ValueError(f"invalid branch name {name!r}")
        self.get(digest)
        (self.root / "refs" / name).write_text(digest + "\n")

    def branch(self, name: str, from_ref: str) -> str:
        if (self.root / "refs" / name).exists():
            raise ValueError(f"branch {name!r} exists")
        digest = self.resolve(from_ref)
        self.set_branch(name, digest)
        return digest

    def restore(self, ref: str) -> LabState:
        """Return the state for a hash or branch (verified). States are never mutated."""
        return self.get(self.resolve(ref))

    def history(self, ref: str) -> list[str]:
        """First-parent chain from ``ref`` back to the root."""
        chain, digest = [], self.resolve(ref)
        while digest:
            chain.append(digest)
            parents = self.get(digest).parents
            digest = parents[0] if parents else ""
        return chain

    def verify_all(self) -> int:
        count = 0
        for path in (self.root / "objects").glob("*.json"):
            self.get("sha256:" + path.stem)
            count += 1
        return count


def compare_states(a: LabState, b: LabState) -> dict[str, Any]:
    """Experiment-setting differences and numeric metric deltas (b - a)."""
    def flat(prefix: str, obj: Any, out: dict[str, Any]) -> dict[str, Any]:
        if isinstance(obj, Mapping):
            for k, v in obj.items():
                flat(f"{prefix}.{k}" if prefix else k, v, out)
        else:
            out[prefix] = _thaw(obj)
        return out
    ea, eb = flat("", a.document.get("experiment", {}), {}), flat("", b.document.get("experiment", {}), {})
    settings = {k: {"a": ea.get(k), "b": eb.get(k)} for k in sorted(set(ea) | set(eb)) if ea.get(k) != eb.get(k)}
    ma, mb = flat("", a.document.get("metrics", {}), {}), flat("", b.document.get("metrics", {}), {})
    deltas = {}
    for k in sorted(set(ma) & set(mb)):
        if isinstance(ma[k], (int, float)) and isinstance(mb[k], (int, float)) and not isinstance(ma[k], bool):
            deltas[k] = {"a": ma[k], "b": mb[k], "delta": mb[k] - ma[k]}
    return {"a": a.hash, "b": b.hash, "setting_differences": settings, "metric_deltas": deltas}


def lineage(store: LabStore, digest: Optional[str]) -> list[dict[str, Any]]:
    out = []
    while digest:
        state = store.get(digest)
        out.append({"hash": digest, "workflow": state.workflow})
        digest = state.parents[0] if state.parents else None
    return out
