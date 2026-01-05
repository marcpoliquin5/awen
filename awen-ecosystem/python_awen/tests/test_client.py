import os
import sys
import json

sys.path.insert(0, os.path.abspath(os.path.join(os.path.dirname(__file__), '..')))

from awen_py.client import compute_gradients, run_ir


def test_run_and_gradient_smoke():
    # This is a smoke test that assumes `awenctl` is available in PATH in the test environment.
    ir = os.path.abspath(os.path.join(os.path.dirname(__file__), '..', 'example_ir.json'))
    # run
    files = run_ir(ir, seed=42)
    assert files.get('results.json') is not None

    # gradient
    res = compute_gradients(ir, ['mzi_0:phase', 'mzi_1:phase'], seed=42, samples=1)
    assert 'gradients' in res
