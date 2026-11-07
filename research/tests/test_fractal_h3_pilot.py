"""H3 pilot (Cantor/Fibonacci stacks): fast checks of the code behind the stored pilot result."""

import hashlib
import json
import sys
from pathlib import Path

import numpy as np
import pytest

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
PILOT = FRACTAL / "results" / "pilot"
sys.path.insert(0, str(FRACTAL))
import h3_stacks as stacks  # noqa: E402


def test_words():
    assert [len(stacks.fibonacci_word(g)) for g in range(8)] == [1, 1, 2, 3, 5, 8, 13, 21]
    assert stacks.fibonacci_word(4) == "ABAAB"
    assert stacks.cantor_word(1) == "ABA"
    assert stacks.cantor_word(2) == "ABABBBABA"


def test_spectrum_matches_awen_physics_tmm():
    for w in (0.4, 1.0, 1.7):
        ours, ref = stacks.tmm_crosscheck(stacks.fibonacci_word(6), 2.3, 1.38, w)
        assert ours == pytest.approx(ref, abs=1e-12)


def test_trace_map_recursion_and_invariant():
    omega = np.linspace(0.05, 1.95, 401)
    x = stacks.trace_map(2.3, 1.38, omega, 10)
    assert np.allclose(x[3:], 2 * x[2:-1] * x[1:-2] - x[:-3], atol=1e-8 * (1 + np.abs(x[3:])))
    inv = stacks.trace_invariant(x)
    ok = np.max(np.abs(x), axis=0) < 1e3
    assert np.max(np.abs(inv - inv[0])[:, ok]) < 1e-7
    q = stacks.trace_invariant(stacks.trace_map(2.3, 1.38, np.array([1.0]), 6))[0, 0]
    assert q == pytest.approx(0.25 * (2.3 / 1.38 - 1.38 / 2.3) ** 2)


def test_quarter_wave_transmission_six_cycle():
    t = [float(stacks.spectrum(stacks.fibonacci_word(g), 2.3, 1.38, np.array([1.0]))[0]) for g in range(2, 16)]
    assert np.allclose(t[:-6], t[6:], atol=1e-10)




def test_pilot_result_is_stored_with_its_hash():
    meta = json.loads((PILOT / "H3_PILOT.json").read_text())
    assert "not evidence" in meta["note"] and meta["source"]["commit"] == "0974f38"
    for name, digest in meta["sha256"].items():
        assert hashlib.sha256((PILOT / name).read_bytes()).hexdigest() == digest, name
    assert json.loads((PILOT / "stacks.json").read_text())["study"] == "awen.fractal-stacks.v1"
