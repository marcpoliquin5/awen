# AWEN Timing, Scheduling, and Coherence Semantics v0.1

**Status:** DRAFT  
**Version:** 0.1.0  
**Date:** 2026-01-05  
**Depends On:** [computation-model.md](computation-model.md), [memory-state.md](memory-state.md), [quantum-coherence.md](quantum-coherence.md)

---

## Purpose

Define the timing model, scheduling semantics, and coherence enforcement for photonic and quantum-photonic computation. This specification ensures:
- Precise time-of-flight accounting in photonic circuits
- Coherence window constraints enforced at compile-time and runtime
- Measurement-feedback latency guarantees
- Multi-wavelength synchronization
- Deterministic scheduling for reproducibility
- Resource contention resolution
- Safety-critical timing constraints

---

## 1. Time Model

### 1.1 Time Domains

AWEN distinguishes **four orthogonal time domains**:

#### Physical Time (Wall-Clock Time)
- **Definition:** Absolute time measured by system clock (UTC, nanosecond precision)
- **Usage:** Profiling, debugging, log correlation
- **Unit:** Nanoseconds since epoch (`u64`)

#### Logical Time (Execution Time)
- **Definition:** Virtual time counter within a single execution DAG
- **Usage:** Causality enforcement, dependency ordering
- **Unit:** Logical ticks (`u64`), incremented deterministically
- **Guarantee:** Same IR + same seed → same logical time sequence

#### Photonic Time (Time-of-Flight)
- **Definition:** Physical propagation delay of light through photonic components
- **Calculation:** `t_flight = distance / (c / n_eff)` where `n_eff` is effective refractive index
- **Usage:** Delay line modeling, arrival time prediction
- **Unit:** Nanoseconds (`f64` for sub-nanosecond precision)
- **Example:** 1 meter fiber (n_eff=1.45) → ~4.8 ns delay

#### Coherence Time
- **Definition:** Duration window where phase relationships remain valid
- **Sources of decoherence:**
  - **Intrinsic:** Material dephasing (T₂ for quantum, thermal drift for classical)
  - **Environmental:** Temperature fluctuations, vibrations
  - **Measurement-induced:** Wavefunction collapse
- **Unit:** Nanoseconds (`u64`)
- **Range:** Picoseconds (lossy waveguides) to milliseconds (cryogenic resonators)

---

## 2. Coherence Window Model

### 2.1 CoherenceWindow Definition

```rust
pub struct CoherenceWindow {
    pub id: String,
    pub start_time_ns: u64,        // Window begins (logical time)
    pub duration_ns: u64,           // Maximum coherence preservation
    pub decoherence_model: String,  // "exponential", "gaussian", "lorentzian"
    pub fidelity_threshold: f64,    // Minimum acceptable fidelity (0-1)
    pub mode_ids: Vec<String>,      // Modes participating in coherence
}
```

### 2.2 Decoherence Models

#### Exponential Decay (Default)
```
F(t) = exp(-t / T₁)
```
- **T₁:** Energy relaxation time
- **Typical:** Resonator storage, excited state lifetimes

#### Gaussian Decay (Inhomogeneous Broadening)
```
F(t) = exp(-(t / T₂*)²)
```
- **T₂*:** Dephasing time
- **Typical:** Ensemble averaging, thermal noise

#### Lorentzian (Homogeneous Broadening)
```
F(t) = 1 / (1 + (t / T₂)²)
```
- **T₂:** Pure dephasing time
- **Typical:** Collision-induced dephasing

### 2.3 Coherence Validation Rules

**Rule 1: Temporal Containment**
```
For any operation Op requiring coherence:
    Op.start_time ≥ Window.start_time
    Op.end_time ≤ Window.start_time + Window.duration_ns
```

**Rule 2: Fidelity Threshold**
```
F(Op.end_time - Op.start_time) ≥ Window.fidelity_threshold
```

**Rule 3: Mode Overlap**
```
Op.input_modes ⊆ Window.mode_ids
Op.output_modes ⊆ Window.mode_ids
```

**Enforcement:** Runtime **MUST REJECT** operations violating any rule.

---

## 3. Time-of-Flight Accounting

### 3.1 Component Delay Model

Every photonic component declares timing parameters:

```rust
pub struct ComponentTiming {
    pub latency_min_ns: f64,        // Best-case propagation time
    pub latency_max_ns: f64,        // Worst-case propagation time
    pub latency_jitter_ns: f64,     // Stochastic variation (RMS)
    pub group_delay_dispersion: f64, // Chromatic dispersion (ps/nm/km)
}
```

#### Common Components
| Component | Latency (ns) | Jitter (ps) | Notes |
|-----------|--------------|-------------|-------|
| Straight waveguide (1 mm) | 0.01 | <0.1 | Negligible for silicon photonics |
| Microring resonator | 0.5-50 | 1-10 | Depends on Q-factor |
| Fiber delay (1 m) | 4.8 | <0.01 | Low dispersion SMF-28 |
| Electro-optic modulator | 0.05-0.5 | 0.1-1 | LiNbO₃ or silicon |
| Detector + ADC | 1-100 | 10-100 | Depends on bandwidth |

### 3.2 Path Delay Calculation

For a photonic path `P = [C₁, C₂, ..., Cₙ]`:

**Best-case arrival time:**
```
T_min(P) = Σᵢ Cᵢ.latency_min_ns
```

**Worst-case arrival time:**
```
T_max(P) = Σᵢ Cᵢ.latency_max_ns
```

**Expected arrival time (Gaussian jitter):**
```
T_exp(P) = Σᵢ (Cᵢ.latency_min_ns + Cᵢ.latency_max_ns) / 2
σ(P) = sqrt(Σᵢ Cᵢ.latency_jitter_ns²)
```

**Schedulers MUST use worst-case bounds for safety-critical timing.**

---

## 4. Scheduling Model

### 4.1 Scheduling Problem Definition

**Given:**
- IR graph `G = (Nodes, Edges)`
- Resource constraints: `{Device, Wavelengths, Memory, Calibration_state}`
- Timing constraints: `{Coherence_windows, Feedback_deadlines, Synchronization_barriers}`

**Objective:**
- Compute `ExecutionPlan: Node → (Start_time, End_time, Resource_allocation)`
- Minimize: Total execution time (makespan)
- Subject to:
  1. **Causality:** Dependencies respected (data + control flow)
  2. **Resource exclusivity:** No double-booking of physical resources
  3. **Coherence windows:** All phase-sensitive ops within valid windows
  4. **Latency bounds:** Feedback loops meet deadlines
  5. **Determinism:** Same IR + seed → same schedule (for reproducibility)

### 4.2 Scheduling Strategies

#### Static Scheduling (Default)
- **When:** Full IR known at compile-time, no runtime branching
- **Algorithm:** Topological sort + list scheduling with resource awareness
- **Guarantee:** Deterministic, reproducible schedule
- **Output:** `ExecutionPlan` serialized in artifact bundle

#### Dynamic Scheduling (Measurement-Conditioned)
- **When:** Control flow depends on measurement outcomes
- **Algorithm:** Pre-compute all possible branches, select at runtime based on measurement
- **Guarantee:** Deterministic given measurement seed
- **Output:** Conditional `ExecutionPlan` with branch annotations

#### Real-Time Scheduling (Hardware Loop)
- **When:** Adaptive feedback requiring minimal latency
- **Algorithm:** Online scheduler with preemption, priority queues
- **Guarantee:** Bounded worst-case latency (provable via real-time analysis)
- **Output:** Schedule trace in observability artifacts

### 4.3 Scheduling Algorithm (Static)

**Phase 1: Dependency Analysis**
```
1. Build dependency DAG from IR edges
2. Compute critical path (longest chain)
3. Identify parallel execution opportunities
```

**Phase 2: Resource Allocation**
```
4. For each node N in topological order:
   a. Check resource availability
   b. Allocate minimum required resources (wavelengths, memory slots)
   c. Reserve resource for N.latency_max_ns duration
```

**Phase 3: Time Assignment**
```
5. For each node N:
   a. Earliest_start = max(dependency_end_times) + propagation_delay
   b. If N requires coherence:
      - Find valid CoherenceWindow W
      - Adjust start_time to fit within W
   c. Latest_start = Earliest_start + slack (for optimization)
   d. Assign: N.start_time = Earliest_start
   e. N.end_time = N.start_time + N.latency_max_ns
```

**Phase 4: Validation**
```
6. Verify all coherence constraints satisfied
7. Check feedback loop latencies ≤ deadlines
8. Validate resource contention resolution
9. Generate ExecutionPlan artifact
```

### 4.4 Resource Contention Resolution

**Priority Levels:**
1. **CRITICAL:** Safety interlocks, coherence-critical operations
2. **HIGH:** Measurement-feedback loops
3. **NORMAL:** Standard compute operations
4. **BACKGROUND:** Calibration, monitoring

**Contention Policy:**
- Higher priority preempts lower priority
- Same priority → FIFO queue
- Preemption triggers re-scheduling of affected subgraph

---

## 5. Measurement-Feedback Latency

### 5.1 Feedback Loop Model

A measurement-feedback loop consists of:
```
[Measure] → [Readout] → [Processing] → [Decision] → [Actuation]
```

**Latency Budget:**
```
L_total = L_measure + L_readout + L_process + L_decision + L_actuation
```

#### Typical Latencies
| Stage | Latency | Technology |
|-------|---------|------------|
| Measure | 0.1-10 ns | SNSPDs, APDs |
| Readout | 1-100 ns | TDC, FPGA |
| Processing | 10-1000 ns | FPGA logic |
| Decision | 1-100 ns | LUT, simple branching |
| Actuation | 0.1-10 ns | EOM switching |

**Total:** **~10 ns - 1 µs** (state-of-art systems)

### 5.2 Deadline Enforcement

**Declaration:**
```rust
pub struct FeedbackLoop {
    pub id: String,
    pub measurement_node: String,
    pub control_node: String,
    pub deadline_ns: u64,           // Hard deadline
    pub priority: Priority,         // Scheduling priority
}
```

**Validation:**
```
For each FeedbackLoop FL:
    path_latency = compute_path_latency(FL.measurement_node, FL.control_node)
    if path_latency > FL.deadline_ns:
        return SchedulingError::DeadlineViolation
```

**Runtime Monitoring:**
- Observability spans track actual latencies
- Violation triggers alert (non-fatal warning) or abort (safety-critical)

---

## 6. Multi-Wavelength Synchronization

### 6.1 Wavelength-Division Multiplexing (WDM)

**Challenge:** Different wavelengths experience different group velocities (chromatic dispersion).

**Dispersion Formula:**
```
Δt = D × L × Δλ
```
- `D`: Dispersion parameter (ps/nm/km)
- `L`: Fiber length (km)
- `Δλ`: Wavelength difference (nm)

**Example:** 
- Fiber: SMF-28 (D ≈ 17 ps/nm/km)
- Length: 10 km
- Wavelengths: 1550 nm, 1551 nm (Δλ = 1 nm)
- Skew: `17 × 10 × 1 = 170 ps`

### 6.2 Synchronization Strategies

#### Pre-Compensation (Static)
- **Method:** Add wavelength-dependent delays before multiplexing
- **Implementation:** Tunable delay lines per channel
- **Accuracy:** ±10 ps (limited by tuning resolution)

#### Active Synchronization (Dynamic)
- **Method:** Measure arrival time skew, adjust delays adaptively
- **Implementation:** Optical correlators, feedback control
- **Accuracy:** ±1 ps (limited by jitter)

#### Software Alignment (Post-Processing)
- **Method:** Timestamp all events, realign in software
- **Implementation:** High-resolution timestamping (TDC)
- **Accuracy:** ±100 ps (limited by TDC precision)

### 6.3 Scheduler Integration

**Wavelength Allocation:**
```rust
pub struct WavelengthChannel {
    pub lambda_nm: f64,             // Center wavelength
    pub bandwidth_ghz: f64,         // Channel bandwidth
    pub dispersion_ps_per_nm: f64,  // Material dispersion
    pub skew_compensation_ns: f64,  // Pre-compensation delay
}
```

**Scheduling Rule:**
```
For operations requiring multi-wavelength coherence:
    1. Allocate wavelengths from same dispersion group
    2. Apply skew compensation to align arrival times
    3. Validate: |arrival_time[i] - arrival_time[j]| < coherence_time / 10
```

---

## 7. Deterministic Scheduling for Reproducibility

### 7.1 Determinism Requirements

**Seed-Driven Decisions:**
- All non-deterministic scheduling choices (e.g., resource tie-breaking) use seeded PRNG
- Same IR + same seed → identical `ExecutionPlan`

**Captured State:**
```rust
pub struct SchedulerState {
    pub seed: u64,
    pub algorithm_version: String,  // "static_v0.1", "dynamic_v0.1"
    pub resource_state_snapshot: HashMap<String, ResourceState>,
    pub coherence_windows: Vec<CoherenceWindow>,
}
```

**Artifact Serialization:**
- `ExecutionPlan` includes full `SchedulerState`
- Replay: Deserialize `SchedulerState`, re-run scheduler
- Verification: Compare original vs replayed plan (must be bit-identical)

### 7.2 Replay Validation

**Test Protocol:**
```
1. Run scheduler with seed S → Plan₁
2. Re-run scheduler with seed S → Plan₂
3. Assert: Plan₁ == Plan₂ (byte-for-byte)
4. Serialize Plan₁ → artifact
5. Deserialize artifact → Plan₃
6. Assert: Plan₁ == Plan₃
```

**CI Enforcement:**
- Integration test: `test_scheduler_determinism`
- Randomized seed test (1000 iterations)
- No flaky failures tolerated

---

## 8. Safety-Critical Timing Constraints

### 8.1 Hard Real-Time Guarantees

**Definition:** Operations with **hard deadlines** that cannot be violated without system failure.

**Examples:**
- **Laser power interlock:** Must shut down within 100 ns if over-power detected
- **Cryogenic qubit decoherence:** Feedback must complete within 1 µs
- **Photodetector saturation:** Must gate input within 10 ns

**Scheduler Requirement:**
- **Worst-case execution time (WCET) analysis**
- **Priority inversion prevention** (priority inheritance protocol)
- **Preemptive scheduling** for critical operations

### 8.2 Constraint Declaration

```rust
pub struct TimingConstraint {
    pub id: String,
    pub constraint_type: ConstraintType,
    pub bound_ns: u64,              // Deadline or minimum separation
    pub violation_action: ViolationAction,
}

pub enum ConstraintType {
    HardDeadline,                   // Must complete before bound_ns
    MinimumSeparation(String, String), // Min time between two nodes
    MaximumLatency(String, String), // Max path latency
}

pub enum ViolationAction {
    Abort,                          // Terminate execution
    Alert,                          // Log warning, continue
    Degrade,                        // Reduce performance (e.g., lower fidelity)
}
```

**Validation:**
```
For each TimingConstraint TC:
    worst_case_time = compute_worst_case(TC)
    if worst_case_time > TC.bound_ns:
        if TC.violation_action == Abort:
            return SchedulingError::HardDeadlineViolation
```

---

## 9. Runtime Scheduling Contracts

### 9.1 Scheduler Trait

```rust
pub trait Scheduler: Send + Sync {
    /// Generate execution plan from IR and constraints
    fn schedule(
        &self,
        graph: &Graph,
        constraints: &SchedulingConstraints,
        seed: u64,
    ) -> Result<ExecutionPlan>;

    /// Validate existing plan against current resource state
    fn validate_plan(
        &self,
        plan: &ExecutionPlan,
        current_state: &ResourceState,
    ) -> Result<()>;

    /// Adapt plan dynamically (for measurement-conditioned execution)
    fn adapt_plan(
        &self,
        plan: &mut ExecutionPlan,
        branch_outcome: &MeasurementOutcome,
    ) -> Result<()>;
}
```

### 9.2 ExecutionPlan Schema

```rust
pub struct ExecutionPlan {
    pub id: String,
    pub seed: u64,
    pub algorithm: String,          // Scheduling algorithm used
    pub makespan_ns: u64,           // Total execution time
    pub critical_path: Vec<String>, // Bottleneck nodes
    pub schedule: HashMap<String, ScheduledNode>,
    pub resource_usage: ResourceUsageReport,
    pub provenance: HashMap<String, String>,
}

pub struct ScheduledNode {
    pub node_id: String,
    pub start_time_ns: u64,
    pub end_time_ns: u64,
    pub allocated_resources: Vec<ResourceAllocation>,
    pub coherence_window_id: Option<String>,
}

pub struct ResourceAllocation {
    pub resource_type: String,      // "wavelength", "memory_slot", "device"
    pub resource_id: String,
    pub start_ns: u64,
    pub end_ns: u64,
}
```

---

## 10. Scheduling Observability

### 10.1 Scheduler Metrics

**Emitted by Scheduler:**
```
- scheduling_latency_ns: Time to compute ExecutionPlan
- makespan_ns: Total execution time (critical path length)
- parallelism_factor: Average parallel nodes per time slice
- resource_utilization: % of available resources used
- coherence_window_violations: Count of rejected operations
- feedback_loop_latencies: Distribution of measured latencies
```

**Integration with Observability System:**
- All metrics reported via `MetricsSink`
- Scheduler spans tracked in timeline (dedicated "Scheduler" lane)
- Critical path highlighted in timeline visualization

### 10.2 Timeline Integration

**Scheduler Lane Events:**
```json
{
  "lane": "Scheduler",
  "events": [
    {"type": "schedule_start", "time_ns": 0, "metadata": {"graph_nodes": 42}},
    {"type": "dependency_analysis", "time_ns": 1000, "metadata": {"critical_path_length": 15}},
    {"type": "resource_allocation", "time_ns": 2000, "metadata": {"allocations": 38}},
    {"type": "coherence_validation", "time_ns": 3000, "metadata": {"windows_validated": 5}},
    {"type": "schedule_complete", "time_ns": 4000, "metadata": {"makespan_ns": 125000}}
  ]
}
```

---

## 11. Advanced Scheduling Topics

### 11.1 Speculative Execution

**Scenario:** Measurement outcome unknown until runtime, but downstream operations can be pre-scheduled.

**Strategy:**
- Pre-compute schedules for all possible measurement outcomes
- Start preparing resources speculatively
- Commit to branch once measurement resolves

**Risk:** Wasted resources if wrong branch prepared

### 11.2 Pipeline Scheduling

**Scenario:** Repetitive operations (e.g., batch processing, iterative algorithms)

**Strategy:**
- Overlap computation of iteration `i+1` with readout of iteration `i`
- Requires double-buffering of resources
- Achieves higher throughput at cost of increased latency

### 11.3 Power-Aware Scheduling

**Objective:** Minimize laser power consumption while meeting performance targets

**Strategy:**
- Cluster wavelength allocations to reduce number of active lasers
- Schedule power-hungry operations during off-peak times
- Use lower-power modes when fidelity requirements permit

---

## 12. Scheduler Implementation Guidelines

### 12.1 Reference Implementation

**Module:** `awen-runtime/src/scheduler/`

**Files:**
```
scheduler/
  mod.rs           - Scheduler trait, ExecutionPlan struct
  static.rs        - StaticScheduler (topological + list scheduling)
  dynamic.rs       - DynamicScheduler (measurement-conditioned)
  realtime.rs      - RealtimeScheduler (priority-based, preemptive)
  constraints.rs   - Constraint validation logic
  resources.rs     - Resource allocation tracker
```

### 12.2 Testing Requirements

**Unit Tests:**
- `test_topological_sort_determinism`
- `test_resource_contention_resolution`
- `test_coherence_window_enforcement`
- `test_feedback_deadline_validation`
- `test_wavelength_skew_compensation`

**Integration Tests:**
- `test_schedule_simple_mzi_chain`
- `test_schedule_measurement_feedback_loop`
- `test_schedule_multi_wavelength_coherence`
- `test_schedule_deterministic_replay`
- `test_schedule_under_resource_pressure`

**Conformance Tests:**
- `test_execution_plan_serialization`
- `test_scheduler_state_reproducibility`
- `test_critical_path_correctness`

---

## 13. Migration Path from v0.1 to v0.2

**v0.1 Scope (Current):**
- Static scheduling only
- Single coherence window per execution
- Fixed resource set
- Best-effort latency (no hard real-time)

**v0.2 Planned Additions:**
- Dynamic scheduler for measurement-conditioned branching
- Multiple overlapping coherence windows
- Elastic resource allocation (cloud-burst)
- Hard real-time guarantees with WCET analysis

**Compatibility:**
- v0.1 `ExecutionPlan` remains valid in v0.2
- v0.2 adds optional fields (backward compatible)
- Scheduler algorithm version tracked in provenance

---

## 14. Example: Scheduling an MZI Feedback Loop

### 14.1 IR Fragment

```json
{
  "nodes": [
    {"id": "laser", "type": "Source", "params": {"wavelength_nm": 1550}},
    {"id": "mzi_1", "type": "MZI", "params": {"phase_upper": 0.0}},
    {"id": "detector", "type": "Detector", "params": {}},
    {"id": "decision", "type": "Conditional", "params": {}},
    {"id": "mzi_2", "type": "MZI", "params": {"phase_upper": 0.0}}
  ],
  "edges": [
    {"src": "laser.out", "dst": "mzi_1.in", "delay_ns": 5.0},
    {"src": "mzi_1.out", "dst": "detector.in", "delay_ns": 10.0},
    {"src": "detector.out", "dst": "decision.in", "delay_ns": 50.0},
    {"src": "decision.out_0", "dst": "mzi_2.in", "delay_ns": 20.0}
  ],
  "feedback_loops": [
    {
      "id": "adaptive_phase",
      "measurement_node": "detector",
      "control_node": "mzi_2",
      "deadline_ns": 100
    }
  ]
}
```

### 14.2 Scheduling Steps

**Step 1: Dependency Analysis**
```
Critical path: laser → mzi_1 → detector → decision → mzi_2
Total latency: 5 + 10 + 50 + 20 = 85 ns (within 100 ns deadline ✓)
```

**Step 2: Resource Allocation**
```
t=0:      Laser starts
t=5:      MZI_1 receives photon
t=15:     Detector receives photon (measurement begins)
t=25:     Measurement complete, result available
t=75:     Decision logic complete (50 ns processing)
t=95:     MZI_2 phase updated (20 ns actuation)
```

**Step 3: Coherence Window Validation**
```
Coherence required: laser → mzi_1 (5 ns duration)
Coherence window: [0, 1000 ns] with T₂ = 10 µs
Fidelity at t=5: exp(-5/10000) ≈ 0.9995 ✓
```

**Step 4: Feedback Validation**
```
Loop latency: 95 - 15 = 80 ns
Deadline: 100 ns
Slack: 20 ns ✓
```

### 14.3 Generated ExecutionPlan

```json
{
  "id": "exec-plan-001",
  "seed": 42,
  "algorithm": "static_v0.1",
  "makespan_ns": 95,
  "schedule": {
    "laser": {"start_time_ns": 0, "end_time_ns": 5},
    "mzi_1": {"start_time_ns": 5, "end_time_ns": 15},
    "detector": {"start_time_ns": 15, "end_time_ns": 25},
    "decision": {"start_time_ns": 25, "end_time_ns": 75},
    "mzi_2": {"start_time_ns": 75, "end_time_ns": 95}
  },
  "feedback_loops": [
    {
      "id": "adaptive_phase",
      "measured_latency_ns": 80,
      "deadline_ns": 100,
      "status": "satisfied"
    }
  ]
}
```

---

## 15. Appendix: Glossary

- **Makespan:** Total execution time from first operation to last completion
- **Critical Path:** Longest dependency chain determining minimum makespan
- **Slack:** Available time margin before deadline violation
- **Jitter:** Random variation in timing (typically Gaussian distribution)
- **Skew:** Systematic timing offset (e.g., dispersion-induced)
- **WCET:** Worst-Case Execution Time (maximum possible latency)
- **Priority Inversion:** High-priority task blocked by low-priority task holding resource
- **Preemption:** Interrupting lower-priority task to run higher-priority task

---

## 16. References

- [AEP-0009: Quantum Coherence](../aeps/AEP-0009-quantum-coherence.md)
- [computation-model.md](computation-model.md) — Section 4: Time & coherence semantics
- [memory-state.md](memory-state.md) — Section 3: Lifetime semantics
- Real-Time Systems textbook (Liu & Layland, 1973) — Scheduling theory
- Quantum Error Correction (Nielsen & Chuang) — Coherence time analysis

---

**End of Specification**
