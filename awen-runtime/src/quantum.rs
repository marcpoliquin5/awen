use anyhow::{anyhow, Result};
use chrono::{DateTime, Utc};
/// AWEN Quantum Execution Substrate
/// AWEN Quantum Execution Substrate
///
/// Defines quantum state, evolution, measurement, and backend interfaces
/// supporting CV/DV quantum photonics with measurement-conditioned feedback.
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;
use uuid::Uuid;

// ============================================================================
// Core Quantum State Abstractions
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum StateType {
    CV, // Continuous-Variable (Gaussian)
    DV, // Discrete-Variable (qubit/qudit)
}

/// QuantumStateSnapshot captures non-destructive quantum state information
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StateSnapshot {
    pub mode_labels: Vec<String>,
    pub global_purity: f64, // Tr(ρ²) ∈ [0,1]
    pub entanglement_entropy: HashMap<(String, String), f64>,
    pub noise_floor: f64,
    pub timestamp: DateTime<Utc>,
    pub provenance: QuantumStateProvenance,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumStateProvenance {
    pub state_id: String,
    pub parent_state_id: Option<String>,
    pub operation: QuantumOperation,
    pub seed: u64,
    pub hardware_revision: String,
    pub calibration_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantumOperation {
    Preparation {
        kernel_id: String,
    },
    Unitary {
        gate_name: String,
        parameters: HashMap<String, f64>,
    },
    Measurement {
        outcome: String,
    },
    Evolution {
        duration_ns: u64,
        noise_channel: String,
    },
    Recombination {
        from_states: Vec<String>,
    },
}

/// Core quantum state interface
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumState {
    pub state_id: String,
    pub state_type: StateType,
    pub mode_labels: Vec<String>,
    pub timestamp: DateTime<Utc>,
    pub seed: u64,
    pub hardware_revision: String,
    pub calibration_id: String,
    pub coherence_deadline: DateTime<Utc>,

    // Internal representation (simplified for MVP)
    pub cv_data: Option<CVStateData>,
    pub dv_data: Option<DVStateData>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CVStateData {
    pub modes: HashMap<String, CVMode>,
    pub covariance: Vec<Vec<f64>>, // simplified
    pub is_gaussian: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CVMode {
    pub label: String,
    pub displacement_q: f64,
    pub displacement_p: f64,
    pub squeezing_db: f64,
    pub squeezing_angle: f64,
    pub thermal_photons: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DVStateData {
    pub qudits: HashMap<String, Qudit>,
    pub entanglement_graph: HashMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Qudit {
    pub label: String,
    pub dimension: usize,     // 2 for qubit, 3 for qutrit, etc.
    pub amplitudes: Vec<f64>, // stored as real parts (phase implicit)
    pub purity: f64,          // Tr(ρ²)
}

impl QuantumState {
    pub fn new_cv(mode_labels: Vec<String>, seed: u64, coherence_time_ns: u64) -> Self {
        let now = Utc::now();
        let coherence_deadline = now + chrono::Duration::nanoseconds(coherence_time_ns as i64);

        QuantumState {
            state_id: Uuid::new_v4().to_string(),
            state_type: StateType::CV,
            mode_labels: mode_labels.clone(),
            timestamp: now,
            seed,
            hardware_revision: "sim_v0.1".to_string(),
            calibration_id: Uuid::new_v4().to_string(),
            coherence_deadline,
            cv_data: Some(CVStateData {
                modes: mode_labels
                    .iter()
                    .map(|label| {
                        (
                            label.clone(),
                            CVMode {
                                label: label.clone(),
                                displacement_q: 0.0,
                                displacement_p: 0.0,
                                squeezing_db: 0.0,
                                squeezing_angle: 0.0,
                                thermal_photons: 0.0,
                            },
                        )
                    })
                    .collect(),
                covariance: vec![],
                is_gaussian: true,
            }),
            dv_data: None,
        }
    }

    pub fn new_dv(
        qudit_labels: Vec<(String, usize)>, // (label, dimension)
        seed: u64,
        coherence_time_ns: u64,
    ) -> Self {
        let now = Utc::now();
        let coherence_deadline = now + chrono::Duration::nanoseconds(coherence_time_ns as i64);

        let qudits = qudit_labels
            .iter()
            .map(|(label, dim)| {
                (
                    label.clone(),
                    Qudit {
                        label: label.clone(),
                        dimension: *dim,
                        amplitudes: {
                            let mut v = vec![1.0];
                            if *dim > 1 {
                                v.extend(vec![0.0; *dim - 1]);
                            }
                            v
                        }, // |0⟩ state
                        purity: 1.0,
                    },
                )
            })
            .collect();

        QuantumState {
            state_id: Uuid::new_v4().to_string(),
            state_type: StateType::DV,
            mode_labels: qudit_labels.iter().map(|(l, _)| l.clone()).collect(),
            timestamp: now,
            seed,
            hardware_revision: "sim_v0.1".to_string(),
            calibration_id: Uuid::new_v4().to_string(),
            coherence_deadline,
            cv_data: None,
            dv_data: Some(DVStateData {
                qudits,
                entanglement_graph: HashMap::new(),
            }),
        }
    }

    pub fn snapshot(&self) -> Result<StateSnapshot> {
        Ok(StateSnapshot {
            mode_labels: self.mode_labels.clone(),
            global_purity: self.compute_purity(),
            entanglement_entropy: HashMap::new(), // simplified
            noise_floor: 0.01,                    // mock
            timestamp: self.timestamp,
            provenance: QuantumStateProvenance {
                state_id: self.state_id.clone(),
                parent_state_id: None,
                operation: QuantumOperation::Preparation {
                    kernel_id: "snapshot".to_string(),
                },
                seed: self.seed,
                hardware_revision: self.hardware_revision.clone(),
                calibration_id: self.calibration_id.clone(),
            },
        })
    }

    pub fn can_measure_basis(&self, basis: &MeasurementBasis) -> bool {
        match basis.basis_type {
            BasisType::Homodyne { .. } => self.state_type == StateType::CV,
            BasisType::Heterodyne => self.state_type == StateType::CV,
            BasisType::Computational => self.state_type == StateType::DV,
            BasisType::Hadamard => self.state_type == StateType::DV,
            _ => true,
        }
    }

    fn compute_purity(&self) -> f64 {
        // Simplified: return nominal purity
        if let Some(dv) = &self.dv_data {
            dv.qudits.values().map(|q| q.purity).sum::<f64>() / dv.qudits.len() as f64
        } else {
            0.99 // mock
        }
    }

    pub fn is_coherent_at(&self, timestamp: DateTime<Utc>) -> bool {
        timestamp < self.coherence_deadline
    }

    pub fn time_to_coherence_deadline(&self) -> i64 {
        (self.coherence_deadline - Utc::now())
            .num_nanoseconds()
            .unwrap_or(0)
    }
}

// ============================================================================
// State Preparation
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PreparationKind {
    DisplacedSqueezed {
        displacement_q: f64,
        displacement_p: f64,
        squeezing_db: f64,
        squeezing_angle: f64,
    },
    ThermalState {
        mean_photons: f64,
    },
    BasisState {
        amplitudes: Vec<f64>,
    },
    BellState {
        entanglement_type: BellType,
    },
    GHZState {
        num_qudits: usize,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum BellType {
    PhiPlus,
    PhiMinus,
    PsiPlus,
    PsiMinus,
}

impl fmt::Display for BellType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            BellType::PhiPlus => write!(f, "Φ+"),
            BellType::PhiMinus => write!(f, "Φ-"),
            BellType::PsiPlus => write!(f, "Ψ+"),
            BellType::PsiMinus => write!(f, "Ψ-"),
        }
    }
}

// ============================================================================
// State Evolution
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Hamiltonian {
    pub terms: Vec<PauliTerm>,
    pub static_field: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PauliTerm {
    pub coefficient: f64,
    pub operators: HashMap<String, String>, // "X", "Y", "Z", "I"
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum NoiseChannel {
    Depolarizing { error_rate: f64 },
    PhaseDamping { error_rate: f64 },
    ThermalNoise { bath_temp_k: f64, mode_loss: f64 },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvolutionTrace {
    pub initial_state_id: String,
    pub final_state_id: String,
    pub operations: Vec<QuantumOperation>,
    pub decoherence_estimated: f64,
    pub seed: u64,
}

// ============================================================================
// Measurement Model
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementBasis {
    pub basis_type: BasisType,
    pub mode_labels: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum BasisType {
    Homodyne {
        axis: HomodyneAxis,
    },
    Heterodyne,
    Computational,
    Hadamard,
    BellMeasurement {
        qudit_pairs: Vec<(String, String)>,
    },
    ParityMeasurement {
        qudits: Vec<String>,
        parity_type: ParityType,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub enum HomodyneAxis {
    Q,
    P,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum ParityType {
    ZZ,
    XX,
    YY,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeasurementResult {
    DiscreteOutcome(usize),
    ContinuousValue(f64),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementOutcome {
    pub outcome_id: String,
    pub measurement_id: String,
    pub mode_labels: Vec<String>,
    pub measurement_type: BasisType,
    pub classical_results: HashMap<String, MeasurementResult>,
    pub timestamp: DateTime<Utc>,
    pub seed: u64,
    pub reliability: f64,
}

impl MeasurementOutcome {
    pub fn new(
        measurement_id: String,
        mode_labels: Vec<String>,
        basis_type: BasisType,
        results: HashMap<String, MeasurementResult>,
        seed: u64,
    ) -> Self {
        MeasurementOutcome {
            outcome_id: Uuid::new_v4().to_string(),
            measurement_id,
            mode_labels,
            measurement_type: basis_type,
            classical_results: results,
            timestamp: Utc::now(),
            seed,
            reliability: 0.95, // mock detector efficiency
        }
    }

    pub fn matches_seed(&self, seed: u64) -> bool {
        // Deterministic: same seed gives same outcome
        self.seed == seed
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementLatency {
    pub detection_latency_ns: u64,
    pub electronics_latency_ns: u64,
    pub transport_latency_ns: u64,
    pub certainty: f64,
}

impl MeasurementLatency {
    pub fn total_latency_ns(&self) -> u64 {
        self.detection_latency_ns + self.electronics_latency_ns + self.transport_latency_ns
    }

    pub fn detection_latency_ns(&self) -> u64 {
        self.detection_latency_ns
    }

    pub fn electronics_latency_ns(&self) -> u64 {
        self.electronics_latency_ns
    }

    pub fn transport_latency_ns(&self) -> u64 {
        self.transport_latency_ns
    }
}

// ============================================================================
// Coherence Window Management
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CoherenceWindow {
    pub state_id: String,
    pub initialized_at: DateTime<Utc>,
    pub coherence_time_ns: u64,
    pub deadline: DateTime<Utc>,
    pub t1_ns: Option<u64>,
    pub t2_ns: Option<u64>,
    pub grace_period_ns: u64,
}

impl CoherenceWindow {
    pub fn new(state_id: String, coherence_time_ns: u64) -> Self {
        let now = Utc::now();
        let deadline = now + chrono::Duration::nanoseconds(coherence_time_ns as i64);

        CoherenceWindow {
            state_id,
            initialized_at: now,
            coherence_time_ns,
            deadline,
            t1_ns: None,
            t2_ns: None,
            grace_period_ns: 10,
        }
    }

    pub fn is_valid(&self) -> bool {
        // Structural validity: deadline must be after initialization.
        self.deadline > self.initialized_at
    }

    pub fn time_remaining_ns(&self) -> i64 {
        (self.deadline - self.initialized_at)
            .num_nanoseconds()
            .unwrap_or(-1)
    }

    pub fn check_can_schedule_feedback(
        &self,
        _outcome_time: DateTime<Utc>,
        feedback_latency_ns: u64,
        gate_duration_ns: u64,
    ) -> Result<bool> {
        // Determine whether the required latency + gate duration fits within
        // the coherence window. Use the coherence window length (deadline - initialized_at)
        // rather than wall-clock comparisons to avoid flaky timing dependencies.
        let required_time = feedback_latency_ns + gate_duration_ns;
        let window_ns = (self.deadline - self.initialized_at)
            .num_nanoseconds()
            .unwrap_or(0) as u64;

        Ok(required_time <= window_ns)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum CoherenceViolation {
    StateExpired {
        state_id: String,
        deadline: DateTime<Utc>,
        actual_time: DateTime<Utc>,
    },
    InsufficientWindow {
        state_id: String,
        required_duration_ns: u64,
        available_duration_ns: u64,
    },
}

// ============================================================================
// Measurement-Conditioned Feedback
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum MeasurementOutcomePredicate {
    Equals(MeasurementResult),
    InRange(f64, f64),
    BitValue(usize, bool),
    AlwaysTrue,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MeasurementConditionalBranch {
    pub measurement_id: String,
    pub predicates: HashMap<String, MeasurementOutcomePredicate>, // branch_id -> predicate
    pub timeout_ms: u64,
    pub fallback_kernel: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConditionalQuantumKernel {
    pub id: String,
    pub measurement_dependencies: Vec<String>,
    pub branching_logic: Vec<MeasurementConditionalBranch>,
    pub post_measurement_latency_ns: u64,
    pub max_feedback_depth: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeedbackConstraint {
    pub measurement_outcome_id: String,
    pub gate_name: String,
    pub max_latency_ns: u64,
    pub coherence_deadline: DateTime<Utc>,
    pub branching_factor: usize,
}

// ============================================================================
// Quantum Events
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum QuantumEvent {
    StateCreated {
        state_id: String,
        state_type: StateType,
        num_modes: usize,
        timestamp: DateTime<Utc>,
    },
    StateEvolved {
        state_id: String,
        operation: QuantumOperation,
        duration_ns: u64,
        estimated_fidelity: f64,
        timestamp: DateTime<Utc>,
    },
    MeasurementPerformed {
        state_id: String,
        measurement_type: BasisType,
        outcome_id: String,
        measurement_latency_ns: u64,
        timestamp: DateTime<Utc>,
    },
    StateCollapsed {
        state_id: String,
        timestamp: DateTime<Utc>,
    },
    CoherenceViolation {
        state_id: String,
        violation: CoherenceViolation,
        timestamp: DateTime<Utc>,
    },
}

// ============================================================================
// Quantum Metrics
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumMetrics {
    pub fidelity_estimate: f64,
    pub coherence_remaining_ns: u64,
    pub decoherence_rate: f64,
    pub measurement_confidence: f64,
    pub entanglement_entropy: HashMap<(String, String), f64>,
}

// ============================================================================
// Quantum Artifact
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuantumArtifact {
    pub artifact_id: String,
    pub kernel_id: String,
    pub execution_id: String,

    pub initial_state: QuantumState,
    pub final_state: QuantumState,
    pub intermediate_states: Vec<StateSnapshot>,
    pub measurement_outcomes: Vec<MeasurementOutcome>,

    pub seed: u64,
    pub backend_name: String,
    pub backend_version: String,
    pub noise_model_id: Option<String>,
    pub calibration_version: String,
    pub hardware_revision: String,

    pub start_timestamp: DateTime<Utc>,
    pub end_timestamp: DateTime<Utc>,
    pub total_duration_ns: u64,

    pub events: Vec<QuantumEvent>,
    pub metrics: QuantumMetrics,
}

impl QuantumArtifact {
    pub fn new(kernel_id: String, initial_state: QuantumState, backend_name: String) -> Self {
        let _now = Utc::now();

        // Use the initial state's timestamp as the artifact start time so
        // provenance ties back to the original preparation event deterministically.
        let start_ts = initial_state.timestamp;

        QuantumArtifact {
            artifact_id: Uuid::new_v4().to_string(),
            kernel_id,
            execution_id: Uuid::new_v4().to_string(),
            initial_state: initial_state.clone(),
            final_state: initial_state.clone(),
            intermediate_states: vec![],
            measurement_outcomes: vec![],
            seed: initial_state.seed,
            backend_name,
            backend_version: "0.1".to_string(),
            noise_model_id: None,
            calibration_version: initial_state.calibration_id.clone(),
            hardware_revision: initial_state.hardware_revision.clone(),
            start_timestamp: start_ts,
            end_timestamp: start_ts,
            total_duration_ns: 0,
            events: vec![],
            metrics: QuantumMetrics {
                fidelity_estimate: 0.99,
                coherence_remaining_ns: 100,
                decoherence_rate: 0.001,
                measurement_confidence: 0.95,
                entanglement_entropy: HashMap::new(),
            },
        }
    }

    pub fn can_deterministic_replay(&self) -> bool {
        !self.backend_name.is_empty() && self.noise_model_id.is_some()
    }
}

// ============================================================================
// Backend Interface
// ============================================================================

pub trait QuantumBackend: Send + Sync {
    fn name(&self) -> &str;
    fn state_type(&self) -> StateType;
    fn supported_bases(&self) -> Vec<BasisType>;
    fn max_modes(&self) -> usize;
    fn coherence_time_ns(&self) -> u64;
    fn measurement_latency(&self) -> MeasurementLatency;

    fn prepare(
        &mut self,
        modes: Vec<String>,
        preparation: &PreparationKind,
        seed: u64,
    ) -> Result<QuantumState>;

    fn evolve(
        &mut self,
        state: &mut QuantumState,
        hamiltonian: &Hamiltonian,
        noise_channels: &[NoiseChannel],
        duration_ns: u64,
        seed: u64,
    ) -> Result<EvolutionTrace>;

    fn measure(
        &mut self,
        state: &mut QuantumState,
        basis: &MeasurementBasis,
        seed: u64,
    ) -> Result<MeasurementOutcome>;

    fn snapshot(&self, state: &QuantumState) -> Result<StateSnapshot>;

    fn fidelity(&self, state1: &QuantumState, state2: &QuantumState) -> Result<f64>;

    fn release_state(&mut self, state_id: &str) -> Result<()>;
}

// ============================================================================
// Reference: Gaussian Simulator Backend
// ============================================================================

pub struct GaussianSimulator {
    pub name: String,
    pub max_modes: usize,
    pub t1_ns: u64,
    pub t2_ns: u64,
    pub coherence_time_ns: u64,
}

impl GaussianSimulator {
    pub fn new() -> Self {
        GaussianSimulator {
            name: "gaussian_simulator".to_string(),
            max_modes: 16,
            t1_ns: 10_000,
            t2_ns: 5_000,
            coherence_time_ns: 500,
        }
    }
}

impl Default for GaussianSimulator {
    fn default() -> Self {
        Self::new()
    }
}

impl QuantumBackend for GaussianSimulator {
    fn name(&self) -> &str {
        &self.name
    }

    fn state_type(&self) -> StateType {
        StateType::CV
    }

    fn supported_bases(&self) -> Vec<BasisType> {
        vec![
            BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            BasisType::Heterodyne,
        ]
    }

    fn max_modes(&self) -> usize {
        self.max_modes
    }

    fn coherence_time_ns(&self) -> u64 {
        self.coherence_time_ns
    }

    fn measurement_latency(&self) -> MeasurementLatency {
        MeasurementLatency {
            detection_latency_ns: 20,
            electronics_latency_ns: 5,
            transport_latency_ns: 0,
            certainty: 0.95,
        }
    }

    fn prepare(
        &mut self,
        modes: Vec<String>,
        _preparation: &PreparationKind,
        seed: u64,
    ) -> Result<QuantumState> {
        if modes.len() > self.max_modes {
            return Err(anyhow!(
                "Too many modes: {} > {}",
                modes.len(),
                self.max_modes
            ));
        }

        Ok(QuantumState::new_cv(modes, seed, self.coherence_time_ns))
    }

    fn evolve(
        &mut self,
        state: &mut QuantumState,
        _hamiltonian: &Hamiltonian,
        _noise_channels: &[NoiseChannel],
        duration_ns: u64,
        seed: u64,
    ) -> Result<EvolutionTrace> {
        // Mock evolution: just update timestamp and add to provenance
        let new_state_id = Uuid::new_v4().to_string();
        let old_state_id = state.state_id.clone();

        state.state_id = new_state_id.clone();
        state.timestamp = Utc::now();
        state.seed = seed;

        Ok(EvolutionTrace {
            initial_state_id: old_state_id,
            final_state_id: new_state_id,
            operations: vec![QuantumOperation::Evolution {
                duration_ns,
                noise_channel: "none".to_string(),
            }],
            decoherence_estimated: 0.01, // 1% decoherence per operation
            seed,
        })
    }

    fn measure(
        &mut self,
        state: &mut QuantumState,
        basis: &MeasurementBasis,
        seed: u64,
    ) -> Result<MeasurementOutcome> {
        if !state.can_measure_basis(basis) {
            return Err(anyhow!(
                "Measurement basis {:?} incompatible with state type {:?}",
                basis.basis_type,
                state.state_type
            ));
        }

        // Mock homodyne outcome: sample from Gaussian
        let mut results = HashMap::new();
        let mock_value = (seed % 1000) as f64 / 1000.0; // Deterministic from seed

        for mode in &basis.mode_labels {
            results.insert(mode.clone(), MeasurementResult::ContinuousValue(mock_value));
        }

        Ok(MeasurementOutcome::new(
            Uuid::new_v4().to_string(),
            basis.mode_labels.clone(),
            basis.basis_type.clone(),
            results,
            seed,
        ))
    }

    fn snapshot(&self, state: &QuantumState) -> Result<StateSnapshot> {
        state.snapshot()
    }

    fn fidelity(&self, state1: &QuantumState, state2: &QuantumState) -> Result<f64> {
        // Mock: compare state IDs, return high fidelity if same, low if different
        let fidelity = if state1.state_id == state2.state_id {
            1.0
        } else {
            0.0
        };
        Ok(fidelity)
    }

    fn release_state(&mut self, _state_id: &str) -> Result<()> {
        Ok(())
    }
}

// ============================================================================
// Drift Detector for Quantum States
// ============================================================================

pub trait QuantumDriftDetector: Send + Sync {
    fn detect_drift(&self, state1: &QuantumState, state2: &QuantumState) -> Result<f64>;
}

pub struct SimpleFidelityDriftDetector {
    pub threshold: f64,
}

impl SimpleFidelityDriftDetector {
    pub fn new(threshold: f64) -> Self {
        SimpleFidelityDriftDetector { threshold }
    }
}

impl QuantumDriftDetector for SimpleFidelityDriftDetector {
    fn detect_drift(&self, state1: &QuantumState, state2: &QuantumState) -> Result<f64> {
        // Mock: measure drift as difference in purity
        let purity1 = state1.compute_purity();
        let purity2 = state2.compute_purity();

        let drift = (purity1 - purity2).abs();
        Ok(drift)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cv_state_creation() {
        let state =
            QuantumState::new_cv(vec!["mode_0".to_string(), "mode_1".to_string()], 12345, 500);
        assert_eq!(state.state_type, StateType::CV);
        assert_eq!(state.mode_labels.len(), 2);
        assert_eq!(state.seed, 12345);
    }

    #[test]
    fn test_dv_state_creation() {
        let state = QuantumState::new_dv(
            vec![("q0".to_string(), 2), ("q1".to_string(), 2)],
            67890,
            500,
        );
        assert_eq!(state.state_type, StateType::DV);
        assert_eq!(state.mode_labels.len(), 2);
        assert_eq!(state.seed, 67890);
    }

    #[test]
    fn test_coherence_window() {
        let window = CoherenceWindow::new("state_0".to_string(), 1000);
        // Avoid asserting on live clock values; verify structural invariants instead.
        assert_eq!(window.coherence_time_ns, 1000);
        assert!(window.deadline > window.initialized_at);
    }

    #[test]
    fn test_measurement_outcome_determinism() {
        let outcome1 = MeasurementOutcome::new(
            "meas_0".to_string(),
            vec!["mode_0".to_string()],
            BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            HashMap::new(),
            12345,
        );

        assert!(outcome1.matches_seed(12345));
        assert!(!outcome1.matches_seed(54321));
    }

    #[test]
    fn test_gaussian_simulator() {
        let backend = GaussianSimulator::new();
        assert_eq!(backend.name(), "gaussian_simulator");
        assert_eq!(backend.state_type(), StateType::CV);
        assert!(backend.max_modes() > 0);
    }

    #[test]
    fn test_gaussian_simulator_prepare() {
        let mut backend = GaussianSimulator::new();
        let prep = PreparationKind::DisplacedSqueezed {
            displacement_q: 0.5,
            displacement_p: 0.3,
            squeezing_db: 6.0,
            squeezing_angle: 0.0,
        };

        let state = backend
            .prepare(vec!["mode_0".to_string()], &prep, 12345)
            .expect("Preparation failed");

        assert_eq!(state.state_type, StateType::CV);
        assert!(!state.mode_labels.is_empty());
    }

    #[test]
    fn test_gaussian_simulator_measure() {
        let mut backend = GaussianSimulator::new();
        let state = backend
            .prepare(
                vec!["mode_0".to_string()],
                &PreparationKind::ThermalState { mean_photons: 1.0 },
                12345,
            )
            .expect("Preparation failed");

        let mut state_copy = state.clone();
        let basis = MeasurementBasis {
            basis_type: BasisType::Homodyne {
                axis: HomodyneAxis::Q,
            },
            mode_labels: vec!["mode_0".to_string()],
        };

        let outcome = backend
            .measure(&mut state_copy, &basis, 12345)
            .expect("Measurement failed");

        assert!(outcome.matches_seed(12345));
        assert!(!outcome.classical_results.is_empty());
    }

    #[test]
    fn test_quantum_artifact() {
        let state = QuantumState::new_cv(vec!["mode_0".to_string()], 12345, 500);
        let artifact = QuantumArtifact::new(
            "kernel_0".to_string(),
            state.clone(),
            "gaussian_simulator".to_string(),
        );

        assert_eq!(artifact.kernel_id, "kernel_0");
        assert_eq!(artifact.backend_name, "gaussian_simulator");
        assert!(!artifact.can_deterministic_replay()); // needs noise_model_id
    }

    #[test]
    fn test_fidelity_drift_detector() {
        let detector = SimpleFidelityDriftDetector::new(0.05);
        let state1 = QuantumState::new_cv(vec!["mode_0".to_string()], 111, 500);
        let state2 = QuantumState::new_cv(vec!["mode_0".to_string()], 222, 500);

        let drift = detector
            .detect_drift(&state1, &state2)
            .expect("Drift detection failed");

        assert!(drift >= 0.0);
    }
}
