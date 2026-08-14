#!/usr/bin/env python3
"""Generate draft release notes bound to exact green CI and checksums."""

from __future__ import annotations

import argparse
from pathlib import Path


parser = argparse.ArgumentParser()
parser.add_argument("--tag", required=True)
parser.add_argument("--commit", required=True)
parser.add_argument("--run-id", required=True)
parser.add_argument("--run-url", required=True)
parser.add_argument("--checksums", required=True, type=Path)
parser.add_argument("--output", required=True, type=Path)
args = parser.parse_args()

changelog = Path("CHANGELOG.md").read_text(encoding="utf-8")
start = changelog.index("## Unreleased") + len("## Unreleased")
end = changelog.find("\n## ", start)
changes = changelog[start : end if end >= 0 else None].strip()
checksums = args.checksums.read_text(encoding="utf-8").strip()

notes = f"""# AWEN {args.tag}

## Changes

{changes}

## Immutable verification evidence

- Commit: `{args.commit}`
- Required quality-gate run: [{args.run_id}]({args.run_url})
- Required context: `AWEN required quality gate`
- Source archive checksums:

```text
{checksums}
```

This draft is not a hardware-performance claim. Any such claim requires a
separate verified end-to-end HIL artifact under `docs/RELEASING.md`.
"""
args.output.write_text(notes, encoding="utf-8", newline="\n")
