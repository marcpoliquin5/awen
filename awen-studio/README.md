# AWEN Studio (local)

A local web app for working with AWEN on your own machine. It is **not hosted
anywhere**, has no accounts, and binds to `127.0.0.1` by default. Every panel
calls a real AWEN backend; nothing in the UI is mocked.

| Panel | Backend | Endpoint(s) |
|---|---|---|
| Projects | on-disk workspace (`awen_studio.workspace`) | `GET/POST /api/projects`, `GET /api/projects/{name}` |
| Model | AWEN-Mini via `awen.Runtime` on `cpu` or `photonic-sim` | `GET /api/models`, `POST /api/models/generate` |
| Compiler | Rust compiler via `awen_native.compile_program`: placement, capability negotiation, health, diagnostics | `POST /api/projects/{name}/compile`, `POST /api/compile` |
| Ops | operator table with verification levels (`awen_native.op_table`) | `GET /api/ops` |
| Noise | noise-contract editing, validation and analytic per-tile noise (`validate_contract`, `predicted_noise`) | `GET /api/noise/default`, `POST /api/noise/validate` |
| Compare | GEMM on the photonic simulator vs NumPy on CPU (error, effective bits, histogram); AWEN-Mini likelihood on both | `POST /api/compare/gemm`, `POST /api/models/likelihood` |
| Experiments | Lab Agent quick / truth / repro runs and full state history per project (`awen_lab`) | `GET/POST /api/projects/{name}/experiments` |
| Report | Markdown or JSON export of project, compile report, experiments and versions | `GET /api/projects/{name}/report(.md)` |

Labels are explicit everywhere: photonic numbers are **simulated** (software
model, not hardware), compiler costs are **estimated** (cost model), and CPU
timings are **measured** on the host. No photonic hardware is attached.

## Run it

```bash
pip install ./awen-native ./awen-models ./awen-qphotonic ./awen-lab ./awen-studio
cd awen-studio/frontend && npm ci && npm run build && cd ../..
awen-studio --workspace ~/.awen-studio      # http://127.0.0.1:8765
```

Run from the repository root (or set `AWEN_REPO`) so the bundled example
programs are found. Without a frontend build the server runs API-only; the
OpenAPI docs are at `/docs`. For frontend development, run `awen-studio` and
`npm run dev` (Vite proxies `/api` to port 8765).

`--host` other than loopback is refused unless `--allow-remote` is given,
because Studio has no authentication.

## Tests

```bash
python -m pytest awen-studio/tests -q
```

`tests/test_api.py` covers every endpoint against the real backends (13
tests). `tests/test_e2e.py` serves the built frontend with uvicorn and drives
the API over HTTP (it checks serving and wiring, not browser rendering). CI
builds the frontend first and sets `AWEN_STUDIO_REQUIRE_FRONTEND=1` so a missing
build fails instead of skipping.
