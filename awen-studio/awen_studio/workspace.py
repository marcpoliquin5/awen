"""Studio projects on local disk.

A project is a directory under the workspace root holding ``project.json``
(name, description, created time), the tensor program and device capability
document it compiles, the last compile report, and a Lab Agent store
(``lab/``) with that project's experiment history.
"""

from __future__ import annotations

import json
import re
import shutil
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

NAME = re.compile(r"^[a-z0-9][a-z0-9-]{0,47}$")

# Examples shipped in the repository; resolved relative to the repo root.
EXAMPLES = {
    "gemm-4x4": ("awen-compiler/examples/gemm_4x4.json", "awen-compiler/capabilities/reference_2x2.json"),
    "gemm-256": ("awen-compiler/examples/gemm_256.json", "awen-compiler/capabilities/pace_like_128.json"),
}


class ProjectError(ValueError):
    """Invalid project name, missing project or example."""


def repo_root() -> Optional[Path]:
    """Repository root (needed for bundled examples): ``$AWEN_REPO``, then the
    current directory and its parents, then this file's parents."""
    import os

    candidates = []
    if os.environ.get("AWEN_REPO"):
        candidates.append(Path(os.environ["AWEN_REPO"]))
    cwd = Path.cwd().resolve()
    candidates += [cwd, *cwd.parents, *Path(__file__).resolve().parents]
    for parent in candidates:
        if (parent / "awen-compiler" / "examples").is_dir():
            return parent
    return None


def utc_now() -> str:
    return datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")


class Workspace:
    def __init__(self, root: str | Path, repo: Optional[Path] = None):
        self.root = Path(root).expanduser().resolve()
        self.root.mkdir(parents=True, exist_ok=True)
        self.repo = repo or repo_root()

    # -- paths ---------------------------------------------------------------
    def _dir(self, name: str) -> Path:
        if not NAME.match(name):
            raise ProjectError("project names are 1-48 chars of a-z, 0-9 and '-', starting with a letter or digit")
        return self.root / name

    def project_dir(self, name: str) -> Path:
        path = self._dir(name)
        if not (path / "project.json").is_file():
            raise ProjectError(f"no project named {name!r}")
        return path

    def lab_store(self, name: str) -> Path:
        return self.project_dir(name) / "lab"

    # -- queries -------------------------------------------------------------
    def examples(self) -> list[dict[str, Any]]:
        out = []
        for key, (program, caps) in EXAMPLES.items():
            available = bool(self.repo and (self.repo / program).is_file() and (self.repo / caps).is_file())
            out.append({"id": key, "program": program, "capabilities": caps, "available": available})
        return out

    def list(self) -> list[dict[str, Any]]:
        projects = []
        for path in sorted(self.root.iterdir()):
            meta = path / "project.json"
            if path.is_dir() and meta.is_file():
                doc = json.loads(meta.read_text())
                doc["experiments"] = len(list((path / "lab" / "objects").glob("*.json"))) if (path / "lab").is_dir() else 0
                doc["has_compile_report"] = (path / "compile_report.json").is_file()
                projects.append(doc)
        return projects

    def get(self, name: str) -> dict[str, Any]:
        path = self.project_dir(name)
        doc = json.loads((path / "project.json").read_text())
        doc["program"] = json.loads((path / "program.json").read_text())
        doc["capabilities"] = json.loads((path / "capabilities.json").read_text())
        report = path / "compile_report.json"
        doc["compile_report"] = json.loads(report.read_text()) if report.is_file() else None
        return doc

    # -- mutations -----------------------------------------------------------
    def create(self, name: str, example: str = "gemm-4x4", description: str = "",
               program: Optional[dict] = None, capabilities: Optional[dict] = None) -> dict[str, Any]:
        path = self._dir(name)
        if path.exists():
            raise ProjectError(f"project {name!r} already exists")
        if program is None or capabilities is None:
            if example not in EXAMPLES:
                raise ProjectError(f"unknown example {example!r}; choose one of {sorted(EXAMPLES)}")
            if self.repo is None:
                raise ProjectError("bundled examples need a repository checkout; pass program and capabilities")
            prog_rel, caps_rel = EXAMPLES[example]
            program = program or json.loads((self.repo / prog_rel).read_text())
            capabilities = capabilities or json.loads((self.repo / caps_rel).read_text())
        path.mkdir(parents=True)
        try:
            meta = {"name": name, "description": description, "example": example, "created_at": utc_now()}
            (path / "project.json").write_text(json.dumps(meta, indent=2) + "\n")
            (path / "program.json").write_text(json.dumps(program, indent=2) + "\n")
            (path / "capabilities.json").write_text(json.dumps(capabilities, indent=2) + "\n")
        except Exception:
            shutil.rmtree(path, ignore_errors=True)
            raise
        return self.get(name)

    def save_compile_report(self, name: str, report: dict[str, Any]) -> None:
        (self.project_dir(name) / "compile_report.json").write_text(json.dumps(report, indent=2) + "\n")
