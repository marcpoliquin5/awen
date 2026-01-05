# AWEN Quantum Execution Substrate v0.1

**Purpose**

Define the complete quantum photonics execution model for AWEN, enabling:
- Classical and quantum photonic computation in unified framework
- CV (continuous-variable) and DV (discrete-variable) quantum state semantics
- Measurement-conditioned feedback with coherence-aware scheduling
- Deterministic replay via seeded RNG and full provenance
- Backend agnosticism (simulators, lab systems, production hardware)
- Non-destructive quantum state snapshots for debugging and artifact capture

**Scope**

This spec defines:
- QuantumState abstract representation (CV/DV)
- State preparation, evolution, measurement models
- Measurement-conditioned branching and feedback semantics
- Coherence window enforcement and decoherence tracking
- Quantum event observability (measurement outcomes, state collapses)
- Backend interface contracts
- Deterministic seeding and replay requirements
- Integration with AWEN artifacts and reproducibility framework
- Quantum memory semantics (integration with classical memory model)

---

## 1. Quantum Photonic State Model

### 1.1 Classical-Field vs. Quantum-Photonic Domains

AWEN supports two fundamental execution domains:

**Classical-Field (CF) Domain:**
- Deterministic photonic computation using classical complex amplitudes
- No randomness (except noise models treated as perturbations)
- No entanglement or superposition
- Execution: optical gates, interferometers, phase modulators

**Quantum Photonic Domain:**
- Probabilistic computation with superposition and entanglement
- Quantum state represented as density operator ρ (general) or statevector |ψ⟩ (pure)
- Measurement outcomes drawn from Born rule
- Decoherence and noise treated as quantum channels
- Execution: state preparation, unitary gates, measurements, feedback

Both domains may be active simultaneously in a single circuit (hybrid computation).

### 1.2 QuantumState Abstraction

All quantum state in AWEN is accessed via the `QuantumState` interface:

```
interface QuantumState {
  // Metadata
  state_id: UUID
  state_type: StateType  // CV | DV
  mode_labels: Vec<String>  // addressable modes
  timestamp: Timestamp
  seed: u64  // for deterministic replay
  hardware_revision: String
  calibration_id: UUID
  coherence_deadline: Timestamp  // absolute time beyond which state degrades
  
  // State access (non-destructive)
  snapshot() -> StateSnapshot
    // Returns fidelity-limited snapshot (some systems can't read full state)
    // Includes: reduced densities, purity, entanglement metrics
    
  fidelity_with(other: &QuantumState) -> f64
    // F = ⟨ψ|ρ|ψ⟩ or Tr(√(√ρ₁ ρ₂ √ρ₁))
    // Used for state comparison in testing
    
  can_measure_basis(basis: MeasurementBasis) -> bool
    // Whether hardware supports this measurement
}

enum StateType {
  CV,  // Continuous-variable (Gaussian, squeezed, displaced modes)
  DV,  // Discrete-variable (qubit/qudit registers)
}

struct StateSnapshot {
  mode_labels: Vec<String>,
  reduced_densities: HashMap<String, DensityMatrix>,  // per-mode
  global_purity: f64,  // Tr(ρ²) ∈ [0,1]
  entanglement_entropy: HashMap<(String, String), f64>,  // pairwise entropy
  noise_floor: f64,  // estimated noise level
  timestamp: Timestamp,
  provenance: QuantumStateProvenance,
}

struct QuantumStateProvenance {
  state_id: UUID,
  parent_state_id: Option<UUID>,  // state before latest operation
  operation: QuantumOperation,  // what created this state
  seed: u64,
  hardware_revision: String,
  calibration_id: UUID,
}

enum QuantumOperation {
  Preparation { kernel_id: String },
  Unitary { gate_name: String, parameters: HashMap<String, f64> },
  Measurement { outcome: MeasurementOutcome },
  Evolution { duration_ns: u64, noise_channel: String },
  Recombination { from_states: Vec<UUID> },  // for feedback branches
}
```

### 1.3 CV (Continuous-Variable) State

CV modes represent Gaussian states with quadrature operators (Q, P):

```
struct CVMode {
  label: String,
  displacement: Complex,  // (α_Q, α_P) = (⟨Q⟩, ⟨P⟩)
  squeezing: Squeezing,   // squeeze parameter
  thermal_photons: f64,   // ⟨a†a⟩ for thermal noise
}

struct Squeezing {
  amount_db: f64,  // squeezing magnitude (dB)
  angle_rad: f64,  // squeezing direction in (Q,P) plane
}

struct CVState {
  modes: HashMap<String, CVMode>,
  covariance: CovarianceMatrix,  // 2N×2N matrix of ⟨ΔQ ΔP⟩ correlations
  is_gaussian: bool,  // false if entanglement with non-Gaussian resources
}

impl CVState {
  // Apply single-mode Gaussian gate
  fn apply_gaussian_gate(&mut self, mode: &str, matrix: &GaussianGate) -> Result<()>;
  
  // Apply two-mode gate (beam splitter, two-mode squeezer, etc.)
  fn apply_two_mode_gate(&mut self, mode1: &str, mode2: &str, 
                         gate: &TwoModeGate) -> Result<()>;
  
  // Measure in Homodyne basis (Q or P quadrature)
  fn measure_homodyne(&mut self, mode: &str, basis: HomodyneAxis) 
    -> Result<(f64, Option<f64>)>;
    // Returns: (measured_value, optional_vacuum_noise)
  
  // Measure in heterodyne basis (both Q and P)
  fn measure_heterodyne(&mut self, mode: &str) 
    -> Result<(f64, f64)>;  // Q and P outcomes
}

enum HomodyneAxis {
  Q,
  P,
  Rotated(f64),  // arbitrary phase rotation
}

enum TwoModeGate {
  BeamSplitter { reflectivity: f64, phase: f64 },
  TwoModeSqueezer { amount_db: f64, phase: f64 },
  Parametric { frequency_shift: f64, coupling: f64 },
}
```

**CV Example (Gaussian Boson Sampling variant):**
- State: 8 squeezed modes at 10 dB squeezing
- Evolution: 5 ns in delay buffer (adds thermal noise ~0.1 photons)
- Measurement: Homodyne on all modes (balanced detectors)
- Result: classical outcomes sampled from Gaussian distribution

### 1.4 DV (Discrete-Variable) State

DV modes represent qubit/qudit registers with logical Hilbert space structure:

```
struct DVState {
  qudits: HashMap<String, Qudit>,  // label -> qudit state
  local_basis: HashMap<String, LocalBasis>,  // measurement basis per qudit
  entanglement_graph: AdjacencyMatrix,  // tracks bipartite/multipartite entanglement
}

struct Qudit {
  label: String,
  dimension: usize,  // 2 = qubit, 3 = qutrit, etc.
  amplitudes: Vec<Complex>,  // |ψ⟩ = Σ_j α_j |j⟩
  purity: f64,  // Tr(ρ²) for mixed states
}

impl DVState {
  // Apply unitary gate on single qudit
  fn apply_gate(&mut self, qudit: &str, gate: &UnitaryMatrix) -> Result<()>;
  
  // Apply controlled gate (CNOT, CZ, etc.)
  fn apply_controlled_gate(&mut self, control: &str, target: &str,
                          controlled_gate: &UnitaryMatrix) -> Result<()>;
  
  // Measure in computational basis
  fn measure_computational(&mut self, qudit: &str) -> Result<usize>;
  // Returns: basis state index (0 to dimension-1)
  
  // Measure in arbitrary basis
  fn measure_basis(&mut self, qudit: &str, basis: &UnitaryMatrix) 
    -> Result<usize>;
}

enum LocalBasis {
  Computational,
  Hadamard,  // |+⟩, |-⟩
  Rotated(f64),  // parameterized by angle
  Arbitrary(UnitaryMatrix),
}
```

**DV Example (Linear Optical Quantum Computing):**
- State: 4 qubits, prepared in |++++⟩ (Hadamard basis superposition)
- Evolution: CNOT on pairs (0→1, 2→3)
- Measurement: computational basis on all 4 qubits
- Result: 4-bit outcome (deterministic given measurement seed)

### 1.5 Hybrid CV/DV States

Some systems encode quantum information across both CV and DV domains:

```
struct HybridQuantumState {
  cv_component: CVState,
  dv_component: DVState,
  cross_mode_entanglement: Vec<(String, String, f64)>,
    // (cv_mode_label, dv_qudit_label, entanglement_measure)
  phase_correlation: HashMap<String, f64>,  // relative phases between CV and DV
}
```

**Hybrid Example (CV-DV fusion):**
- 4 CV modes squeezed in Q quadrature
- 2 DV qubits in |++⟩ superposition
- Coupling: each qubit controls squeezing angle of 2 CV modes
- Result: measurement-conditioned CV state preparation

---

## 2. State Preparation & Evolution

### 2.1 State Preparation

Preparation is a non-unitary operation that creates quantum state from vacuum:

```
interface StatePreparation {
  prepare(
    modes: Vec<String>,
    parameters: HashMap<String, f64>,
    seed: u64
  ) -> Result<QuantumState>;
}

enum PreparationKind {
  // CV preparations
  DisplacedSqueezed {
    displacement: (f64, f64),  // (α_Q, α_P)
    squeezing_db: f64,
    squeezing_angle: f64,
  },
  Gaussian {
    covariance: CovarianceMatrix,
    displacement: Vec<Complex>,
  },
  ThermalState {
    mean_photons: f64,
  },
  
  // DV preparations
  BasisState {
    state_vector: Vec<Complex>,  // explicit amplitudes
  },
  BellState {
    entanglement_type: BellType,  // |Φ+⟩, |Φ-⟩, |Ψ+⟩, |Ψ-⟩
  },
  GHZState {
    num_qudits: usize,
  },
  
  // Hybrid
  HybridProduct {
    cv_prep: Box<PreparationKind>,
    dv_prep: Box<PreparationKind>,
  },
}

enum BellType {
  PhiPlus,  // (|00⟩ + |11⟩) / √2
  PhiMinus,  // (|00⟩ - |11⟩) / √2
  PsiPlus,  // (|01⟩ + |10⟩) / √2
  PsiMinus,  // (|01⟩ - |10⟩) / √2
}
```

### 2.2 State Evolution

State evolution under Hamiltonian or Lindblad dynamics:

```
interface StateEvolver {
  evolve(
    state: &mut QuantumState,
    duration_ns: u64,
    hamiltonian: &Hamiltonian,
    noise_channels: &[NoiseChannel],
    seed: u64
  ) -> Result<EvolutionTrace>;
}

struct Hamiltonian {
  terms: Vec<PauliTerm>,  // H = Σ c_j σ_j
  static_field: Option<FieldProfile>,  // time-independent part
  modulation: Option<TimeVaryingField>,  // time-dependent drives
}

struct PauliTerm {
  coefficient: Complex,
  operators: HashMap<String, PauliOp>,  // qudit label -> X,Y,Z
}

enum PauliOp {
  I, X, Y, Z,
}

struct TimeVaryingField {
  field_type: FieldType,  // phase, amplitude, detuning
  modulation: ModulationEnvelope,
  frequency: f64,
  amplitude: f64,
}

enum ModulationEnvelope {
  Rectangular { duration_ns: u64 },
  Gaussian { sigma_ns: f64 },
  STIRAP { ramp_rate: f64 },
}

enum NoiseChannel {
  // Single-qudit channels
  Depolarizing { error_rate: f64 },
  PhaseDamping { error_rate: f64 },
  AmplitudeDamping { decay_rate: f64 },
  
  // CV-specific
  ThermalNoise { bath_temp_k: f64, mode_loss: f64 },
  PhaseNoise { spectral_density: f64 },
  
  // Two-body
  CrossMode { mode_pair: (String, String), coupling: f64 },
}

struct EvolutionTrace {
  initial_state_id: UUID,
  final_state_id: UUID,
  operations: Vec<QuantumOperation>,  // chronological list of ops
  decoherence_estimated: f64,  // 1 - final_fidelity
  seed: u64,
}
```

**Evolution Example:**
- Initial: |ψ⟩ = (|0⟩ + |1⟩) / √2
- Hamiltonian: H = ω σ_z / 2 (free precession)
- Duration: 100 ns
- Noise: phase damping (T₂ = 1 μs)
- Result: final state with Rabi oscillation, reduced coherence

---

## 3. Measurement Model

### 3.1 Measurement Types

```
struct MeasurementBasis {
  basis_type: BasisType,
  mode_labels: Vec<String>,
}

enum BasisType {
  // CV measurements
  Homodyne {
    axis: HomodyneAxis,  // Q, P, or rotated
  },
  Heterodyne,  // simultaneous Q and P (noisy)
  HomodyneTomography {
    num_angles: usize,  // 4, 8, 16, ...
  },
  BalancedHomodyne {
    integration_time_ns: u64,
  },
  
  // DV measurements
  Computational,  // |0⟩, |1⟩, ...
  Hadamard,  // |+⟩, |-⟩
  Arbitrary {
    rotation_matrix: UnitaryMatrix,
  },
  
  // Joint measurements
  BellMeasurement {
    qudit_pairs: Vec<(String, String)>,
  },
  ParityMeasurement {
    qudits: Vec<String>,
    parity_type: ParityType,
  },
  
  // Non-destructive
  NonDestructiveQubitParity {
    qudits: Vec<String>,
    backaction: BackactionModel,
  },
}

enum ParityType {
  ZZ,  // product of Z outcomes
  XX,
  YY,
}

enum BackactionModel {
  Perfect,  // no state change after measurement
  ProjectiveBased { fidelity: f64 },  // partial projection
  None,  // measurement doesn't affect state
}

struct MeasurementOutcome {
  outcome_id: UUID,
  measurement_id: UUID,
  mode_labels: Vec<String>,
  measurement_type: BasisType,
  classical_results: HashMap<String, MeasurementResult>,
  timestamp: Timestamp,
  seed: u64,  // for reproducibility
  post_measurement_state: Option<QuantumState>,  // if non-destructive
  reliability: f64,  // confidence in outcome (detector efficiency, etc.)
}

enum MeasurementResult {
  DiscreteOutcome(usize),  // qubit/qudit basis state index
  ContinuousValue(f64),  // homodyne voltage
  DiscreteApproximation {  // discretized homodyne
    bin: usize,
    bin_width: f64,
  },
  BinaryResult(bool),  // parity bit
}

impl MeasurementOutcome {
  fn matches_seed(seed: u64) -> bool {
    // Deterministic: given seed, outcome is always the same
  }
}
```

### 3.2 Measurement Timing & Latency

Measurement latency must be included in scheduling:

```
struct MeasurementLatency {
  detection_latency_ns: u64,  // sensor response time
  electronics_latency_ns: u64,  // ADC, firmware processing
  transport_latency_ns: u64,  // network roundtrip if remote
  total_latency_ns: u64,  // sum
  certainty: f64,  // jitter as percentage of total
}

impl MeasurementBasis {
  fn expected_latency(&self, hardware: &HardwareProfile) 
    -> Result<MeasurementLatency>;
}
```

**Example Latencies:**
- Homodyne: 10-50 ns (sensor) + 5 ns (electronics) + 0 ns (local) = 15-55 ns
- Single-photon detector: 500 ns (dead time) + 10 ns (electronics) = 510 ns
- Remote measurement (cloud): 50 ns (local) + 10 ms (network) = 10.05 ms

---

## 4. Measurement-Conditioned Feedback & Branching

### 4.1 Conditional Execution Model

Measurement outcomes enable classical feedback that controls subsequent quantum operations:

```
struct MeasurementConditionalBranch {
  measurement_id: UUID,
  branches: HashMap<MeasurementOutcomePredicate, QuantumKernel>,
  timeout_ms: u64,  // max time to wait for measurement
  fallback_kernel: Option<QuantumKernel>,  // if timeout
}

enum MeasurementOutcomePredicate {
  Equals(MeasurementResult),  // outcome == specific value
  InRange(f64, f64),  // outcome in [min, max]
  BitValue(usize, bool),  // specific qudit in specific state
  Parity(bool),  // parity == even or odd
  AlwaysTrue,  // unconditional (for deterministic ops)
}

struct ConditionalQuantumKernel {
  id: UUID,
  measurement_dependencies: Vec<UUID>,  // which measurements gate this kernel
  branching_logic: Vec<MeasurementConditionalBranch>,
  post_measurement_latency: u64,  // ns between measurement and next op
  max_feedback_depth: u32,  // prevent infinite loops
}
```

### 4.2 Coherence-Aware Scheduling

Measurement-conditioned feedback must respect coherence windows:

```
struct FeedbackConstraint {
  measurement_outcome_id: UUID,
  gate_to_apply: QuantumGate,
  max_latency_ns: u64,  // deadline for gate to execute after measurement
  coherence_deadline: Timestamp,  // absolute time when coherence expires
  branching_factor: usize,  // how many branches from this measurement
}

enum SchedulingDecision {
  Sequential {
    // Measure, wait for outcome, apply branch gate
    // Latency: measurement_latency + branching_latency + gate_duration
  },
  Probabilistic {
    // Apply all branch gates simultaneously with weights
    // If measurement outcome unknown, use Born rule probabilities
  },
  PrecompensatedFeedback {
    // Pre-apply compensating gates during measurement integration
    // Reduces feedback latency but requires lookahead knowledge
  },
}

impl CoherenceWindow {
  fn can_schedule_feedback(
    &self,
    outcome_time: Timestamp,
    feedback_latency: u64,
    gate_duration: u64
  ) -> bool {
    // Check if outcome + latency + gate_duration < coherence_deadline
  }
}
```

**Example Workflow:**
```
1. Prepare: (|0⟩ + |1⟩) / √2
2. Measure in computational basis → outcome ∈ {0, 1}
3. Conditional feedback:
   - If outcome == 0: apply phase gate (adds 0.1 ns)
   - If outcome == 1: apply Hadamard gate (adds 0.5 ns)
4. Measure again
5. Deadline: 100 ns from preparation
   - Measurement latency: 20 ns
   - Remaining coherence budget: 80 ns
   - Gate duration: ≤ 0.5 ns ✓
```

---

## 5. Coherence Windows & Decoherence Tracking

### 5.1 Coherence Window Model

Every quantum state has a coherence deadline beyond which superposition is no longer reliable:

```
struct CoherenceWindow {
  state_id: UUID,
  initialized_at: Timestamp,
  coherence_time_ns: u64,  // T₂ or equivalent
  deadline: Timestamp,  // initialized_at + coherence_time_ns
  
  // Component times
  T1_ns: Option<u64>,  // amplitude damping time (energy decay)
  T2_ns: Option<u64>,  // phase damping time (dephasing)
  T2_star_ns: Option<u64>,  // inhomogeneous dephasing
  
  // Tolerance for violation
  grace_period_ns: u64,  // how much beyond deadline is acceptable
}

enum CoherenceViolation {
  StateExpired {
    state_id: UUID,
    deadline: Timestamp,
    actual_time: Timestamp,
  },
  InsufficientWindow {
    state_id: UUID,
    required_duration_ns: u64,
    available_duration_ns: u64,
  },
  MultiModeDecoherence {
    states: Vec<UUID>,
    entanglement_lost: bool,
  },
}

interface CoherenceManager {
  check_window(&self, state_id: UUID) -> Result<CoherenceWindow>;
  
  extend_window(&mut self, state_id: UUID, 
                additional_ns: u64) -> Result<()>;
    // e.g., cool the system, reduce noise
  
  check_relative_phase(&self, state1: UUID, state2: UUID) 
    -> Result<f64>;
    // phase coherence between two states (0 = no coherence, 1 = perfect)
}
```

### 5.2 Decoherence Estimation

Decoherence is tracked and estimated via noise models:

```
struct DecoherenceModel {
  model_type: DecoherenceType,
  timestamp: Timestamp,
  parameters: HashMap<String, f64>,
}

enum DecoherenceType {
  Exponential {
    T1_ns: f64,
    T2_ns: f64,
  },
  Gaussian {
    sigma_ns: f64,  // time-dependent dephasing
  },
  EnvironmentalCoupling {
    bath_spectral_density: String,  // e.g., "1/f" or model ID
    coupling_strength: f64,
  },
  Calculated {
    from_master_equation: LindbladdianForm,
  },
}

impl DecoherenceModel {
  fn fidelity_after_delay(&self, delay_ns: u64) -> f64 {
    // Predict state fidelity after propagation delay
    // F(t) = exp(-t/T₂) for exponential model
  }
}
```

---

## 6. Backend Interface Contract

### 6.1 QuantumBackend Trait

All quantum execution flows through a pluggable backend:

```
trait QuantumBackend: Send + Sync {
  // Identification
  fn name(&self) -> &str;  // "gaussian_simulator", "qiskit", "photonq", ...
  fn state_type(&self) -> StateType;  // CV, DV, or hybrid
  
  // Capabilities
  fn supported_bases(&self) -> Vec<BasisType>;
  fn max_modes(&self) -> usize;
  fn coherence_time_ns(&self) -> u64;
  fn measurement_latency(&self) -> MeasurementLatency;
  
  // Execution
  fn prepare(
    &mut self,
    modes: Vec<String>,
    preparation: &PreparationKind,
    seed: u64
  ) -> Result<QuantumState>;
  
  fn evolve(
    &mut self,
    state: &mut QuantumState,
    hamiltonian: &Hamiltonian,
    noise_channels: &[NoiseChannel],
    duration_ns: u64,
    seed: u64
  ) -> Result<EvolutionTrace>;
  
  fn measure(
    &mut self,
    state: &mut QuantumState,
    basis: &MeasurementBasis,
    seed: u64
  ) -> Result<MeasurementOutcome>;
  
  fn apply_gate(
    &mut self,
    state: &mut QuantumState,
    gate: &QuantumGate,
    mode_labels: Vec<String>
  ) -> Result<()>;
  
  // State inspection (non-destructive)
  fn snapshot(&self, state: &QuantumState) -> Result<StateSnapshot>;
  
  fn fidelity(&self, state1: &QuantumState, state2: &QuantumState) 
    -> Result<f64>;
  
  // Resource limits
  fn available_memory_gateops(&self) -> usize;
  
  // Cleanup
  fn release_state(&mut self, state_id: UUID) -> Result<()>;
}

enum QuantumGate {
  // Single-qudit (DV)
  PauliX, PauliY, PauliZ,
  Hadamard,
  SPhase,  // √Z
  T,  // π/8 gate
  Rx(f64),
  Ry(f64),
  Rz(f64),
  
  // Two-qudit
  CNOT(String, String),  // control, target
  CZ(String, String),
  SWAP(String, String),
  Toffoli(String, String, String),  // control1, control2, target
  
  // CV gates
  GaussianGate(String, GaussianGate),  // mode, gate
  TwoModeGate(String, String, TwoModeGate),  // mode1, mode2, gate
  
  // Measurement
  Measurement(String, MeasurementBasis),  // mode, basis
}
```

### 6.2 Backend Registration & Selection

```
struct BackendRegistry {
  backends: HashMap<String, Box<dyn QuantumBackend>>,
  default_backend: String,
}

impl BackendRegistry {
  fn register(&mut self, backend: Box<dyn QuantumBackend>) -> Result<()> {
    self.backends.insert(backend.name().to_string(), backend);
  }
  
  fn select(&mut self, name: &str) -> Result<&mut dyn QuantumBackend> {
    self.backends.get_mut(name)
      .ok_or_else(|| anyhow!("Backend not found: {}", name))
  }
  
  fn auto_select_for(&self, state_type: StateType) 
    -> Result<&dyn QuantumBackend> {
    // Choose best backend for state type
    self.backends.values()
      .filter(|b| b.state_type() == state_type)
      .max_by_key(|b| b.coherence_time_ns())
  }
}
```

---

## 7. Observability & Quantum Events

### 7.1 Quantum Events

Every quantum operation emits observable events:

```
enum QuantumEvent {
  StateCreated {
    state_id: UUID,
    state_type: StateType,
    num_modes: usize,
    timestamp: Timestamp,
  },
  
  StateEvolved {
    state_id: UUID,
    operation: QuantumOperation,
    duration_ns: u64,
    estimated_fidelity: f64,
    timestamp: Timestamp,
  },
  
  MeasurementPerformed {
    state_id: UUID,
    measurement_type: BasisType,
    outcome: MeasurementOutcome,
    measurement_latency_ns: u64,
    timestamp: Timestamp,
  },
  
  StateCollapsed {
    state_id: UUID,
    post_measurement_state: Option<QuantumState>,
    timestamp: Timestamp,
  },
  
  CoherenceViolation {
    state_id: UUID,
    violation: CoherenceViolation,
    timestamp: Timestamp,
  },
  
  DriftDetected {
    state_id: UUID,
    calibration_version: UUID,
    drift_magnitude: f64,
    recommended_action: String,
  },
}

impl QuantumEvent {
  fn emit_to_tracer(self, tracer: &Tracer) -> Result<()>;
}
```

### 7.2 Quantum Metrics

```
struct QuantumMetrics {
  fidelity_estimate: f64,  // state purity
  coherence_remaining_ns: u64,  // time until deadline
  decoherence_rate: f64,  // dB/ns
  measurement_confidence: f64,  // SNR
  entanglement_entropy: HashMap<(String, String), f64>,
}
```

---

## 8. Deterministic Replay & Reproducibility

### 8.1 Seeding & Determinism

All quantum randomness is seeded for reproducibility:

```
struct QuantumSeed {
  base_seed: u64,
  per_operation_seed: HashMap<UUID, u64>,  // operation_id -> seed
}

impl QuantumSeed {
  fn derive_for_operation(operation_id: UUID) -> u64;
    // Deterministic: same operation_id -> same seed
  
  fn advance(&mut self);
    // Increment to next operation's seed
}

enum RandomnessSource {
  Seeded(u64),  // reproducible
  Hardware,  // non-reproducible (true randomness)
}
```

### 8.2 Quantum Artifact Capture

Every quantum computation must capture full provenance:

```
struct QuantumArtifact {
  artifact_id: UUID,
  kernel_id: String,
  execution_id: UUID,
  
  // Quantum data
  initial_state: QuantumState,
  final_state: QuantumState,
  intermediate_states: Vec<StateSnapshot>,
  measurement_outcomes: Vec<MeasurementOutcome>,
  
  // Reproducibility data
  seed: u64,
  backend_name: String,
  backend_version: String,
  noise_model_id: Option<UUID>,
  calibration_version: UUID,
  hardware_revision: String,
  
  // Timing
  start_timestamp: Timestamp,
  end_timestamp: Timestamp,
  total_duration_ns: u64,
  
  // Observability
  events: Vec<QuantumEvent>,
  metrics: QuantumMetrics,
}

impl QuantumArtifact {
  fn can_deterministic_replay(&self) -> bool {
    self.seed.is_some() && 
    self.backend_name.is_some() &&
    self.noise_model_id.is_some()
  }
  
  fn replay(&self, engine: &Engine) -> Result<QuantumState> {
    // Deterministically regenerate final_state given artifact
  }
}
```

---

## 9. Quantum Module Integration with AWEN Runtime

### 9.1 Engine Integration

The Engine must be aware of quantum state and coherence:

```
impl Engine {
  fn run_graph_with_quantum(
    &mut self,
    graph: &ComputationGraph,
    initial_quantum_state: Option<QuantumState>,
    backend: Box<dyn QuantumBackend>
  ) -> Result<EngineOutput> {
    // 1. Validate coherence windows
    self.validate_coherence_windows(&graph)?;
    
    // 2. Schedule measurement-conditioned branches
    let schedule = self.schedule_with_quantum_feedback(&graph)?;
    
    // 3. Execute nodes, emitting quantum events
    for node in schedule {
      if node.is_quantum {
        self.execute_quantum_node(&node, &mut backend)?;
      } else {
        self.execute_classical_node(&node)?;
      }
    }
    
    // 4. Capture quantum artifact
    let artifact = self.capture_quantum_artifact()?;
    
    Ok(EngineOutput {
      classical_outputs: /* ... */,
      quantum_artifact: Some(artifact),
      traces: self.traces,
      events: self.events,
    })
  }
  
  fn validate_coherence_windows(&self, graph: &ComputationGraph) 
    -> Result<()>;
  
  fn schedule_with_quantum_feedback(&self, graph: &ComputationGraph) 
    -> Result<Vec<ExecutionNode>>;
}
```

### 9.2 State Storage Integration

Quantum state may be stored and retrieved from HybridRegister:

```
struct HybridRegister {
  id: String,
  storage_type: StorageType,
  quantum_capacity_ns: u64,  // how long state persists
  classical_data: HashMap<String, ClassicalValue>,
  quantum_state: Option<QuantumState>,
  coherence_window: Option<CoherenceWindow>,
}

impl HybridRegister {
  fn store_quantum_state(&mut self, state: QuantumState) -> Result<()> {
    // Verify coherence_time < quantum_capacity_ns
    self.quantum_state = Some(state);
  }
  
  fn retrieve_quantum_state(&mut self) -> Result<QuantumState> {
    self.quantum_state.take()
      .ok_or_else(|| anyhow!("No quantum state stored"))
  }
  
  fn quantum_state_is_valid(&self) -> bool {
    if let Some(window) = &self.coherence_window {
      window.deadline > Timestamp::now()
    } else {
      false
    }
  }
}
```

---

## 10. Reference Implementation: CV Gaussian Simulator

### 10.1 GaussianSimulator Backend

A reference CV backend using Gaussian state representation:

```
struct GaussianSimulator {
  name: String,
  max_modes: usize,
  T1_ns: u64,
  T2_ns: u64,
  current_state: Option<CVState>,
  rng: ChaCha20Rng,
}

impl QuantumBackend for GaussianSimulator {
  fn name(&self) -> &str { "gaussian_simulator" }
  
  fn state_type(&self) -> StateType { StateType::CV }
  
  fn prepare(&mut self, modes: Vec<String>, 
             prep: &PreparationKind, seed: u64) -> Result<QuantumState> {
    match prep {
      PreparationKind::DisplacedSqueezed { displacement, 
                                           squeezing_db, 
                                           squeezing_angle } => {
        let mut state = CVState::new();
        for mode in &modes {
          state.add_mode(mode, CVMode {
            label: mode.clone(),
            displacement: Complex::new(displacement.0, displacement.1),
            squeezing: Squeezing {
              amount_db: *squeezing_db,
              angle_rad: *squeezing_angle,
            },
            thermal_photons: 0.0,
          });
        }
        Ok(QuantumState { /* ... */ })
      },
      _ => Err(anyhow!("Unsupported preparation for CV"))
    }
  }
  
  fn measure(&mut self, state: &mut QuantumState,
             basis: &MeasurementBasis, 
             seed: u64) -> Result<MeasurementOutcome> {
    // Sample homodyne outcome from Gaussian distribution
    let mut rng = ChaCha20Rng::seed_from_u64(seed);
    // Draw outcome from N(Re(α), var) for Q quadrature, etc.
  }
}
```

---

## 11. Integration Test Scenario

### 11.1 Full Quantum Workflow

```
Scenario: Quantum State Preparation → Evolution → Measurement → Feedback

1. State Preparation:
   - Prepare 4 CV modes, displaced and squeezed
   - State v1: state_id="prep_0", deadline=now+500ns

2. Delay Buffer Storage:
   - Store state in 50 ns delay buffer
   - Retrieve with coherence check ✓

3. Unitary Evolution:
   - Apply 2-mode beam splitter on modes 0,1
   - Duration 10 ns
   - State v2: parent=v1, fidelity ~0.99

4. Measurement:
   - Homodyne measure mode 0 in Q quadrature
   - Outcome: q_value = 0.75 (Gaussian sample)
   - Post-measurement state: v3 (conditional on outcome)

5. Conditional Feedback:
   - If q_value > 0.5: apply phase shift π/4
   - If q_value ≤ 0.5: apply phase shift -π/4
   - Latency: 20 ns (within coherence window)
   - State v4: parent=v3

6. Artifact Capture:
   - quantum_artifact.json contains:
     - state_ids: [v1, v2, v3, v4]
     - measurement_outcomes: [{ outcome: 0.75, seed: 12345, ... }]
     - seed: 12345 (reproducible)
     - backend: "gaussian_simulator"
     - provenance: full lineage
```

---

## 12. Example: Linear Optical Quantum Computing (DV)

```
Scenario: 4-Qubit Boson Sampling Preparation

1. Prepare 4 qubits in |++++⟩:
   state = DV State {
     qudits: {
       "q0": |+⟩ = (|0⟩+|1⟩)/√2,
       "q1": |+⟩,
       "q2": |+⟩,
       "q3": |+⟩,
     },
   }
   state_id = "boson_sample_0"
   deadline = now + 1000 ns

2. Apply CNOT ladder:
   CNOT(q0, q1), CNOT(q2, q3)
   → state = (|0⟩|0⟩ + |0⟩|1⟩ + |1⟩|0⟩ + |1⟩|1⟩) / 2 on pairs
   state_id = "boson_sample_1"

3. Measure q0, q1, q2, q3 in computational basis:
   outcomes = [0, 1, 0, 1]  (deterministic given seed)

4. Conditional feedback (example):
   If q0 == 1: apply Hadamard to q3
   → Applies Hadamard

5. Final measurement:
   outcomes_final = [?, ?, ?, ?]  (depends on feedback state)
   artifact records full trace
```

---

## 13. Interaction with Other AWEN Subsystems

### 13.1 With Observability

- All quantum state transitions → spans in tracer
- Measurement outcomes → metrics
- Coherence violations → error events
- Timeline includes quantum operation durations

### 13.2 With Reproducibility

- Quantum artifacts stored in execution bundle
- Seeds enable deterministic replay
- Measurement outcomes captured alongside IR
- Artifact lineage tracks parent states

### 13.3 With Calibration

- Calibration kernels may include quantum measurement objectives
- Cost function: fidelity of prepared state vs. target
- Optimizer adjusts gate parameters to maximize fidelity
- Versioned calibration state applies to quantum gates

### 13.4 With Scheduling

- Coherence windows impose hard deadlines on measurement-conditioned branches
- Quantum operations have duration contracts
- Scheduler ensures feedback latencies stay under coherence budget
- Preemption not allowed mid-quantum-operation

### 13.5 With Memory & State

- Quantum state stored in HybridRegister
- DelayBuffer accommodates quantum state with coherence decay
- ResonatorStore supports long-term quantum storage
- Classical results retrieved as classical state

---

## 14. Quantum Conformance Checklist

All implementations must satisfy:

- [ ] QuantumState interface fully implemented (CV or DV or both)
- [ ] State preparation supports ≥2 preparation kinds
- [ ] State evolution with ≥1 Hamiltonian type
- [ ] ≥2 measurement bases supported
- [ ] Measurement-conditioned branching working with ≥1 branch depth
- [ ] Coherence window enforcement (deadline checks)
- [ ] Decoherence estimation (fidelity predictions)
- [ ] Seeding for deterministic replay
- [ ] Measurement outcomes captured in artifacts
- [ ] QuantumBackend trait implemented (reference or real)
- [ ] Integration with Engine.run_graph_with_quantum()
- [ ] Quantum events emitted to tracer
- [ ] ≥10 integration tests covering prep → evolve → measure → feedback
- [ ] Coherence violation detection tests
- [ ] Deterministic replay tests (same seed → same outcome)
- [ ] CI gate: quantum-conformance job validates all above

---

## References

- AEP-0009: Quantum Coherence & State Memory Model v0.1
- computation-model.md (Section 1: Core primitives, 2: Photonic State Model)
- observability.md (Quantum events, metrics)
- reproducibility.md (Artifact capture, seeded determinism)
- calibration.md (Quantum measurement objectives, optimizer integration)
- timing-scheduling.md (Coherence windows, measurement-conditioned deadlines)
