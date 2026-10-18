#!/usr/bin/env python3
"""Generate draft release notes bound to exact green CI, checksums, and build info."""

from __future__ import annotations

import argparse
import json
import re
from pathlib import Path

SCOPE = """## What this release is

AWEN is a compiler, runtime, and toolchain for photonic and hybrid
electronic-photonic AI computation. **Everything in this release runs on
conventional CPUs.** No photonic hardware was available to the project: every
photonic result is produced by AWEN's software simulator and noise model, and
every compiler cost figure is a cost-model estimate. Nothing here demonstrates
physical photonic acceleration, energy savings, or quantum advantage.

Not tested in this release: CUDA/GPU code paths (no GPU in CI or on the build
host), any physical photonic or quantum-photonic device, platforms other than
Linux x86_64. See `docs/EVIDENCE.md` for every command, test count, and
measured result, and for what is simulated, estimated, or blocked."""


def changelog_section(version: str) -> str:
    changelog = Path("CHANGELOG.md").read_text(encoding="utf-8")
    base = version.split("-", 1)[0]
    for heading in (f"## {version}", f"## {base}", "## Unreleased"):
        match = re.search(rf"^{re.escape(heading)}\b.*$", changelog, flags=re.M)
        if match:
            start = match.end()
            end = changelog.find("\n## ", start)
            return changelog[start : end if end >= 0 else None].strip()
    raise SystemExit("CHANGELOG.md has no section for this version or Unreleased")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--tag", required=True)
    parser.add_argument("--commit", required=True)
    parser.add_argument("--run-id", required=True)
    parser.add_argument("--run-url", required=True)
    parser.add_argument("--checksums", required=True, type=Path)
    parser.add_argument("--build-info", type=Path)
    parser.add_argument("--output", required=True, type=Path)
    args = parser.parse_args()

    version = args.tag.removeprefix("v")
    changes = changelog_section(version)
    checksums = args.checksums.read_text(encoding="utf-8").strip()
    build = ""
    if args.build_info:
        info = json.loads(args.build_info.read_text(encoding="utf-8"))
        rebuild = info.get("awenctl_rebuild", {})
        build = f"""
## Build

- Toolchain: {info.get('rustc')}; Python {info.get('python')}; {info.get('maturin')}; Node {info.get('node')}
- Target: `{info.get('target')}`; `SOURCE_DATE_EPOCH={info.get('source_date_epoch')}`
- `awenctl` built twice in separate target directories:
  {'identical SHA-256 (bit-for-bit reproducible on this runner)' if rebuild.get('reproducible') else 'SHA-256 values differ (not reproducible on this runner)'}
"""
    notes = f"""# AWEN {args.tag}

{SCOPE}

## Artifacts

| File | Contents |
|---|---|
| `awenctl-{version}-x86_64-unknown-linux-gnu.tar.gz` | `awenctl` CLI (compile, execute, caps, profile, plugin signing) |
| `awen_native-*.whl` | Rust compiler + photonic GEMM simulator bindings (built with maturin) |
| `awen_py-*.whl`, `awen_models-*.whl`, `awen_*-*.whl` | Python SDK, AWEN-Mini and model tools, physics, quantum-photonic, systems, Lab Agent, Studio |
| `awen-mini-checkpoint-{version}.tar.gz` | AWEN-Mini weights (safetensors), config, tokenizer, training metadata |
| `*.cdx.json`, `SBOM-SUMMARY.json` | CycloneDX SBOMs (Rust crates, Python build environment) |
| `awen-{version}.tar.gz` | source archive of the tagged commit |
| `BUILD-INFO.json`, `SHA256SUMS` | toolchains and checksums; verify with `sha256sum -c SHA256SUMS` |
{build}
## Changes

{changes}

## Immutable verification evidence

- Commit: `{args.commit}`
- Required quality-gate run: [{args.run_id}]({args.run_url})
- Required context: `AWEN required quality gate`
- Checksums:

```text
{checksums}
```

Every asset has a Sigstore-signed build-provenance attestation from the
release workflow: `gh attestation verify <file> -R marcpoliquin5/awen`.

This release is not a hardware-performance claim. Any such claim requires a
separate verified end-to-end HIL artifact under `docs/RELEASING.md`.
"""
    args.output.write_text(notes, encoding="utf-8", newline="\n")


if __name__ == "__main__":
    main()
