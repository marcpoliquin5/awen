# awen-compiler

`awen-compiler` is the first executable compiler slice for AWEN. It turns a typed rank-2 GEMM program into a capability-specific schedule without assuming that photonics is always faster or accurate enough.

## Implemented contracts

- `awen.tensor.v1`: tensors carry rank-2 shape, dtype, layout, optional literal data, and per-operation accuracy requirements.
- `awen.device-capability.v1`: backends advertise operation/tiling legality, matrix-core shape, wavelengths, rates, coherence, ADC/DAC/effective precision, bit slicing, saturation, dynamic range, loss/power parameters, complex support, accumulation, host/link boundaries, ABI compatibility, and calibration requirements/profile.
- `awen.backend-health.v1`: a timestamped query result carries availability, temperature, drift, usable channels, disabled components, unavailable resources, and the active calibration identity.
- `awen.photonic.classical.v1`: every selected GEMM is tiled with offsets, edge sizes, precision, wavelength allocation, timing, accumulation, and calibration identity.
- `awen.device.v1`: explicit calibration, configure, upload, execute, accumulate, download, and host-fallback commands.

The cost model always includes two optical/electrical crossings for a standalone photonic region plus host transfer and reconfiguration. `auto` can therefore keep small or unsupported GEMMs on the CPU.

## Library API

```rust
use awen_compiler::{compile_with_backend, BackendSnapshot, CompileOptions, TensorProgram};

let program: TensorProgram = serde_json::from_str(input_json)?;
let snapshot: BackendSnapshot = serde_json::from_str(snapshot_json)?;
let artifact = compile_with_backend(&program, &snapshot, CompileOptions::default())?;
```

Use `benchmark(&program, &artifact)` only when the input tensors include literal data. It is a deterministic reference/conformance path, not a hardware-performance measurement.

`compile` remains the deterministic offline convenience API and constructs a
snapshot at the embedded calibration timestamp. Runtime execution should query
health and call `compile_with_backend`. Calibration freshness is evaluated
against the supplied health observation, never the compiler wall clock.

## Relationship to the MLIR compiler

The JSON Tensor IR remains a bootstrap representation and a Rust semantic
reference. The production foundation is now under `awen-mlir`, with registered
TableGen dialects and a normalized StableHLO `dot_general` to Device IR path.
Do not extend the JSON parser into a parallel general-purpose compiler
infrastructure.

This crate also owns the platform-independent decoder for `AWENEXE` 1.x so the
runtime can consume the MLIR compiler's command table without linking MLIR or
using JSON shell-out glue.

The next compiler work is tracked under [the compiler epic](https://github.com/marcpoliquin5/awen/issues/5).
