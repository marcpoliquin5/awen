#!/usr/bin/env python3
"""Fail closed on repository hygiene, policy, workflow, and license drift."""

from __future__ import annotations

import hashlib
import json
import re
import subprocess
import sys
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]


def fail(message: str) -> None:
    print(f"repository policy violation: {message}", file=sys.stderr)
    raise SystemExit(1)


def repository_files() -> list[Path]:
    result = subprocess.run(
        ["git", "ls-files", "--cached", "--others", "--exclude-standard"],
        cwd=ROOT,
        check=True,
        capture_output=True,
        text=True,
    )
    return [ROOT / line for line in result.stdout.splitlines() if (ROOT / line).is_file()]


files = repository_files()
relative = {path.relative_to(ROOT).as_posix() for path in files}

required = {
    "CHANGELOG.md",
    "CODE_OF_CONDUCT.md",
    "CONTRIBUTING.md",
    "GOVERNANCE.md",
    "LICENSE",
    "MAINTAINERS.md",
    "README.md",
    "SECURITY.md",
    ".github/CODEOWNERS",
    ".github/dependabot.yml",
    ".github/pull_request_template.md",
    ".github/ISSUE_TEMPLATE/bug.yml",
    ".github/ISSUE_TEMPLATE/feature.yml",
    ".github/ISSUE_TEMPLATE/config.yml",
    "docs/IMPLEMENTATION-STATUS.md",
    "docs/RELEASING.md",
}
missing = sorted(required - relative)
if missing:
    fail(f"required files are missing: {', '.join(missing)}")

forbidden_suffixes = {
    ".dll", ".dylib", ".exe", ".gcda", ".gcno", ".o", ".obj",
    ".profdata", ".profraw", ".pyc", ".pyo", ".rlib", ".rmeta", ".so",
}
forbidden_parts = {
    ".mypy_cache", ".pytest_cache", ".ruff_cache", ".tox", ".venv",
    "__pycache__", "build", "coverage", "dist", "htmlcov", "target",
}
generated_names = {
    "awen_benchmark.json", "awen_compilation.json", "claims.json",
    "claims.md", "migration-report.json",
}
for path in files:
    rel = path.relative_to(ROOT)
    posix = rel.as_posix()
    if path.suffix.lower() in forbidden_suffixes:
        fail(f"generated binary is tracked: {posix}")
    if forbidden_parts.intersection(rel.parts):
        fail(f"generated directory is tracked: {posix}")
    if path.name in generated_names or any(
        part.startswith(("awen_run_", "awen_grad_", "awen_hil_artifacts"))
        for part in rel.parts
    ):
        fail(f"run artifact is tracked: {posix}")
    if path.name.startswith(".env") and path.name != ".env.example":
        fail(f"environment file is tracked: {posix}")

removed_products = ("awen-cloud/", "awen-studio/", "awen-ecosystem/kernels/datacom/")
for rel in relative:
    if rel.startswith(removed_products):
        fail(f"removed nonfunctional product surface returned: {rel}")

marker_words = (
    "TO" + "DO", "FIX" + "ME", "T" + "BD", "PLACE" + "HOLDER",
    "SCAF" + "FOLD",
)
rust_markers = ("to" + "do!", "un" + "implemented!")
marker_pattern = re.compile(
    r"(?<![A-Za-z0-9])(?:" + "|".join(marker_words) + r")(?:s)?(?![A-Za-z0-9])|(?:"
    + "|".join(re.escape(marker) for marker in rust_markers)
    + r")\s*\(",
    re.IGNORECASE,
)
text_suffixes = {
    ".c", ".cc", ".cpp", ".h", ".hpp", ".json", ".md", ".mlir",
    ".proto", ".py", ".rs", ".toml", ".txt", ".yml", ".yaml",
}
for path in files:
    if path.suffix.lower() not in text_suffixes or path.name == "Cargo.lock":
        continue
    content = path.read_text(encoding="utf-8")
    for line_number, line in enumerate(content.splitlines(), start=1):
        yaml_hint_key = "place" + "holder"
        if path.suffix.lower() in {".yml", ".yaml"} and re.match(
            rf"^\s*{yaml_hint_key}\s*:", line, re.IGNORECASE
        ):
            continue
        if marker_pattern.search(line):
            fail(
                "unowned development marker in "
                f"{path.relative_to(ROOT).as_posix()}:{line_number}"
            )

licenses = [
    (ROOT / "LICENSE").read_text(encoding="utf-8").strip(),
    (ROOT / "awen-runtime/LICENSE").read_text(encoding="utf-8").strip(),
    (ROOT / "awen-spec/LICENSE").read_text(encoding="utf-8").strip(),
]
if len(set(licenses)) != 1:
    fail("root, runtime, and specification license texts differ")
for clause in ("Permission is hereby granted", "THE SOFTWARE IS PROVIDED \"AS IS\""):
    if clause not in licenses[0]:
        fail(f"MIT license is incomplete; missing {clause!r}")

metadata_checks = {
    "awen-compiler/Cargo.toml": ('license = "MIT"', "repository = "),
    "awen-runtime/Cargo.toml": ('license = "MIT"', "repository = "),
    "awen-ecosystem/python_awen/pyproject.toml": ('license = "MIT"', "Repository = "),
}
for name, needles in metadata_checks.items():
    content = (ROOT / name).read_text(encoding="utf-8")
    for needle in needles:
        if needle not in content:
            fail(f"{name} is missing package metadata {needle!r}")

workflow_dir = ROOT / ".github/workflows"
workflows = sorted(workflow_dir.glob("*.yml"))
expected_workflows = {
    "hardware-benchmark.yml", "observability-quality-gate.yml", "release.yml",
}
if {path.name for path in workflows} != expected_workflows:
    fail("workflow set must contain one automated gate and two manual workflows")
names: dict[str, str] = {}
for path in workflows:
    content = path.read_text(encoding="utf-8")
    match = re.search(r"(?m)^name:\s*(.+?)\s*$", content)
    if not match:
        fail(f"workflow has no name: {path.name}")
    name = match.group(1)
    if name in names:
        fail(f"duplicate workflow name {name!r} in {names[name]} and {path.name}")
    names[name] = path.name
    automated = re.findall(r"(?m)^  (push|pull_request):\s*$", content)
    if path.name == "observability-quality-gate.yml":
        if set(automated) != {"push", "pull_request"}:
            fail("required quality workflow must run on push and pull request")
        if content.count("name: AWEN required quality gate") != 1:
            fail("required workflow must expose exactly one stable job context")
    elif automated:
        fail(f"manual workflow {path.name} has automated triggers")

marketplace = json.loads((ROOT / "awen-ecosystem/marketplace/index.json").read_text())
for entry in marketplace:
    manifest = (ROOT / "awen-ecosystem/marketplace" / entry["manifest"]).resolve()
    if not manifest.is_relative_to(ROOT) or not manifest.is_file():
        fail(f"marketplace manifest escapes or is missing: {entry['manifest']}")
    digest = hashlib.sha256(manifest.read_bytes()).hexdigest()
    if entry.get("checksum_algorithm") != "sha256" or entry.get("checksum") != digest:
        fail(f"marketplace checksum drift for {entry['name']}")

print(f"repository policy passed for {len(files)} files and {len(workflows)} workflows")
