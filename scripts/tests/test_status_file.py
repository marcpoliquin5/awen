"""Checks for STATUS.md (run: python -m pytest scripts/tests).

Master Prompt v2 section 3 keeps STATUS.md under ~150 lines and moves history to docs/history/.
These tests keep that limit, check that every relative link in STATUS.md resolves to a file in the
repository, and that every docs/history/ file STATUS.md links to says what it was moved from.
"""

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[2]
STATUS = ROOT / "STATUS.md"
MAX_LINES = 150
LINK = re.compile(r"\]\(([^)\s]+)\)")


def relative_links(text):
    """Return the relative link targets in markdown text, without #anchors."""
    targets = []
    for target in LINK.findall(text):
        if re.match(r"^[a-z]+:", target) or target.startswith("#"):
            continue
        targets.append(target.split("#", 1)[0])
    return targets


def test_status_is_under_line_limit():
    lines = STATUS.read_text(encoding="utf-8").splitlines()
    assert len(lines) <= MAX_LINES, f"STATUS.md has {len(lines)} lines; move history to docs/history/"


def test_status_relative_links_resolve():
    text = STATUS.read_text(encoding="utf-8")
    missing = [t for t in relative_links(text) if not (ROOT / t).is_file()]
    assert missing == []


def test_status_links_history_and_history_names_its_source():
    text = STATUS.read_text(encoding="utf-8")
    history = sorted({t for t in relative_links(text) if t.startswith("docs/history/")})
    assert history, "STATUS.md should link the docs/history/ file that holds its moved text"
    for target in history:
        body = (ROOT / target).read_text(encoding="utf-8")
        assert "Moved verbatim from `STATUS.md` at main `" in body, target


def test_relative_links_parser():
    sample = "[a](docs/x.md) [b](https://example.org/y) [c](#anchor) [d](docs/z.md#sec)"
    assert relative_links(sample) == ["docs/x.md", "docs/z.md"]
