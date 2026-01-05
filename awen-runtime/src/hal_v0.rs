use anyhow::{anyhow, Result};
/// Hardware Abstraction Layer (HAL) v0.2 - Device Backend Management
///
/// Phase 2, Section 2.3 - Expands from Phase 1.4 HAL with:
/// - Real device backend support
/// - Device discovery and capability negotiation
/// - Multiple measurement modes (homodyne, heterodyne, direct)
/// - Real-time calibration integration
/// - Fault detection and graceful degradation
/// - Resource allocation and preemption
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================================
// Device Types & Capabilities
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceType {
    Simulator,
    SiliconPhotonics,
    InPGaAs,
    HybridPhotonics,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCapabilities {
    // Topology
    pub waveguides: usize,
    pub couplers: usize,
    pub phase_shifters: usize,
    pub detectors: usize,
    pub memory_elements: usize,

    // Electrical characteristics
    pub phase_shifter_range_radians: f64,
    pub power_handling_mw: f64,
    pub insertion_loss_db: f64,
    pub crosstalk_db: f64,

    // Measurement capabilities
    pub supports_homodyne: bool,
    pub supports_heterodyne: bool,
    pub supports_direct_detection: bool,

    // Temporal
    pub min_phase_pulse_ns: u64,
    pub phase_change_latency_ns: u64,
    pub measurement_readout_latency_ns: u64,
    pub coherence_time_us: u64,

    // Safety limits
    pub min_phase_voltage: f64,
    pub max_phase_voltage: f64,
    pub max_sustained_power_mw: f64,
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        // Conservative defaults (simulator-like)
        Self {
            waveguides: 8,
            couplers: 4,
            phase_shifters: 8,
            detectors: 2,
            memory_elements: 3,
            phase_shifter_range_radians: 2.0 * std::f64::consts::PI,
            power_handling_mw: 50.0,
            insertion_loss_db: 1.5,
            crosstalk_db: -20.0,
            supports_homodyne: true,
            supports_heterodyne: true,
            supports_direct_detection: true,
            min_phase_pulse_ns: 100,
            phase_change_latency_ns: 100,
            measurement_readout_latency_ns: 1000,
            coherence_time_us: 10000,
            min_phase_voltage: 0.0,
            max_phase_voltage: 10.0,
            max_sustained_power_mw: 50.0,
        }
    }
}

// ============================================================================
// Measurement Modes
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum MeasurementMode {
    Homodyne,
    Heterodyne,
    DirectDetection,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomodyneConfig {
    pub lo_phase_radians: f64,
    pub lo_power_mw: f64,
    pub vna_frequency_ghz: u32,
    pub integration_time_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HomodyneResult {
    pub quadrature_i: f64,
    pub quadrature_q: f64,
    pub variance: f64,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeterodyneConfig {
    pub signal_frequency_ghz: f64,
    pub lo_frequency_ghz: f64,
    pub intermediate_frequency_ghz: f64,
    pub demod_bandwidth_ghz: u32,
    pub integration_time_ms: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HeterodyneResult {
    pub magnitude: f64,
    pub phase: f64,
    pub snr_db: f64,
    pub timestamp_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectDetectionConfig {
    pub wavelength_nm: f64,
    pub integration_time_us: u32,
    pub dark_count_threshold: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DirectDetectionResult {
    pub photon_count: u32,
    pub dark_count: u32,
    pub click_probability: f64,
    pub timestamp_ns: u64,
}

// ============================================================================
// Calibration & Resource State
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceCalibrationState {
    pub phase_shifter_calibration: HashMap<usize, PhaseCalibration>,
    pub detector_calibration: HashMap<usize, DetectorCalibration>,
    pub last_update_timestamp: u64,
    pub validity_window_hours: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhaseCalibration {
    pub nominal_voltage_range: (f64, f64),
    pub thermal_drift_per_degree: f64,
    pub hysteresis_radians: f64,
}

impl Default for PhaseCalibration {
    fn default() -> Self {
        Self {
            nominal_voltage_range: (0.0, 10.0),
            thermal_drift_per_degree: 0.001,
            hysteresis_radians: 0.01,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectorCalibration {
    pub quantum_efficiency: f64,
    pub dark_count_hz: f64,
    pub saturation_count_khz: f64,
}

impl Default for DetectorCalibration {
    fn default() -> Self {
        Self {
            quantum_efficiency: 0.8,
            dark_count_hz: 100.0,
            saturation_count_khz: 10.0,
        }
    }
}

// ============================================================================
// Device Metrics & Health
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceMetrics {
    pub phases_completed: u64,
    pub measurements_taken: u64,
    pub total_execution_time_ns: u64,
    pub average_fidelity: f64,
    pub measurement_success_rate: f64,
    pub phase_shifter_accuracy: f64,
    pub peak_temperature_celsius: f64,
    pub average_power_consumption_mw: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Faulty,
}

// ============================================================================
// Fault Detection
// ============================================================================

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub enum DeviceFault {
    PhaseShifterOpen,
    CouplerMisalignment,
    WaveguideBend,
    WaveguideScattering,
    DetectorDarkCurrentHigh,
    LaserFrequencyDrift,
    ThermalThrottle,
    TemperatureUnstable,
    CalibrationStale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FaultDetectionThresholds {
    pub waveguide_loss_threshold_db: f64,
    pub phase_shifter_drift_radians_per_ms: f64,
    pub detector_dark_current_threshold_hz: f32,
    pub thermal_slope_celsius_per_second: f64,
}

impl Default for FaultDetectionThresholds {
    fn default() -> Self {
        Self {
            waveguide_loss_threshold_db: 0.5,
            phase_shifter_drift_radians_per_ms: 0.001,
            detector_dark_current_threshold_hz: 1000.0,
            thermal_slope_celsius_per_second: 0.5,
        }
    }
}

// ============================================================================
// Configuration
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HalConfig {
    pub default_backend: String,
    pub measurement_mode_priority: Vec<MeasurementMode>,
    pub auto_calibration_enabled: bool,
    pub max_thermal_throttle_events: u32,
    pub telemetry_enabled: bool,
    pub health_check_interval_ms: u32,
}

impl Default for HalConfig {
    fn default() -> Self {
        Self {
            default_backend: "simulator".to_string(),
            measurement_mode_priority: vec![
                MeasurementMode::DirectDetection,
                MeasurementMode::Heterodyne,
                MeasurementMode::Homodyne,
            ],
            auto_calibration_enabled: true,
            max_thermal_throttle_events: 5,
            telemetry_enabled: true,
            health_check_interval_ms: 1000,
        }
    }
}

// ============================================================================
// Backend Trait & Registry
// ============================================================================

pub trait PhotonicBackend: Send + Sync {
    fn capabilities(&self) -> Result<DeviceCapabilities>;
    fn device_type(&self) -> DeviceType;
    fn device_id(&self) -> String;

    fn set_phase_shifter(&mut self, index: usize, phase_radians: f64) -> Result<()>;
    fn set_coupler_split(&mut self, index: usize, ratio: f64) -> Result<()>;

    fn measure_homodyne(&mut self, config: HomodyneConfig) -> Result<HomodyneResult>;
    fn measure_heterodyne(&mut self, config: HeterodyneConfig) -> Result<HeterodyneResult>;
    fn measure_direct(&mut self, config: DirectDetectionConfig) -> Result<DirectDetectionResult>;

    fn load_calibration(&mut self, state: DeviceCalibrationState) -> Result<()>;
    fn get_calibration(&self) -> Result<DeviceCalibrationState>;

    fn initialize(&mut self) -> Result<()>;
    fn shutdown(&mut self) -> Result<()>;
    fn health_check(&mut self) -> Result<HealthStatus>;
}

pub struct BackendRegistry {
    backends: HashMap<String, Box<dyn PhotonicBackend>>,
    default_backend: Option<String>,
}

impl BackendRegistry {
    pub fn new() -> Self {
        Self {
            backends: HashMap::new(),
            default_backend: None,
        }
    }

    pub fn register(&mut self, id: String, backend: Box<dyn PhotonicBackend>) -> Result<()> {
        if self.backends.contains_key(&id) {
            return Err(anyhow!("Backend {} already registered", id));
        }
        self.backends.insert(id, backend);
        Ok(())
    }

    pub fn get(&self, id: &str) -> Result<&dyn PhotonicBackend> {
        self.backends
            .get(id)
            .ok_or_else(|| anyhow!("Backend {} not found", id))
            .map(|b| b.as_ref())
    }

    pub fn list_backends(&self) -> Vec<String> {
        self.backends.keys().cloned().collect()
    }

    pub fn set_default(&mut self, id: String) -> Result<()> {
        if !self.backends.contains_key(&id) {
            return Err(anyhow!("Backend {} not found", id));
        }
        self.default_backend = Some(id);
        Ok(())
    }

    pub fn get_default(&self) -> Result<&dyn PhotonicBackend> {
        let id = self
            .default_backend
            .as_ref()
            .ok_or_else(|| anyhow!("No default backend set"))?;
        self.get(id)
    }
}

impl Default for BackendRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Simulator Backend (Phase 1.4 compatible)
// ============================================================================

pub struct SimulatorBackend {
    pub capabilities: DeviceCapabilities,
    pub calibration: DeviceCalibrationState,
    pub metrics: DeviceMetrics,
}

impl SimulatorBackend {
    pub fn new() -> Self {
        Self {
            capabilities: DeviceCapabilities::default(),
            calibration: DeviceCalibrationState {
                phase_shifter_calibration: (0..8)
                    .map(|i| (i, PhaseCalibration::default()))
                    .collect(),
                detector_calibration: (0..2)
                    .map(|i| (i, DetectorCalibration::default()))
                    .collect(),
                last_update_timestamp: 0,
                validity_window_hours: 24,
            },
            metrics: DeviceMetrics {
                phases_completed: 0,
                measurements_taken: 0,
                total_execution_time_ns: 0,
                average_fidelity: 0.95,
                measurement_success_rate: 0.99,
                phase_shifter_accuracy: 0.98,
                peak_temperature_celsius: 25.0,
                average_power_consumption_mw: 10.0,
            },
        }
    }
}

impl Default for SimulatorBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl PhotonicBackend for SimulatorBackend {
    fn capabilities(&self) -> Result<DeviceCapabilities> {
        Ok(self.capabilities.clone())
    }

    fn device_type(&self) -> DeviceType {
        DeviceType::Simulator
    }

    fn device_id(&self) -> String {
        "simulator_v0.2".to_string()
    }

    fn set_phase_shifter(&mut self, _index: usize, _phase_radians: f64) -> Result<()> {
        Ok(())
    }

    fn set_coupler_split(&mut self, _index: usize, _ratio: f64) -> Result<()> {
        Ok(())
    }

    fn measure_homodyne(&mut self, config: HomodyneConfig) -> Result<HomodyneResult> {
        self.metrics.measurements_taken += 1;
        Ok(HomodyneResult {
            quadrature_i: 0.5 * config.lo_phase_radians.cos(),
            quadrature_q: 0.5 * config.lo_phase_radians.sin(),
            variance: 0.1,
            timestamp_ns: 0,
        })
    }

    fn measure_heterodyne(&mut self, _config: HeterodyneConfig) -> Result<HeterodyneResult> {
        self.metrics.measurements_taken += 1;
        Ok(HeterodyneResult {
            magnitude: 0.8,
            phase: 0.5,
            snr_db: 20.0,
            timestamp_ns: 0,
        })
    }

    fn measure_direct(&mut self, _config: DirectDetectionConfig) -> Result<DirectDetectionResult> {
        self.metrics.measurements_taken += 1;
        Ok(DirectDetectionResult {
            photon_count: 100,
            dark_count: 2,
            click_probability: 0.95,
            timestamp_ns: 0,
        })
    }

    fn load_calibration(&mut self, state: DeviceCalibrationState) -> Result<()> {
        self.calibration = state;
        Ok(())
    }

    fn get_calibration(&self) -> Result<DeviceCalibrationState> {
        Ok(self.calibration.clone())
    }

    fn initialize(&mut self) -> Result<()> {
        Ok(())
    }

    fn shutdown(&mut self) -> Result<()> {
        Ok(())
    }

    fn health_check(&mut self) -> Result<HealthStatus> {
        Ok(HealthStatus::Healthy)
    }
}

// ============================================================================
// HAL Manager (Main Interface)
// ============================================================================

pub struct HalManager {
    pub config: HalConfig,
    pub registry: BackendRegistry,
    pub metrics: DeviceMetrics,
}

impl HalManager {
    pub fn new(config: HalConfig) -> Self {
        Self {
            config,
            registry: BackendRegistry::new(),
            metrics: DeviceMetrics {
                phases_completed: 0,
                measurements_taken: 0,
                total_execution_time_ns: 0,
                average_fidelity: 0.95,
                measurement_success_rate: 0.99,
                phase_shifter_accuracy: 0.98,
                peak_temperature_celsius: 25.0,
                average_power_consumption_mw: 10.0,
            },
        }
    }

    pub fn register_simulator(&mut self) -> Result<()> {
        let simulator = Box::new(SimulatorBackend::new());
        self.registry.register("simulator".to_string(), simulator)?;
        self.registry.set_default("simulator".to_string())?;
        Ok(())
    }

    pub fn get_device(&self, id: &str) -> Result<&dyn PhotonicBackend> {
        self.registry.get(id)
    }

    pub fn get_default_device(&self) -> Result<&dyn PhotonicBackend> {
        self.registry.get_default()
    }

    pub fn discover_devices(&self) -> Vec<String> {
        self.registry.list_backends()
    }

    pub fn validate_execution_plan(
        &self,
        device_id: &str,
        phase_count: usize,
        total_duration_ns: u64,
    ) -> Result<bool> {
        let device = self.get_device(device_id)?;
        let caps = device.capabilities()?;

        // Check coherence window
        if total_duration_ns > caps.coherence_time_us * 1000 {
            return Err(anyhow!(
                "Execution time exceeds coherence window: {} > {}",
                total_duration_ns,
                caps.coherence_time_us * 1000
            ));
        }

        // Check phase count feasible
        if phase_count > 1000 {
            return Err(anyhow!("Too many phases: {}", phase_count));
        }

        Ok(true)
    }
}

impl Default for HalManager {
    fn default() -> Self {
        let mut manager = Self::new(HalConfig::default());
        let _ = manager.register_simulator();
        manager
    }
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_simulator_backend_creation() {
        let backend = SimulatorBackend::new();
        assert_eq!(backend.device_id(), "simulator_v0.2");
        assert_eq!(backend.device_type(), DeviceType::Simulator);
    }

    #[test]
    fn test_device_capabilities_default() {
        let caps = DeviceCapabilities::default();
        assert_eq!(caps.waveguides, 8);
        assert_eq!(caps.detectors, 2);
        assert!(caps.supports_homodyne);
        assert!(caps.supports_heterodyne);
        assert!(caps.supports_direct_detection);
    }

    #[test]
    fn test_backend_registry() {
        let mut registry = BackendRegistry::new();
        let backend = Box::new(SimulatorBackend::new());
        assert!(registry.register("sim".to_string(), backend).is_ok());
        assert!(registry.get("sim").is_ok());
        assert!(registry.get("nonexistent").is_err());
    }

    #[test]
    fn test_backend_default_setting() {
        let mut registry = BackendRegistry::new();
        let backend = Box::new(SimulatorBackend::new());
        registry.register("sim".to_string(), backend).unwrap();
        registry.set_default("sim".to_string()).unwrap();
        assert!(registry.get_default().is_ok());
    }

    #[test]
    fn test_homodyne_measurement() {
        let mut backend = SimulatorBackend::new();
        let config = HomodyneConfig {
            lo_phase_radians: 0.0,
            lo_power_mw: 20.0,
            vna_frequency_ghz: 10,
            integration_time_ms: 100,
        };
        let result = backend.measure_homodyne(config).unwrap();
        assert!(result.quadrature_i.is_finite());
        assert!(result.quadrature_q.is_finite());
    }

    #[test]
    fn test_heterodyne_measurement() {
        let mut backend = SimulatorBackend::new();
        let config = HeterodyneConfig {
            signal_frequency_ghz: 10.5,
            lo_frequency_ghz: 10.0,
            intermediate_frequency_ghz: 0.5,
            demod_bandwidth_ghz: 1,
            integration_time_ms: 50,
        };
        let result = backend.measure_heterodyne(config).unwrap();
        assert!(result.magnitude > 0.0);
        assert!(result.snr_db > 0.0);
    }

    #[test]
    fn test_direct_detection() {
        let mut backend = SimulatorBackend::new();
        let config = DirectDetectionConfig {
            wavelength_nm: 1550.0,
            integration_time_us: 100,
            dark_count_threshold: 10,
        };
        let result = backend.measure_direct(config).unwrap();
        assert!(result.photon_count > 0);
        assert!(result.click_probability <= 1.0);
    }

    #[test]
    fn test_hal_manager_creation() {
        let manager = HalManager::default();
        assert_eq!(manager.discover_devices().len(), 1);
        assert!(manager.get_default_device().is_ok());
    }

    #[test]
    fn test_execution_plan_validation_within_coherence() {
        let manager = HalManager::default();
        assert!(manager
            .validate_execution_plan("simulator", 10, 5_000_000) // 5ms < 10ms coherence
            .unwrap());
    }

    #[test]
    fn test_execution_plan_validation_exceeds_coherence() {
        let manager = HalManager::default();
        assert!(manager
            .validate_execution_plan("simulator", 100, 15_000_000) // 15ms > 10ms coherence
            .is_err());
    }

    #[test]
    fn test_calibration_state_management() {
        let mut backend = SimulatorBackend::new();
        let original = backend.get_calibration().unwrap();
        assert_eq!(original.phase_shifter_calibration.len(), 8);

        let mut new_state = original.clone();
        new_state.validity_window_hours = 48;
        backend.load_calibration(new_state).unwrap();
        let loaded = backend.get_calibration().unwrap();
        assert_eq!(loaded.validity_window_hours, 48);
    }

    #[test]
    fn test_health_check() {
        let mut backend = SimulatorBackend::new();
        let health = backend.health_check().unwrap();
        assert_eq!(health, HealthStatus::Healthy);
    }

    #[test]
    fn test_measurement_mode_priority() {
        let config = HalConfig::default();
        assert_eq!(
            config.measurement_mode_priority[0],
            MeasurementMode::DirectDetection
        );
        assert_eq!(
            config.measurement_mode_priority[1],
            MeasurementMode::Heterodyne
        );
        assert_eq!(
            config.measurement_mode_priority[2],
            MeasurementMode::Homodyne
        );
    }

    #[test]
    fn test_multiple_measurements() {
        let mut backend = SimulatorBackend::new();
        let count_before = backend.metrics.measurements_taken;

        let _ = backend.measure_direct(DirectDetectionConfig {
            wavelength_nm: 1550.0,
            integration_time_us: 100,
            dark_count_threshold: 10,
        });

        assert_eq!(backend.metrics.measurements_taken, count_before + 1);
    }

    #[test]
    fn test_fault_detection_thresholds_default() {
        let thresholds = FaultDetectionThresholds::default();
        assert!(thresholds.waveguide_loss_threshold_db > 0.0);
        assert!(thresholds.detector_dark_current_threshold_hz > 0.0);
    }
}
