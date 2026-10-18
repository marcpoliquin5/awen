#!/usr/bin/env python3
"""Verify a built AWEN release directory before it is published.

Checks, in order:

1. ``SHA256SUMS`` covers every file and every checksum matches.
2. The expected artifacts exist (awenctl tarball, wheels, checkpoint, SBOMs,
   source archive, BUILD-INFO.json).
3. The awenctl binary from the tarball runs (``--help``, ``plugin keygen``)
   and compiles nothing it should not: it is executed from a temp dir.
4. All wheels install into a fresh virtual environment (``--no-index`` for
   the AWEN wheels; third-party dependencies come from the configured index
   unless ``--offline`` is given with a pre-populated environment), and:
   ``awen_native`` compiles a program with the Rust compiler and runs the
   photonic GEMM simulator; AWEN-Mini generates text on CPU and on the
   photonic simulator; the checkpoint tarball's weights hash equals the
   weights inside the awen-models wheel; awen-studio serves its bundled
   frontend.

Usage: python scripts/verify_release_artifacts.py DIST [--system-site-packages]
"""

from __future__ import annotations

import argparse
import hashlib
import io
import json
import subprocess
import sys
import tarfile
import tempfile
import venv
import zipfile
from pathlib import Path

EXPECTED_WHEELS = ("awen_native", "awen_py", "awen_models")
OPTIONAL_WHEELS = ("awen_physics", "awen_qphotonic", "awen_systems", "awen_lab", "awen_studio")


def fail(message: str) -> None:
    print(f"release verification FAILED: {message}", file=sys.stderr)
    raise SystemExit(1)


def sha256(path: Path) -> str:
    return hashlib.sha256(path.read_bytes()).hexdigest()


def check_checksums(dist: Path) -> dict[str, str]:
    sums = {}
    for line in (dist / "SHA256SUMS").read_text().splitlines():
        digest, name = line.split(maxsplit=1)
        sums[name.lstrip("*")] = digest
    files = {p.relative_to(dist).as_posix() for p in dist.rglob("*") if p.is_file() and p.name != "SHA256SUMS"}
    if files != set(sums):
        fail(f"SHA256SUMS mismatch: missing {sorted(files - set(sums))}, extra {sorted(set(sums) - files)}")
    for name, digest in sums.items():
        if sha256(dist / name) != digest:
            fail(f"checksum mismatch for {name}")
    print(f"checksums ok for {len(sums)} files")
    return sums


def wheel(dist: Path, name: str) -> Path | None:
    hits = sorted(dist.glob(f"{name}-*.whl"))
    return hits[0] if hits else None


def run(cmd: list[str], **kw) -> str:
    result = subprocess.run(cmd, capture_output=True, text=True, **kw)
    if result.returncode != 0:
        fail(f"{' '.join(map(str, cmd))} exited {result.returncode}:\n{result.stdout}\n{result.stderr}")
    return result.stdout


SMOKE = r'''
import hashlib, json, sys, zipfile
from pathlib import Path
import numpy as np
import awen_native, awen
from awen_models.checkpoint import bundled_checkpoint_dir

prog = json.loads(sys.argv[1]); caps = json.loads(sys.argv[2])
artifact = awen_native.compile_program(prog, caps)
assert artifact["backend_id"] == caps["backend_id"], artifact["backend_id"]
x = np.ones((4, 8)); w = np.ones((2, 8))
y, info = awen_native.photonic_gemm(x, w, awen_native.ideal_contract())
assert np.allclose(y, 8.0), y
cpu = awen.Runtime(device="cpu").load_model("awen-mini")
text = cpu.generate("ROMEO:", max_new_tokens=16, seed=0)
assert text.startswith("ROMEO:") and len(text) == 22
sim = awen.Runtime(device="photonic-sim").load_model("awen-mini")
assert sim.execution.fidelity == "simulated"
assert len(sim.generate("ROMEO:", max_new_tokens=2, seed=0)) == 8
weights = bundled_checkpoint_dir() / "model.safetensors"
out = {"weights_sha256": hashlib.sha256(weights.read_bytes()).hexdigest(), "sample": text}
try:
    from awen_studio.cli import default_static_dir
    static = default_static_dir()
    out["studio_static"] = bool(static and (static / "index.html").is_file())
except ImportError:
    out["studio_static"] = None
print(json.dumps(out))
'''


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("dist", type=Path)
    parser.add_argument("--system-site-packages", action="store_true",
                        help="let the test venv see already-installed third-party packages (e.g. torch)")
    args = parser.parse_args()
    dist = args.dist.resolve()
    sums = check_checksums(dist)
    info = json.loads((dist / "BUILD-INFO.json").read_text())
    version = info["version"]

    required = [f"awenctl-{version}-x86_64-unknown-linux-gnu.tar.gz", f"awen-mini-checkpoint-{version}.tar.gz",
                f"awen-{version}.tar.gz", "SBOM-SUMMARY.json", "awen-runtime.cdx.json"]
    for name in required:
        if name not in sums:
            fail(f"missing artifact {name}")
    wheels = {n: wheel(dist, n) for n in EXPECTED_WHEELS + OPTIONAL_WHEELS}
    for name in EXPECTED_WHEELS:
        if wheels[name] is None:
            fail(f"missing wheel {name}")
    print("artifacts present:", ", ".join(sorted(p.name for p in wheels.values() if p)))

    with tempfile.TemporaryDirectory() as tmp:
        tmp = Path(tmp)
        with tarfile.open(dist / required[0]) as tar:
            tar.extractall(tmp, filter="data")
        awenctl = tmp / f"awenctl-{version}-x86_64-unknown-linux-gnu" / "awenctl"
        run([str(awenctl), "--help"])
        run([str(awenctl), "plugin", "keygen", str(tmp / "k")])
        print("awenctl runs")

        with tarfile.open(dist / required[1]) as tar:
            member = tar.extractfile("awen-mini/model.safetensors")
            ckpt_sha = hashlib.sha256(member.read()).hexdigest()
        with zipfile.ZipFile(wheels["awen_models"]) as zf:
            wheel_sha = hashlib.sha256(zf.read("awen_models/checkpoints/awen-mini/model.safetensors")).hexdigest()
        if ckpt_sha != wheel_sha:
            fail("checkpoint tarball weights differ from the awen-models wheel")
        if wheels["awen_studio"] is not None:
            with zipfile.ZipFile(wheels["awen_studio"]) as zf:
                if "awen_studio/static/index.html" not in zf.namelist():
                    fail("awen-studio wheel does not bundle the frontend")

        env_dir = tmp / "venv"
        venv.create(env_dir, with_pip=True, system_site_packages=args.system_site_packages)
        py = str(env_dir / "bin" / "python")
        run([py, "-m", "pip", "install", "--quiet", "--disable-pip-version-check",
             *[str(p) for p in wheels.values() if p]])
        with tarfile.open(dist / required[2]) as tar:
            prefix = f"awen-{version}/"
            prog = tar.extractfile(prefix + "awen-compiler/examples/gemm_4x4.json").read().decode()
            caps = tar.extractfile(prefix + "awen-compiler/capabilities/reference_2x2.json").read().decode()
        result = json.loads(run([py, "-c", SMOKE, prog, caps], cwd=tmp).strip().splitlines()[-1])
        if result["weights_sha256"] != ckpt_sha:
            fail("installed checkpoint differs from the release checkpoint")
        if wheels["awen_studio"] is not None and not result["studio_static"]:
            fail("installed awen-studio cannot find its bundled frontend")
        print("installed wheels ok; sample:", json.dumps(result["sample"]))
    print(f"release {version} verified")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
