# AWEN Memory & State Model v0.1

**Status:** DRAFT  
**Version:** 0.1.0  
**Date:** 2026-01-05  
**Depends On:** [computation-model.md](computation-model.md), [quantum-coherence.md](quantum-coherence.md)

---

## Purpose

Define the memory and state semantics for photonic and quantum-photonic computation, including:
- Photonic buffer abstractions (delay lines, resonators, hybrid registers)
- Lifetime and coherence constraints
- State evolution semantics (deterministic vs probabilistic)
- Hybrid memory architecture (photonic ↔ electronic ↔ cloud)
- Provenance tracking for state snapshots

---

## 1. Memory Model Overview

Photonics lacks traditional RAM. AWEN provides **three memory primitive abstractions**:

### 1.1 DelayBuffer (Photonic FIFO)

**Physical Realization:** Optical fiber delay lines, waveguide spirals

```
DelayBuffer {
    id: String,
    latency_ns: u64,              // Fixed time-of-flight delay
    insertion_loss_db: f64,       // Attenuation per pass
    coherence_time_ns: u64,       // Maximum coherence preservation
    bandwidth_thz: f64,           // Supported frequency range
    capacity_modes: usize,        // Simultaneous wavelengths/modes
}
```

**Semantics:**
- **FIFO only:** No random access
- **Deterministic latency:** Output appears exactly `latency_ns` after input
- **Lossy:** Amplitude decays by `insertion_loss_db` per pass
- **Coherence-limited:** Phase relationships preserved only within `coherence_time_ns`
- **Read-once:** Output photons consumed on readout (destructive)

**Use Cases:**
- Time-domain multiplexing
- Synchronization buffers
- Recirculation loops for iterative algorithms

### 1.2 ResonatorStore (Photonic Cache)

**Physical Realization:** Microring resonators, Fabry-Pérot cavities

```
ResonatorStore {
    id: String,
    lifetime_ns: u64,             // Exponential decay time (τ)
    coupling_q: f64,              // Quality factor (Q)
    bandwidth_ghz: f64,           // Resonance bandwidth
    read_efficiency: f64,         // Fraction retrieved on readout
    write_efficiency: f64,        // Fraction stored on write
}
```

**Semantics:**
- **Addressable by wavelength:** Each resonance stores one mode
- **Exponential decay:** Stored amplitude decays as `A(t) = A₀ exp(-t/τ)`
- **Non-destructive readout (partial):** Read fraction `read_efficiency`, remainder stays
- **Lossy write:** Only `write_efficiency` fraction coupled in
- **Bandwidth-limited:** Narrow linewidth (~GHz) restricts pulsewidth

**Use Cases:**
- Temporary state storage (microsecond timescales)
- Quantum memory for entangled states
- Caching intermediate results in multi-stage computations

### 1.3 HybridRegister (Electronic/Photonic Bridge)

**Physical Realization:** Electro-optic modulators + electronic SRAM + photodetectors

```
HybridRegister {
    id: String,
    capacity_bits: usize,         // Electronic storage capacity
    photonic_ports: Vec<PortID>,  // Coupled photonic modes
    read_latency_ns: u64,         // Electronic → photonic conversion
    write_latency_ns: u64,        // Photonic → electronic conversion
    fidelity: f64,                // Conversion fidelity (0-1)
}
```

**Semantics:**
- **Random access:** Full addressability via electronic control
- **Persistent:** Survives coherence time limits (indefinite storage)
- **Bidirectional conversion:**
  - **Photonic → Electronic:** Measure photonic mode, store classical result
  - **Electronic → Photonic:** Modulate laser based on stored value
- **Lossy conversion:** Fidelity < 1 due to detection/modulation noise
- **Latency overhead:** Adds `read_latency_ns` or `write_latency_ns` to critical path

**Use Cases:**
- Classical control parameters
- Measurement results storage
- Checkpoint/restore for hybrid algorithms
- Interface to cloud storage via network

---

## 2. State Evolution Semantics

### 2.1 State Representation

```rust
QuantumState {
    id: String,
    modes: Vec<QuantumMode>,           // Per-mode amplitudes/phases
    coherence_window: CoherenceWindow, // Temporal validity
    seed: Option<u64>,                 // Deterministic replay seed
    provenance: HashMap<String, String>, // Metadata
}

QuantumMode {
    mode_id: String,
    mode_type: String,                 // "classical", "quantum_fock", "mixed"
    photon_numbers: Option<Vec<u32>>,  // Fock basis truncation
    amplitudes: Option<Vec<Complex64>>, // Complex amplitudes
    phases: Option<Vec<f64>>,          // Relative phases
}

CoherenceWindow {
    start_ns: u64,
    end_ns: u64,
    decoherence_timescale_ns: f64,     // T₂ dephasing time
    cross_mode_decoherence_ns: f64,    // Multi-mode coherence
    idle_time_budget_ns: u64,          // Max idle before recalibration
}
```

### 2.2 Evolution Operators

State evolution follows **unitary evolution** (gate operations) and **non-unitary evolution** (measurement, loss, decoherence):

#### Unitary Gates
- **Phase Shift (PS):** `U_PS(φ) = exp(iφ)`
- **Beam Splitter (BS):** `U_BS(θ) = [[cos θ, -sin θ], [sin θ, cos θ]]`
- **Squeezing (SQZ):** `U_SQZ(r, φ) = exp(r(e^{iφ}â² - e^{-iφ}â†²))`
- **Parametric Down-Conversion (PDC):** Two-mode squeezing for entanglement

**Determinism:** Unitary gates are **fully deterministic** given same input state and parameters.

#### Non-Unitary Operations
- **Measurement:** Probabilistic collapse to eigenstate
- **Loss:** Amplitude decay `A → A√η` (η = transmission efficiency)
- **Decoherence:** Phase diffusion, amplitude damping

**Determinism:** Requires **seeded PRNG** for reproducible sampling. Runtime tracks seed in provenance.

### 2.3 Determinism Modes

```rust
enum DeterminismMode {
    Experimental,      // Live hardware randomness
    ReplaySeeded,      // Fixed seed for all stochastic events
    ReplayTraced,      // Replay captured measurement outcomes
}
```

**Experimental:** Uses hardware RNG, measurement outcomes are stochastic.  
**ReplaySeeded:** Uses fixed seed (stored in artifact), outcomes deterministic given seed.  
**ReplayTraced:** Replays captured measurement outcomes from artifact (bit-exact).

---

## 3. Lifetime & Persistence Semantics

### 3.1 Coherence Lifetime (τ_coh)

**Definition:** Maximum time duration for which phase relationships remain meaningful.

**Enforcement:**
```rust
trait CoherenceManager {
    fn validate_coherence(&self, state: &QuantumState, current_time_ns: u64) -> Result<()>;
}
```

**Violations:**
- State accessed after `coherence_window.end_ns` → **CoherenceViolation error**
- Operations requiring interference executed outside window → **PhaseDriftWarning**

**Mitigation:**
- Runtime inserts **coherence barriers** (synchronization points)
- Scheduler plans operations to fit within coherence windows
- Calibration refreshes phase references

### 3.2 Storage Lifetime (τ_storage)

**Definition:** Maximum duration data persists in memory primitive before loss/decay.

| Primitive | Lifetime | Persistence |
|-----------|----------|-------------|
| DelayBuffer | Fixed latency (μs-ms) | Deterministic FIFO |
| ResonatorStore | Exponential decay (ns-μs) | Lossy, exponential |
| HybridRegister | Indefinite | Persistent (electronic) |

**Example:**
```rust
// DelayBuffer: photons emerge after fixed delay
buffer.write(mode, t0);
mode_out = buffer.read(t0 + latency_ns); // Deterministic timing

// ResonatorStore: exponential decay
store.write(mode, t0);
mode_out = store.read(t0 + Δt); // Amplitude *= exp(-Δt/τ)

// HybridRegister: persistent until overwritten
register.write(mode, t0);
mode_out = register.read(t0 + 1_000_000_ns); // No decay
```

### 3.3 Recalibration Triggers

**Drift Detection:** Monitor observable metrics (visibility, fidelity, phase stability)

```rust
if time_since_last_calibration > drift_threshold {
    runtime.trigger_recalibration();
    // Updates calibration_state in artifact
}
```

**Impact on State:**
- Calibration may update phase offsets, coupling parameters
- State provenance records calibration version used

---

## 4. Hybrid Memory Architecture

### 4.1 Three-Tier Model

```
┌─────────────────────────────────────────┐
│  Cloud Storage (Persistent, Low BW)    │
│  - Artifact bundles                     │
│  - Calibration database                 │
│  - Checkpoint/restore for long runs     │
└─────────────────┬───────────────────────┘
                  │ Network I/O (ms-s)
┌─────────────────▼───────────────────────┐
│  Electronic Memory (Persistent, Fast)   │
│  - HybridRegisters                      │
│  - Classical control parameters         │
│  - Measurement outcomes                 │
└─────────────────┬───────────────────────┘
                  │ E-O Conversion (ns-μs)
┌─────────────────▼───────────────────────┐
│  Photonic Memory (Transient, Ultrafast) │
│  - DelayBuffers (FIFO)                  │
│  - ResonatorStores (cache)              │
│  - Coherent quantum states              │
└─────────────────────────────────────────┘
```

### 4.2 Cross-Domain Transfer

**Photonic → Electronic:**
```rust
// Destructive measurement
outcome = detector.measure(photonic_mode, seed);
hybrid_register.write(outcome.value);
```

**Electronic → Photonic:**
```rust
// Modulation
classical_value = hybrid_register.read(address);
photonic_mode = modulator.encode(classical_value);
```

**Electronic → Cloud:**
```rust
// Artifact export
artifact_bundle = engine.collect_state_snapshot();
cloud_storage.upload(artifact_bundle);
```

### 4.3 Memory Hierarchy Operations

| Operation | Source | Destination | Latency | Fidelity |
|-----------|--------|-------------|---------|----------|
| `store_photonic` | Photonic | DelayBuffer | ~10ns | 0.95 |
| `cache_photonic` | Photonic | ResonatorStore | ~100ns | 0.90 |
| `persist_classical` | Photonic | HybridRegister | ~1μs | 0.99 |
| `checkpoint` | HybridRegister | Cloud | ~10ms | 1.00 |
| `restore` | Cloud | HybridRegister | ~10ms | 1.00 |

---

## 5. Provenance & Metadata Tracking

### 5.1 State Snapshot Provenance

Every `QuantumState` carries:
```rust
provenance: {
    "origin": "engine.run_graph",
    "parent_state_id": "qstate-42-parent",
    "last_gate": "BS(theta=0.785)",
    "measurement": "mode:detector_0 outcome:1",
    "calibration_version": "calib_v2.3.1",
    "coherence_window_id": "coh-1000000-gaussian",
    "seed": "0xDEADBEEF",
}
```

### 5.2 Memory Primitive Telemetry

Each memory operation logs:
```json
{
  "timestamp_ns": 1234567890,
  "primitive_type": "ResonatorStore",
  "primitive_id": "resonator_0",
  "operation": "read",
  "mode_id": "mode_5",
  "input_amplitude": 0.8,
  "output_amplitude": 0.72,
  "decay_factor": 0.90,
  "time_since_write_ns": 500
}
```

Feeds into observability system ([AEP-0005](../aeps/AEP-0005-observability.md)).

---

## 6. State Checkpointing & Restoration

### 6.1 Checkpoint Contract

**Purpose:** Save complete state snapshot to persistent storage for:
- Long-running computations (pause/resume)
- Fault tolerance
- Comparison across hardware revisions

**Checkpoint Contents:**
```
checkpoint/
├── quantum_states.json       # All active QuantumState snapshots
├── memory_buffers.json       # DelayBuffer/ResonatorStore contents
├── hybrid_registers.json     # HybridRegister values
├── coherence_windows.json    # Active coherence windows
├── calibration_state.json    # Current calibration parameters
└── provenance.json           # Metadata (timestamp, hardware, seed)
```

### 6.2 Restore Semantics

**Restore guarantees:**
- **Electronic state:** Bit-exact restoration
- **Photonic state:** Best-effort (re-prepare photonic modes via modulators)
- **Coherence:** New coherence window created (cannot restore phase relationships)

**Use Case:**
```rust
// Long computation checkpoint
let checkpoint = engine.checkpoint_state();
artifact_storage.save(checkpoint);

// ... System reboot or migration ...

// Restore and continue
let checkpoint = artifact_storage.load("checkpoint_id");
engine.restore_state(checkpoint)?;
engine.resume_execution()?;
```

---

## 7. Probabilistic vs Deterministic State Model

### 7.1 Probabilistic Operations

**Sources of randomness:**
1. Quantum measurement (inherent probabilistic collapse)
2. Shot noise (Poisson photon statistics)
3. Thermal noise (electronic amplifier noise)
4. Phase noise (laser linewidth)

**Handling:**
- Runtime samples from probability distributions
- Uses seeded PRNG for reproducibility
- Logs seed + distribution parameters in provenance

### 7.2 Deterministic Replay

**Requirements for bit-exact replay:**
1. Same IR graph
2. Same initial parameters
3. Same calibration state
4. Same seed for all stochastic operations
5. Same runtime version (implementation-dependent floating-point)

**Verification:**
```bash
awenctl run graph.json --seed 42 --mode replay-seeded
# Artifact ID: awen_abc123...

awenctl replay awen_abc123 --verify
# ✓ Bit-exact verification PASSED
```

---

## 8. Runtime Implementation Contract

### 8.1 StateEvolver Trait

```rust
pub trait StateEvolver: Send + Sync {
    /// Apply unitary gate (deterministic)
    fn evolve_state(
        &self,
        state: &QuantumState,
        gate: &str,
        params: &HashMap<String, f64>
    ) -> Result<QuantumState>;

    /// Perform measurement (probabilistic if seed=None)
    fn measure(
        &self,
        state: &QuantumState,
        mode_id: &str,
        seed: Option<u64>
    ) -> Result<MeasurementOutcome>;

    /// Check coherence validity
    fn is_coherent(&self, state: &QuantumState, current_time_ns: u64) -> bool;
}
```

### 8.2 MemoryPrimitive Trait

```rust
pub trait MemoryPrimitive: Send + Sync {
    /// Write photonic mode to memory
    fn write(&mut self, mode: QuantumMode, timestamp_ns: u64) -> Result<()>;

    /// Read photonic mode from memory (may be lossy/delayed)
    fn read(&mut self, timestamp_ns: u64) -> Result<Option<QuantumMode>>;

    /// Get lifetime/decay characteristics
    fn lifetime_ns(&self) -> u64;

    /// Check if data still valid at given time
    fn is_valid(&self, write_time_ns: u64, read_time_ns: u64) -> bool;
}
```

### 8.3 CoherenceManager Trait

```rust
pub trait CoherenceManager: Send + Sync {
    /// Create coherence window for execution segment
    fn create_window(
        &self,
        start_ns: u64,
        duration_ns: u64,
        decoherence_model: &str
    ) -> Result<CoherenceWindow>;

    /// Validate state within coherence constraints
    fn validate_coherence(
        &self,
        state: &QuantumState,
        current_time_ns: u64
    ) -> Result<()>;

    /// Insert coherence barrier (synchronization)
    fn insert_barrier(
        &mut self,
        barrier_time_ns: u64
    ) -> Result<()>;
}
```

---

## 9. Integration with Observability & Artifacts

### 9.1 State Evolution Traces

**Each state transition generates observability event:**
```json
{
  "event_type": "state_evolution",
  "timestamp_ns": 1234567890,
  "state_id": "qstate-42",
  "gate": "BS",
  "params": {"theta": 0.785},
  "input_modes": ["mode_0", "mode_1"],
  "output_modes": ["mode_0", "mode_1"],
  "amplitude_delta": 0.05,
  "phase_shift_rad": 0.1
}
```

### 9.2 Artifact Bundle Integration

**State snapshots included in artifact bundle:**
```
awen_<id>/
├── execution/
│   ├── quantum_states.json   # Full state history
│   ├── measurements.json     # All measurement outcomes
│   └── coherence_logs.json   # Coherence violations/warnings
└── ...
```

**Reference:** [AEP-0006 Reproducibility](../aeps/AEP-0006-reproducibility-artifacts.md)

---

## 10. Conformance Requirements

### 10.1 Runtime Implementations MUST:

1. **Implement StateEvolver trait** for unitary + measurement operations
2. **Enforce coherence windows** via CoherenceManager
3. **Track provenance** for all state transitions
4. **Log state snapshots** in observability artifacts
5. **Support deterministic replay** via seeded PRNG
6. **Validate memory lifetime** constraints (reject expired reads)
7. **Export state history** in artifact bundles

### 10.2 Validation Tests

**Test Suite:** `tests/state_integration.rs`

Required tests:
- `test_unitary_evolution_deterministic` — Same inputs → same outputs
- `test_measurement_seeded_replay` — Same seed → same measurement outcomes
- `test_coherence_window_enforcement` — Reject operations outside window
- `test_delay_buffer_fifo_semantics` — Correct latency + loss
- `test_resonator_exponential_decay` — Verify decay model
- `test_hybrid_register_persistence` — Data survives long durations
- `test_state_provenance_tracking` — All transitions logged
- `test_checkpoint_restore_roundtrip` — Restore matches original state

---

## 11. Example: Hybrid Memory Algorithm

**Use Case:** Store intermediate result in resonator, later retrieve for computation

```rust
// 1. Prepare input state
let input_state = engine.prepare_state(|0⟩);

// 2. Apply gate sequence
let state_1 = evolver.evolve_state(&input_state, "BS", &params)?;
let state_2 = evolver.evolve_state(&state_1, "PS", &phase_params)?;

// 3. Store in resonator (coherent storage)
let resonator = ResonatorStore::new("res_0", lifetime_ns=500);
resonator.write(state_2.modes[0], current_time_ns)?;

// 4. Perform other operations (elapsed time: 200ns)
let side_computation = engine.run_subroutine()?;

// 5. Retrieve from resonator (partial decay)
let stored_mode = resonator.read(current_time_ns + 200)?;
// Amplitude reduced by exp(-200/500) ≈ 0.67

// 6. Continue computation with retrieved state
let final_state = evolver.evolve_state_with_mode(&side_computation, stored_mode)?;
```

---

## 12. Future Extensions (v0.2+)

- **Multi-mode entanglement tracking** (beyond pairwise)
- **Advanced decoherence models** (non-Markovian, non-Gaussian)
- **Quantum error correction primitives** (stabilizer codes, GKP states)
- **Distributed quantum memory** (fiber-linked resonators)
- **Machine learning-guided state tomography** (adaptive measurement)

---

## References

1. [computation-model.md](computation-model.md) — Core computation primitives
2. [quantum-coherence.md](quantum-coherence.md) — Coherence window semantics
3. [AEP-0005: Observability](../aeps/AEP-0005-observability.md) — State telemetry
4. [AEP-0006: Reproducibility](../aeps/AEP-0006-reproducibility-artifacts.md) — State provenance in artifacts
5. C. Weedbrook et al., "Gaussian quantum information," Rev. Mod. Phys. 84, 621 (2012)
6. J. Carolan et al., "Universal linear optics," Science 349, 711 (2015)

---

**Version History:**
- v0.1.0 (2026-01-05): Initial specification with memory primitives, state evolution, lifetime semantics
