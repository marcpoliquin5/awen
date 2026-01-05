# AWEN Calibration & Drift Model v0.2

**Status:** DRAFT  
**Version:** 0.2.0  
**Date:** 2026-01-05  
**Depends On:** [computation-model.md](computation-model.md), [observability.md](observability.md), [reproducibility.md](reproducibility.md)

---

## Purpose

Define calibration as **first-class computation** in AWEN, including:
- Calibration kernels as schedulable tasks
- Drift detection and recalibration triggers
- Versioned calibration artifacts
- Closed-loop optimization semantics
- Integration with reproducibility and observability systems

---

## 1. Calibration Problem Statement

### 1.1 Why Calibration is Critical

Photonic systems suffer from **systematic drift** due to:
- **Thermal fluctuations:** Refractive index changes with temperature (Δn/ΔT ≈ 10⁻⁵/K for silicon)
- **Device aging:** Photodegradation, material fatigue
- **Fabrication variance:** Process variations across wafers
- **Environmental coupling:** Vibrations, humidity, electromagnetic interference

**Consequence:** Without calibration, photonic circuits drift out of specification within seconds to hours.

### 1.2 Calibration as Computation

AWEN treats calibration as **executable code**, not offline configuration:

**Traditional approach (rejected):**
```
1. Manually measure device
2. Update config file
3. Restart system
4. Hope it works
```

**AWEN approach (mandated):**
```
1. Define calibration kernel (measurement sequence + cost function)
2. Schedule calibration as first-class task
3. Runtime executes calibration, produces artifact
4. Artifact captures full provenance (parameters, measurements, optimizer state)
5. Recalibration triggered automatically on drift detection
```

---

## 2. Calibration Kernel Model

### 2.1 CalibrationKernel Schema

```rust
pub struct CalibrationKernel {
    pub id: String,
    pub target_nodes: Vec<String>,       // Nodes to calibrate (e.g., ["mzi_0", "mzi_1"])
    pub parameters_to_tune: Vec<String>, // Parameters to optimize (e.g., ["phase_upper", "phase_lower"])
    pub cost_function: CostFunction,     // Objective to minimize
    pub measurement_sequence: Vec<MeasurementStep>,
    pub optimizer_config: OptimizerConfig,
    pub safety_constraints: SafetyConstraints,
    pub schedule: CalibrationSchedule,
}
```

### 2.2 Cost Function Types

#### Option A: Classical Cost Function
```rust
pub enum CostFunction {
    /// Minimize target expression (e.g., "1 - extinction_ratio")
    Minimize {
        expression: String,              // e.g., "abs(measured_power - target_power)"
        target_value: Option<f64>,
    },
    
    /// Maximize target expression (e.g., "transmission")
    Maximize {
        expression: String,
    },
    
    /// Match target spectrum
    MatchSpectrum {
        target_spectrum: Vec<(f64, f64)>, // (wavelength_nm, power_dbm)
        tolerance_db: f64,
    },
    
    /// Custom function (plugin-defined)
    Custom {
        plugin_id: String,
        config: serde_json::Value,
    },
}
```

#### Option B: Differentiable Cost Function
```rust
pub struct DifferentiableCostFunction {
    pub expression: String,              // Differentiable expression
    pub gradient_provider: String,       // "adjoint" or "finite_difference"
    pub parameters: Vec<String>,         // Parameters for which gradients are computed
}
```

### 2.3 Measurement Sequence

```rust
pub struct MeasurementStep {
    pub step_id: String,
    pub action: MeasurementAction,
    pub expected_duration_ns: u64,
}

pub enum MeasurementAction {
    /// Set parameter to specific value
    SetParameter {
        node_id: String,
        param_name: String,
        value: f64,
    },
    
    /// Measure sensor output
    ReadSensor {
        sensor_id: String,
        integration_time_ns: u64,
    },
    
    /// Wait for settling time
    Wait {
        duration_ns: u64,
    },
    
    /// Compute derived metric
    Compute {
        metric_name: String,
        expression: String,              // e.g., "sqrt(I^2 + Q^2)"
    },
}
```

### 2.4 Optimizer Configuration

```rust
pub struct OptimizerConfig {
    pub algorithm: OptimizerAlgorithm,
    pub max_iterations: usize,
    pub convergence_threshold: f64,
    pub initial_guess: Option<HashMap<String, f64>>,
}

pub enum OptimizerAlgorithm {
    /// Gradient descent (requires differentiable cost function)
    GradientDescent {
        learning_rate: f64,
        momentum: f64,
    },
    
    /// Nelder-Mead simplex (gradient-free)
    NelderMead {
        initial_simplex_size: f64,
    },
    
    /// Bayesian optimization (sample-efficient)
    BayesianOptimization {
        acquisition_function: String,   // "EI", "UCB", "PI"
        num_initial_samples: usize,
    },
    
    /// Simulated annealing (global optimization)
    SimulatedAnnealing {
        initial_temperature: f64,
        cooling_schedule: String,       // "exponential", "linear"
    },
}
```

### 2.5 Safety Constraints

```rust
pub struct SafetyConstraints {
    /// Hard limits (violating these aborts calibration)
    pub hard_limits: HashMap<String, (f64, f64)>,  // param -> (min, max)
    
    /// Soft limits (violating these triggers warning)
    pub soft_limits: HashMap<String, (f64, f64)>,
    
    /// Maximum power limits (safety-critical)
    pub max_optical_power_dbm: Option<f64>,
    pub max_electrical_current_ma: Option<f64>,
    pub max_voltage_v: Option<f64>,
    
    /// Timeout (abort if calibration takes too long)
    pub timeout_seconds: u64,
}
```

### 2.6 Calibration Schedule

```rust
pub enum CalibrationSchedule {
    /// Run once before execution
    PreRun,
    
    /// Run periodically during execution
    Periodic {
        interval_seconds: u64,
    },
    
    /// Run when drift detected
    OnDrift {
        drift_threshold: f64,
        check_interval_seconds: u64,
    },
    
    /// Run manually (user-triggered)
    Manual,
    
    /// Background asynchronous calibration
    Asynchronous {
        priority: Priority,
    },
}
```

---

## 3. Drift Detection Model

### 3.1 Drift Metrics

```rust
pub struct DriftMetric {
    pub metric_id: String,
    pub metric_type: DriftMetricType,
    pub threshold: f64,
    pub measurement_interval_seconds: u64,
}

pub enum DriftMetricType {
    /// Parameter drift from calibrated value
    ParameterDrift {
        node_id: String,
        param_name: String,
        nominal_value: f64,
    },
    
    /// Performance metric drift (e.g., extinction ratio)
    PerformanceDrift {
        metric_name: String,           // "extinction_ratio", "insertion_loss"
        nominal_value: f64,
        tolerance: f64,
    },
    
    /// Thermal drift (temperature change)
    ThermalDrift {
        sensor_id: String,
        nominal_temperature_c: f64,
        max_delta_c: f64,
    },
    
    /// Temporal drift (time-based recalibration)
    TemporalDrift {
        max_age_seconds: u64,
    },
}
```

### 3.2 Drift Detection Algorithm

```rust
pub trait DriftDetector: Send + Sync {
    /// Check if recalibration is needed
    fn detect_drift(
        &self,
        current_state: &CalibrationState,
        measurements: &[Measurement],
    ) -> Result<DriftReport>;
}

pub struct DriftReport {
    pub drift_detected: bool,
    pub drift_metrics: Vec<DriftMetricValue>,
    pub recommended_action: RecalibrationAction,
}

pub struct DriftMetricValue {
    pub metric_id: String,
    pub current_value: f64,
    pub nominal_value: f64,
    pub delta: f64,
    pub threshold_exceeded: bool,
}

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

pub enum Urgency {
    Low,       // Recalibrate when convenient
    Medium,    // Recalibrate soon (within minutes)
    High,      // Recalibrate immediately (within seconds)
    Critical,  // Abort execution and recalibrate
}
```

---

## 4. Calibration State & Versioning

### 4.1 CalibrationState Schema

```rust
pub struct CalibrationState {
    pub calibration_id: String,
    pub version: u64,                    // Monotonically increasing
    pub timestamp: String,               // ISO 8601
    pub node_calibrations: HashMap<String, NodeCalibration>,
    pub provenance: CalibrationProvenance,
}

pub struct NodeCalibration {
    pub node_id: String,
    pub parameters: HashMap<String, f64>,
    pub metadata: NodeCalibrationMetadata,
}

pub struct NodeCalibrationMetadata {
    pub cost_function_value: f64,
    pub convergence_iterations: usize,
    pub measurement_snr_db: f64,
    pub confidence: f64,                 // 0-1 scale
    pub calibration_duration_seconds: f64,
}

pub struct CalibrationProvenance {
    pub calibration_kernel_id: String,
    pub optimizer_algorithm: String,
    pub measurement_count: usize,
    pub parent_calibration_id: Option<String>, // For drift-triggered recalibration
    pub hardware_revision: String,
    pub temperature_c: Option<f64>,
    pub seed: Option<u64>,               // For reproducibility
}
```

### 4.2 Version Management

**Versioning Rules:**
1. **Monotonic increment:** Each calibration produces `version = prev_version + 1`
2. **Artifact emission:** Each calibration version produces a `CalibrationArtifact`
3. **Lineage tracking:** Recalibration references parent calibration ID
4. **Timestamp-based lookup:** Runtime can query "calibration state at time T"

**Example Timeline:**
```
t=0:     v1 (pre-run calibration)         → artifact_calib_001
t=300:   v2 (thermal drift detected)      → artifact_calib_002 (parent: 001)
t=600:   v3 (periodic recalibration)      → artifact_calib_003 (parent: 002)
```

---

## 5. Calibration Artifact Schema

### 5.1 Artifact Structure

```rust
pub struct CalibrationArtifact {
    pub artifact_id: String,
    pub artifact_type: String,           // "calibration"
    pub calibration_state: CalibrationState,
    pub measurements: Vec<CalibrationMeasurement>,
    pub optimizer_trace: OptimizerTrace,
    pub cost_function_history: Vec<(usize, f64)>, // (iteration, cost)
    pub provenance: HashMap<String, String>,
}

pub struct CalibrationMeasurement {
    pub measurement_id: String,
    pub timestamp_ns: u64,
    pub sensor_id: String,
    pub value: f64,
    pub unit: String,
    pub snr_db: Option<f64>,
}

pub struct OptimizerTrace {
    pub algorithm: String,
    pub iterations: Vec<OptimizerIteration>,
    pub convergence_reason: String,      // "threshold_met", "max_iterations", "timeout"
}

pub struct OptimizerIteration {
    pub iteration: usize,
    pub parameters: HashMap<String, f64>,
    pub cost: f64,
    pub gradient: Option<HashMap<String, f64>>, // For gradient-based methods
    pub timestamp_ns: u64,
}
```

### 5.2 Artifact Export Format

**Directory Structure:**
```
awen_calibration_<id>/
├── manifest.json
├── calibration_state.json
├── measurements/
│   ├── measurement_001.json
│   ├── measurement_002.json
│   └── ...
├── optimizer_trace/
│   ├── iterations.jsonl
│   └── convergence_summary.json
├── provenance/
│   ├── citation.txt
│   └── lineage.json
└── checksums.json
```

---

## 6. Runtime Integration

### 6.1 Calibration Executor

```rust
pub trait CalibrationExecutor: Send + Sync {
    /// Execute calibration kernel
    fn execute_calibration(
        &self,
        kernel: &CalibrationKernel,
        initial_state: Option<&CalibrationState>,
    ) -> Result<CalibrationArtifact>;
    
    /// Apply calibration state to device
    fn apply_calibration(
        &self,
        state: &CalibrationState,
        safety: &SafetyConstraints,
    ) -> Result<()>;
    
    /// Query current calibration state
    fn get_current_calibration(&self) -> Result<CalibrationState>;
}
```

### 6.2 Engine Integration

**Non-Bypassable Chokepoint:** Engine tracks calibration state across execution.

```rust
impl Engine {
    pub fn run_graph_with_calibration(
        &self,
        graph: &Graph,
        calibration_kernel: Option<&CalibrationKernel>,
        seed: Option<u64>,
    ) -> Result<(PathBuf, CalibrationState)> {
        // Phase 1: Pre-run calibration
        let calibration_state = if let Some(kernel) = calibration_kernel {
            self.calibration_executor.execute_calibration(kernel, None)?
        } else {
            CalibrationState::default()
        };
        
        // Phase 2: Apply calibration
        self.calibration_executor.apply_calibration(&calibration_state, &kernel.safety_constraints)?;
        
        // Phase 3: Execute graph (with drift monitoring)
        let artifact_path = self.run_graph_internal(graph, seed, Some(&calibration_state))?;
        
        // Phase 4: Emit calibration artifact
        let calib_artifact = CalibrationArtifact {
            artifact_id: format!("calib-{}", calibration_state.calibration_id),
            calibration_state: calibration_state.clone(),
            // ... (populated during execution)
        };
        export_calibration_artifact(&calib_artifact)?;
        
        Ok((artifact_path, calibration_state))
    }
}
```

### 6.3 Drift Monitoring Loop

```rust
pub struct DriftMonitor {
    detector: Box<dyn DriftDetector>,
    check_interval_seconds: u64,
    recalibration_threshold: f64,
}

impl DriftMonitor {
    pub async fn monitor_loop(
        &self,
        calibration_executor: Arc<dyn CalibrationExecutor>,
        calibration_kernel: CalibrationKernel,
    ) {
        loop {
            tokio::time::sleep(Duration::from_secs(self.check_interval_seconds)).await;
            
            let current_state = calibration_executor.get_current_calibration().unwrap();
            let measurements = self.collect_measurements().await;
            
            let drift_report = self.detector.detect_drift(&current_state, &measurements).unwrap();
            
            if drift_report.drift_detected {
                match drift_report.recommended_action {
                    RecalibrationAction::Recalibrate { urgency, target_nodes } => {
                        if urgency >= Urgency::Medium {
                            // Trigger recalibration
                            let new_state = calibration_executor
                                .execute_calibration(&calibration_kernel, Some(&current_state))
                                .unwrap();
                            
                            calibration_executor.apply_calibration(&new_state, &calibration_kernel.safety_constraints).unwrap();
                        }
                    }
                    _ => {}
                }
            }
        }
    }
}
```

---

## 7. Closed-Loop Optimization

### 7.1 Gradient-Based Calibration

For differentiable cost functions (e.g., transmission loss):

```rust
pub struct GradientCalibrator {
    gradient_provider: Box<dyn GradientProvider>,
    learning_rate: f64,
    max_iterations: usize,
}

impl GradientCalibrator {
    pub fn calibrate(
        &self,
        cost_function: &DifferentiableCostFunction,
        initial_params: &HashMap<String, f64>,
    ) -> Result<CalibrationState> {
        let mut params = initial_params.clone();
        let mut cost_history = Vec::new();
        
        for iteration in 0..self.max_iterations {
            // Compute cost and gradients
            let cost = self.evaluate_cost(&params)?;
            let gradients = self.gradient_provider.compute_gradients(&params, cost_function)?;
            
            cost_history.push((iteration, cost));
            
            // Gradient descent step
            for (param_name, grad) in gradients {
                let current_value = params.get(&param_name).unwrap();
                params.insert(param_name.clone(), current_value - self.learning_rate * grad);
            }
            
            // Check convergence
            if cost < convergence_threshold {
                break;
            }
        }
        
        Ok(CalibrationState {
            parameters: params,
            cost_function_value: cost_history.last().unwrap().1,
            // ... (full state)
        })
    }
}
```

### 7.2 Gradient-Free Calibration (Nelder-Mead)

For non-differentiable cost functions (e.g., extinction ratio):

```rust
pub struct NelderMeadCalibrator {
    simplex_size: f64,
    max_iterations: usize,
}

impl NelderMeadCalibrator {
    pub fn calibrate(
        &self,
        cost_function: &CostFunction,
        initial_params: &HashMap<String, f64>,
    ) -> Result<CalibrationState> {
        // Initialize simplex (N+1 points in N-dimensional parameter space)
        let mut simplex = self.initialize_simplex(initial_params);
        
        for iteration in 0..self.max_iterations {
            // Sort simplex by cost
            simplex.sort_by(|a, b| a.cost.partial_cmp(&b.cost).unwrap());
            
            // Compute centroid (excluding worst point)
            let centroid = self.compute_centroid(&simplex[0..simplex.len()-1]);
            
            // Try reflection
            let reflected = self.reflect(&simplex[simplex.len()-1], &centroid);
            
            // Nelder-Mead logic (reflection, expansion, contraction, shrink)
            // ... (standard Nelder-Mead algorithm)
            
            // Check convergence (simplex size < threshold)
            if self.simplex_diameter(&simplex) < convergence_threshold {
                break;
            }
        }
        
        // Best point is simplex[0]
        Ok(CalibrationState {
            parameters: simplex[0].params.clone(),
            cost_function_value: simplex[0].cost,
            // ...
        })
    }
}
```

---

## 8. Integration Test Scenario

### 8.1 Full Calibration Loop Test

**Scenario:** Run → Drift → Auto Recalibrate → Artifact Captures Full Story

```rust
#[test]
fn test_calibration_drift_recalibration_loop() {
    // Step 1: Define calibration kernel
    let calibration_kernel = CalibrationKernel {
        id: "mzi_calibration".to_string(),
        target_nodes: vec!["mzi_0".to_string()],
        parameters_to_tune: vec!["phase_upper".to_string()],
        cost_function: CostFunction::Minimize {
            expression: "1.0 - extinction_ratio".to_string(),
            target_value: Some(0.01), // Target 99% extinction
        },
        measurement_sequence: vec![
            MeasurementStep {
                step_id: "measure_extinction".to_string(),
                action: MeasurementAction::ReadSensor {
                    sensor_id: "detector_0".to_string(),
                    integration_time_ns: 1000,
                },
                expected_duration_ns: 2000,
            },
        ],
        optimizer_config: OptimizerConfig {
            algorithm: OptimizerAlgorithm::GradientDescent {
                learning_rate: 0.1,
                momentum: 0.9,
            },
            max_iterations: 100,
            convergence_threshold: 0.01,
            initial_guess: None,
        },
        safety_constraints: SafetyConstraints::default(),
        schedule: CalibrationSchedule::PreRun,
    };
    
    // Step 2: Run initial calibration
    let engine = Engine::new();
    let graph = load_test_graph("mzi_chain.json");
    let (artifact_path_1, calib_state_1) = engine
        .run_graph_with_calibration(&graph, Some(&calibration_kernel), Some(42))
        .unwrap();
    
    // Verify initial calibration artifact
    assert!(artifact_path_1.exists());
    let calib_artifact_1 = load_calibration_artifact(&artifact_path_1);
    assert_eq!(calib_artifact_1.calibration_state.version, 1);
    
    // Step 3: Inject thermal drift
    inject_thermal_drift(&engine, 5.0); // +5°C temperature change
    
    // Step 4: Detect drift
    let drift_detector = ThresholdDriftDetector::new(0.1); // 10% threshold
    let measurements = collect_measurements(&engine);
    let drift_report = drift_detector.detect_drift(&calib_state_1, &measurements).unwrap();
    
    assert!(drift_report.drift_detected);
    assert!(matches!(drift_report.recommended_action, RecalibrationAction::Recalibrate { .. }));
    
    // Step 5: Auto-recalibration
    let calibration_executor = ReferenceCalibrationExecutor::new();
    let calib_state_2 = calibration_executor
        .execute_calibration(&calibration_kernel, Some(&calib_state_1))
        .unwrap();
    
    // Verify recalibration artifact
    assert_eq!(calib_state_2.version, 2); // Version incremented
    assert_eq!(calib_state_2.provenance.parent_calibration_id, Some(calib_state_1.calibration_id));
    
    // Step 6: Verify full provenance chain
    let lineage = load_calibration_lineage(&calib_state_2);
    assert_eq!(lineage.len(), 2); // v1 → v2
    assert_eq!(lineage[0].version, 1);
    assert_eq!(lineage[1].version, 2);
    
    println!("✓ Full calibration loop: initial → drift → recalibrate → provenance tracked");
}
```

---

## 9. Observability Integration

### 9.1 Calibration Spans

All calibration operations emit observability spans:

```json
{
  "span_id": "calib-exec-001",
  "name": "calibration.execute",
  "start_iso": "2026-01-05T10:00:00Z",
  "end_iso": "2026-01-05T10:00:15Z",
  "attributes": {
    "calibration_kernel_id": "mzi_calibration",
    "target_nodes": ["mzi_0"],
    "optimizer_algorithm": "gradient_descent",
    "iterations": 42,
    "final_cost": 0.008,
    "convergence": "threshold_met"
  }
}
```

### 9.2 Calibration Metrics

```
- calibration_executions_total (counter)
- calibration_duration_seconds (histogram)
- calibration_cost_final (gauge)
- calibration_iterations (histogram)
- drift_detection_checks_total (counter)
- drift_detections_total (counter)
- recalibrations_triggered_total (counter)
```

### 9.3 Calibration Events

```json
{
  "event_id": "evt-drift-001",
  "timestamp_iso": "2026-01-05T10:05:00Z",
  "severity": "warning",
  "type": "drift_detected",
  "message": "Thermal drift exceeded threshold: 5.2°C delta",
  "metadata": {
    "drift_metric": "thermal_drift",
    "threshold": "3.0",
    "current_value": "5.2",
    "recommended_action": "recalibrate"
  }
}
```

---

## 10. Example: MZI Extinction Ratio Calibration

### 10.1 Problem

An MZI (Mach-Zehnder Interferometer) has two phase shifters. Goal: Calibrate phases to maximize extinction ratio (minimize cross-port transmission).

### 10.2 Calibration Kernel

```json
{
  "id": "mzi_extinction_calibration",
  "target_nodes": ["mzi_0"],
  "parameters_to_tune": ["phase_upper", "phase_lower"],
  "cost_function": {
    "Minimize": {
      "expression": "cross_port_power / through_port_power",
      "target_value": 0.01
    }
  },
  "measurement_sequence": [
    {
      "step_id": "set_phases",
      "action": {
        "SetParameter": {
          "node_id": "mzi_0",
          "param_name": "phase_upper",
          "value": "$phase_upper"
        }
      }
    },
    {
      "step_id": "measure_through",
      "action": {
        "ReadSensor": {
          "sensor_id": "detector_through",
          "integration_time_ns": 1000
        }
      }
    },
    {
      "step_id": "measure_cross",
      "action": {
        "ReadSensor": {
          "sensor_id": "detector_cross",
          "integration_time_ns": 1000
        }
      }
    }
  ],
  "optimizer_config": {
    "algorithm": {
      "NelderMead": {
        "initial_simplex_size": 0.1
      }
    },
    "max_iterations": 50,
    "convergence_threshold": 0.01,
    "initial_guess": {
      "phase_upper": 0.0,
      "phase_lower": 0.0
    }
  }
}
```

### 10.3 Expected Result

After calibration:
- Extinction ratio > 20 dB (99% suppression)
- Artifact contains 50 measurement iterations
- Cost function converged to 0.008
- Calibration took ~750ms

---

## 11. Migration Path from v0.1 to v0.2

**v0.1 (Legacy):**
- Manual calibration files
- No drift detection
- No versioning

**v0.2 (Current):**
- Calibration kernels as code
- Automatic drift detection
- Versioned calibration artifacts
- Closed-loop optimization

**v0.3 (Planned):**
- Multi-node co-calibration
- Adaptive optimizer selection
- Machine learning-based drift prediction

---

## 12. Appendix: Glossary

- **Extinction Ratio:** Power ratio between MZI outputs (measure of interference quality)
- **Drift:** Time-varying change in device parameters
- **Recalibration:** Updating calibration parameters in response to drift
- **Cost Function:** Objective function minimized during calibration
- **Convergence:** Reaching target cost function value within threshold

---

## 13. References

- [AEP-0007: Calibration as Computation](../aeps/AEP-0007-calibration-computation.md)
- [computation-model.md](computation-model.md) — Section 9: Calibration as first-class computation
- [observability.md](observability.md) — Integration with calibration spans/metrics
- [reproducibility.md](reproducibility.md) — Calibration artifact schema

---

**End of Specification**
