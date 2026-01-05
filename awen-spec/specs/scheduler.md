% Scheduler v0.1 - Dynamic Execution Planning for Photonics Circuits
% AWEN Architecture Specification
% 2026-01-06

# Scheduler v0.1: Dynamic Execution Planning

**Phase:** 2, Section 2.2  
**Version:** 0.1  
**Status:** SPECIFICATION (In Development)  
**Last Updated:** 2026-01-06

## Overview

## 1. Overview

The **Scheduler** is the execution planning layer that generates optimal schedules for photonic computation graphs within coherence windows. It transforms a computation graph into an ExecutionPlan suitable for execution by the Engine, respecting temporal constraints, coherence deadlines, and resource availability.

### 1.1 Design Principles

1. **Engine Integration**: Generates ExecutionPlan for Engine.run_graph() consumption
2. **Coherence-Aware**: All schedules respect coherence window deadlines (T₁/T₂)
3. **Resource-Conscious**: Tracks available photonic resources (waveguides, couplers, detectors)
4. **Measurement-Aware**: Schedules measurement-conditioned branches with feedback latencies
5. **Optimizable**: Supports multiple scheduling strategies (static, dynamic, greedy, optimal)
6. **Non-Blocking**: Provides feedback to calibration for timing adjustments

### 1.2 Phase 2.1 vs Phase 2.2 Evolution

| Aspect | Phase 1.4 (v0.1) | Phase 2.2 (v0.1) |
|--------|------------------|------------------|
| **Core Type** | StaticScheduler | DynamicScheduler (expands static) |
| **Input** | Graph | Graph + Coherence window + Resources |
| **Output** | ExecutionPlan | ExecutionPlan with resource allocation |
| **Scheduling** | Topological sort | Topological sort + dynamic adjustment |
| **Constraints** | Temporal edges | Temporal edges + coherence deadlines + feedback latencies |
| **Measurement** | Ignored | Conditional branching with feedback paths |
| **Resources** | None | Waveguide, coupler, detector allocation |
| **Optimization** | None | Critical path, fidelity-aware scheduling |

---

## 2. Execution Planning

### 2.1 ExecutionPlan Structure (Input from Engine)

The Scheduler consumes Engine's ExecutionPlan and optionally refines it:

```rust
pub struct ExecutionPlan {
    pub plan_id: String,                      // Unique plan identifier
    pub graph_id: String,                     // Source graph ID
    pub phases: Vec<ExecutionPhase>,          // Ordered execution phases
    pub total_duration_ns: u64,               // Total execution time
    pub resource_allocation: Option<ResourceAllocation>,  // [NEW] Phase 2.2
}

pub struct ExecutionPhase {
    pub phase_id: usize,                      // Phase number
    pub nodes_to_execute: Vec<String>,        // Node IDs in this phase
    pub is_parallel: bool,                    // Can nodes run in parallel?
    pub duration_ns: u64,                     // Phase duration
    pub resource_requirements: Option<Vec<ResourceRequirement>>,  // [NEW]
    pub coherence_deadline_ns: Option<u64>,   // [NEW] When must this phase complete?
}

pub struct ResourceAllocation {
    pub waveguides: HashMap<String, Vec<String>>,    // node_id → [waveguide assignments]
    pub couplers: HashMap<String, String>,           // node_id → coupler
    pub detectors: HashMap<String, String>,          // measurement_id → detector
    pub priority: Vec<String>,                       // Execution priority order
}

pub struct ResourceRequirement {
    pub resource_type: ResourceType,
    pub count: usize,
    pub exclusive: bool,  // Cannot share with other nodes
}

pub enum ResourceType {
    Waveguide,
    Coupler,
    Detector,
    Memory,
}
```

### 2.2 Scheduling Algorithm

The Scheduler uses a **constraint-aware topological sort with coherence deadline propagation**:

```
1. INPUT: ComputationGraph, CoherenceWindow, AvailableResources
   
2. BUILD dependency graph
   - For each edge: src_node → dst_node (temporal dependency)
   - For each measurement feedback edge: (feedback latency)
   
3. PROPAGATE coherence deadlines backward from leaves
   - Each node gets a "must complete by" time
   - Accounts for node execution time + downstream requirements
   - Example: If leaf node (detector) must complete by 10ms, and
     it depends on a gate that takes 1μs, gate must complete by 9.999ms
   
4. TOPOLOGICAL SORT with phase assignment
   - Start with root nodes (no dependencies)
   - Assign to earliest phase where all dependencies are satisfied
   - Group nodes into phases (depth-first or breadth-first)
   
5. RESOURCE ALLOCATION
   - For each phase, allocate available resources to nodes
   - Respect resource limits (only N waveguides, M couplers)
   - Track exclusive vs. shared resource requirements
   
6. MEASUREMENT-CONDITIONAL BRANCH SCHEDULING
   - For measurement-conditioned edges:
     * Schedule measurement first
     * After feedback latency (typically 100-500ns), schedule branches
     * Ensure coherence budget covers measurement + latency + branch execution
   
7. OPTIMIZATION (optional, Phase 2.2+)
   - Minimize total execution time (critical path)
   - Maximize fidelity by avoiding tight coherence deadlines
   - Balance resource utilization
   
8. OUTPUT: ExecutionPlan with:
   - Ordered phases
   - Resource assignments
   - Coherence deadlines per phase
   - Feedback latency tracking
```

### 2.3 Scheduling Strategies

#### 2.3.1 StaticScheduler (Phase 1.4, retained)

**Properties:**
- Deterministic (same graph → same schedule every time)
- No feedback from execution
- Fast (O(V + E) complexity)
- Conservative (ignores some optimization opportunities)

**Use Case:** Predictable circuits, development/testing

**Example:**
```rust
let scheduler = StaticScheduler::new();
let plan = scheduler.schedule(&graph)?;
// Returns fixed ExecutionPlan, always same for same graph
```

#### 2.3.2 DynamicScheduler (NEW, Phase 2.2)

**Properties:**
- Adaptive (responds to runtime information)
- Feedback-aware (integrates with Engine execution results)
- Slower (O(V² + E) in worst case, typically better)
- Optimized (maximizes fidelity, minimizes time)

**Use Case:** Long circuits, measurement-driven algorithms, optimization loops

**Feedback Integration:**
```rust
pub struct SchedulingFeedback {
    pub actual_execution_time: u64,
    pub fidelity_achieved: f64,
    pub coherence_consumed: u64,
    pub resource_contention: bool,
}

impl DynamicScheduler {
    pub fn schedule_with_feedback(
        &mut self,
        graph: &ComputationGraph,
        previous_feedback: Option<SchedulingFeedback>,
    ) -> Result<ExecutionPlan> {
        // Adjust schedule based on previous execution feedback
        // Example: If previous run had resource contention, space out nodes more
        // Example: If coherence budget was tight, move nodes earlier
    }
}
```

#### 2.3.3 GreedyScheduler (Future, Phase 2.3)

**Properties:**
- Fast (O(V log V + E))
- Good for simple cases
- Not optimal for complex graphs

#### 2.3.4 OptimalScheduler (Future, Phase 2.4+)

**Properties:**
- Slow (NP-hard in general case)
- Produces optimal schedule (minimum makespan)
- Uses branch-and-bound or dynamic programming

---

## 3. Coherence Window Management in Scheduling

### 3.1 Coherence Deadline Propagation

For a given ComputationGraph with coherence window W:

```
1. Leaf node L executes at time T_L
   → Must complete by: T_L + 1ns (essentially immediately)
   
2. Parent node P of L:
   → P executes at time T_P
   → P must complete before L starts
   → Must complete by: T_L - EDGE_LATENCY
   
3. Propagate backward:
   → Each node N on path to root has "deadline" D_N
   → D_N = min(deadlines of children) - execution_time_N - edge_latency
   
4. Check feasibility:
   → If any D_N < 0, coherence window is too short
   → Scheduler fails or requests longer window
```

### 3.2 Example: Mach-Zehnder Interferometer

```
Circuit:
  Input(0ns) → MZI(phase) → Detector(200ns) → Output

Coherence window: 10ms

Scheduling:
  Phase 0: Input (0ns)
           └─ Deadline: 10ms
  
  Phase 1: MZI (1μs execution)
           └─ Deadline: 10ms - 1μs = 9.999ms
  
  Phase 2: Detector (500ns execution)
           └─ Deadline: 10ms - 500ns = 9.9995ms
  
Status: ✅ All deadlines satisfied within 10ms window
```

---

## 4. Measurement-Conditioned Scheduling

### 4.1 Conditional Branching Structure

A measurement-conditioned branch has:
- **Measurement node M**: Measures quantum state
- **Feedback latency L_f**: Time from measurement to outcome availability
- **Conditional nodes C₀, C₁, ...**: Different branches based on outcome

```
Circuit:
  [...] → Measure(M) → branch_0 (if outcome==0) → [...]
                    → branch_1 (if outcome==1) → [...]
                    → branch_2 (if outcome>=2) → [...]
```

### 4.2 Scheduling with Feedback Latency

```
1. Schedule measurement M at time T_M
   
2. Measurement takes 500ns (execution)
   
3. Feedback latency L_f = 100ns (outcome available at T_M + 500ns + 100ns = T_M + 600ns)
   
4. Schedule branches starting at T_M + 600ns
   - branch_0 starts at T_M + 600ns
   - branch_1 starts at T_M + 600ns
   - branch_2 starts at T_M + 600ns
   
5. All branches must complete within coherence window
```

### 4.3 Conservative Scheduling (Phase 2.2)

For safety, Phase 2.2 schedules all branches in sequence (conservative):

```
Phase 1: Measure at time 0
         Branches wait for outcome (100ns latency)
         
Phase 2: Execute branch_0 (path for outcome==0)
         Scheduled at time 600ns
         
Phase 3: Execute branch_1 (path for outcome==1)
         Scheduled at time 600ns + branch_0_duration
         
Phase 4: Execute branch_2 (path for outcome>=2)
         Scheduled at time 600ns + branch_0_duration + branch_1_duration
```

Future versions (Phase 2.4+) can do **aggressive scheduling** where branches run in parallel with dedicated resources.

---

## 5. Resource Allocation

### 5.1 Resource Model

Each photonic circuit has limited resources:

```rust
pub struct Photonica {
    pub waveguides: usize,        // Number of parallel waveguides
    pub couplers: usize,          // Number of tunable couplers
    pub detectors: usize,         // Number of detectors
    pub memory_elements: usize,   // Delay buffers, resonators
}

// Example: Silicon photonic chip
let device = Photonica {
    waveguides: 8,
    couplers: 4,
    detectors: 2,
    memory_elements: 3,
};
```

### 5.2 Resource Allocation Algorithm

```
1. For each phase in ExecutionPlan:
   
2. For each node in phase:
   - If node is MZI: Allocate 1 coupler (exclusive)
   - If node is Detector: Allocate 1 detector (exclusive)
   - If node is Delay: Allocate 1 memory element (exclusive)
   - If node is Classical gate: Uses existing waveguide routing
   
3. Check feasibility:
   - Sum couplers ≤ available couplers?
   - Sum detectors ≤ available detectors?
   - Sum memory ≤ available memory?
   
4. If infeasible:
   - Move nodes to later phases (split phase)
   - OR request additional resources
   - OR fail with clear error message
```

### 5.3 Waveguide Routing

For multi-waveguide chips, the Scheduler tracks routing:

```rust
pub struct WaveguideRoute {
    pub start_waveguide: usize,
    pub end_waveguide: usize,
    pub path: Vec<usize>,         // Intermediate waveguides
    pub crossings: usize,         // Number of waveguide crossings
    pub loss_db: f64,             // Routing loss
}

// Allocation respects:
// 1. No resource conflicts (same waveguide can't route two signals in same phase)
// 2. Minimal loss (prefer short paths)
// 3. Minimal crossings (reduce cross-talk)
```

---

## 6. Integration with Engine

### 6.1 Scheduler ↔ Engine Interface

```
┌─────────────────────────────────────────────┐
│         Application/Algorithm               │
└────────────────────┬────────────────────────┘
                     │ ComputationGraph
                     ▼
        ┌────────────────────────┐
        │     SCHEDULER v0.1     │ ← Generates plan
        │  - StaticScheduler     │  - Allocates resources
        │  - DynamicScheduler    │  - Checks coherence
        └────────────┬───────────┘
                     │ ExecutionPlan
                     ▼
        ┌────────────────────────┐
        │    ENGINE v0.2         │ ← Executes plan
        │  - Validates graph     │  - Enforces safety
        │  - Runs phases         │  - Manages coherence
        └────────────┬───────────┘
                     │ ExecutionResult
                     ▼
        ┌────────────────────────┐
        │   Observability/Artifacts
        │  - Spans, metrics
        │  - Result bundle
        └────────────────────────┘
```

### 6.2 Scheduler Registration

```rust
pub trait SchedulingStrategy: Send + Sync {
    fn schedule(
        &self,
        graph: &ComputationGraph,
        device: Option<&Photonica>,
        coherence_window_ns: u64,
    ) -> Result<ExecutionPlan>;
    
    fn name(&self) -> &str;
    fn is_deterministic(&self) -> bool;
}

pub struct SchedulerRegistry {
    strategies: HashMap<String, Arc<dyn SchedulingStrategy>>,
}

impl SchedulerRegistry {
    pub fn new() -> Self { ... }
    pub fn register(&mut self, name: &str, strategy: Arc<dyn SchedulingStrategy>) { ... }
    pub fn get(&self, name: &str) -> Option<Arc<dyn SchedulingStrategy>> { ... }
}
```

---

## 7. Configuration & Tuning

### 7.1 Scheduling Options

```rust
pub struct SchedulingConfig {
    // Strategy selection
    pub strategy: SchedulingStrategy,  // Static, Dynamic, Greedy
    pub optimization_level: u8,        // 0-3 (higher = slower but better)
    
    // Coherence tuning
    pub min_coherence_margin_ns: u64,  // How much margin to leave before deadline?
    pub assume_feedback_latency_ns: u64, // Expected measurement feedback latency
    
    // Resource constraints
    pub available_waveguides: usize,
    pub available_couplers: usize,
    pub available_detectors: usize,
    
    // Optimization objectives
    pub minimize_makespan: bool,        // Minimize total execution time
    pub maximize_fidelity: bool,        // Prefer schedules that avoid tight deadlines
    pub minimize_resource_usage: bool,  // Prefer sparse schedules
    
    // Constraints
    pub max_phase_duration_ns: u64,
    pub max_total_duration_ns: u64,
}

impl Default for SchedulingConfig {
    fn default() -> Self {
        Self {
            strategy: SchedulingStrategy::Static,
            optimization_level: 1,
            min_coherence_margin_ns: 100_000,  // 100μs margin
            assume_feedback_latency_ns: 100,   // 100ns
            available_waveguides: 8,
            available_couplers: 4,
            available_detectors: 2,
            minimize_makespan: true,
            maximize_fidelity: true,
            minimize_resource_usage: false,
            max_phase_duration_ns: 1_000_000,  // 1ms per phase
            max_total_duration_ns: 10_000_000, // 10ms total
        }
    }
}
```

### 7.2 Schedule Validation

```rust
pub struct ScheduleValidator {
    config: SchedulingConfig,
}

impl ScheduleValidator {
    pub fn validate(&self, plan: &ExecutionPlan, graph: &ComputationGraph) -> Result<Vec<Warning>> {
        let mut warnings = Vec::new();
        
        // 1. Check all nodes scheduled
        // 2. Check coherence deadlines satisfied
        // 3. Check resource requirements met
        // 4. Check measurement feedback latencies
        // 5. Check temporal constraints
        // 6. Check fidelity margins
        
        if warnings.is_empty() {
            Ok(warnings)
        } else {
            Err(anyhow!("{} schedule validation warnings", warnings.len()))
        }
    }
}
```

---

## 8. Conformance Requirements

### 8.1 Definition-of-Done (18 items)

- [ ] Spec-first: scheduler.md complete (14+ sections)
- [ ] DynamicScheduler implementation with multi-strategy support
- [ ] Coherence deadline propagation algorithm
- [ ] Measurement-conditional branch scheduling
- [ ] Resource allocation with feasibility checking
- [ ] StaticScheduler retention from Phase 1.4
- [ ] SchedulingConfig with comprehensive options
- [ ] Integration with Engine.run_graph()
- [ ] Error handling (infeasible schedules, deadline violations)
- [ ] Performance metrics (scheduling time, plan quality)
- [ ] 30+ integration tests covering all strategies
- [ ] Measurement-conditional scheduling tests
- [ ] Resource allocation tests
- [ ] Coherence deadline validation tests
- [ ] Deterministic scheduling tests
- [ ] Dynamic scheduling with feedback tests
- [ ] scheduler-conformance CI/CD job
- [ ] Documentation (README section, SECTIONS.md update)

### 8.2 Testing Strategy

**Test Categories:**

1. **Strategy Tests (8 tests)**
   - StaticScheduler determinism
   - DynamicScheduler with feedback
   - Resource allocation correctness
   - Coherence deadline feasibility

2. **Measurement-Conditional Tests (6 tests)**
   - Simple branching (if/else)
   - Multi-branch (switch)
   - Nested conditionals
   - Feedback latency handling

3. **Coherence Tests (6 tests)**
   - Deadline propagation correctness
   - Tight windows (should fail)
   - Loose windows (should succeed)
   - Complex graphs with multiple branches

4. **Resource Tests (5 tests)**
   - Single resource exhaustion
   - Multiple resource exhaustion
   - Optimal allocation
   - Infeasible circuits

5. **Integration Tests (5 tests)**
   - End-to-end scheduling to execution
   - Engine integration
   - Feedback loop integration
   - Large circuits (100+ nodes)

### 8.3 Metrics

| Metric | Target |
|--------|--------|
| Specification lines | 1200+ |
| Implementation lines | 800+ |
| Test cases | 30+ |
| CI validation steps | 12+ |
| Code coverage | >90% |
| Documentation | SECTIONS.md, README, examples |

---

## 9. Future Enhancements (Phase 2.3+)

### 9.1 Phase 2.3: Advanced Scheduling
- GreedyScheduler (O(V log V) fast scheduling)
- Parallel branch execution (aggressive scheduling for conditionals)
- Loop unrolling and feedback optimization

### 9.2 Phase 2.4: Optimal Scheduling
- OptimalScheduler (branch-and-bound)
- Integer linear programming formulation
- Stochastic optimization (simulated annealing)

### 9.3 Phase 2.5: Hardware-Specific Scheduling
- Device-aware scheduling (chip-specific resource models)
- Cross-talk minimization
- Temperature-aware scheduling

---

## 10. Glossary

| Term | Definition |
|------|-----------|
| **ExecutionPlan** | Ordered list of phases, each with nodes to execute and resource allocations |
| **Phase** | Set of nodes that can execute in parallel, respecting dependencies |
| **Coherence Deadline** | Time by which a node must complete before quantum state decoheres |
| **Feedback Latency** | Time from measurement to outcome availability |
| **Resource** | Physical element (waveguide, coupler, detector) required by node |
| **Makespan** | Total time from start to finish of execution |
| **Fidelity** | Quantum state quality (1.0 = perfect, 0.0 = completely mixed) |

---

## References

- AEP-0001: Computation Model
- AEP-0003: Kernel Semantics
- AEP-0009: Quantum Coherence
- Engine.md (Phase 2.1 Specification)
- Timing-Scheduling.md (Phase 1.4 Specification)

---

**Version:** 0.1  
**Status:** SPECIFICATION (Ready for Implementation)  
**Date:** 2026-01-06  
**Next Phase:** Scheduler runtime implementation (scheduler_v0.rs)
