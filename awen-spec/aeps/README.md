AWEN Enhancement Proposals (AEPs)

The AEP process documents major changes to the AWEN specification. Create a new file `AEP-XXXX-description.md` using the template in `template.md` and submit as a PR.

Template fields:
- Title
- Summary
- Motivation
- Specification
- Backwards compatibility
- Test plan

Current proposals:

- `AEP-0001` through `AEP-0009`: computation, IR, kernels, plugins, observability, artifacts, calibration, differentiability, and quantum coherence.
- `AEP-0010`: heterogeneous compiler stack and MLIR/StableHLO lowering architecture.
- `AEP-0011`: MLIR embedding, dialect layout, and StableHLO GEMM import.
- `AEP-0012`: binary compiler/runtime executable ABI 1.0.
- `AEP-0013`: full-system cost model, provenance, uncertainty, benchmark fitting, and deterministic autotuning.
- `AEP-0014`: crossing-aware whole-graph CPU/GPU/photonic partitioning, residency, fusion, memory pressure, and explainable traces.
- `AEP-0015`: executable awenBLAS kernel registry, numerical semantics, structured operators, capability/cost dispatch, deterministic simulation, and measured conformance evidence.
- `AEP-0016`: in-process framework runtime, PyTorch compiler backend, portable JAX/StableHLO import, NumPy surface, and compiled C/C++ ABI.
- `AEP-0017`: explicit storage/compute/accumulator/output precision, scaling, signed bit slicing, calibration compensation, deterministic analog noise, and attributed error contracts.
- `AEP-0018`: immutable calibration snapshots, topology/fingerprint binding, measured cell/channel routing, fault remapping, artifact lineage, and drift-triggered safe refresh.
- `AEP-0019`: reproducible full-system and hardware-in-the-loop benchmark evidence, physical-driver protocol, immutable artifacts, and verified claim generation.
