# Compiler architecture and invariants

## Stages

1. Validate the Tensor IR version, tensor identity, rank, non-zero dimensions, literal data length, dtype agreement, transpose semantics, output shape, and accuracy contract.
2. Validate capability, runtime ABI, plugin ABI, physical cross-field constraints, and the timestamped health snapshot.
3. Negotiate operation, dtype, transpose, partial-tile, precision, resource, and calibration eligibility. Invalid or unavailable candidates retain an explicit digital fallback.
4. Build a full-system cost context from tensor layout/error requirements, the device and health snapshot, execution controls, and provenance-bearing model parameters.
5. Autotune legal tile sizes, bit slices, wavelength counts, accumulation modes, batching, and boundary fusion for latency, energy, accuracy, or throughput.
6. Select CPU or photonic execution. Forced photonic mode fails rather than silently violating precision/capability requirements or using an incomplete cost comparison.
7. Tile selected GEMMs across M, N, and K using the winning plan. K tiles explicitly accumulate; edge tiles retain their actual sizes only when the backend permits partial tiles.
8. Emit classical Photonic IR and Device IR. Literal tensor values are not copied into compiler artifacts.
9. Optionally execute the compiled tiles in the reference simulator, compare them with `awenBLAS`'s digital reference, and attach external predicted-versus-observed measurements.

## Non-negotiable invariants

- Backend capability and calibration versions are explicit.
- Runtime/plugin ABI compatibility and the exact health snapshot are explicit.
- Calibration age is measured against the health observation for deterministic replay.
- Missing or unavailable capability facts force a diagnosed digital fallback.
- No unsupported dtype or insufficient effective precision is silently accepted.
- CPU fallback has a recorded reason.
- Boundary crossings are counted in the placement artifact.
- No decision compares optical propagation time without its host, memory, conversion, power, calibration, and accumulation costs.
- Each estimate declares units through field names, component totals, uncertainty intervals, and parameter provenance.
- Each decision records its complete graph/device/calibration/model fingerprint, selected plan, ranked alternatives, and explanation.
- Each tile contains source dtype, optical effective bits, ADC/DAC bits, bit slices, accumulation mode, wavelengths, timing, and calibration identity.
- A benchmark passes only when its declared absolute or relative tolerance is met.
- Reference capability numbers are simulation inputs, not vendor benchmarks.

## Deliberate limitations of v0.1

- Placement is per GEMM; graph-region fusion and tensor residency require the partitioner in issue #9.
- The reference cost model is deliberately conservative. Production values require immutable hardware benchmark artifacts and periodic refitting.
- The simulator models block quantization and a scalar calibrated transfer function, not full optical noise or per-cell defects.
- Rank-2 row-major and column-major indexing are supported. General strided and blocked layouts require a future layout abstraction.
- Complex arithmetic is represented in capabilities/types but has no executable GEMM kernel.
- The bootstrap JSON frontend must be replaced by MLIR/StableHLO, not expanded into a competing infrastructure.

## Production lowering direction

```text
torch graph / StableHLO
        -> awen.tensor dialect
        -> crossing-aware region partition
        -> precision + calibration + tiling + autotuning
        -> awen.photonic | awen.qphotonic | CPU/GPU lowering
        -> awen.device executable
        -> runtime/HAL command submission
```

The runtime must consume a versioned executable package directly; subprocesses and temporary JSON discovery remain prototype-only interfaces.
