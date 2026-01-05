// AWEN Calibration Module
// First-class calibration with drift detection and closed-loop optimization

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Calibration Kernel Definition
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationKernel {
    pub id: String,
    pub target_nodes: Vec<String>,
    pub parameters_to_tune: Vec<String>,
    pub cost_function: CostFunction,
    pub measurement_sequence: Vec<MeasurementStep>,
    pub optimizer_config: OptimizerConfig,
    pub safety_constraints: SafetyConstraints,
    pub schedule: CalibrationSchedule,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CostFunction {
    Minimize {
        expression: String,
        target_value: Option<f64>,
    },
    Maximize {
        expression: String,
    },
    MatchSpectrum {
        target_spectrum: Vec<(f64, f64)>,
        tolerance_db: f64,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct MeasurementStep {
    pub step_id: String,
    pub action: MeasurementAction,
    pub expected_duration_ns: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum MeasurementAction {
    SetParameter {
        node_id: String,
        param_name: String,
        value: f64,
    },
    ReadSensor {
        sensor_id: String,
        integration_time_ns: u64,
    },
    Wait {
        duration_ns: u64,
    },
    Compute {
        metric_name: String,
        expression: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptimizerConfig {
    pub algorithm: OptimizerAlgorithm,
    pub max_iterations: usize,
    pub convergence_threshold: f64,
    pub initial_guess: Option<HashMap<String, f64>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum OptimizerAlgorithm {
    GradientDescent {
        learning_rate: f64,
        momentum: f64,
    },
    NelderMead {
        initial_simplex_size: f64,
    },
    BayesianOptimization {
        acquisition_function: String,
        num_initial_samples: usize,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SafetyConstraints {
    pub hard_limits: HashMap<String, (f64, f64)>,
    pub soft_limits: HashMap<String, (f64, f64)>,
    pub max_optical_power_dbm: Option<f64>,
    pub timeout_seconds: u64,
}

impl Default for SafetyConstraints {
    fn default() -> Self {
        SafetyConstraints {
            hard_limits: HashMap::new(),
            soft_limits: HashMap::new(),
            max_optical_power_dbm: Some(10.0), // 10 dBm default limit
            timeout_seconds: 300,              // 5 minutes
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum CalibrationSchedule {
    PreRun,
    Periodic {
        interval_seconds: u64,
    },
    OnDrift {
        drift_threshold: f64,
        check_interval_seconds: u64,
    },
    Manual,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Calibration State & Versioning
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationState {
    pub calibration_id: String,
    pub version: u64,
    pub timestamp: String,
    pub node_calibrations: HashMap<String, NodeCalibration>,
    pub provenance: CalibrationProvenance,
}

impl Default for CalibrationState {
    fn default() -> Self {
        CalibrationState {
            calibration_id: "default-calib".to_string(),
            version: 0,
            timestamp: chrono::Utc::now().to_rfc3339(),
            node_calibrations: HashMap::new(),
            provenance: CalibrationProvenance::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeCalibration {
    pub node_id: String,
    pub parameters: HashMap<String, f64>,
    pub metadata: NodeCalibrationMetadata,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NodeCalibrationMetadata {
    pub cost_function_value: f64,
    pub convergence_iterations: usize,
    pub measurement_snr_db: f64,
    pub confidence: f64,
    pub calibration_duration_seconds: f64,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct CalibrationProvenance {
    pub calibration_kernel_id: String,
    pub optimizer_algorithm: String,
    pub measurement_count: usize,
    pub parent_calibration_id: Option<String>,
    pub hardware_revision: String,
    pub temperature_c: Option<f64>,
    pub seed: Option<u64>,
}

impl Default for CalibrationProvenance {
    fn default() -> Self {
        CalibrationProvenance {
            calibration_kernel_id: "unknown".to_string(),
            optimizer_algorithm: "unknown".to_string(),
            measurement_count: 0,
            parent_calibration_id: None,
            hardware_revision: "v0.1".to_string(),
            temperature_c: None,
            seed: None,
        }
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Drift Detection
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait DriftDetector: Send + Sync {
    fn detect_drift(
        &self,
        current_state: &CalibrationState,
        measurements: &[Measurement],
    ) -> Result<DriftReport>;
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Measurement {
    pub measurement_id: String,
    pub timestamp_ns: u64,
    pub sensor_id: String,
    pub value: f64,
    pub unit: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftReport {
    pub drift_detected: bool,
    pub drift_metrics: Vec<DriftMetricValue>,
    pub recommended_action: RecalibrationAction,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DriftMetricValue {
    pub metric_id: String,
    pub current_value: f64,
    pub nominal_value: f64,
    pub delta: f64,
    pub threshold_exceeded: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
pub enum RecalibrationAction {
    NoAction,
    Recalibrate {
        urgency: Urgency,
        target_nodes: Vec<String>,
    },
    Alert {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub enum Urgency {
    Low = 0,
    Medium = 1,
    High = 2,
    Critical = 3,
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Threshold-Based Drift Detector (Reference Implementation)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct ThresholdDriftDetector {
    threshold: f64,
}

impl ThresholdDriftDetector {
    pub fn new(threshold: f64) -> Self {
        ThresholdDriftDetector { threshold }
    }
}

impl DriftDetector for ThresholdDriftDetector {
    fn detect_drift(
        &self,
        current_state: &CalibrationState,
        measurements: &[Measurement],
    ) -> Result<DriftReport> {
        let mut drift_metrics = Vec::new();
        let mut drift_detected = false;

        // Check each measurement against calibrated values
        for measurement in measurements {
            // Find corresponding calibrated parameter
            for (node_id, node_calib) in &current_state.node_calibrations {
                if let Some(&calibrated_value) = node_calib.parameters.get(&measurement.sensor_id) {
                    let delta = (measurement.value - calibrated_value).abs();
                    let relative_delta = delta / calibrated_value.abs().max(1e-10);

                    let threshold_exceeded = relative_delta > self.threshold;
                    if threshold_exceeded {
                        drift_detected = true;
                    }

                    drift_metrics.push(DriftMetricValue {
                        metric_id: format!("{}.{}", node_id, measurement.sensor_id),
                        current_value: measurement.value,
                        nominal_value: calibrated_value,
                        delta: relative_delta,
                        threshold_exceeded,
                    });
                }
            }
        }

        let recommended_action = if drift_detected {
            let urgency = if drift_metrics.iter().any(|m| m.delta > self.threshold * 2.0) {
                Urgency::High
            } else {
                Urgency::Medium
            };

            RecalibrationAction::Recalibrate {
                urgency,
                target_nodes: current_state.node_calibrations.keys().cloned().collect(),
            }
        } else {
            RecalibrationAction::NoAction
        };

        Ok(DriftReport {
            drift_detected,
            drift_metrics,
            recommended_action,
        })
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Calibration Executor Trait
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub trait CalibrationExecutor: Send + Sync {
    fn execute_calibration(
        &self,
        kernel: &CalibrationKernel,
        initial_state: Option<&CalibrationState>,
    ) -> Result<CalibrationState>;

    fn apply_calibration(&self, state: &CalibrationState, safety: &SafetyConstraints)
        -> Result<()>;

    fn get_current_calibration(&self) -> Result<CalibrationState>;
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Reference Calibration Executor (Nelder-Mead)
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

pub struct ReferenceCalibrationExecutor {
    current_state: std::sync::Mutex<CalibrationState>,
}

impl ReferenceCalibrationExecutor {
    pub fn new() -> Self {
        ReferenceCalibrationExecutor {
            current_state: std::sync::Mutex::new(CalibrationState::default()),
        }
    }

    fn evaluate_cost_function(
        &self,
        cost_function: &CostFunction,
        params: &HashMap<String, f64>,
    ) -> Result<f64> {
        match cost_function {
            CostFunction::Minimize {
                expression: _,
                target_value,
            } => {
                // Simple mock cost: quadratic distance from target
                let target = target_value.unwrap_or(0.0);
                let param_sum: f64 = params.values().sum();
                let cost = (param_sum - target).powi(2);
                Ok(cost)
            }
            CostFunction::Maximize { expression: _ } => {
                // Mock: negate for minimization
                let param_sum: f64 = params.values().sum();
                Ok(-param_sum)
            }
            CostFunction::MatchSpectrum {
                target_spectrum,
                tolerance_db: _,
            } => {
                // Mock: sum of squared errors
                let error: f64 = target_spectrum.iter().map(|(_, power)| power.powi(2)).sum();
                Ok(error)
            }
        }
    }

    fn nelder_mead_optimize(
        &self,
        kernel: &CalibrationKernel,
        initial_params: &HashMap<String, f64>,
    ) -> Result<(HashMap<String, f64>, f64, usize)> {
        let simplex_size = match &kernel.optimizer_config.algorithm {
            OptimizerAlgorithm::NelderMead {
                initial_simplex_size,
            } => *initial_simplex_size,
            _ => 0.1,
        };

        let mut best_params = initial_params.clone();
        let mut best_cost = self.evaluate_cost_function(&kernel.cost_function, &best_params)?;
        let mut iterations = 0;

        // Simplified Nelder-Mead: just perturb parameters iteratively
        for iter in 0..kernel.optimizer_config.max_iterations {
            iterations = iter + 1;

            // Try random perturbation
            let mut trial_params = best_params.clone();
            for (_key, value) in trial_params.iter_mut() {
                *value += (rand::random::<f64>() - 0.5) * 2.0 * simplex_size;
            }

            let trial_cost = self.evaluate_cost_function(&kernel.cost_function, &trial_params)?;

            if trial_cost < best_cost {
                best_params = trial_params;
                best_cost = trial_cost;
            }
            // Check convergence
            if best_cost < kernel.optimizer_config.convergence_threshold {
                break;
            }
        }

        Ok((best_params, best_cost, iterations))
    }
}

impl CalibrationExecutor for ReferenceCalibrationExecutor {
    fn execute_calibration(
        &self,
        kernel: &CalibrationKernel,
        initial_state: Option<&CalibrationState>,
    ) -> Result<CalibrationState> {
        let start_time = std::time::Instant::now();

        // Initialize parameters
        let initial_params = if let Some(guess) = &kernel.optimizer_config.initial_guess {
            guess.clone()
        } else {
            let mut params = HashMap::new();
            for param in &kernel.parameters_to_tune {
                params.insert(param.clone(), 0.0);
            }
            params
        };

        // Run optimization
        let (optimized_params, final_cost, iterations) = match &kernel.optimizer_config.algorithm {
            OptimizerAlgorithm::NelderMead { .. } => {
                self.nelder_mead_optimize(kernel, &initial_params)?
            }
            OptimizerAlgorithm::GradientDescent { .. } => {
                // Simplified: use Nelder-Mead as fallback
                self.nelder_mead_optimize(kernel, &initial_params)?
            }
            OptimizerAlgorithm::BayesianOptimization { .. } => {
                // Simplified: use Nelder-Mead as fallback
                self.nelder_mead_optimize(kernel, &initial_params)?
            }
        };

        let duration = start_time.elapsed().as_secs_f64();

        // Build node calibrations
        let mut node_calibrations = HashMap::new();
        for node_id in &kernel.target_nodes {
            node_calibrations.insert(
                node_id.clone(),
                NodeCalibration {
                    node_id: node_id.clone(),
                    parameters: optimized_params.clone(),
                    metadata: NodeCalibrationMetadata {
                        cost_function_value: final_cost,
                        convergence_iterations: iterations,
                        measurement_snr_db: 20.0, // Mock SNR
                        confidence: 0.95,
                        calibration_duration_seconds: duration,
                    },
                },
            );
        }

        // Increment version
        let version = if let Some(prev_state) = initial_state {
            prev_state.version + 1
        } else {
            1
        };

        let calibration_state = CalibrationState {
            calibration_id: format!("calib-{}", uuid::Uuid::new_v4()),
            version,
            timestamp: chrono::Utc::now().to_rfc3339(),
            node_calibrations,
            provenance: CalibrationProvenance {
                calibration_kernel_id: kernel.id.clone(),
                optimizer_algorithm: format!("{:?}", kernel.optimizer_config.algorithm),
                measurement_count: iterations * kernel.measurement_sequence.len(),
                parent_calibration_id: initial_state.map(|s| s.calibration_id.clone()),
                hardware_revision: "v0.2".to_string(),
                temperature_c: Some(25.0), // Mock temperature
                seed: Some(42),
            },
        };

        // Update current state
        *self.current_state.lock().unwrap() = calibration_state.clone();

        Ok(calibration_state)
    }

    fn apply_calibration(
        &self,
        state: &CalibrationState,
        safety: &SafetyConstraints,
    ) -> Result<()> {
        // Validate safety constraints
        for node_calib in state.node_calibrations.values() {
            for (param_name, &value) in &node_calib.parameters {
                // Check hard limits
                if let Some(&(min, max)) = safety.hard_limits.get(param_name) {
                    if value < min || value > max {
                        return Err(anyhow!(
                            "Safety violation: parameter {} = {} outside hard limits [{}, {}]",
                            param_name,
                            value,
                            min,
                            max
                        ));
                    }
                }
            }
        }

        // Mock: Apply calibration (in real system, write to device)
        println!("Applied calibration version {} to device", state.version);

        Ok(())
    }

    fn get_current_calibration(&self) -> Result<CalibrationState> {
        Ok(self.current_state.lock().unwrap().clone())
    }
}

impl Default for ReferenceCalibrationExecutor {
    fn default() -> Self {
        Self::new()
    }
}

// -----------------------------
// Basic compatibility helpers
// -----------------------------

use serde_json::Value as JsonValue;
use std::fs;
use std::path::Path;
use uuid::Uuid;

/// A small, file-backed calibration format used for lightweight runtimes and tests.
#[derive(Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct BasicCalibrationState {
    pub handle: String,
    pub scale_factors: HashMap<String, f64>,
}

/// Generate a default basic calibration state.
pub fn basic_generate_default_state() -> BasicCalibrationState {
    let mut m = HashMap::new();
    m.insert("power".to_string(), 1.05);
    BasicCalibrationState {
        handle: format!("cal-{}", Uuid::new_v4()),
        scale_factors: m,
    }
}

/// Persist a basic calibration state to disk under `dir/handles/<handle>.json`.
pub fn basic_save_state<P: AsRef<Path>>(state: &BasicCalibrationState, dir: P) -> Result<()> {
    let mut base = dir.as_ref().to_path_buf();
    base.push("handles");
    fs::create_dir_all(&base)?;
    base.push(format!("{}.json", state.handle));
    let s = serde_json::to_string_pretty(state)?;
    fs::write(base, s)?;
    Ok(())
}

/// Load a basic calibration state by handle if present.
pub fn basic_load_state<P: AsRef<Path>>(
    handle: &str,
    dir: P,
) -> Result<Option<BasicCalibrationState>> {
    let mut base = dir.as_ref().to_path_buf();
    base.push("handles");
    base.push(format!("{}.json", handle));
    if !base.exists() {
        return Ok(None);
    }
    let data = fs::read(&base)?;
    let st = serde_json::from_slice::<BasicCalibrationState>(&data)?;
    Ok(Some(st))
}

/// Apply basic scale factors to params JSON object.
pub fn basic_apply_to_params(
    state: &BasicCalibrationState,
    params: Option<JsonValue>,
) -> Option<JsonValue> {
    match params {
        Some(JsonValue::Object(mut map)) => {
            for (k, v) in map.iter_mut() {
                if let Some(scale) = state.scale_factors.get(k) {
                    if let Some(n) = v.as_f64() {
                        *v = JsonValue::from(n * scale);
                    }
                }
            }
            Some(JsonValue::Object(map))
        }
        other => other,
    }
}

// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
// Unit Tests
// ━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calibration_kernel_serialization() {
        let kernel = CalibrationKernel {
            id: "test_kernel".to_string(),
            target_nodes: vec!["mzi_0".to_string()],
            parameters_to_tune: vec!["phase".to_string()],
            cost_function: CostFunction::Minimize {
                expression: "loss".to_string(),
                target_value: Some(0.01),
            },
            measurement_sequence: vec![],
            optimizer_config: OptimizerConfig {
                algorithm: OptimizerAlgorithm::NelderMead {
                    initial_simplex_size: 0.1,
                },
                max_iterations: 100,
                convergence_threshold: 0.01,
                initial_guess: None,
            },
            safety_constraints: SafetyConstraints::default(),
            schedule: CalibrationSchedule::PreRun,
        };

        let json = serde_json::to_string(&kernel).unwrap();
        let deserialized: CalibrationKernel = serde_json::from_str(&json).unwrap();
        assert_eq!(kernel.id, deserialized.id);
    }

    #[test]
    fn test_calibration_state_versioning() {
        let state_v1 = CalibrationState {
            calibration_id: "calib-001".to_string(),
            version: 1,
            timestamp: chrono::Utc::now().to_rfc3339(),
            node_calibrations: HashMap::new(),
            provenance: CalibrationProvenance::default(),
        };

        let mut state_v2 = state_v1.clone();
        state_v2.version = 2;
        state_v2.provenance.parent_calibration_id = Some("calib-001".to_string());

        assert_eq!(state_v1.version, 1);
        assert_eq!(state_v2.version, 2);
        assert_eq!(
            state_v2.provenance.parent_calibration_id,
            Some("calib-001".to_string())
        );
    }

    #[test]
    fn test_drift_detection() {
        let calibration_state = CalibrationState {
            calibration_id: "calib-001".to_string(),
            version: 1,
            timestamp: chrono::Utc::now().to_rfc3339(),
            node_calibrations: {
                let mut map = HashMap::new();
                map.insert(
                    "mzi_0".to_string(),
                    NodeCalibration {
                        node_id: "mzi_0".to_string(),
                        parameters: {
                            let mut params = HashMap::new();
                            params.insert("phase".to_string(), 1.0);
                            params
                        },
                        metadata: NodeCalibrationMetadata {
                            cost_function_value: 0.01,
                            convergence_iterations: 10,
                            measurement_snr_db: 20.0,
                            confidence: 0.95,
                            calibration_duration_seconds: 1.0,
                        },
                    },
                );
                map
            },
            provenance: CalibrationProvenance::default(),
        };

        let measurements = vec![Measurement {
            measurement_id: "meas-001".to_string(),
            timestamp_ns: 1000,
            sensor_id: "phase".to_string(),
            value: 1.2, // 20% drift from nominal 1.0
            unit: "radians".to_string(),
        }];

        let detector = ThresholdDriftDetector::new(0.1); // 10% threshold
        let report = detector
            .detect_drift(&calibration_state, &measurements)
            .unwrap();

        assert!(report.drift_detected);
        assert_eq!(report.drift_metrics.len(), 1);
        assert!(report.drift_metrics[0].threshold_exceeded);
    }

    #[test]
    fn test_safety_constraint_validation() {
        let state = CalibrationState {
            calibration_id: "calib-001".to_string(),
            version: 1,
            timestamp: chrono::Utc::now().to_rfc3339(),
            node_calibrations: {
                let mut map = HashMap::new();
                map.insert(
                    "mzi_0".to_string(),
                    NodeCalibration {
                        node_id: "mzi_0".to_string(),
                        parameters: {
                            let mut params = HashMap::new();
                            params.insert("voltage".to_string(), 15.0); // Exceeds limit
                            params
                        },
                        metadata: NodeCalibrationMetadata {
                            cost_function_value: 0.01,
                            convergence_iterations: 10,
                            measurement_snr_db: 20.0,
                            confidence: 0.95,
                            calibration_duration_seconds: 1.0,
                        },
                    },
                );
                map
            },
            provenance: CalibrationProvenance::default(),
        };

        let mut safety = SafetyConstraints::default();
        safety
            .hard_limits
            .insert("voltage".to_string(), (0.0, 10.0));

        let executor = ReferenceCalibrationExecutor::new();
        let result = executor.apply_calibration(&state, &safety);

        assert!(result.is_err());
    }
}
