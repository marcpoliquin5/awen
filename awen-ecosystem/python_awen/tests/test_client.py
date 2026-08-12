import inspect
import subprocess

import numpy as np

import awen_py
from awen_py import client


def test_normal_public_api_has_no_subprocess_or_temporary_file_dependency(monkeypatch):
    def forbidden(*args, **kwargs):
        raise AssertionError("normal execution attempted a subprocess")

    monkeypatch.setattr(subprocess, "run", forbidden)
    result = awen_py.gemm(np.eye(2), np.array([[2.0, 3.0], [4.0, 5.0]]))
    np.testing.assert_array_equal(result, [[2.0, 3.0], [4.0, 5.0]])
    assert "client" not in inspect.getsource(awen_py.gemm)
    assert "compute_gradients" not in awen_py.__all__
    assert "run_ir" not in awen_py.__all__


def test_legacy_artifact_bridge_is_explicitly_debug_only():
    assert client.compute_gradients is client.compute_gradients_cli_debug
    assert client.run_ir is client.run_ir_cli_debug
