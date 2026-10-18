#!/usr/bin/env python3
"""Fail if shipped Python code can execute untrusted model code or unpickle weights.

AWEN's model-loading policy (see SECURITY.md, "Safe model loading"):

* Hugging Face loaders always pass ``trust_remote_code=False`` (never ``True``),
  and weight loads pass ``use_safetensors=True``.
* No ``pickle.load``/``pickle.loads``/``dill``/``joblib.load`` on model data.
* ``torch.load`` only with ``weights_only=True``.

Test files are excluded (they may round-trip in-memory tensors they created).
The check is lexical on purpose: it is cheap, has no dependencies, and any
exception must be argued in review by editing this allowlist.
"""

from __future__ import annotations

import ast
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
SKIP_PARTS = {"tests", "build", "target", "node_modules", ".venv", "__pycache__"}
FORBIDDEN_CALLS = {
    ("pickle", "load"), ("pickle", "loads"), ("dill", "load"), ("dill", "loads"),
    ("joblib", "load"), ("cloudpickle", "load"), ("cloudpickle", "loads"),
}
HF_LOADERS = {"from_pretrained"}


def shipped_python_files() -> list[Path]:
    out = []
    for path in sorted(ROOT.rglob("*.py")):
        rel = path.relative_to(ROOT)
        if SKIP_PARTS.intersection(rel.parts) or rel.name.startswith("test_"):
            continue
        out.append(path)
    return out


def dotted(node: ast.AST) -> tuple[str, ...]:
    parts: list[str] = []
    while isinstance(node, ast.Attribute):
        parts.append(node.attr)
        node = node.value
    if isinstance(node, ast.Name):
        parts.append(node.id)
    return tuple(reversed(parts))


def kw(call: ast.Call, name: str) -> ast.AST | None:
    for keyword in call.keywords:
        if keyword.arg == name:
            return keyword.value
    return None


def is_const(node: ast.AST | None, value: object) -> bool:
    return isinstance(node, ast.Constant) and node.value is value


def check_file(path: Path) -> list[str]:
    try:
        tree = ast.parse(path.read_text(encoding="utf-8"), filename=str(path))
    except SyntaxError as error:  # pragma: no cover - CI would catch elsewhere
        return [f"{path}: cannot parse: {error}"]
    problems = []
    try:
        rel = path.relative_to(ROOT)
    except ValueError:
        rel = path
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        name = dotted(node.func)
        where = f"{rel}:{node.lineno}"
        if len(name) >= 2 and name[-2:] in FORBIDDEN_CALLS:
            problems.append(f"{where}: {'.'.join(name)} can execute arbitrary code")
        if name[-2:] == ("torch", "load") and not is_const(kw(node, "weights_only"), True):
            problems.append(f"{where}: torch.load without weights_only=True")
        remote = kw(node, "trust_remote_code")
        if remote is not None and not is_const(remote, False):
            problems.append(f"{where}: trust_remote_code must be the literal False")
        if name and name[-1] in HF_LOADERS and remote is None:
            problems.append(f"{where}: {name[-1]} must pass trust_remote_code=False explicitly")
    return problems


def main() -> int:
    files = shipped_python_files()
    problems = [p for f in files for p in check_file(f)]
    if problems:
        print("safe model loading check failed:", file=sys.stderr)
        print("\n".join(f"- {p}" for p in problems), file=sys.stderr)
        return 1
    print(f"safe model loading check passed for {len(files)} shipped Python files")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
