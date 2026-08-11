# AEP-0010: Heterogeneous compiler stack

Status: Draft

## Summary

AWEN will use a multi-level compiler architecture for programmable physical linear algebra. Framework tensor graphs lower through a typed tensor dialect, device-independent classical or quantum-photonic dialects, and a capability/calibration-specific Device IR. The runtime executes the resulting artifact and retains scheduling, safety, observability, and reproducibility responsibilities.

## Motivation

The legacy node IR and V5 string operation schema describe laboratory/runtime actions but do not carry enough information for tensor compilation: shapes, dynamic dimensions, dtype, layout, precision/error contracts, complex encoding, topology constraints, boundary conversions, or hardware capability. These omissions prevent defensible placement, tiling, numerical validation, and cost comparison.

## Specification

The production pipeline is:

```text
PyTorch/JAX/C++/NumPy
  -> StableHLO or framework graph
  -> awen.tensor
  -> graph partition/fusion/tiling/precision/cost/calibration passes
  -> awen.photonic | awen.qphotonic | CPU/GPU IR
  -> awen.device
  -> versioned executable artifact
  -> AWEN runtime/HAL/backend
```

MLIR is the required compiler infrastructure. StableHLO is the preferred portable tensor interchange. The bootstrap Rust/JSON compiler is a testable semantic prototype for GEMM, capability negotiation, costs, tiling, calibration, and command emission; it is not a substitute for the MLIR pipeline.

Classical analog photonics and quantum photonics use separate dialects because their state, measurement, precision, scheduling, and correctness rules differ. Shared tensor, device, artifact, backend, and runtime infrastructure remains reusable.

Every placement decision must consider full optical/electrical boundaries, reconfiguration, data movement, precision, and calibration. Every artifact records the capability and calibration snapshot used. Missing or stale facts cause conservative digital fallback.

## Backwards compatibility

Legacy node Graph IR and Photonic IR V5 remain readable by the runtime during migration. Conversion tooling must reject ambiguous string operations rather than invent semantics. New compiler artifacts use independent semantic versions.

## Test plan

- Dialect/parser/verifier/round-trip tests.
- StableHLO GEMM import and deterministic lowering goldens.
- Capability and calibration validation tests.
- M/N/K and edge-tile tests.
- CPU/photonics placement tests including conversion-dominated fallback.
- Precision-contract rejection and calibrated numerical comparison.
- Runtime executable ABI compatibility tests.
- End-to-end framework-to-simulator and, later, hardware-in-the-loop tests.
