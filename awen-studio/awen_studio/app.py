"""FastAPI backend for AWEN Studio.

Every endpoint calls a real AWEN backend: ``awen_native`` (Rust compiler,
operator table, photonic noise model), ``awen``/``awen_models`` (AWEN-Mini on
CPU or the photonic simulator) and ``awen_lab`` (experiment history). Nothing
here fabricates results: every number in a response comes from one of those
calls, and every photonic number is labelled ``simulated``. Compiler cost
figures are labelled ``estimated`` (cost-model outputs, not measurements).

The app is meant for ``127.0.0.1``. It has no authentication and must not be
exposed on a network interface.
"""

from __future__ import annotations

import hashlib
import json
import math
import platform
import time
from datetime import datetime, timezone
from pathlib import Path
from typing import Any, Optional

import numpy as np
from fastapi import FastAPI, HTTPException
from fastapi.responses import FileResponse, PlainTextResponse
from fastapi.staticfiles import StaticFiles
from pydantic import BaseModel, Field

from .workspace import ProjectError, Workspace

MAX_GEMM_DIM = 512
MAX_NEW_TOKENS = 400
LABELS = {
    "measured": "measured on this machine",
    "simulated": "computed by the AWEN photonic simulator (software); not a hardware measurement",
    "estimated": "compiler cost-model estimate; not a measurement",
}


# -- request models ----------------------------------------------------------
class ProjectCreate(BaseModel):
    name: str
    example: str = "gemm-4x4"
    description: str = ""


class GenerateRequest(BaseModel):
    model: str = "awen-mini"
    device: str = Field("cpu", pattern="^(cpu|photonic-sim)$")
    prompt: str = Field("ROMEO:", min_length=1, max_length=2000)
    max_new_tokens: int = Field(120, ge=1, le=MAX_NEW_TOKENS)
    temperature: float = Field(0.8, ge=0.0, le=2.0)
    top_k: Optional[int] = Field(40, ge=1, le=65)
    seed: int = 0
    noise: Optional[dict[str, Any]] = None


class LikelihoodRequest(BaseModel):
    text: str = Field(..., min_length=2, max_length=4000)
    noise: Optional[dict[str, Any]] = None
    seed: int = 0


class NoiseRequest(BaseModel):
    contract: dict[str, Any]
    k: int = Field(64, ge=1, le=4096)
    signal: float = Field(1.0, gt=0)


class CompareRequest(BaseModel):
    m: int = Field(32, ge=1, le=MAX_GEMM_DIM)
    k: int = Field(128, ge=1, le=MAX_GEMM_DIM)
    n: int = Field(16, ge=1, le=MAX_GEMM_DIM)
    contract: Optional[dict[str, Any]] = None
    tile_k: int = Field(64, ge=1, le=MAX_GEMM_DIM)
    seed: int = 0
    input_seed: int = 0


class CompileRequest(BaseModel):
    program: Optional[dict[str, Any]] = None
    capabilities: Optional[dict[str, Any]] = None


class ExperimentRequest(BaseModel):
    workflow: str = Field("quick", pattern="^(quick|truth|repro)$")
    experiment: Optional[dict[str, Any]] = None
    ref: Optional[str] = None


# -- helpers -----------------------------------------------------------------
def _native():
    try:
        import awen_native
    except ImportError as error:  # pragma: no cover - environment dependent
        raise HTTPException(503, "awen_native is not installed (pip install ./awen-native)") from error
    return awen_native


def _contract(contract: Optional[dict[str, Any]]) -> dict[str, Any]:
    native = _native()
    base = native.default_contract()
    if contract:
        base.update(contract)
    try:
        return native.validate_contract(base)
    except ValueError as error:
        raise HTTPException(422, f"invalid noise contract: {error}") from error


def _hash(obj: Any) -> str:
    return "sha256:" + hashlib.sha256(json.dumps(obj, sort_keys=True).encode()).hexdigest()


def _summarize_compile(artifact: dict[str, Any]) -> dict[str, Any]:
    placement = []
    for entry in artifact.get("placement", []):
        backend = entry.get("selected_backend")
        chosen = entry.get(backend, {}) if isinstance(entry.get(backend), dict) else {}
        placement.append({
            "op_id": entry.get("op_id"),
            "selected_backend": backend,
            "objective": entry.get("objective"),
            "latency_ns": chosen.get("latency_ns"),
            "energy_uj": chosen.get("energy_uj"),
            "effective_bits": chosen.get("effective_bits"),
            "estimated_error_fraction": chosen.get("estimated_error_fraction"),
            "label": "estimated",
        })
    return {
        "ok": True,
        "backend_id": artifact.get("backend_id"),
        "artifact_version": artifact.get("artifact_version"),
        "source_graph_fingerprint": artifact.get("source_graph_fingerprint"),
        "capability_negotiations": artifact.get("capability_negotiations", []),
        "placement": placement,
        "diagnostics": artifact.get("diagnostics", []),
        "health": artifact.get("health_snapshot"),
        "labels": {"placement": LABELS["estimated"]},
    }


def _capability_summary(caps: dict[str, Any]) -> dict[str, Any]:
    keys = ("capability_version", "backend_id", "backend_kind", "matrix_rows", "matrix_cols",
            "supported_ops", "precision", "max_effective_bits")
    summary = {k: caps[k] for k in keys if k in caps}
    summary["fields"] = sorted(caps)
    return summary


def _versions() -> dict[str, Optional[str]]:
    from importlib import metadata

    out: dict[str, Optional[str]] = {"python": platform.python_version()}
    for dist in ("awen-native", "awen-models", "awen-lab", "awen-studio", "torch", "numpy", "fastapi"):
        try:
            out[dist] = metadata.version(dist)
        except metadata.PackageNotFoundError:
            out[dist] = None
    return out


def create_app(workspace: str | Path, static_dir: Optional[str | Path] = None) -> FastAPI:
    ws = Workspace(workspace)
    app = FastAPI(title="AWEN Studio", version="0.1.0",
                  description="Local AWEN Studio API. Simulation only; no hardware path.")
    model_cache: dict[tuple, Any] = {}

    def project_or_404(name: str) -> Path:
        try:
            return ws.project_dir(name)
        except ProjectError as error:
            raise HTTPException(404, str(error)) from error

    def loaded_model(model: str, device: str, noise: Optional[dict[str, Any]], seed: int):
        from awen import Runtime

        key = (model, device, _hash(noise) if noise else None, seed if device == "photonic-sim" else 0)
        if key not in model_cache:
            if len(model_cache) > 8:
                model_cache.clear()
            try:
                model_cache[key] = Runtime(device=device, noise=noise, seed=seed).load_model(model)
            except FileNotFoundError as error:
                raise HTTPException(404, str(error)) from error
        return model_cache[key]

    # -- system --------------------------------------------------------------
    @app.get("/api/health")
    def health() -> dict[str, Any]:
        return {"ok": True, "workspace": str(ws.root), "versions": _versions(),
                "hardware": "none: every photonic path is the software simulator", "labels": LABELS}

    # -- projects ------------------------------------------------------------
    @app.get("/api/projects")
    def list_projects() -> dict[str, Any]:
        return {"projects": ws.list(), "examples": ws.examples()}

    @app.post("/api/projects", status_code=201)
    def create_project(body: ProjectCreate) -> dict[str, Any]:
        try:
            return ws.create(body.name, body.example, body.description)
        except ProjectError as error:
            raise HTTPException(400, str(error)) from error

    @app.get("/api/projects/{name}")
    def get_project(name: str) -> dict[str, Any]:
        project_or_404(name)
        return ws.get(name)

    # -- compiler ------------------------------------------------------------
    def run_compile(program: dict[str, Any], caps: dict[str, Any]) -> dict[str, Any]:
        native = _native()
        start = time.perf_counter()
        try:
            artifact = native.compile_program(program, caps)
        except (ValueError, RuntimeError) as error:
            return {"ok": False, "error": str(error), "diagnostics": [str(error)],
                    "capabilities": _capability_summary(caps)}
        summary = _summarize_compile(artifact)
        summary["compile_seconds"] = time.perf_counter() - start
        summary["capabilities"] = _capability_summary(caps)
        return summary

    @app.post("/api/projects/{name}/compile")
    def compile_project(name: str) -> dict[str, Any]:
        project_or_404(name)
        doc = ws.get(name)
        report = run_compile(doc["program"], doc["capabilities"])
        report["compiled_at"] = datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ")
        ws.save_compile_report(name, report)
        return report

    @app.post("/api/compile")
    def compile_documents(body: CompileRequest) -> dict[str, Any]:
        if body.program is None or body.capabilities is None:
            raise HTTPException(422, "program and capabilities are both required")
        return run_compile(body.program, body.capabilities)

    @app.get("/api/ops")
    def ops() -> dict[str, Any]:
        table = _native().op_table()
        levels: dict[str, int] = {}
        for row in table:
            levels[row["level"]] = levels.get(row["level"], 0) + 1
        return {"ops": table, "levels": levels}

    # -- noise contract ------------------------------------------------------
    @app.get("/api/noise/default")
    def noise_default() -> dict[str, Any]:
        native = _native()
        return {"default": native.default_contract(), "ideal": native.ideal_contract(),
                "note": "The default contract is an illustrative operating point, not a measured device."}

    @app.post("/api/noise/validate")
    def noise_validate(body: NoiseRequest) -> dict[str, Any]:
        native = _native()
        contract = _contract(body.contract)
        predicted = native.predicted_noise(body.k, signal=body.signal, contract=contract)
        return {"contract": contract, "predicted": predicted, "k": body.k, "label": "simulated"}

    # -- photonic-sim vs CPU -------------------------------------------------
    @app.post("/api/compare/gemm")
    def compare_gemm(body: CompareRequest) -> dict[str, Any]:
        native = _native()
        contract = _contract(body.contract)
        rng = np.random.default_rng(body.input_seed)
        x = rng.uniform(-1.0, 1.0, (body.m, body.k))
        w = rng.uniform(-1.0, 1.0, (body.n, body.k))
        t0 = time.perf_counter()
        cpu = x @ w.T
        cpu_s = time.perf_counter() - t0
        t0 = time.perf_counter()
        try:
            sim, info = native.photonic_gemm(x, w, contract, tile_k=body.tile_k, seed=body.seed)
        except ValueError as error:
            raise HTTPException(422, str(error)) from error
        sim_s = time.perf_counter() - t0
        err = np.asarray(sim) - cpu
        rms = float(np.sqrt(np.mean(err ** 2)))
        ref = float(np.sqrt(np.mean(cpu ** 2)))
        rel = rms / ref if ref > 0 else 0.0
        bits = native.effective_bits_from_sigma(rel) if rel > 0 else None
        hist, edges = np.histogram(err, bins=24)
        return {
            "shape_mkn": [body.m, body.k, body.n],
            "contract": contract,
            "cpu": {"label": "measured", "seconds": cpu_s},
            "photonic_sim": {"label": "simulated", "seconds": sim_s, "info": info},
            "error": {"rms": rms, "relative_rms": rel, "max_abs": float(np.max(np.abs(err))),
                      "effective_bits": None if bits is None or not math.isfinite(bits) else float(bits),
                      "histogram": {"counts": hist.tolist(), "edges": edges.tolist()}},
            "sample": {"cpu": cpu[:4, :4].tolist(), "photonic_sim": np.asarray(sim)[:4, :4].tolist()},
            "labels": {"seconds": "host wall-clock of the software paths; says nothing about photonic hardware speed"},
        }

    # -- models --------------------------------------------------------------
    @app.get("/api/models")
    def models() -> dict[str, Any]:
        try:
            from awen_models.checkpoint import bundled_checkpoint_dir
        except ImportError as error:
            raise HTTPException(503, "awen-models is not installed") from error
        path = bundled_checkpoint_dir("awen-mini")
        meta = json.loads((path / "metadata.json").read_text())
        weights = path / "model.safetensors"
        return {"models": [{
            "id": "awen-mini",
            "parameters": meta.get("parameters"),
            "config": meta.get("config"),
            "dataset": meta.get("dataset", {}).get("name"),
            "final": meta.get("final"),
            "hardware": meta.get("hardware"),
            "evidence": meta.get("evidence"),
            "weights_sha256": hashlib.sha256(weights.read_bytes()).hexdigest(),
            "devices": ["cpu", "photonic-sim"],
        }]}

    @app.post("/api/models/generate")
    def generate(body: GenerateRequest) -> dict[str, Any]:
        noise = _contract(body.noise) if (body.device == "photonic-sim" and body.noise) else None
        model = loaded_model(body.model, body.device, noise, body.seed)
        t0 = time.perf_counter()
        try:
            text = model.generate(body.prompt, max_new_tokens=body.max_new_tokens, temperature=body.temperature,
                                  top_k=body.top_k, seed=body.seed)
        except (ValueError, KeyError) as error:
            raise HTTPException(422, f"generation failed: {error}") from error
        seconds = time.perf_counter() - t0
        info = model.execution
        return {"text": text, "completion": text[len(body.prompt):], "seconds": seconds,
                "tokens_per_second": body.max_new_tokens / seconds if seconds > 0 else None,
                "execution": {"device": info.device, "fidelity": info.fidelity, "gemm_backend": info.gemm_backend,
                              "notes": list(info.notes)}}

    @app.post("/api/models/likelihood")
    def likelihood(body: LikelihoodRequest) -> dict[str, Any]:
        noise = _contract(body.noise) if body.noise else None
        try:
            cpu = loaded_model("awen-mini", "cpu", None, 0).log_likelihood(body.text)
            sim = loaded_model("awen-mini", "photonic-sim", noise, body.seed).log_likelihood(body.text)
        except (ValueError, KeyError) as error:
            raise HTTPException(422, f"text contains characters outside the model vocabulary: {error}") from error
        return {"cpu_nats": cpu, "photonic_sim_nats": sim, "delta_nats": sim - cpu,
                "labels": {"cpu_nats": "measured", "photonic_sim_nats": "simulated"}}

    # -- experiments (Lab Agent) ---------------------------------------------
    def agent_for(name: str):
        from awen_lab import LabAgent

        project_or_404(name)
        return LabAgent(ws.lab_store(name))

    @app.get("/api/projects/{name}/experiments")
    def experiments(name: str) -> dict[str, Any]:
        agent = agent_for(name)
        branches = agent.store.branches()
        head = branches.get("main")
        # Every stored state (not only the first-parent chain: repro and compare
        # states point at other parents), newest first.
        digests = ["sha256:" + p.stem for p in (agent.store.root / "objects").glob("*.json")]
        history = []
        for digest in digests:
            state = agent.store.get(digest).to_dict()
            history.append({"hash": digest, "workflow": state["workflow"], "parents": state["parents"],
                            "created_at": state["provenance"]["created_at"], "experiment": state["experiment"],
                            "metrics": state["metrics"], "on_main": False})
        main_chain = set(agent.store.history(head)) if head else set()
        for item in history:
            item["on_main"] = item["hash"] in main_chain
        history.sort(key=lambda item: (item["created_at"], item["on_main"]), reverse=True)
        return {"branches": branches, "history": history, "verified_states": agent.store.verify_all()}

    @app.post("/api/projects/{name}/experiments", status_code=201)
    def run_experiment(name: str, body: ExperimentRequest) -> dict[str, Any]:
        from awen_lab import BoundsViolation, HardwareActionRefused

        agent = agent_for(name)
        try:
            if body.workflow == "repro":
                if not body.ref:
                    raise HTTPException(422, "repro needs ref")
                state = agent.repro(body.ref)
            else:
                if body.experiment is None:
                    raise HTTPException(422, f"{body.workflow} needs an experiment")
                state = getattr(agent, body.workflow)(body.experiment)
        except (HardwareActionRefused, BoundsViolation) as error:
            raise HTTPException(403, f"{type(error).__name__}: {error}") from error
        except (ValueError, KeyError) as error:
            raise HTTPException(422, str(error)) from error
        return {"hash": state.hash, "workflow": state.workflow, "metrics": state.metrics}

    # -- report export ------------------------------------------------------
    def build_report(name: str) -> dict[str, Any]:
        doc = get_project(name)
        exp = experiments(name)
        return {
            "version": "awen.studio-report.v1",
            "generated_at": datetime.now(timezone.utc).strftime("%Y-%m-%dT%H:%M:%SZ"),
            "project": {k: doc[k] for k in ("name", "description", "example", "created_at")},
            "program_sha256": _hash(doc["program"]),
            "capabilities_sha256": _hash(doc["capabilities"]),
            "compile_report": doc["compile_report"],
            "experiments": exp["history"],
            "versions": _versions(),
            "statement": "All photonic results are simulated by AWEN's software models; compiler costs are "
                         "estimates; no photonic hardware was involved.",
        }

    @app.get("/api/projects/{name}/report")
    def report_json(name: str) -> dict[str, Any]:
        project_or_404(name)
        return build_report(name)

    @app.get("/api/projects/{name}/report.md", response_class=PlainTextResponse)
    def report_markdown(name: str) -> str:
        project_or_404(name)
        r = build_report(name)
        lines = [f"# AWEN Studio report: {r['project']['name']}", "", f"Generated {r['generated_at']}.", "",
                 f"> {r['statement']}", "", "## Compile", ""]
        c = r["compile_report"]
        if c is None:
            lines.append("Not compiled yet.")
        elif not c.get("ok"):
            lines.append(f"Compile failed: {c.get('error')}")
        else:
            lines += [f"Backend `{c['backend_id']}`.", "", "| op | backend | latency ns (est.) | energy uJ (est.) |",
                      "|---|---|---|---|"]
            for p in c["placement"]:
                lines.append(f"| {p['op_id']} | {p['selected_backend']} | {p['latency_ns']} | {p['energy_uj']} |")
            lines += ["", "Diagnostics:", ""] + [f"- {d}" for d in c["diagnostics"]]
        lines += ["", "## Experiments", ""]
        if not r["experiments"]:
            lines.append("None recorded.")
        for e in r["experiments"]:
            lines.append(f"- `{e['hash'][:19]}` {e['workflow']} at {e['created_at']}")
        lines += ["", "## Versions", ""] + [f"- {k}: {v}" for k, v in r["versions"].items()]
        return "\n".join(lines) + "\n"

    # -- static frontend -------------------------------------------------------
    if static_dir is not None and Path(static_dir, "index.html").is_file():
        static = Path(static_dir)
        app.mount("/assets", StaticFiles(directory=static / "assets"), name="assets")

        @app.get("/", include_in_schema=False)
        def index() -> FileResponse:
            return FileResponse(static / "index.html")

    return app
