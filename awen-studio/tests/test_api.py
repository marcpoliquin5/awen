"""AWEN Studio API: every panel's endpoint against the real backends."""

import pytest

pytest.importorskip("awen_native")
pytest.importorskip("awen_lab")
pytest.importorskip("awen_models")
from fastapi.testclient import TestClient  # noqa: E402

from awen_studio import create_app  # noqa: E402
from awen_studio.cli import main as cli_main  # noqa: E402

GEMM = {"kind": "photonic_gemm", "target": "photonic-sim", "shape_mkn": [16, 64, 8],
        "contract": {"photons_per_mac": 1000.0}, "seeds": [1, 2]}


@pytest.fixture
def client(tmp_path):
    return TestClient(create_app(tmp_path / "ws"))


@pytest.fixture
def project(client):
    r = client.post("/api/projects", json={"name": "demo", "example": "gemm-4x4", "description": "test"})
    assert r.status_code == 201, r.text
    return "demo"


def test_health_reports_versions_and_no_hardware(client):
    body = client.get("/api/health").json()
    assert body["ok"] and body["versions"]["python"]
    assert body["hardware"].startswith("none")


def test_project_list_create_get_and_validation(client, project):
    listing = client.get("/api/projects").json()
    assert [p["name"] for p in listing["projects"]] == ["demo"]
    assert {e["id"] for e in listing["examples"]} == {"gemm-4x4", "gemm-256"}
    doc = client.get("/api/projects/demo").json()
    assert doc["program"]["ir_version"] == "awen.tensor.v1" and doc["compile_report"] is None
    assert client.post("/api/projects", json={"name": "demo"}).status_code == 400
    assert client.post("/api/projects", json={"name": "../etc"}).status_code == 400
    assert client.post("/api/projects", json={"name": "x", "example": "nope"}).status_code == 400
    assert client.get("/api/projects/missing").status_code == 404


def test_compile_project_runs_the_rust_compiler_and_persists(client, project):
    report = client.post(f"/api/projects/{project}/compile").json()
    assert report["ok"] and report["backend_id"] == "reference-photonic-2x2"
    assert report["placement"][0]["op_id"] == "test_projection"
    assert report["placement"][0]["label"] == "estimated"
    assert report["diagnostics"] and report["capability_negotiations"]
    assert client.get(f"/api/projects/{project}").json()["compile_report"]["ok"]


def test_compile_reports_errors_as_diagnostics(client):
    r = client.post("/api/compile", json={"program": {"bad": 1}, "capabilities": {"also": "bad"}}).json()
    assert r["ok"] is False and r["diagnostics"]


def test_op_table(client):
    body = client.get("/api/ops").json()
    names = {row["name"] for row in body["ops"]}
    assert "gemm" in names and len(names) >= 12
    assert sum(body["levels"].values()) == len(body["ops"])


def test_noise_contract_default_validate_and_reject(client):
    default = client.get("/api/noise/default").json()
    assert default["default"]["version"] == "awen.photonic-noise-contract.v1"
    ok = client.post("/api/noise/validate", json={"contract": {"photons_per_mac": 5000.0}, "k": 128}).json()
    assert ok["contract"]["photons_per_mac"] == 5000.0 and ok["predicted"]
    bad = client.post("/api/noise/validate", json={"contract": {"photons_per_mac": -1.0}})
    assert bad.status_code == 422


def test_gemm_compare_ideal_is_exact_and_noise_is_nonzero(client, tmp_path):
    import awen_native

    ideal = client.post("/api/compare/gemm", json={"m": 8, "k": 32, "n": 4,
                                                   "contract": awen_native.ideal_contract()}).json()
    assert ideal["error"]["max_abs"] < 1e-9 and ideal["photonic_sim"]["label"] == "simulated"
    noisy = client.post("/api/compare/gemm", json={"m": 8, "k": 32, "n": 4, "seed": 3}).json()
    assert noisy["error"]["relative_rms"] > 0 and sum(noisy["error"]["histogram"]["counts"]) == 32
    assert client.post("/api/compare/gemm", json={"m": 100000}).status_code == 422


def test_models_and_generate_on_cpu_and_photonic_sim(client):
    models = client.get("/api/models").json()["models"]
    assert models[0]["id"] == "awen-mini" and len(models[0]["weights_sha256"]) == 64
    cpu = client.post("/api/models/generate", json={"prompt": "ROMEO:", "max_new_tokens": 8, "seed": 1}).json()
    assert cpu["text"].startswith("ROMEO:") and len(cpu["completion"]) == 8
    assert cpu["execution"]["fidelity"] == "measured"
    again = client.post("/api/models/generate", json={"prompt": "ROMEO:", "max_new_tokens": 8, "seed": 1}).json()
    assert again["text"] == cpu["text"]
    sim = client.post("/api/models/generate", json={"device": "photonic-sim", "prompt": "ROMEO:",
                                                    "max_new_tokens": 3, "seed": 1}).json()
    assert sim["execution"]["fidelity"] == "simulated" and len(sim["completion"]) == 3
    assert client.post("/api/models/generate", json={"device": "quantum"}).status_code == 422


def test_likelihood_cpu_vs_photonic_sim(client):
    import awen_native

    body = client.post("/api/models/likelihood", json={"text": "To be, or not to be.",
                                                       "noise": awen_native.ideal_contract()}).json()
    assert abs(body["delta_nats"]) < 1e-4


def test_experiments_history_and_hardware_refusal(client, project):
    assert client.get(f"/api/projects/{project}/experiments").json()["history"] == []
    quick = client.post(f"/api/projects/{project}/experiments", json={"workflow": "quick", "experiment": GEMM})
    assert quick.status_code == 201, quick.text
    truth = client.post(f"/api/projects/{project}/experiments", json={"workflow": "truth", "experiment": GEMM}).json()
    assert truth["metrics"]["checks"]
    repro = client.post(f"/api/projects/{project}/experiments",
                        json={"workflow": "repro", "ref": quick.json()["hash"]}).json()
    assert repro["metrics"]["reproduced"] is True and repro["hash"].startswith("sha256:")
    hist = client.get(f"/api/projects/{project}/experiments").json()
    assert sorted(h["workflow"] for h in hist["history"]) == ["quick", "repro", "truth"]
    assert hist["branches"]["main"] == repro["hash"]
    assert next(h for h in hist["history"] if h["hash"] == repro["hash"])["on_main"]
    assert hist["verified_states"] == 3
    hw = client.post(f"/api/projects/{project}/experiments",
                     json={"workflow": "quick", "experiment": {**GEMM, "target": "real-chip"}})
    assert hw.status_code == 403


def test_report_export_json_and_markdown(client, project):
    client.post(f"/api/projects/{project}/compile")
    client.post(f"/api/projects/{project}/experiments", json={"workflow": "quick", "experiment": GEMM})
    report = client.get(f"/api/projects/{project}/report").json()
    assert report["version"] == "awen.studio-report.v1"
    assert report["compile_report"]["ok"] and len(report["experiments"]) == 1
    assert "no photonic hardware" in report["statement"]
    md = client.get(f"/api/projects/{project}/report.md").text
    assert md.startswith("# AWEN Studio report: demo") and "test_projection" in md


def test_static_frontend_is_served_when_built(tmp_path):
    static = tmp_path / "dist"
    (static / "assets").mkdir(parents=True)
    (static / "index.html").write_text("<!doctype html><title>AWEN Studio</title>")
    client = TestClient(create_app(tmp_path / "ws", static_dir=static))
    assert "AWEN Studio" in client.get("/").text


def test_cli_refuses_non_loopback_without_flag():
    with pytest.raises(SystemExit):
        cli_main(["--host", "0.0.0.0"])
