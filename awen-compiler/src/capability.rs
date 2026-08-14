use crate::ir::{DType, GemmShape};
use crate::physical_design::PhysicalDesignBinding;
use crate::precision::AnalogNoiseModel;
use anyhow::{bail, Context, Result};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const CAPABILITY_VERSION: &str = "awen.device-capability.v1";
pub const HEALTH_VERSION: &str = "awen.backend-health.v1";
pub const CALIBRATION_SNAPSHOT_VERSION: &str = "awen.calibration-snapshot.v1";
pub const RUNTIME_ABI_VERSION: &str = "awen.runtime-backend.v1";
pub const PLUGIN_ABI_VERSION: &str = "awen.backend-plugin.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MatrixCore {
    pub m: usize,
    pub n: usize,
    pub k: usize,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum CoherenceMode {
    Coherent,
    Incoherent,
    Both,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AccumulationMode {
    Optical,
    Digital,
    Hybrid,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum BitSlicingMode {
    None,
    TwosComplement,
    SignedMagnitude,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum SaturationMode {
    Clamp,
    Error,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DynamicRange {
    pub minimum: f64,
    pub maximum: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OperationKind {
    Gemm,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct OperationCapability {
    pub operation: OperationKind,
    pub supports_transpose_lhs: bool,
    pub supports_transpose_rhs: bool,
    pub supports_partial_m: bool,
    pub supports_partial_n: bool,
    pub supports_partial_k: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationRequirements {
    pub required: bool,
    pub maximum_age_seconds: u64,
    pub temperature_tolerance_c: f64,
    pub drift_tolerance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationEnvironment {
    pub temperature_c: f64,
    pub laser_power_mw: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationCell {
    pub id: String,
    pub row: usize,
    pub column: usize,
    pub gain: f64,
    pub offset: f64,
    pub phase_error_radians: f64,
    pub insertion_loss_db: f64,
    pub uncertainty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationSpareCell {
    pub id: String,
    pub gain: f64,
    pub offset: f64,
    pub phase_error_radians: f64,
    pub insertion_loss_db: f64,
    pub uncertainty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationChannel {
    pub id: String,
    pub wavelength_nm: f64,
    pub gain: f64,
    pub phase_error_radians: f64,
    pub insertion_loss_db: f64,
    pub uncertainty: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationProfile {
    pub snapshot_version: String,
    pub id: String,
    pub fingerprint: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_id: Option<String>,
    pub backend_id: String,
    pub topology_fingerprint: String,
    pub measured_at: String,
    pub environment: CalibrationEnvironment,
    pub gain: f64,
    pub offset: f64,
    pub phase_error_radians: f64,
    pub insertion_loss_db: f64,
    pub uncertainty: f64,
    #[serde(default)]
    pub cells: Vec<CalibrationCell>,
    #[serde(default)]
    pub spare_cells: Vec<CalibrationSpareCell>,
    #[serde(default)]
    pub channels: Vec<CalibrationChannel>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct DeviceCapabilities {
    pub capability_version: String,
    pub runtime_abi_version: String,
    pub plugin_abi_version: String,
    pub backend_id: String,
    pub matrix_core: MatrixCore,
    pub supported_operations: Vec<OperationCapability>,
    pub supported_dtypes: Vec<DType>,
    pub supported_wavelengths_nm: Vec<f64>,
    pub modulation_rate_gbaud: f64,
    pub coherence_mode: CoherenceMode,
    pub adc_bits: u8,
    pub dac_bits: u8,
    pub effective_bits: u8,
    pub bit_slicing_modes: Vec<BitSlicingMode>,
    pub saturation_mode: SaturationMode,
    pub input_dynamic_range: DynamicRange,
    pub analog_noise: AnalogNoiseModel,
    pub sample_rate_gsps: f64,
    pub reconfiguration_latency_ns: f64,
    pub detector_bandwidth_ghz: f64,
    pub insertion_loss_budget_db: f64,
    pub supports_complex: bool,
    pub simultaneous_channels: usize,
    pub accumulation_modes: Vec<AccumulationMode>,
    pub calibration_requirements: CalibrationRequirements,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_profile: Option<CalibrationProfile>,
    pub physical_design: PhysicalDesignBinding,
    pub host_bandwidth_gbps: f64,
    pub link_bandwidth_gbps: f64,
    pub boundary_latency_ns: f64,
    pub laser_power_mw: f64,
    pub total_power_budget_mw: f64,
    pub dac_energy_pj_per_sample: f64,
    pub adc_energy_pj_per_sample: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum HealthStatus {
    Healthy,
    Degraded,
    Unavailable,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct BackendHealth {
    pub health_version: String,
    pub backend_id: String,
    pub observed_at: String,
    pub status: HealthStatus,
    pub temperature_c: f64,
    pub drift: f64,
    pub available_channels: usize,
    pub disabled_components: Vec<String>,
    pub unavailable_resources: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_profile_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub calibration_fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct NegotiationDiagnostic {
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct CapabilityNegotiation {
    pub backend_id: String,
    pub operation: OperationKind,
    pub eligible: bool,
    pub diagnostics: Vec<NegotiationDiagnostic>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct BackendSnapshot {
    pub capabilities: DeviceCapabilities,
    pub health: BackendHealth,
}

impl DeviceCapabilities {
    pub fn pace_like_128() -> Self {
        let mut capabilities = Self {
            capability_version: CAPABILITY_VERSION.to_string(),
            runtime_abi_version: RUNTIME_ABI_VERSION.to_string(),
            plugin_abi_version: PLUGIN_ABI_VERSION.to_string(),
            backend_id: "reference-pace-like-128".to_string(),
            matrix_core: MatrixCore {
                m: 128,
                n: 128,
                k: 128,
            },
            supported_operations: vec![OperationCapability {
                operation: OperationKind::Gemm,
                supports_transpose_lhs: true,
                supports_transpose_rhs: true,
                supports_partial_m: true,
                supports_partial_n: true,
                supports_partial_k: true,
            }],
            supported_dtypes: vec![DType::F16, DType::Bf16, DType::Int8, DType::Int4],
            supported_wavelengths_nm: (0..16).map(|index| 1530.0 + index as f64 * 1.6).collect(),
            modulation_rate_gbaud: 20.0,
            coherence_mode: CoherenceMode::Both,
            adc_bits: 10,
            dac_bits: 10,
            effective_bits: 8,
            bit_slicing_modes: vec![BitSlicingMode::None, BitSlicingMode::TwosComplement],
            saturation_mode: SaturationMode::Clamp,
            input_dynamic_range: DynamicRange {
                minimum: -1.0,
                maximum: 1.0,
            },
            analog_noise: AnalogNoiseModel {
                shot_noise_fraction: 0.000_05,
                thermal_noise_fraction: 0.000_02,
                phase_noise_radians: 0.000_01,
                detector_noise_fraction: 0.000_02,
            },
            sample_rate_gsps: 20.0,
            reconfiguration_latency_ns: 2_000.0,
            detector_bandwidth_ghz: 20.0,
            insertion_loss_budget_db: 12.0,
            supports_complex: false,
            simultaneous_channels: 16,
            accumulation_modes: vec![AccumulationMode::Digital, AccumulationMode::Hybrid],
            calibration_requirements: CalibrationRequirements {
                required: true,
                maximum_age_seconds: 3_600,
                temperature_tolerance_c: 0.5,
                drift_tolerance: 0.01,
            },
            calibration_profile: None,
            physical_design: PhysicalDesignBinding::reference_open_pdk(),
            host_bandwidth_gbps: 256.0,
            link_bandwidth_gbps: 256.0,
            boundary_latency_ns: 500.0,
            laser_power_mw: 500.0,
            total_power_budget_mw: 2_000.0,
            dac_energy_pj_per_sample: 2.0,
            adc_energy_pj_per_sample: 4.0,
        };
        capabilities.calibration_profile = Some(CalibrationProfile {
            snapshot_version: CALIBRATION_SNAPSHOT_VERSION.to_string(),
            id: "reference-room-temperature-v1".to_string(),
            fingerprint: "sha256:ef64f6a8fd46dfaaf57cd499e664a958faca5f99e6ee21bde8b0a54dca1bb8e6"
                .to_string(),
            parent_id: None,
            backend_id: capabilities.backend_id.clone(),
            topology_fingerprint: capabilities.topology_fingerprint(),
            measured_at: "2026-08-11T22:00:00Z".to_string(),
            environment: CalibrationEnvironment {
                temperature_c: 22.0,
                laser_power_mw: 500.0,
            },
            gain: 0.997,
            offset: 0.0002,
            phase_error_radians: 0.001,
            insertion_loss_db: 1.2,
            uncertainty: 0.002,
            cells: vec![CalibrationCell {
                id: "cell-0-0".to_string(),
                row: 0,
                column: 0,
                gain: 0.996,
                offset: 0.000_1,
                phase_error_radians: 0.001_1,
                insertion_loss_db: 1.25,
                uncertainty: 0.002_1,
            }],
            spare_cells: vec![
                CalibrationSpareCell {
                    id: "spare-cell-a".to_string(),
                    gain: 0.998,
                    offset: 0.000_1,
                    phase_error_radians: 0.000_8,
                    insertion_loss_db: 1.1,
                    uncertainty: 0.001_5,
                },
                CalibrationSpareCell {
                    id: "spare-cell-b".to_string(),
                    gain: 0.994,
                    offset: 0.000_2,
                    phase_error_radians: 0.001_4,
                    insertion_loss_db: 1.4,
                    uncertainty: 0.002_5,
                },
            ],
            channels: (0..16)
                .map(|index| CalibrationChannel {
                    id: format!("channel-{index}"),
                    wavelength_nm: 1530.0 + index as f64 * 1.6,
                    gain: 0.999 - index as f64 * 0.000_05,
                    phase_error_radians: 0.000_5 + index as f64 * 0.000_01,
                    insertion_loss_db: 0.8 + index as f64 * 0.01,
                    uncertainty: 0.001 + index as f64 * 0.000_02,
                })
                .collect(),
        });
        capabilities
    }

    pub fn topology_fingerprint(&self) -> String {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(self.backend_id.as_bytes());
        for dimension in [self.matrix_core.m, self.matrix_core.n, self.matrix_core.k] {
            bytes.extend_from_slice(&(dimension as u64).to_le_bytes());
        }
        bytes.extend_from_slice(&(self.simultaneous_channels as u64).to_le_bytes());
        for wavelength in &self.supported_wavelengths_nm {
            bytes.extend_from_slice(&wavelength.to_bits().to_le_bytes());
        }
        bytes.extend_from_slice(self.physical_design.content_fingerprint().as_bytes());
        format!("fnv1a64:{:016x}", fnv1a64(&bytes))
    }

    pub fn validate(&self) -> Result<()> {
        validate_version(&self.capability_version, CAPABILITY_VERSION, "capability")?;
        validate_version(
            &self.runtime_abi_version,
            RUNTIME_ABI_VERSION,
            "runtime ABI",
        )?;
        validate_version(&self.plugin_abi_version, PLUGIN_ABI_VERSION, "plugin ABI")?;
        if self.backend_id.trim().is_empty() {
            bail!("backend_id must not be empty");
        }
        if self.matrix_core.m == 0 || self.matrix_core.n == 0 || self.matrix_core.k == 0 {
            bail!("matrix core dimensions must be non-zero");
        }
        if self.supported_operations.is_empty() {
            bail!("device must advertise at least one supported operation");
        }
        ensure_unique(
            self.supported_operations
                .iter()
                .map(|operation| operation.operation),
            "supported operations",
        )?;
        if self.supported_dtypes.is_empty() {
            bail!("device must advertise at least one supported dtype");
        }
        ensure_unique(self.supported_dtypes.iter().copied(), "supported dtypes")?;
        let has_complex_dtype = self.supported_dtypes.iter().any(|dtype| dtype.is_complex());
        if has_complex_dtype != self.supports_complex {
            bail!(
                "supports_complex must agree with whether supported_dtypes contains a complex dtype"
            );
        }
        if self.supported_wavelengths_nm.is_empty() {
            bail!("photonic device must advertise at least one wavelength");
        }
        validate_positive_values(&self.supported_wavelengths_nm, "supported wavelengths")?;
        let mut wavelengths = HashSet::new();
        if self
            .supported_wavelengths_nm
            .iter()
            .any(|value| !wavelengths.insert(value.to_bits()))
        {
            bail!("supported wavelengths must not contain duplicates");
        }
        validate_positive(self.modulation_rate_gbaud, "modulation_rate_gbaud")?;
        validate_positive(self.sample_rate_gsps, "sample_rate_gsps")?;
        validate_positive(self.host_bandwidth_gbps, "host_bandwidth_gbps")?;
        validate_positive(self.link_bandwidth_gbps, "link_bandwidth_gbps")?;
        validate_positive(self.detector_bandwidth_ghz, "detector_bandwidth_ghz")?;
        validate_non_negative(
            self.reconfiguration_latency_ns,
            "reconfiguration_latency_ns",
        )?;
        validate_non_negative(self.boundary_latency_ns, "boundary_latency_ns")?;
        validate_non_negative(self.insertion_loss_budget_db, "insertion_loss_budget_db")?;
        validate_non_negative(self.laser_power_mw, "laser_power_mw")?;
        validate_positive(self.total_power_budget_mw, "total_power_budget_mw")?;
        if self.laser_power_mw > self.total_power_budget_mw {
            bail!("laser_power_mw cannot exceed total_power_budget_mw");
        }
        validate_non_negative(self.dac_energy_pj_per_sample, "dac_energy_pj_per_sample")?;
        validate_non_negative(self.adc_energy_pj_per_sample, "adc_energy_pj_per_sample")?;
        if self.adc_bits == 0 || self.dac_bits == 0 || self.effective_bits == 0 {
            bail!("ADC, DAC, and effective precision must be non-zero");
        }
        if self.bit_slicing_modes.is_empty() {
            bail!("device must advertise at least one bit-slicing mode");
        }
        ensure_unique(self.bit_slicing_modes.iter().copied(), "bit-slicing modes")?;
        if (self.effective_bits > self.adc_bits || self.effective_bits > self.dac_bits)
            && !self
                .bit_slicing_modes
                .iter()
                .any(|mode| *mode != BitSlicingMode::None)
        {
            bail!(
                "effective_bits above ADC or DAC precision requires a non-trivial bit-slicing mode"
            );
        }
        validate_finite(
            self.input_dynamic_range.minimum,
            "input dynamic-range minimum",
        )?;
        validate_finite(
            self.input_dynamic_range.maximum,
            "input dynamic-range maximum",
        )?;
        if self.input_dynamic_range.minimum >= self.input_dynamic_range.maximum {
            bail!("input dynamic-range minimum must be less than maximum");
        }
        self.analog_noise.validate()?;
        if self.simultaneous_channels == 0 {
            bail!("simultaneous_channels must be non-zero");
        }
        if self.simultaneous_channels > self.supported_wavelengths_nm.len() {
            bail!("simultaneous_channels cannot exceed the advertised wavelength count");
        }
        if self.accumulation_modes.is_empty() {
            bail!("device must advertise at least one accumulation mode");
        }
        ensure_unique(
            self.accumulation_modes.iter().copied(),
            "accumulation modes",
        )?;
        validate_non_negative(
            self.calibration_requirements.temperature_tolerance_c,
            "calibration temperature tolerance",
        )?;
        validate_non_negative(
            self.calibration_requirements.drift_tolerance,
            "calibration drift tolerance",
        )?;
        if self.calibration_requirements.required
            && self.calibration_requirements.maximum_age_seconds == 0
        {
            bail!("required calibration must have a non-zero maximum age");
        }
        self.physical_design.validate()?;
        if let Some(profile) = &self.calibration_profile {
            profile.validate(self)?;
        }
        Ok(())
    }

    pub fn supports(&self, dtype: DType) -> bool {
        self.supported_dtypes.contains(&dtype) && (!dtype.is_complex() || self.supports_complex)
    }

    pub fn operation(&self, kind: OperationKind) -> Option<&OperationCapability> {
        self.supported_operations
            .iter()
            .find(|operation| operation.operation == kind)
    }
}

impl CalibrationProfile {
    fn validate(&self, capabilities: &DeviceCapabilities) -> Result<()> {
        validate_version(
            &self.snapshot_version,
            CALIBRATION_SNAPSHOT_VERSION,
            "calibration snapshot",
        )?;
        if self.id.trim().is_empty() {
            bail!("calibration profile id must not be empty");
        }
        if self.fingerprint.trim().is_empty() {
            bail!("calibration fingerprint must not be empty");
        }
        validate_calibration_fingerprint(&self.fingerprint, "calibration fingerprint")?;
        if self
            .parent_id
            .as_ref()
            .is_some_and(|parent| parent.trim().is_empty() || parent == &self.id)
        {
            bail!("calibration parent id must be non-empty and differ from the snapshot id");
        }
        if self.backend_id != capabilities.backend_id {
            bail!(
                "calibration profile backend '{}' does not match capability backend '{}'",
                self.backend_id,
                capabilities.backend_id
            );
        }
        let expected_topology = capabilities.topology_fingerprint();
        if self.topology_fingerprint != expected_topology {
            bail!(
                "calibration topology fingerprint '{}' does not match device topology '{}'",
                self.topology_fingerprint,
                expected_topology
            );
        }
        parse_timestamp(&self.measured_at, "calibration measured_at")?;
        validate_finite(
            self.environment.temperature_c,
            "calibration environment temperature",
        )?;
        validate_non_negative(
            self.environment.laser_power_mw,
            "calibration environment laser power",
        )?;
        if self.environment.laser_power_mw > capabilities.total_power_budget_mw {
            bail!("calibration environment laser power exceeds the device power budget");
        }
        validate_transfer_metrics(
            self.gain,
            self.offset,
            self.phase_error_radians,
            self.insertion_loss_db,
            self.uncertainty,
            "calibration transfer",
        )?;
        let mut component_ids = HashSet::new();
        let mut logical_cells = HashSet::new();
        for cell in &self.cells {
            if !component_ids.insert(cell.id.as_str()) || cell.id.trim().is_empty() {
                bail!("calibration cell ids must be non-empty and unique");
            }
            if !logical_cells.insert((cell.row, cell.column)) {
                bail!("calibration cell coordinates must be unique");
            }
            if cell.row >= capabilities.matrix_core.m || cell.column >= capabilities.matrix_core.n {
                bail!(
                    "calibration cell '{}' lies outside the matrix topology",
                    cell.id
                );
            }
            validate_transfer_metrics(
                cell.gain,
                cell.offset,
                cell.phase_error_radians,
                cell.insertion_loss_db,
                cell.uncertainty,
                "cell calibration",
            )?;
        }
        for spare in &self.spare_cells {
            if !component_ids.insert(spare.id.as_str()) || spare.id.trim().is_empty() {
                bail!("calibration cell and spare ids must be non-empty and unique");
            }
            validate_transfer_metrics(
                spare.gain,
                spare.offset,
                spare.phase_error_radians,
                spare.insertion_loss_db,
                spare.uncertainty,
                "spare-cell calibration",
            )?;
        }
        let mut channel_ids = HashSet::new();
        let mut channel_wavelengths = HashSet::new();
        for channel in &self.channels {
            if !channel_ids.insert(channel.id.as_str()) || channel.id.trim().is_empty() {
                bail!("calibration channel ids must be non-empty and unique");
            }
            if !channel_wavelengths.insert(channel.wavelength_nm.to_bits()) {
                bail!("calibration channel wavelengths must be unique");
            }
            if !capabilities
                .supported_wavelengths_nm
                .iter()
                .any(|wavelength| wavelength.to_bits() == channel.wavelength_nm.to_bits())
            {
                bail!(
                    "calibration channel '{}' is not present in the device topology",
                    channel.id
                );
            }
            validate_transfer_metrics(
                channel.gain,
                0.0,
                channel.phase_error_radians,
                channel.insertion_loss_db,
                channel.uncertainty,
                "channel calibration",
            )?;
        }
        Ok(())
    }
}

impl BackendHealth {
    pub fn validate(&self, capabilities: &DeviceCapabilities) -> Result<()> {
        validate_version(&self.health_version, HEALTH_VERSION, "health")?;
        if self.backend_id != capabilities.backend_id {
            bail!(
                "health backend '{}' does not match capability backend '{}'",
                self.backend_id,
                capabilities.backend_id
            );
        }
        parse_timestamp(&self.observed_at, "health observed_at")?;
        validate_finite(self.temperature_c, "health temperature")?;
        validate_non_negative(self.drift, "health drift")?;
        if self.available_channels > capabilities.simultaneous_channels {
            bail!(
                "health available_channels {} exceeds capability simultaneous_channels {}",
                self.available_channels,
                capabilities.simultaneous_channels
            );
        }
        if let Some(profile) = &capabilities.calibration_profile {
            let disabled_channels = profile
                .channels
                .iter()
                .filter(|channel| self.disabled_components.contains(&channel.id))
                .count();
            let maximum_available = capabilities
                .simultaneous_channels
                .saturating_sub(disabled_channels);
            if self.available_channels > maximum_available {
                bail!(
                    "health available_channels {} contradicts {} disabled calibrated channels",
                    self.available_channels,
                    disabled_channels
                );
            }
        }
        ensure_non_empty_unique(&self.disabled_components, "disabled components")?;
        ensure_non_empty_unique(&self.unavailable_resources, "unavailable resources")?;
        if self
            .calibration_profile_id
            .as_ref()
            .is_some_and(|id| id.trim().is_empty())
        {
            bail!("health calibration_profile_id must not be empty");
        }
        if self
            .calibration_fingerprint
            .as_ref()
            .is_some_and(|fingerprint| fingerprint.trim().is_empty())
        {
            bail!("health calibration_fingerprint must not be empty");
        }
        if let Some(fingerprint) = &self.calibration_fingerprint {
            validate_calibration_fingerprint(fingerprint, "health calibration_fingerprint")?;
        }
        if self.calibration_profile_id.is_some() != self.calibration_fingerprint.is_some() {
            bail!(
                "health must provide calibration_profile_id and calibration_fingerprint together"
            );
        }
        Ok(())
    }
}

impl BackendSnapshot {
    /// Construct a deterministic offline snapshot for tools that received only a
    /// capability document. Required calibration is evaluated at its measured
    /// timestamp; runtime execution should use a queried health snapshot instead.
    pub fn offline(capabilities: DeviceCapabilities) -> Result<Self> {
        let observed_at = capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.measured_at.clone())
            .unwrap_or_else(|| "1970-01-01T00:00:00Z".to_string());
        let calibration_profile_id = capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.id.clone());
        let calibration_fingerprint = capabilities
            .calibration_profile
            .as_ref()
            .map(|profile| profile.fingerprint.clone());
        let health = BackendHealth {
            health_version: HEALTH_VERSION.to_string(),
            backend_id: capabilities.backend_id.clone(),
            observed_at,
            status: HealthStatus::Healthy,
            temperature_c: capabilities
                .calibration_profile
                .as_ref()
                .map_or(20.0, |profile| profile.environment.temperature_c),
            drift: 0.0,
            available_channels: capabilities.simultaneous_channels,
            disabled_components: Vec::new(),
            unavailable_resources: Vec::new(),
            calibration_profile_id,
            calibration_fingerprint,
        };
        Self::new(capabilities, health)
    }

    pub fn new(capabilities: DeviceCapabilities, health: BackendHealth) -> Result<Self> {
        capabilities.validate()?;
        health.validate(&capabilities)?;
        Ok(Self {
            capabilities,
            health,
        })
    }

    pub fn negotiate_gemm(
        &self,
        shape: GemmShape,
        dtype: DType,
        minimum_effective_bits: Option<u8>,
        transpose_lhs: bool,
        transpose_rhs: bool,
    ) -> CapabilityNegotiation {
        let mut diagnostics = Vec::new();
        let capabilities = &self.capabilities;
        let health = &self.health;

        if health.status == HealthStatus::Unavailable {
            reject(
                &mut diagnostics,
                "backend_unavailable",
                "backend health status is unavailable",
            );
        }
        if health.available_channels == 0 {
            reject(
                &mut diagnostics,
                "no_channels",
                "backend has no available wavelength channels",
            );
        }
        if health
            .unavailable_resources
            .iter()
            .any(|resource| resource == "matrix_core")
        {
            reject(
                &mut diagnostics,
                "matrix_core_unavailable",
                "the matrix core is unavailable",
            );
        }
        if let Some(profile) = &capabilities.calibration_profile {
            let disabled_cells = profile
                .cells
                .iter()
                .filter(|cell| health.disabled_components.contains(&cell.id))
                .count();
            let healthy_spares = profile
                .spare_cells
                .iter()
                .filter(|spare| !health.disabled_components.contains(&spare.id))
                .count();
            if disabled_cells > healthy_spares {
                reject(
                    &mut diagnostics,
                    "calibration_remap_capacity_exhausted",
                    format!(
                        "{disabled_cells} calibrated cells are disabled but only {} calibrated spares are available",
                        healthy_spares
                    ),
                );
            }
        }
        let Some(operation) = capabilities.operation(OperationKind::Gemm) else {
            reject(
                &mut diagnostics,
                "operation_unsupported",
                "backend does not advertise GEMM",
            );
            return negotiation(capabilities, diagnostics);
        };
        if transpose_lhs && !operation.supports_transpose_lhs {
            reject(
                &mut diagnostics,
                "transpose_lhs_unsupported",
                "backend does not support a transposed left operand",
            );
        }
        if transpose_rhs && !operation.supports_transpose_rhs {
            reject(
                &mut diagnostics,
                "transpose_rhs_unsupported",
                "backend does not support a transposed right operand",
            );
        }
        if !shape.m.is_multiple_of(capabilities.matrix_core.m) && !operation.supports_partial_m {
            reject(
                &mut diagnostics,
                "partial_m_unsupported",
                "M dimension requires a partial tile that the backend does not support",
            );
        }
        if !shape.n.is_multiple_of(capabilities.matrix_core.n) && !operation.supports_partial_n {
            reject(
                &mut diagnostics,
                "partial_n_unsupported",
                "N dimension requires a partial tile that the backend does not support",
            );
        }
        if !shape.k.is_multiple_of(capabilities.matrix_core.k) && !operation.supports_partial_k {
            reject(
                &mut diagnostics,
                "partial_k_unsupported",
                "K dimension requires a partial tile that the backend does not support",
            );
        }
        if !capabilities.supports(dtype) {
            reject(
                &mut diagnostics,
                "dtype_unsupported",
                format!("backend does not support dtype {dtype:?}"),
            );
        }
        let can_bit_slice = capabilities
            .bit_slicing_modes
            .iter()
            .any(|mode| *mode != BitSlicingMode::None);
        if minimum_effective_bits
            .is_some_and(|bits| bits > capabilities.effective_bits && !can_bit_slice)
        {
            reject(
                &mut diagnostics,
                "precision_insufficient",
                "backend effective precision is below the operation contract and no bit-slicing mode is available",
            );
        }
        self.check_calibration(&mut diagnostics);
        negotiation(capabilities, diagnostics)
    }

    fn check_calibration(&self, diagnostics: &mut Vec<NegotiationDiagnostic>) {
        let requirements = &self.capabilities.calibration_requirements;
        if !requirements.required {
            return;
        }
        let Some(profile) = &self.capabilities.calibration_profile else {
            reject(
                diagnostics,
                "calibration_missing",
                "backend requires calibration but no profile was supplied",
            );
            return;
        };
        if self.health.calibration_profile_id.as_deref() != Some(profile.id.as_str()) {
            reject(
                diagnostics,
                "calibration_mismatch",
                "health snapshot does not confirm the advertised calibration profile",
            );
        }
        if self.health.calibration_fingerprint.as_deref() != Some(profile.fingerprint.as_str()) {
            reject(
                diagnostics,
                "calibration_fingerprint_mismatch",
                "health snapshot does not confirm the exact calibration fingerprint",
            );
        }
        let measured = parse_timestamp(&profile.measured_at, "calibration measured_at")
            .expect("validated capability timestamp");
        let observed = parse_timestamp(&self.health.observed_at, "health observed_at")
            .expect("validated health timestamp");
        let age = observed.signed_duration_since(measured).num_seconds();
        if age < 0 {
            reject(
                diagnostics,
                "calibration_from_future",
                "calibration timestamp is later than the health observation",
            );
        } else if age as u64 > requirements.maximum_age_seconds {
            reject(
                diagnostics,
                "calibration_expired",
                format!(
                    "calibration is {age} seconds old; maximum is {} seconds",
                    requirements.maximum_age_seconds
                ),
            );
        }
        if (self.health.temperature_c - profile.environment.temperature_c).abs()
            > requirements.temperature_tolerance_c
        {
            reject(
                diagnostics,
                "temperature_out_of_range",
                "health temperature is outside the calibration tolerance",
            );
        }
        if self.health.drift > requirements.drift_tolerance {
            reject(
                diagnostics,
                "drift_out_of_range",
                "measured drift exceeds the calibration tolerance",
            );
        }
    }
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self::pace_like_128()
    }
}

fn negotiation(
    capabilities: &DeviceCapabilities,
    diagnostics: Vec<NegotiationDiagnostic>,
) -> CapabilityNegotiation {
    CapabilityNegotiation {
        backend_id: capabilities.backend_id.clone(),
        operation: OperationKind::Gemm,
        eligible: diagnostics.is_empty(),
        diagnostics,
    }
}

fn reject(
    diagnostics: &mut Vec<NegotiationDiagnostic>,
    code: impl Into<String>,
    message: impl Into<String>,
) {
    diagnostics.push(NegotiationDiagnostic {
        code: code.into(),
        message: message.into(),
    });
}

fn validate_version(actual: &str, expected: &str, kind: &str) -> Result<()> {
    if actual == expected {
        return Ok(());
    }
    bail!("unsupported {kind} version '{actual}'; this build supports '{expected}'")
}

fn validate_finite(value: f64, field: &str) -> Result<()> {
    if !value.is_finite() {
        bail!("{field} must be finite");
    }
    Ok(())
}

fn validate_positive(value: f64, field: &str) -> Result<()> {
    validate_finite(value, field)?;
    if value <= 0.0 {
        bail!("{field} must be positive");
    }
    Ok(())
}

fn validate_non_negative(value: f64, field: &str) -> Result<()> {
    validate_finite(value, field)?;
    if value < 0.0 {
        bail!("{field} must be non-negative");
    }
    Ok(())
}

fn validate_transfer_metrics(
    gain: f64,
    offset: f64,
    phase_error_radians: f64,
    insertion_loss_db: f64,
    uncertainty: f64,
    field: &str,
) -> Result<()> {
    validate_finite(gain, &format!("{field} gain"))?;
    validate_finite(offset, &format!("{field} offset"))?;
    validate_finite(phase_error_radians, &format!("{field} phase error"))?;
    validate_non_negative(insertion_loss_db, &format!("{field} insertion loss"))?;
    validate_non_negative(uncertainty, &format!("{field} uncertainty"))?;
    if gain == 0.0 {
        bail!("{field} gain must be non-zero");
    }
    Ok(())
}

fn validate_calibration_fingerprint(value: &str, field: &str) -> Result<()> {
    let valid = value
        .strip_prefix("sha256:")
        .is_some_and(|digest| digest.len() == 64 && digest.bytes().all(is_lower_hex))
        || value
            .strip_prefix("fnv1a64:")
            .is_some_and(|digest| digest.len() == 16 && digest.bytes().all(is_lower_hex));
    if !valid {
        bail!("{field} must be a sha256 or fnv1a64 lowercase hexadecimal digest");
    }
    Ok(())
}

fn is_lower_hex(byte: u8) -> bool {
    byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf2_9ce4_8422_2325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }
    hash
}

fn validate_positive_values(values: &[f64], field: &str) -> Result<()> {
    for value in values {
        validate_positive(*value, field)?;
    }
    Ok(())
}

fn ensure_unique<T: Eq + std::hash::Hash>(
    values: impl IntoIterator<Item = T>,
    field: &str,
) -> Result<()> {
    let mut seen = HashSet::new();
    if values.into_iter().any(|value| !seen.insert(value)) {
        bail!("{field} must not contain duplicates");
    }
    Ok(())
}

fn ensure_non_empty_unique(values: &[String], field: &str) -> Result<()> {
    if values.iter().any(|value| value.trim().is_empty()) {
        bail!("{field} must not contain empty identifiers");
    }
    ensure_unique(values.iter(), field)
}

fn parse_timestamp(value: &str, field: &str) -> Result<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(value)
        .with_context(|| format!("{field} must be an RFC 3339 timestamp"))
        .map(|timestamp| timestamp.with_timezone(&Utc))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn healthy_snapshot() -> BackendSnapshot {
        let capabilities = DeviceCapabilities::pace_like_128();
        BackendSnapshot::new(
            capabilities.clone(),
            BackendHealth {
                health_version: HEALTH_VERSION.to_string(),
                backend_id: capabilities.backend_id.clone(),
                observed_at: "2026-08-11T22:30:00Z".to_string(),
                status: HealthStatus::Healthy,
                temperature_c: 22.1,
                drift: 0.002,
                available_channels: 16,
                disabled_components: Vec::new(),
                unavailable_resources: Vec::new(),
                calibration_profile_id: Some("reference-room-temperature-v1".to_string()),
                calibration_fingerprint: capabilities
                    .calibration_profile
                    .as_ref()
                    .map(|profile| profile.fingerprint.clone()),
            },
        )
        .expect("reference snapshot must validate")
    }

    #[test]
    fn rejects_version_skew() {
        let mut capabilities = DeviceCapabilities::pace_like_128();
        capabilities.capability_version = "awen.device-capability.v2".to_string();
        let error = capabilities
            .validate()
            .expect_err("future major version must fail");
        assert!(error
            .to_string()
            .contains("supports 'awen.device-capability.v1'"));
    }

    #[test]
    fn rejects_contradictory_complex_advertisement() {
        let mut capabilities = DeviceCapabilities::pace_like_128();
        capabilities.supports_complex = true;
        let error = capabilities
            .validate()
            .expect_err("complex flag without complex dtype must fail");
        assert!(error.to_string().contains("supports_complex"));
    }

    #[test]
    fn rejects_cross_backend_and_zero_gain_calibration_profiles() {
        let mut capabilities = DeviceCapabilities::pace_like_128();
        capabilities
            .calibration_profile
            .as_mut()
            .expect("reference calibration")
            .backend_id = "different-device".to_string();
        let error = capabilities
            .validate()
            .expect_err("cross-backend calibration must fail");
        assert!(error.to_string().contains("does not match"));

        let mut capabilities = DeviceCapabilities::pace_like_128();
        capabilities
            .calibration_profile
            .as_mut()
            .expect("reference calibration")
            .gain = 0.0;
        let error = capabilities
            .validate()
            .expect_err("zero-gain calibration must fail");
        assert!(error.to_string().contains("gain must be non-zero"));
    }

    #[test]
    fn expired_calibration_is_ineligible() {
        let mut snapshot = healthy_snapshot();
        snapshot.health.observed_at = "2026-08-12T00:00:01Z".to_string();
        let negotiation = snapshot.negotiate_gemm(
            GemmShape {
                m: 128,
                n: 128,
                k: 128,
            },
            DType::F16,
            Some(8),
            false,
            false,
        );
        assert!(!negotiation.eligible);
        assert!(negotiation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "calibration_expired"));
    }

    #[test]
    fn partial_tile_constraint_is_enforced() {
        let mut snapshot = healthy_snapshot();
        snapshot.capabilities.supported_operations[0].supports_partial_m = false;
        let negotiation = snapshot.negotiate_gemm(
            GemmShape {
                m: 129,
                n: 128,
                k: 128,
            },
            DType::F16,
            Some(8),
            false,
            false,
        );
        assert!(!negotiation.eligible);
        assert_eq!(negotiation.diagnostics[0].code, "partial_m_unsupported");
    }

    #[test]
    fn unavailable_resources_are_ineligible() {
        let mut snapshot = healthy_snapshot();
        snapshot
            .health
            .unavailable_resources
            .push("matrix_core".to_string());
        let negotiation = snapshot.negotiate_gemm(
            GemmShape {
                m: 128,
                n: 128,
                k: 128,
            },
            DType::F16,
            Some(8),
            false,
            false,
        );
        assert!(!negotiation.eligible);
        assert!(negotiation
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == "matrix_core_unavailable"));
    }
}
