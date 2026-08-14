# AWEN runtime architecture

```text
versioned input / AWENEXE
          |
          v
  validation chokepoint -----> typed failure
          |
          v
 engine / executable loader
          |
          +----> scheduler ----> HAL/plugin ----> simulator or external backend
          |
          +----> calibration and health binding
          |
          +----> observability and content-addressed artifacts
```

- `src/executable.rs` decodes and validates the compiler's binary command table.
- `src/chokepoint.rs` keeps classical and quantum-photonic programs separate.
- `src/engine_v2.rs` and `src/engine` execute typed and legacy paths.
- `src/scheduler`, `src/hal`, and `src/plugins` own timing, devices, discovery,
  signatures, health, and external integration.
- `src/calibration` binds runtime state to exact backend/topology snapshots.
- `src/observability` writes spans, events, metrics, and timelines.
- `src/storage` exports, verifies, imports, and replays artifact bundles.
- `src/benchmark.rs` owns comparable full-system HIL evidence and fail-closed
  public claim generation.

The compiler owns placement and command generation. The runtime does not parse
compiler JSON to execute AWENEXE, and plugins cannot weaken compiler validation.
External physical-design tools exchange only the typed AEP-0021 evidence
boundary. The reference simulator and checked capabilities are not measurements
of a physical accelerator.
