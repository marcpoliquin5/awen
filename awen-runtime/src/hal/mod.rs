// Hardware Abstraction Layer (v0.1)
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// Device capability categories. Backends declare which capabilities they provide.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub enum ChannelType {
    Optical,
    Electrical,
    Thermal,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Capability {
    pub name: String,
    pub channel: ChannelType,
    pub metadata: Option<HashMap<String, String>>,
}

/// Minimal device abstraction used by the runtime. Implementations may represent lab instruments
/// or simulated devices. Device implementations must expose stable capability descriptors and a
/// set of control primitives (set_param / read_sensor) used by calibration and control loops.
pub trait Device {
    fn id(&self) -> String;
    fn capabilities(&self) -> Vec<Capability>;

    /// Set a named parameter on the device (e.g., `mzi_3:phase`) to a value. Returns an error string on failure.
    fn set_param(&self, name: &str, value: f64) -> Result<(), String> { let _ = (name, value); Ok(()) }

    /// Read a named sensor or observable (e.g., `detector_1:power`). Returns value or error.
    fn read_sensor(&self, name: &str) -> Result<f64, String> { let _ = name; Ok(0.0) }
}

/// Lab-specific device trait exposing safety-constrained calibration primitives.
pub trait LabDevice: Device {
    /// Apply a calibration map (parameter -> voltage/current) with optional safety limits.
    fn apply_calibration(&self, mapping: &HashMap<String, f64>, safety: Option<&SafetyLimits>) -> Result<CalibrationResult, String>;

    /// Query device health and status metadata for observability and reproducibility.
    fn health_report(&self) -> HashMap<String, String>;
}

/// Safety limits for control operations.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct SafetyLimits {
    pub max_voltage: Option<f64>,
    pub min_voltage: Option<f64>,
    pub max_temperature: Option<f64>,
    pub notes: Option<String>,
}

/// Calibration application result.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CalibrationResult {
    pub success: bool,
    pub applied: HashMap<String, f64>,
    pub warnings: Vec<String>,
}

/// Default simulated device implementation used by the reference HAL.
///
/// Note: `SimulatedDevice` is intentionally crate-private to prevent external code from
/// constructing it directly and bypassing runtime safety chokepoints. External users must
/// go through runtime APIs (e.g., `Engine::apply_calibration`).
pub(crate) struct SimulatedDevice;
impl Device for SimulatedDevice {
    fn id(&self) -> String { "simulated".into() }
    fn capabilities(&self) -> Vec<Capability> {
        vec![
            Capability { name: "mzi".into(), channel: ChannelType::Optical, metadata: None },
            Capability { name: "ring".into(), channel: ChannelType::Optical, metadata: None },
            Capability { name: "detector".into(), channel: ChannelType::Optical, metadata: None },
        ]
    }
}

impl SimulatedDevice {
    /// Convenience constructor (crate-private)
    pub(crate) fn new() -> Self { Self {} }
}

/// Examples and enforcement: external attempts to construct `SimulatedDevice` should fail.
///
/// ```compile_fail
/// // external code cannot construct the crate-private `SimulatedDevice` type
/// let _ = awen_runtime::hal::SimulatedDevice::new();
/// ```

impl LabDevice for SimulatedDevice {
    fn apply_calibration(&self, mapping: &HashMap<String, f64>, _safety: Option<&SafetyLimits>) -> Result<CalibrationResult, String> {
        // In simulation we apply safety limits if provided and echo back applied values.
        let mut applied = mapping.clone();
        let mut warnings: Vec<String> = Vec::new();
        if let Some(s) = _safety {
            for (k, v) in mapping.iter() {
                let mut val = *v;
                if let Some(max_v) = s.max_voltage {
                    if val > max_v { warnings.push(format!("{} above max_voltage ({}), clamping", k, max_v)); val = max_v; }
                }
                if let Some(min_v) = s.min_voltage {
                    if val < min_v { warnings.push(format!("{} below min_voltage ({}), clamping", k, min_v)); val = min_v; }
                }
                applied.insert(k.clone(), val);
            }
        }
        Ok(CalibrationResult { success: true, applied, warnings })
    }

    fn health_report(&self) -> HashMap<String, String> {
        let mut m = HashMap::new();
        m.insert("status".into(), "simulated-ok".into());
        m
    }
}
