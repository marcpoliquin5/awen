# AWEN Engine Execution Core v0.2

**Purpose**

Define the Engine as the **mandatory, non-bypassable execution chokepoint** for all AWEN computation graphs. The Engine enforces:
- Coherence window validation before quantum operations
- Calibration state integration (initial + adaptive recalibration)
- Measurement-conditioned branching with latency guarantees
- Observability instrumentation (spans, metrics, events)
- Artifact emission (deterministic, reproducible)
- Safety constraint enforcement
- Deterministic replay via seeded execution

**Scope**

This spec defines:
- IR Graph execution model
- Execution plan generation (topological sort + feedback scheduling)
- Node execution semantics (classical, quantum, measurement, calibration)
- Measurement-conditioned control flow
- Coherence window management during execution
- Safety violation detection and handling
- Artifact bundle generation (non-bypassable)
- Deterministic replay contract
- Engine integration with HAL, Scheduler, Calibration, Observability

---

## 1. Execution Model Overview

### 1.1 Mandatory Engine Chokepoint

All AWEN computation, regardless of domain (classical photonics, quantum photonics, calibration), flows through:

```
Engine.run_graph(
  graph: &ComputationGraph,
  calibration_state: Option<&CalibrationState>,
  backend: Option<&dyn QuantumBackend>,
  execution_mode: ExecutionMode,
  seed: Option<u64>
) -> Result<ExecutionResult>
```

**Non-Bypassable:** Direct kernel/HAL calls bypass observability, safety, and artifact capture. This is not allowed in production AWEN usage.

### 1.2 Execution Domains

```
enum ExecutionDomain {
  ClassicalField {
    // Deterministic photonic computation
    // Input/output: complex amplitudes
    propagation_model: PropagationModel,
  },
  Quantum {
    // Probabilistic quantum computation
    // Input/output: quantum states
    backend: Box<dyn QuantumBackend>,
    noise_model: Option<NoiseModel>,
  },
  Calibration {
    // Automated parameter tuning
    // Input: cost function + measurement sequence
    cost_function: CostFunction,
    optimizer: OptimizerAlgorithm,
  },
  Measurement {
    // Quantum measurement with classical outcome
    basis: MeasurementBasis,
    post_measurement_action: PostMeasurementAction,
  },
}

enum ExecutionMode {
  Experimental,
    // Use live hardware + physical randomness
    // Non-reproducible (hardware noise, true RNG)
  
  DeterministicReplay,
    // Use captured noise model + seeded RNG
    // Reproducible: same seed → same execution
  
  Simulator,
    // Use reference simulator
    // Deterministic, controlled noise
}

enum PropagationModel {
  DirectMode,  // Direct mode arithmetic (MZI, phase shifter)
  ScatteringMatrix,  // Unitary S-matrix propagation
  WaveEquation,  // PDE-based propagation (for future extensions)
}

enum PostMeasurementAction {
  Collapse,  // Standard measurement collapse (destructive)
  Continue,  // Non-destructive measurement
  ConditionalBranching {
    branches: HashMap<MeasurementOutcomePredicate, NodeId>,
    default_branch: Option<NodeId>,
  },
}
```

---

## 2. Computation Graph (IR Execution Semantics)

### 2.1 ComputationGraph Structure

```
pub struct ComputationGraph {
  pub graph_id: String,
  pub nodes: Vec<ComputationNode>,
  pub edges: Vec<Edge>,  // Dataflow: output port → input port
  pub temporal_edges: Vec<TemporalEdge>,  // Time dependencies: node A before node B
  pub measurement_feedback_edges: Vec<MeasurementFeedbackEdge>,
  pub root_nodes: Vec<String>,  // Entry points
  pub leaf_nodes: Vec<String>,  // Exit points (outputs)
}

pub struct ComputationNode {
  pub id: String,
  pub node_type: NodeType,
  pub parameters: HashMap<String, ParameterValue>,
  pub input_ports: Vec<Port>,
  pub output_ports: Vec<Port>,
  pub domain: ExecutionDomain,
  pub timing_contract: TimingContract,
  pub safety_constraints: Option<SafetyConstraint>,
}

pub enum NodeType {
  // Classical photonic nodes
  MZI { extinction_ratio_target: f64 },
  PhaseShifter { dynamic: bool },
  BeamSplitter { 50_50: bool },
  Coupler { coupling_ratio: f64 },
  Delay { latency_ns: u64 },
  
  // Quantum nodes
  QuantumGate { gate_name: String },
  QuantumMeasure { basis: MeasurementBasis },
  StatePreparation { prep_kind: PreparationKind },
  
  // Control nodes
  Calibration { cost_function: CostFunction },
  Conditional { predicate: ConditionalPredicate },
  
  // Memory nodes
  DelayBufferRead { buffer_id: String },
  DelayBufferWrite { buffer_id: String },
  ResonatorRead { resonator_id: String },
}

pub struct Port {
  pub port_id: String,
  pub mode_label: String,
  pub data_type: DataType,
}

pub enum DataType {
  ClassicalMode(Complex),  // Single complex amplitude
  QuantumMode(QuantumState),  // Full quantum state
  QuantumMeasurement(MeasurementOutcome),  // Classical outcome from measurement
  CalibrationParameter(f64),  // Single real parameter
}

pub struct TimingContract {
  pub duration_ns: u64,  // How long node takes to execute
  pub coherence_requirement_ns: Option<u64>,  // How much coherence time required
  pub feedback_latency_budget_ns: Option<u64>,  // Max time between measurement & feedback
}

pub struct Edge {
  pub from_node: String,
  pub from_port: String,
  pub to_node: String,
  pub to_port: String,
}

pub struct TemporalEdge {
  pub from_node: String,
  pub to_node: String,
  pub constraint: TemporalConstraint,
}

pub enum TemporalConstraint {
  Immediately,  // Node B starts right after A finishes
  WithDelay(u64),  // B starts N nanoseconds after A finishes
  Synchronous,  // A and B must be synchronized (coherence requirement)
}

pub struct MeasurementFeedbackEdge {
  pub measurement_node: String,
  pub dependent_node: String,
  pub max_latency_ns: u64,
}
```

### 2.2 IR Validation

Before execution, the Engine must validate:

```
fn validate_graph(graph: &ComputationGraph) -> Result<()> {
  // 1. Acyclic check: no data-flow cycles (classical)
  //    (feedback loops allowed if they use measurement outcomes)
  
  // 2. Port type compatibility: output DataType matches input DataType
  
  // 3. Coherence feasibility: for quantum nodes, can coherence windows overlap?
  
  // 4. Timing feasibility: can measurement feedback fit in coherence windows?
  
  // 5. Memory feasibility: do delay buffers/resonators have sufficient capacity?
  
  // 6. Measurement-conditioned branching: 
  //    - all branches have valid destinations
  //    - all predicates are unambiguous
  
  // 7. Calibration integration:
  //    - cost functions reference valid measurement nodes
  //    - optimizer hyperparameters are sensible
}
```

---

## 3. Execution Plan Generation

### 3.1 ExecutionPlan Data Structure

```
pub struct ExecutionPlan {
  pub plan_id: String,
  pub graph_id: String,
  pub phases: Vec<ExecutionPhase>,
  pub total_duration_ns: u64,
  pub resource_allocation: ResourceAllocation,
  pub coherence_windows: Vec<CoherenceWindow>,
}

pub struct ExecutionPhase {
  pub phase_id: usize,
  pub nodes_to_execute: Vec<String>,
  pub is_parallel: bool,  // Can these nodes run in parallel?
  pub temporal_constraints: Vec<TemporalConstraint>,
  pub duration_ns: u64,
  pub resource_requirements: ResourceRequirements,
}

pub struct ResourceAllocation {
  pub optical_modes: HashMap<String, ModeAllocation>,
  pub memory_buffers: HashMap<String, MemoryAllocation>,
  pub quantum_backend_time: Option<u64>,
}

pub struct ResourceRequirements {
  pub num_optical_modes: usize,
  pub num_memory_buffers: usize,
  pub requires_quantum_backend: bool,
  pub requires_calibration: bool,
}
```

### 3.2 Plan Generation Algorithm

```
fn generate_execution_plan(graph: &ComputationGraph) -> Result<ExecutionPlan> {
  // 1. Topological sort of nodes (data-flow DAG)
  let topo_order = topological_sort(&graph.edges)?;
  
  // 2. Coherence window analysis:
  //    - Identify quantum nodes
  //    - Compute minimum coherence time needed
  //    - Check temporal constraints
  
  // 3. Measurement-conditioned branching:
  //    - For each measurement node, compute branches
  //    - Allocate time budget for feedback latency
  //    - Build conditional execution paths
  
  // 4. Scheduling:
  //    - Group nodes into phases (sequential or parallel)
  //    - Enforce temporal edges (A before B)
  //    - Respect feedback loop deadlines
  
  // 5. Resource allocation:
  //    - Assign optical modes to nodes
  //    - Map memory buffers
  //    - Compute total execution duration
  
  Ok(ExecutionPlan { /* ... */ })
}
```

---

## 4. Node Execution Semantics

### 4.1 Classical Photonic Node Execution

```
fn execute_classical_node(
  node: &ComputationNode,
  inputs: HashMap<String, Complex>,
  calibration_state: &CalibrationState,
) -> Result<HashMap<String, Complex>> {
  
  let params = node.parameters.merge_with_calibration(calibration_state)?;
  
  match node.node_type {
    NodeType::MZI { .. } => {
      // MZI: U = exp(i φ_upper) * [cos(θ), sin(θ); -sin(θ), cos(θ)] * exp(i φ_lower)
      let phase_upper = params.get("phase_upper")?;
      let theta = params.get("theta")?;
      let phase_lower = params.get("phase_lower")?;
      
      let input_a = inputs.get("in_0")?;
      let input_b = inputs.get("in_1")?;
      
      let out_0 = exp(i * phase_upper) * (cos(theta) * input_a + sin(theta) * input_b);
      let out_1 = exp(i * phase_upper) * (-sin(theta) * input_a + cos(theta) * input_b) * exp(i * phase_lower);
      
      Ok(hashmap!{
        "out_0".to_string() => out_0,
        "out_1".to_string() => out_1,
      })
    },
    
    NodeType::PhaseShifter { .. } => {
      let phase = params.get("phase")?;
      Ok(inputs.iter()
        .map(|(port, amp)| (port.clone(), amp * exp(i * phase)))
        .collect())
    },
    
    // ... other classical nodes
  }
}
```

### 4.2 Quantum Node Execution

```
fn execute_quantum_node(
  node: &ComputationNode,
  state: &mut QuantumState,
  backend: &mut dyn QuantumBackend,
  coherence_mgr: &CoherenceManager,
  seed: u64,
) -> Result<(QuantumState, QuantumEvent)> {
  
  // Check coherence window is still valid
  if !state.is_coherent() {
    return Err(anyhow!("Coherence deadline exceeded"));
  }
  
  match node.node_type {
    NodeType::QuantumGate { gate_name } => {
      let gate = parse_gate_name(&gate_name)?;
      backend.apply_gate(state, &gate, seed)?;
      Ok((state.clone(), QuantumEvent::GateApplied { gate_name }))
    },
    
    NodeType::QuantumMeasure { basis } => {
      let outcome = backend.measure(state, &basis, seed)?;
      let event = QuantumEvent::MeasurementPerformed { outcome_id: outcome.outcome_id.clone() };
      
      // Update state: post-measurement state (possibly collapsed)
      if let Some(post_state) = &outcome.post_measurement_state {
        *state = post_state.clone();
      }
      
      Ok((state.clone(), event))
    },
    
    NodeType::StatePreparation { prep_kind } => {
      *state = backend.prepare(state.mode_labels.clone(), prep_kind, seed)?;
      Ok((state.clone(), QuantumEvent::StateCreated { state_id: state.state_id.clone() }))
    },
  }
}
```

### 4.3 Measurement-Conditioned Branching

```
fn execute_conditional_branch(
  measurement_outcome: &MeasurementOutcome,
  branches: &HashMap<MeasurementOutcomePredicate, String>,  // predicate -> node_id
) -> Result<String> {
  
  for (predicate, target_node) in branches {
    if matches_predicate(measurement_outcome, predicate) {
      return Ok(target_node.clone());
    }
  }
  
  Err(anyhow!("No branch matches measurement outcome"))
}

fn matches_predicate(
  outcome: &MeasurementOutcome,
  predicate: &MeasurementOutcomePredicate,
) -> bool {
  match predicate {
    MeasurementOutcomePredicate::Equals(value) => {
      &outcome.classical_results[0] == value
    },
    MeasurementOutcomePredicate::InRange(min, max) => {
      match &outcome.classical_results[0] {
        MeasurementResult::ContinuousValue(v) => *v >= *min && *v <= *max,
        _ => false,
      }
    },
    _ => false,
  }
}
```

---

## 5. Coherence Window Management During Execution

### 5.1 Coherence Tracking

```
struct ExecutionContext {
  current_coherence_window: CoherenceWindow,
  quantum_state: Option<QuantumState>,
  time_elapsed_ns: u64,
  time_budget_remaining_ns: u64,
}

impl ExecutionContext {
  fn check_coherence_before_quantum_op(
    &self,
    op_duration_ns: u64,
  ) -> Result<()> {
    
    if let Some(state) = &self.quantum_state {
      if !state.is_coherent_at(Utc::now()) {
        return Err(anyhow!("Quantum state incoherent"));
      }
    }
    
    // Check time budget
    if op_duration_ns > self.time_budget_remaining_ns {
      return Err(anyhow!(
        "Operation exceeds coherence budget: {} > {}",
        op_duration_ns,
        self.time_budget_remaining_ns
      ));
    }
    
    Ok(())
  }
  
  fn update_after_node_execution(&mut self, node: &ComputationNode) -> Result<()> {
    self.time_elapsed_ns += node.timing_contract.duration_ns;
    self.time_budget_remaining_ns -= node.timing_contract.duration_ns;
    
    // Update quantum state timestamp
    if let Some(state) = &mut self.quantum_state {
      state.timestamp = Utc::now();
    }
    
    Ok(())
  }
}
```

---

## 6. Safety Constraint Enforcement

### 6.1 Safety Validation

```
fn validate_safety_constraints(
  node: &ComputationNode,
  parameters: &HashMap<String, f64>,
  state: &Option<QuantumState>,
) -> Result<()> {
  
  if let Some(constraint) = &node.safety_constraints {
    // Hard limits
    for (param_name, limit) in &constraint.hard_limits {
      let value = parameters.get(param_name)?;
      if value > limit {
        return Err(anyhow!(
          "Hard limit violated: {} = {} exceeds {}",
          param_name, value, limit
        ));
      }
    }
    
    // Soft limits (warning only)
    for (param_name, limit) in &constraint.soft_limits {
      let value = parameters.get(param_name)?;
      if value > limit {
        emit_warning!("Soft limit warning: {} = {}", param_name, value);
      }
    }
    
    // Quantum state safety (coherence + fidelity)
    if let Some(quantum_state) = state {
      if !quantum_state.is_coherent() {
        return Err(anyhow!("Quantum state incoherent: cannot execute"));
      }
      
      if quantum_state.compute_purity() < constraint.min_fidelity {
        return Err(anyhow!(
          "State fidelity {} below minimum {}",
          quantum_state.compute_purity(),
          constraint.min_fidelity
        ));
      }
    }
  }
  
  Ok(())
}
```

---

## 7. Observability Integration

### 7.1 Span Instrumentation

```
fn execute_with_instrumentation<F>(
  engine_context: &EngineContext,
  node_id: &str,
  duration_ns: u64,
  execute_fn: F,
) -> Result<()>
where
  F: FnOnce() -> Result<()>,
{
  let tracer = &engine_context.observability.tracer;
  let mut span = tracer.start_span(
    &format!("engine.execute_node"),
    span_attributes!{
      "node_id" => node_id,
      "duration_ns" => duration_ns,
      "coherence_remaining_ns" => engine_context.coherence_budget_remaining,
      "subsystem" => "engine",
    }
  );
  
  let result = execute_fn();
  
  if let Err(e) = &result {
    span.set_attribute("error", true);
    span.set_attribute("error_message", e.to_string());
  }
  
  span.end();
  result
}
```

### 7.2 Metrics Emission

```
fn emit_execution_metrics(
  metrics: &MetricsSink,
  node_id: &str,
  duration_ns: u64,
  success: bool,
) {
  metrics.counter("engine.nodes_executed").increment();
  metrics.counter("engine.total_duration_ns").add(duration_ns);
  
  if success {
    metrics.counter("engine.nodes_succeeded").increment();
  } else {
    metrics.counter("engine.nodes_failed").increment();
  }
  
  metrics.histogram("engine.node_duration_ns").record(duration_ns);
}
```

---

## 8. Artifact Emission (Non-Bypassable)

### 8.1 ExecutionResult & Artifact Bundle

```
pub struct ExecutionResult {
  pub execution_id: String,
  pub graph_id: String,
  pub seed: u64,
  pub execution_mode: ExecutionMode,
  pub start_timestamp: DateTime<Utc>,
  pub end_timestamp: DateTime<Utc>,
  pub total_duration_ns: u64,
  
  pub outputs: HashMap<String, OutputValue>,
  pub measurement_outcomes: Vec<MeasurementOutcome>,
  pub state_trajectory: Vec<QuantumState>,
  
  pub execution_plan: ExecutionPlan,
  pub node_execution_log: Vec<NodeExecutionLog>,
  
  pub calibration_state_initial: Option<CalibrationState>,
  pub calibration_state_final: Option<CalibrationState>,
  pub recalibrations_triggered: usize,
  
  pub safety_violations: Vec<SafetyViolation>,
  pub coherence_violations: Vec<CoherenceViolation>,
  
  pub traces: Vec<Span>,
  pub metrics: HashMap<String, MetricValue>,
  pub events: Vec<ExecutionEvent>,
}

pub struct NodeExecutionLog {
  pub node_id: String,
  pub start_timestamp: DateTime<Utc>,
  pub end_timestamp: DateTime<Utc>,
  pub duration_ns: u64,
  pub inputs: HashMap<String, String>,  // serialized
  pub outputs: HashMap<String, String>,  // serialized
  pub success: bool,
  pub error: Option<String>,
}

pub struct SafetyViolation {
  pub node_id: String,
  pub constraint: String,
  pub actual_value: f64,
  pub limit: f64,
  pub timestamp: DateTime<Utc>,
}
```

### 8.2 Artifact Generation

```
fn finalize_and_emit_artifact(
  engine_context: &EngineContext,
  result: &ExecutionResult,
) -> Result<ArtifactBundle> {
  
  // 1. Create artifact bundle
  let mut bundle = ArtifactBundle::new(result.execution_id.clone());
  
  // 2. Store execution result
  bundle.store_json("execution_result.json", result)?;
  
  // 3. Store observability data
  bundle.store_jsonl("traces.jsonl", &result.traces)?;
  bundle.store_json("metrics.json", &result.metrics)?;
  bundle.store_jsonl("events.jsonl", &result.events)?;
  
  // 4. Store computation graph (IR)
  bundle.store_json("ir/executed_graph.json", &engine_context.graph)?;
  
  // 5. Store calibration state
  if let Some(cal_state) = &result.calibration_state_final {
    bundle.store_json("calibration/final.json", cal_state)?;
  }
  
  // 6. Compute deterministic ID (SHA256 of inputs)
  bundle.compute_deterministic_id(
    &engine_context.graph,
    &result.seed,
    &result.calibration_state_initial,
  )?;
  
  // 7. Generate citation
  bundle.generate_citation()?;
  
  // 8. Sign bundle (optional)
  bundle.sign()?;
  
  Ok(bundle)
}
```

---

## 9. Deterministic Replay Contract

### 9.1 Replay Requirement

```
pub struct ReplaySpec {
  pub artifact_bundle_id: String,
  pub seed: u64,
  pub noise_model_id: Option<String>,
  pub calibration_version: Option<String>,
  pub backend_name: String,
}

impl ReplaySpec {
  fn enable_deterministic_replay(&self) -> Result<()> {
    // Verify all required fields are present
    if self.seed == 0 {
      return Err(anyhow!("Seed must be non-zero"));
    }
    
    if self.noise_model_id.is_none() {
      return Err(anyhow!("Noise model required for deterministic replay"));
    }
    
    Ok(())
  }
}

fn replay_execution(spec: &ReplaySpec) -> Result<ExecutionResult> {
  // 1. Load original artifact bundle
  let bundle = ArtifactBundle::load(&spec.artifact_bundle_id)?;
  let original_graph = bundle.load_graph()?;
  let original_seed = spec.seed;
  
  // 2. Restore execution context (same seed, noise model, calibration)
  let execution_mode = ExecutionMode::DeterministicReplay;
  
  // 3. Re-run graph with deterministic seed
  let engine = Engine::new();
  let result = engine.run_graph(
    &original_graph,
    original_seed,
    execution_mode,
  )?;
  
  // 4. Verify outputs match original
  let original_result = bundle.load_execution_result()?;
  if result.outputs != original_result.outputs {
    return Err(anyhow!("Replay outputs differ from original!"));
  }
  
  Ok(result)
}
```

---

## 10. Error Handling & Violation Recovery

### 10.1 Violation Types

```
pub enum ExecutionViolation {
  CoherenceExpired {
    state_id: String,
    deadline: DateTime<Utc>,
  },
  SafetyLimitExceeded {
    node_id: String,
    parameter: String,
    value: f64,
    limit: f64,
  },
  MeasurementFeedbackTimeout {
    measurement_id: String,
    max_latency_ns: u64,
  },
  MemoryBufferExhausted {
    buffer_id: String,
  },
}

enum ViolationRecoveryStrategy {
  Abort,  // Stop execution immediately
  Alert,  // Emit warning but continue (soft constraint)
  Recalibrate,  // Try to recalibrate and retry
}
```

### 10.2 Recovery Logic

```
fn handle_violation(
  violation: ExecutionViolation,
  recovery_strategy: ViolationRecoveryStrategy,
  calibration_executor: &CalibrationExecutor,
) -> Result<()> {
  
  match recovery_strategy {
    ViolationRecoveryStrategy::Abort => {
      Err(anyhow!("Execution aborted due to violation: {:?}", violation))
    },
    
    ViolationRecoveryStrategy::Alert => {
      emit_warning!("Non-fatal violation: {:?}", violation);
      Ok(())
    },
    
    ViolationRecoveryStrategy::Recalibrate => {
      match &violation {
        ExecutionViolation::SafetyLimitExceeded { node_id, .. } => {
          // Trigger recalibration for node
          calibration_executor.recalibrate_node(node_id)?;
          Ok(())
        },
        _ => Err(anyhow!("Cannot recover from: {:?}", violation)),
      }
    },
  }
}
```

---

## 11. Integration with AWEN Subsystems

### 11.1 Calibration Integration

Engine calls `calibration_executor.get_current_state()` before node execution and applies calibration parameters:

```
let calibration_state = calibration_executor.get_current_calibration()?;
let calibrated_params = node.parameters.apply_calibration(&calibration_state)?;

// Monitor for drift during execution
let drift_report = drift_detector.detect_drift(&calibration_state)?;
if drift_report.urgency >= Urgency::High {
  // Trigger async recalibration
  calibration_executor.schedule_recalibration_async()?;
}
```

### 11.2 Scheduler Integration

Engine uses Scheduler to build execution plan:

```
let scheduler = Scheduler::new();
let execution_plan = scheduler.build_plan(
  &graph,
  &coherence_manager,
  &resource_allocator,
)?;
```

### 11.3 Observability Integration

Every node execution emits spans, metrics, events:

```
let span = tracer.start_span("engine.execute_node", attributes);
// ... execute ...
span.end();

metrics.counter("engine.nodes_executed").increment();
events.log(ExecutionEvent::NodeCompleted { node_id, duration_ns });
```

### 11.4 Memory Integration

Engine coordinates with memory subsystem:

```
let memory = MemoryController::new();
memory.allocate_buffer("delay_buffer_0", DelayBuffer { latency_ns: 100 })?;
// ... execute nodes ...
memory.free_buffer("delay_buffer_0")?;
```

---

## 12. Engine State Machine

```
pub enum EngineState {
  Idle,
  ValidatingGraph,
  GeneratingPlan,
  Executing {
    current_phase: usize,
    nodes_completed: usize,
    total_nodes: usize,
  },
  Finalizing,
  Emitting,
  Complete,
  Failed(String),
}

impl Engine {
  fn transition(&mut self, new_state: EngineState) {
    emit_event!(EngineStateChange { from: self.state, to: new_state });
    self.state = new_state;
  }
}
```

---

## 13. Complete Engine Execution Flow

```
pub fn run_graph(
  &mut self,
  graph: &ComputationGraph,
  calibration_state: Option<&CalibrationState>,
  backend: Option<&dyn QuantumBackend>,
  seed: Option<u64>,
) -> Result<ExecutionResult> {
  
  let run_seed = seed.unwrap_or_else(|| thread_rng().gen());
  let run_id = Uuid::new_v4().to_string();
  
  // 1. Validate IR graph
  self.transition(EngineState::ValidatingGraph);
  validate_graph(graph)?;
  
  // 2. Generate execution plan
  self.transition(EngineState::GeneratingPlan);
  let plan = self.scheduler.build_plan(graph)?;
  
  // 3. Initialize execution context
  let mut exec_ctx = ExecutionContext {
    run_id: run_id.clone(),
    graph: graph.clone(),
    seed: run_seed,
    start_time: Utc::now(),
    coherence_mgr: self.coherence_manager.clone(),
    quantum_state: None,
    quantum_backend: backend,
    calibration_state: calibration_state.cloned(),
    observability: self.observability.clone(),
  };
  
  // 4. Execute phases
  self.transition(EngineState::Executing { current_phase: 0, nodes_completed: 0, total_nodes: plan.nodes.len() });
  
  for (phase_idx, phase) in plan.phases.iter().enumerate() {
    for node_id in &phase.nodes_to_execute {
      let node = graph.nodes.iter().find(|n| &n.id == node_id)?;
      
      // Execute node with full instrumentation
      self.execute_node(&mut exec_ctx, node)?;
      
      exec_ctx.nodes_completed += 1;
      self.transition(EngineState::Executing {
        current_phase: phase_idx,
        nodes_completed: exec_ctx.nodes_completed,
        total_nodes: plan.nodes.len(),
      });
    }
  }
  
  // 5. Finalize execution result
  self.transition(EngineState::Finalizing);
  let mut result = ExecutionResult {
    execution_id: run_id.clone(),
    graph_id: graph.graph_id.clone(),
    seed: run_seed,
    start_timestamp: exec_ctx.start_time,
    end_timestamp: Utc::now(),
    total_duration_ns: (Utc::now() - exec_ctx.start_time).num_nanoseconds().unwrap_or(0) as u64,
    outputs: exec_ctx.outputs,
    measurement_outcomes: exec_ctx.measurement_outcomes,
    state_trajectory: exec_ctx.state_trajectory,
    execution_plan: plan,
    node_execution_log: exec_ctx.execution_log,
    calibration_state_initial: calibration_state.cloned(),
    calibration_state_final: exec_ctx.calibration_state.clone(),
    recalibrations_triggered: exec_ctx.recalibrations_count,
    safety_violations: exec_ctx.safety_violations,
    coherence_violations: exec_ctx.coherence_violations,
    traces: exec_ctx.observability.traces(),
    metrics: exec_ctx.observability.metrics(),
    events: exec_ctx.observability.events(),
  };
  
  // 6. Emit artifact bundle
  self.transition(EngineState::Emitting);
  let artifact = finalize_and_emit_artifact(&exec_ctx, &result)?;
  result.artifact_id = artifact.artifact_id.clone();
  
  self.transition(EngineState::Complete);
  
  Ok(result)
}
```

---

## 14. Engine Conformance Checklist

All Engine implementations must satisfy:

- [ ] ComputationGraph IR execution with topological sorting
- [ ] ExecutionPlan generation (phases, parallelism, temporal constraints)
- [ ] Classical photonic node execution (MZI, PhaseShifter, BeamSplitter, Delay)
- [ ] Quantum node execution with QuantumBackend trait
- [ ] Measurement-conditioned branching with predicates
- [ ] Coherence window enforcement (deadline checks before quantum ops)
- [ ] Calibration state integration (read + apply during execution)
- [ ] Safety constraint validation (hard + soft limits)
- [ ] Observability instrumentation (spans, metrics, events)
- [ ] Non-bypassable artifact emission (deterministic bundle ID)
- [ ] Deterministic replay (same seed → same execution)
- [ ] Error handling and violation recovery
- [ ] Integration with Scheduler, Calibration, HAL, Memory subsystems
- [ ] ≥20 integration tests covering all node types + error conditions
- [ ] CI gate: engine-conformance job validates all above

---

## References

- AEP-0001: AWEN Computation Model
- computation-model.md (Section 1-7)
- observability.md (Integration sections)
- reproducibility.md (Artifact semantics)
- calibration.md (Cost functions, optimization)
- quantum-runtime.md (Backend interface, measurement model)
- timing-scheduling.md (Temporal constraints, coherence windows)
