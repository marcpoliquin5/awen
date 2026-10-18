"""Box-counting pilot (exploratory, not an H1-H4 test): fast checks of the code behind the stored result."""

import hashlib
import json
import sys
from pathlib import Path

import numpy as np
import pytest

FRACTAL = Path(__file__).resolve().parents[1] / "experiments" / "multiplexing-fractal"
PILOT = FRACTAL / "results" / "pilot"
sys.path.insert(0, str(FRACTAL))
import x_boxcount as boxcount  # noqa: E402


def test_graph_dimension_of_smooth_curve_is_one():
    a = np.linspace(-1, 1, 1025)
    assert boxcount.graph_dimension(a, np.sin(3 * a) + a ** 2, [4, 8, 16, 32, 64, 128]) == pytest.approx(1.0, abs=0.08)


def test_hill_estimator_recovers_pareto_tail():
    rng = np.random.default_rng(0)
    # density ~ x^-alpha with alpha = 3  <=>  Pareto shape a = alpha - 1 = 2
    sample = rng.pareto(2.0, 200000) + 1
    assert boxcount.hill_tail_exponent(sample, 0.1) == pytest.approx(3.0, rel=0.05)


def test_pilot_result_is_stored_with_its_hash():
    meta = json.loads((PILOT / "BOXCOUNT_PILOT.json").read_text())
    assert "not evidence" in meta["note"] and meta["source"]["commit"] == "0974f38"
    for name, digest in meta["sha256"].items():
        assert hashlib.sha256((PILOT / name).read_bytes()).hexdigest() == digest, name
    assert json.loads((PILOT / "boxcount.json").read_text())["study"] == "awen.fractal-boxcount.v1"
