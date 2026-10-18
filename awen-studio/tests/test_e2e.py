"""End-to-end smoke test over real HTTP: the built frontend and the API served by uvicorn.

Needs the frontend build (``npm ci && npm run build`` in ``awen-studio/frontend``).
CI sets ``AWEN_STUDIO_REQUIRE_FRONTEND=1`` so a missing build fails instead of skipping.
This checks serving and wiring, not browser rendering.
"""

import json
import os
import re
import socket
import threading
import time
import urllib.request
from pathlib import Path

import pytest

pytest.importorskip("awen_native")
pytest.importorskip("awen_lab")
pytest.importorskip("awen_models")
uvicorn = pytest.importorskip("uvicorn")

from awen_studio import create_app  # noqa: E402

DIST = Path(__file__).resolve().parents[1] / "frontend" / "dist"
PANELS = ("Projects", "Model", "Compiler", "Ops", "Noise", "Compare", "Experiments", "Report")


def _free_port() -> int:
    with socket.socket() as s:
        s.bind(("127.0.0.1", 0))
        return s.getsockname()[1]


def _get(url: str) -> str:
    with urllib.request.urlopen(url, timeout=30) as r:
        return r.read().decode()


def _post(url: str, body: dict) -> dict:
    req = urllib.request.Request(url, data=json.dumps(body).encode(), headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=60) as r:
        return json.loads(r.read())


@pytest.fixture
def server(tmp_path):
    if not (DIST / "index.html").is_file():
        if os.environ.get("AWEN_STUDIO_REQUIRE_FRONTEND") == "1":
            pytest.fail(f"frontend build missing at {DIST}")
        pytest.skip("frontend not built")
    port = _free_port()
    config = uvicorn.Config(create_app(tmp_path / "ws", static_dir=DIST), host="127.0.0.1", port=port,
                            log_level="warning")
    srv = uvicorn.Server(config)
    thread = threading.Thread(target=srv.run, daemon=True)
    thread.start()
    for _ in range(100):
        if srv.started:
            break
        time.sleep(0.05)
    yield f"http://127.0.0.1:{port}"
    srv.should_exit = True
    thread.join(timeout=10)


def test_frontend_and_api_end_to_end(server):
    index = _get(server + "/")
    assert "<title>AWEN Studio</title>" in index
    script = re.search(r'src="(/assets/[^"]+\.js)"', index).group(1)
    bundle = _get(server + script)
    for panel in PANELS:
        assert f'"{panel}"' in bundle, panel
    for endpoint in ("/api/projects", "/api/models/generate", "/api/compare/gemm", "/api/noise/validate", "/api/ops"):
        assert endpoint in bundle, endpoint

    _post(server + "/api/projects", {"name": "e2e", "example": "gemm-4x4"})
    assert _post(server + "/api/projects/e2e/compile", {})["ok"]
    gen = _post(server + "/api/models/generate", {"prompt": "KING:", "max_new_tokens": 5, "device": "photonic-sim"})
    assert gen["execution"]["fidelity"] == "simulated" and len(gen["completion"]) == 5
    _post(server + "/api/projects/e2e/experiments", {"workflow": "quick", "experiment": {
        "kind": "photonic_gemm", "target": "photonic-sim", "shape_mkn": [8, 32, 4], "seeds": [1]}})
    md = _get(server + "/api/projects/e2e/report.md")
    assert "# AWEN Studio report: e2e" in md and "quick" in md
