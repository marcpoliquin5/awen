use crate::ir::DType;
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};

pub const CAPABILITY_VERSION: &str = "awen.device-capability.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
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

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AccumulationMode {
    Optical,
    Digital,
    Hybrid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationRequirements {
    pub required: bool,
    pub maximum_age_seconds: u64,
    pub temperature_tolerance_c: f64,
    pub drift_tolerance: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CalibrationProfile {
    pub id: String,
    pub measured_at: String,
    pub temperature_c: f64,
    pub gain: f64,
    pub offset: f64,
    pub phase_error_radians: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct DeviceCapabilities {
    pub capability_version: String,
    pub backend_id: String,
    pub matrix_core: MatrixCore,
    pub supported_dtypes: Vec<DType>,
    pub supported_wavelengths_nm: Vec<f64>,
    pub modulation_rate_gbaud: f64,
    pub coherence_mode: CoherenceMode,
    pub adc_bits: u8,
    pub dac_bits: u8,
    pub effective_bits: u8,
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
    pub host_bandwidth_gbps: f64,
    pub boundary_latency_ns: f64,
    pub laser_power_mw: f64,
    pub dac_energy_pj_per_sample: f64,
    pub adc_energy_pj_per_sample: f64,
}

impl DeviceCapabilities {
    pub fn pace_like_128() -> Self {
        Self {
            capability_version: CAPABILITY_VERSION.to_string(),
            backend_id: "reference-pace-like-128".to_string(),
            matrix_core: MatrixCore {
                m: 128,
                n: 128,
                k: 128,
            },
            supported_dtypes: vec![DType::F16, DType::Bf16, DType::Int8, DType::Int4],
            supported_wavelengths_nm: (0..16).map(|index| 1530.0 + index as f64 * 1.6).collect(),
            modulation_rate_gbaud: 20.0,
            coherence_mode: CoherenceMode::Both,
            adc_bits: 10,
            dac_bits: 10,
            effective_bits: 8,
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
            calibration_profile: Some(CalibrationProfile {
                id: "reference-room-temperature-v1".to_string(),
                measured_at: "2026-08-11T00:00:00Z".to_string(),
                temperature_c: 22.0,
                gain: 0.997,
                offset: 0.0002,
                phase_error_radians: 0.001,
            }),
            host_bandwidth_gbps: 256.0,
            boundary_latency_ns: 500.0,
            laser_power_mw: 500.0,
            dac_energy_pj_per_sample: 2.0,
            adc_energy_pj_per_sample: 4.0,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.capability_version != CAPABILITY_VERSION {
            bail!(
                "unsupported capability version '{}'; expected '{}'",
                self.capability_version,
                CAPABILITY_VERSION
            );
        }
        if self.backend_id.trim().is_empty() {
            bail!("backend_id must not be empty");
        }
        if self.matrix_core.m == 0 || self.matrix_core.n == 0 || self.matrix_core.k == 0 {
            bail!("matrix core dimensions must be non-zero");
        }
        if self.supported_dtypes.is_empty() {
            bail!("device must advertise at least one supported dtype");
        }
        if self.supported_wavelengths_nm.is_empty() {
            bail!("photonic device must advertise at least one wavelength");
        }
        if self.modulation_rate_gbaud <= 0.0
            || self.sample_rate_gsps <= 0.0
            || self.host_bandwidth_gbps <= 0.0
        {
            bail!("device rates and host bandwidth must be positive");
        }
        if self.adc_bits == 0 || self.dac_bits == 0 || self.effective_bits == 0 {
            bail!("ADC, DAC, and effective precision must be non-zero");
        }
        if self.simultaneous_channels == 0 {
            bail!("simultaneous_channels must be non-zero");
        }
        if self.calibration_requirements.required && self.calibration_profile.is_none() {
            bail!("backend requires calibration but supplied no calibration profile");
        }
        if let Some(profile) = &self.calibration_profile {
            if profile.id.trim().is_empty() || profile.gain == 0.0 {
                bail!("calibration profile must have a non-empty id and non-zero gain");
            }
        }
        Ok(())
    }

    pub fn supports(&self, dtype: DType) -> bool {
        self.supported_dtypes.contains(&dtype) && (!dtype.is_complex() || self.supports_complex)
    }
}

impl Default for DeviceCapabilities {
    fn default() -> Self {
        Self::pace_like_128()
    }
}
