# Compiler architecture and invariants

## Stages

1. Validate the Tensor IR version, tensor identity, rank, non-zero dimensions, literal data length, dtype agreement, transpose semantics, output shape, and accuracy contract.
2. Validate the backend capability version and physical constraints. Calibration-required backends cannot compile without a usable profile.
3. Estimate CPU and photonic costs. Photonic estimates include transfer, two conversion boundaries, reconfiguration, tiled conversion/compute time, laser energy, and ADC/DAC energy.
4. Select CPU or photonic execution for each current GEMM. Forced photonic mode fails rather than silently violating precision/capability requirements.
5. Tile selected GEMMs across M, N, and K. K tiles explicitly accumulate; edge tiles retain their actual sizes.
6. Emit classical Photonic IR and Device IR. Literal tensor values are not copied into compiler artifacts.
7. Optionally execute the compiled tiles in the reference simulator and compare them with `awenBLAS`'s digital reference.

## Non-negotiable invariants

- Backend capability and calibration versions are explicit.
- No unsupported dtype or insufficient effective precision is silently accepted.
- CPU fallback has a recorded reason.
- Boundary crossings are counted in the placement artifact.
- Each tile contains source dtype, optical effective bits, ADC/DAC bits, accumulation mode, wavelengths, timing, and calibration identity.
- A benchmark passes only when its declared absolute or relative tolerance is met.
- Reference capability numbers are simulation inputs, not vendor benchmarks.

## Deliberate limitations of v0.1

- Placement is per GEMM; graph-region fusion and tensor residency require the partitioner in issue #9.
- The cost model is dimensional and conservative but not hardware-fitted; issue #10 adds measured parameter provenance, uncertainty, and autotuning.
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
