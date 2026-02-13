# AWEN Vision Overview

## The One-Liner

**AWEN = "CUDA for photonics and quantum photonics"** — a universal runtime, specification, and toolchain for programming photonic hardware.

## The Problem

Photonic computing (using light instead of electrons) lacks a standard software stack. Every lab, foundry, and hardware platform has its own bespoke tooling. There is no equivalent of CUDA/OpenCL that lets you write once and run across simulators, lab hardware, and production chips.

## The Solution: Five Pillars

| Pillar | Directory | Purpose |
|--------|-----------|---------|
| **Standard** | `awen-spec/` | Formal specifications (9 AEPs), IR schema, governance |
| **Runtime** | `awen-runtime/` | Rust engine that executes photonic circuits across backends |
| **Studio** | `awen-studio/` | Desktop UI (Tauri + React) for visual circuit design |
| **Ecosystem** | `awen-ecosystem/` | Plugins, PDKs, kernels, Python bindings, marketplace |
| **Cloud** | `awen-cloud/` | Multi-tenant execution, billing, collaboration (future) |

## Architectural Principles

1. **Material-agnostic** — Si, SiN, InP, LN, hybrid; cross-foundry portable
2. **Non-bypassable chokepoint** — All execution flows through a single `execute()` gateway that enforces calibration, artifact capture, and telemetry
3. **Calibration as computation** — Drift, noise, and variability are first-class, not afterthoughts
4. **Reproducibility by default** — Every run produces a hermetically sealed artifact bundle (IR + parameters + calibration state + environment + results)
5. **Differentiable photonics** — Native gradient computation (adjoint + finite-difference) for optimization and ML
6. **Quantum-ready** — Fock-space state management, coherence windows, measurement-conditioned feedback

## How the Specs Map to Code

```
AEP-0001 (Computation Model)  →  engine/, quantum.rs, state/
AEP-0002 (IR)                 →  ir/, example_ir.json
AEP-0003 (Kernel Semantics)   →  engine/, plugins/
AEP-0004 (Plugin System)      →  plugins/, ecosystem/
AEP-0005 (Observability)      →  observability/ (traces, timeline, metrics)
AEP-0006 (Reproducibility)    →  storage/ (bundles, deterministic IDs, export)
AEP-0007 (Calibration)        →  calibration/
AEP-0008 (Differentiable)     →  gradients.rs
AEP-0009 (Quantum Coherence)  →  quantum.rs, state/
```

## Phased Roadmap

| Phase | Focus | Status |
|-------|-------|--------|
| 1.x | Spec foundations, IR schema, initial engine | Done |
| 2.1 | Engine v0.1 | Done |
| 2.2 | Scheduler | Done |
| 2.3 | Observability (traces, metrics, timelines) | Done |
| 2.4 | Reference simulator (5 noise models, 3 measurement modes) | Done |
| 2.5 | Control + Calibration (feedback loops, adaptive measurement) | 50% |
| **2.6** | **Artifacts & Storage (bundles, replay, citation)** | **35-40%** |
| 2.7+ | Cloud integration, artifact registry, collaborative execution | Future |
| 3.x | Studio UX, plugin marketplace, signed plugins | Future |

Overall platform maturity: ~75-80% complete (~37,000 lines, 250+ tests).

## The End State

A researcher or engineer can:

1. **Define** a photonic circuit in AWEN IR (JSON graph)
2. **Simulate** it on the reference simulator
3. **Execute** it on real hardware via the HAL
4. **Optimize** parameters via differentiable gradients
5. **Calibrate** automatically with drift tracking
6. **Package** results into a reproducible artifact bundle
7. **Share & Replay** with deterministic seeds for publication
8. **Cite** with auto-generated BibTeX

## Key Entry Points

- **CLI**: `awen-runtime/src/bin/awenctl.rs` — `awenctl run` and `awenctl gradient`
- **Library root**: `awen-runtime/src/lib.rs` — all core modules
- **Python**: `awen-ecosystem/python_awen/awen_py/__init__.py` — `run_ir()`, `compute_gradients()`
- **Master spec**: `awen-spec/MasterPrompt.md`
- **V5 spec**: `awen-spec/PHOTONICS-V5-SPEC.md`
- **AEPs**: `awen-spec/aeps/AEP-000*.md`
