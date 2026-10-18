"""Tests for the release scripts (checksum verification and release notes)."""

import hashlib
import importlib.util
import subprocess
import sys
from pathlib import Path

import pytest

SCRIPTS = Path(__file__).resolve().parents[1]


def load(name):
    spec = importlib.util.spec_from_file_location(name, SCRIPTS / f"{name}.py")
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


verify = load("verify_release_artifacts")


def write_dist(tmp_path):
    dist = tmp_path / "dist"
    dist.mkdir()
    (dist / "a.txt").write_text("alpha")
    (dist / "b.bin").write_bytes(b"\x00\x01")
    lines = [f"{hashlib.sha256((dist / n).read_bytes()).hexdigest()}  {n}" for n in ("a.txt", "b.bin")]
    (dist / "SHA256SUMS").write_text("\n".join(lines) + "\n")
    return dist


def test_checksums_pass(tmp_path):
    assert set(verify.check_checksums(write_dist(tmp_path))) == {"a.txt", "b.bin"}


def test_tampered_file_fails(tmp_path):
    dist = write_dist(tmp_path)
    (dist / "a.txt").write_text("tampered")
    with pytest.raises(SystemExit):
        verify.check_checksums(dist)


def test_unlisted_file_fails(tmp_path):
    dist = write_dist(tmp_path)
    (dist / "extra.whl").write_text("x")
    with pytest.raises(SystemExit):
        verify.check_checksums(dist)


def test_release_notes_are_honest_and_bound_to_checksums(tmp_path):
    dist = write_dist(tmp_path)
    out = tmp_path / "notes.md"
    subprocess.run([sys.executable, str(SCRIPTS / "build_release_evidence.py"), "--tag", "v0.1.0-rc.1",
                    "--commit", "deadbeef", "--run-id", "7", "--run-url", "https://example.invalid/run/7",
                    "--checksums", str(dist / "SHA256SUMS"), "--output", str(out)],
                   check=True, cwd=SCRIPTS.parent)
    notes = out.read_text()
    assert "Everything in this release runs on" in notes and "conventional CPUs" in notes
    assert "No photonic hardware" in notes and "quantum advantage" in notes
    assert "deadbeef" in notes and (dist / "SHA256SUMS").read_text().split()[0] in notes
