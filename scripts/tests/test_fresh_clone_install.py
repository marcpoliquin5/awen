"""Checks that the fresh-clone install script matches docs/install.md and runs in the quality gate.

run: python -m pytest scripts/tests
"""

import re
from pathlib import Path

import yaml

ROOT = Path(__file__).resolve().parents[2]
DOC = ROOT / "docs" / "install.md"
SCRIPT = ROOT / "scripts" / "fresh_clone_install.sh"
WORKFLOW = ROOT / ".github/workflows/observability-quality-gate.yml"
STEP = "Fresh-clone install from the pinned CPU lockfile (docs/install.md)"


def commands(text):
    """Logical shell commands: backslash continuations joined, whitespace collapsed, comments dropped."""
    joined = re.sub(r"\\\n\s*", " ", text)
    out = []
    for line in joined.splitlines():
        line = re.sub(r"\s+#.*$", "", line).strip()
        if line and not line.startswith("#"):
            out.append(re.sub(r"\s+", " ", line))
    return out


def doc_section_commands(heading):
    doc = DOC.read_text(encoding="utf-8")
    section = re.search(rf"^## {re.escape(heading)}\n(.*?)(?=^## )", doc, flags=re.M | re.S)
    assert section, heading
    blocks = re.findall(r"```bash\n(.*?)```", section.group(1), flags=re.S)
    assert len(blocks) == 1, heading
    return commands(blocks[0])


def script_commands():
    return commands(SCRIPT.read_text(encoding="utf-8"))


def test_commands_helper_joins_continuations():
    assert commands("a \\\n  b  c  # note\n# skip\n\nd\n") == ["a b c", "d"]


def test_install_commands_match_doc_in_order():
    doc = doc_section_commands("Pinned CPU install")
    script = script_commands()
    assert len(doc) == 5
    start = script.index(doc[0])
    assert script[start:start + len(doc)] == doc


def test_verify_commands_match_doc_in_order():
    doc = doc_section_commands("Verify the install")
    script = script_commands()
    assert len(doc) == 4
    start = script.index(doc[0])
    assert script[start:start + len(doc)] == doc
    install_end = script.index("python -m pip check")
    assert start > install_end


def test_script_uses_a_fresh_clone_and_hashes():
    text = SCRIPT.read_text(encoding="utf-8")
    assert "set -euo pipefail" in text
    assert "git clone --quiet --no-checkout" in text
    assert 'git checkout --quiet --detach "$COMMIT"' in text
    assert "mktemp -d" in text
    assert "--require-hashes" in text
    assert SCRIPT.stat().st_mode & 0o111, "script must be executable"


def test_quality_gate_runs_the_script_after_python_setup():
    workflow = yaml.safe_load(WORKFLOW.read_text(encoding="utf-8"))
    (job,) = workflow["jobs"].values()
    names = [s.get("name") for s in job["steps"]]
    assert names.count(STEP) == 1
    step = job["steps"][names.index(STEP)]
    assert step["run"].strip() == "scripts/fresh_clone_install.sh"
    assert step["env"]["PYTHON"] == "python3.12"
    assert names.index(STEP) > names.index("Set up Python 3.12")
    assert names.index(STEP) > names.index("Install pinned Rust toolchain (rust-toolchain.toml)")
