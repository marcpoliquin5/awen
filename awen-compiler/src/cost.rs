use crate::capability::{AccumulationMode, BackendHealth, DeviceCapabilities, SaturationMode};
use crate::ir::{DType, GemmShape, Layout};
use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

pub const COST_MODEL_VERSION: &str = "awen.cost-model.v1";
pub const OBSERVATION_SET_VERSION: &str = "awen.cost-observations.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OptimizationObjective {
    Latency,
    Energy,
    Accuracy,
    Throughput,
}

impl OptimizationObjective {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "latency" => Some(Self::Latency),
            "energy" => Some(Self::Energy),
            "accuracy" => Some(Self::Accuracy),
            "throughput" => Some(Self::Throughput),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TargetBackend {
    Auto,
    Cpu,
    Photonic,
}

impl TargetBackend {
    pub fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "auto" => Some(Self::Auto),
            "cpu" => Some(Self::Cpu),
            "photonic" => Some(Self::Photonic),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ParameterSource {
    Measured,
    VendorSpecified,
    Simulated,
    Assumed,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ParameterProvenance {
    pub parameter: String,
    pub source: ParameterSource,
    pub reference: String,
    pub uncertainty_fraction: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct EstimateInterval {
    pub lower: f64,
    pub expected: f64,
    pub upper: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct LatencyBreakdownNs {
    pub scheduling: f64,
    pub host_transfer: f64,
    pub memory: f64,
    pub boundary_conversion: f64,
    pub reconfiguration: f64,
    pub calibration_amortization: f64,
    pub dac: f64,
    pub modulation: f64,
    pub optical_propagation: f64,
    pub detection: f64,
    pub adc: f64,
    pub digital_accumulation: f64,
}

impl LatencyBreakdownNs {
    pub fn total(&self) -> f64 {
        self.scheduling
            + self.host_transfer
            + self.memory
            + self.boundary_conversion
            + self.reconfiguration
            + self.calibration_amortization
            + self.dac
            + self.modulation
            + self.optical_propagation
            + self.detection
            + self.adc
            + self.digital_accumulation
    }

    fn scale(&mut self, factor: f64) {
        self.scheduling *= factor;
        self.host_transfer *= factor;
        self.memory *= factor;
        self.boundary_conversion *= factor;
        self.reconfiguration *= factor;
        self.calibration_amortization *= factor;
        self.dac *= factor;
        self.modulation *= factor;
        self.optical_propagation *= factor;
        self.detection *= factor;
        self.adc *= factor;
        self.digital_accumulation *= factor;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Default)]
pub struct EnergyBreakdownUj {
    pub host_transfer: f64,
    pub memory: f64,
    pub dac: f64,
    pub modulation: f64,
    pub laser: f64,
    pub detector: f64,
    pub adc: f64,
    pub digital_accumulation: f64,
    pub support_system: f64,
    pub calibration_amortization: f64,
}

impl EnergyBreakdownUj {
    pub fn total(&self) -> f64 {
        self.host_transfer
            + self.memory
            + self.dac
            + self.modulation
            + self.laser
            + self.detector
            + self.adc
            + self.digital_accumulation
            + self.support_system
            + self.calibration_amortization
    }

    fn scale(&mut self, factor: f64) {
        self.host_transfer *= factor;
        self.memory *= factor;
        self.dac *= factor;
        self.modulation *= factor;
        self.laser *= factor;
        self.detector *= factor;
        self.adc *= factor;
        self.digital_accumulation *= factor;
        self.support_system *= factor;
        self.calibration_amortization *= factor;
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct CostEstimate {
    pub latency_ns: f64,
    pub latency_interval_ns: EstimateInterval,
    pub energy_uj: f64,
    pub energy_interval_uj: EstimateInterval,
    pub throughput_gops: f64,
    pub effective_bits: u8,
    pub estimated_error_fraction: f64,
    pub error_interval: EstimateInterval,
    pub latency_breakdown_ns: LatencyBreakdownNs,
    pub energy_breakdown_uj: EnergyBreakdownUj,
    pub provenance: Vec<ParameterProvenance>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct TuningPlan {
    pub tile_m: usize,
    pub tile_n: usize,
    pub tile_k: usize,
    pub bit_slices: u8,
    pub wavelength_channels: usize,
    pub accumulation_mode: AccumulationMode,
    pub batch_size: usize,
    pub fuse_boundaries: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TuningCandidate {
    pub plan: TuningPlan,
    pub estimate: CostEstimate,
    pub objective_score: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AutotuneResult {
    pub cost_model_version: String,
    pub seed: u64,
    pub fingerprint: String,
    pub objective: OptimizationObjective,
    pub selected: TuningCandidate,
    pub alternatives: Vec<TuningCandidate>,
    pub explanation: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PlacementDecision {
    pub op_id: String,
    pub selected_backend: TargetBackend,
    pub objective: OptimizationObjective,
    pub cpu: CostEstimate,
    pub photonic: Option<CostEstimate>,
    pub selected_plan: Option<TuningPlan>,
    pub alternatives: Vec<TuningCandidate>,
    pub decision_fingerprint: String,
    pub optical_electrical_boundary_crossings: u32,
    pub tile_count: usize,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy)]
pub struct DigitalBaseline {
    pub throughput_tops: f64,
    pub energy_pj_per_mac: f64,
    pub launch_latency_ns: f64,
    pub effective_bits: u8,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationCostProfile {
    pub lhs_layout: Layout,
    pub rhs_layout: Layout,
    pub output_layout: Layout,
    pub sparsity_fraction: f64,
    pub structured_sparsity: bool,
    pub input_error_fraction: f64,
    pub maximum_input_magnitude: Option<f64>,
    pub maximum_absolute_error: Option<f64>,
    pub maximum_relative_error: Option<f64>,
}

impl Default for OperationCostProfile {
    fn default() -> Self {
        Self {
            lhs_layout: Layout::RowMajor,
            rhs_layout: Layout::RowMajor,
            output_layout: Layout::RowMajor,
            sparsity_fraction: 0.0,
            structured_sparsity: false,
            input_error_fraction: 0.0,
            maximum_input_magnitude: None,
            maximum_absolute_error: None,
            maximum_relative_error: None,
        }
    }
}

impl OperationCostProfile {
    pub fn validate(&self) -> Result<()> {
        fraction(self.sparsity_fraction, "sparsity_fraction")?;
        fraction(self.input_error_fraction, "input_error_fraction")?;
        if let Some(value) = self.maximum_input_magnitude {
            non_negative(value, "maximum_input_magnitude")?;
        }
        for (value, name) in [
            (self.maximum_absolute_error, "maximum_absolute_error"),
            (self.maximum_relative_error, "maximum_relative_error"),
        ] {
            if let Some(value) = value {
                non_negative(value, name)?;
            }
        }
        Ok(())
    }

    fn layout_conversion_count(self) -> usize {
        [self.lhs_layout, self.rhs_layout, self.output_layout]
            .into_iter()
            .filter(|layout| *layout != Layout::RowMajor)
            .count()
    }

    fn error_contract_satisfied(self, estimate: &CostEstimate) -> bool {
        self.maximum_absolute_error
            .is_none_or(|bound| estimate.estimated_error_fraction <= bound)
            && self
                .maximum_relative_error
                .is_none_or(|bound| estimate.estimated_error_fraction <= bound)
    }
}

impl Default for DigitalBaseline {
    fn default() -> Self {
        Self {
            throughput_tops: 25.0,
            energy_pj_per_mac: 20.0,
            launch_latency_ns: 2_500.0,
            effective_bits: 16,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CostModelInputs {
    pub model_version: String,
    pub scheduling_latency_ns: f64,
    pub memory_bandwidth_gbps: f64,
    pub memory_energy_pj_per_byte: f64,
    pub host_transfer_energy_pj_per_byte: f64,
    pub modulator_energy_pj_per_sample: f64,
    pub detector_energy_pj_per_sample: f64,
    pub digital_accumulation_energy_pj_per_mac: f64,
    pub digital_accumulation_throughput_gops: f64,
    pub optical_propagation_ns: f64,
    pub calibration_amortization_ns: f64,
    pub calibration_amortization_energy_uj: f64,
    pub support_power_mw: f64,
    pub signal_to_noise_ratio_db: f64,
    pub insertion_loss_db: f64,
    pub drift_fraction: f64,
    pub disabled_component_fraction: f64,
    pub latency_calibration_factor: f64,
    pub energy_calibration_factor: f64,
    pub error_calibration_offset: f64,
    pub provenance: Vec<ParameterProvenance>,
}

impl CostModelInputs {
    pub fn from_capabilities(capabilities: &DeviceCapabilities, source: ParameterSource) -> Self {
        let reference = format!("{} capability snapshot", capabilities.backend_id);
        let profile_uncertainty = capabilities
            .calibration_profile
            .as_ref()
            .map_or(0.05, |profile| profile.uncertainty.max(0.001));
        let parameters = [
            "host/link/memory transfer",
            "DAC/ADC conversion",
            "modulation/detection",
            "optical propagation",
            "laser/support power",
            "digital accumulation",
            "reconfiguration/calibration",
            "SNR/insertion loss",
            "queueing/overlap/residency",
            "tensor layout/sparsity/error",
        ];
        Self {
            model_version: COST_MODEL_VERSION.to_string(),
            scheduling_latency_ns: 250.0,
            memory_bandwidth_gbps: capabilities
                .host_bandwidth_gbps
                .min(capabilities.link_bandwidth_gbps)
                * 0.75,
            memory_energy_pj_per_byte: 0.5,
            host_transfer_energy_pj_per_byte: 1.0,
            modulator_energy_pj_per_sample: 1.0,
            detector_energy_pj_per_sample: 1.0,
            digital_accumulation_energy_pj_per_mac: 0.25,
            digital_accumulation_throughput_gops: 500.0,
            optical_propagation_ns: 0.1,
            calibration_amortization_ns: capabilities.reconfiguration_latency_ns * 0.01,
            calibration_amortization_energy_uj: 0.001,
            support_power_mw: (capabilities.total_power_budget_mw - capabilities.laser_power_mw)
                .max(0.0),
            signal_to_noise_ratio_db: 6.02 * f64::from(capabilities.effective_bits) + 1.76,
            insertion_loss_db: 0.0,
            drift_fraction: 0.0,
            disabled_component_fraction: 0.0,
            latency_calibration_factor: 1.0,
            energy_calibration_factor: 1.0,
            error_calibration_offset: 0.0,
            provenance: parameters
                .into_iter()
                .map(|parameter| ParameterProvenance {
                    parameter: parameter.to_string(),
                    source,
                    reference: reference.clone(),
                    uncertainty_fraction: profile_uncertainty,
                })
                .collect(),
        }
    }

    pub fn from_snapshot(
        capabilities: &DeviceCapabilities,
        health: &BackendHealth,
        source: ParameterSource,
    ) -> Self {
        let mut model = Self::from_capabilities(capabilities, source);
        model.drift_fraction = health.drift;
        model.disabled_component_fraction = if capabilities.simultaneous_channels == 0 {
            1.0
        } else {
            1.0 - health
                .available_channels
                .min(capabilities.simultaneous_channels) as f64
                / capabilities.simultaneous_channels as f64
        };
        model.provenance.push(ParameterProvenance {
            parameter: "drift and disabled components".to_string(),
            source,
            reference: format!(
                "{} health snapshot at {}",
                health.backend_id, health.observed_at
            ),
            uncertainty_fraction: capabilities
                .calibration_profile
                .as_ref()
                .map_or(0.05, |profile| profile.uncertainty.max(0.001)),
        });
        model
    }

    pub fn calibrated_from_reports(mut self, reports: &[ModelErrorReport]) -> Result<Self> {
        self.validate()?;
        if reports.is_empty() {
            bail!("cost-model calibration requires at least one benchmark report");
        }
        let latency_factor = reports
            .iter()
            .map(|report| report.observed.latency_ns / report.predicted.latency_ns)
            .sum::<f64>()
            / reports.len() as f64;
        let energy_factor = reports
            .iter()
            .map(|report| report.observed.energy_uj / report.predicted.energy_uj.max(f64::EPSILON))
            .sum::<f64>()
            / reports.len() as f64;
        let error_offset = reports
            .iter()
            .map(|report| {
                report.observed.error_fraction - report.predicted.estimated_error_fraction
            })
            .sum::<f64>()
            / reports.len() as f64;
        positive(latency_factor, "calibrated latency factor")?;
        positive(energy_factor, "calibrated energy factor")?;
        self.latency_calibration_factor *= latency_factor;
        self.energy_calibration_factor *= energy_factor;
        self.error_calibration_offset = (self.error_calibration_offset + error_offset).max(0.0);
        let artifacts = reports
            .iter()
            .map(|report| report.observed.artifact_id.as_str())
            .collect::<Vec<_>>()
            .join(",");
        self.provenance.push(ParameterProvenance {
            parameter: "latency, energy, and numerical calibration factors".to_string(),
            source: ParameterSource::Measured,
            reference: artifacts,
            uncertainty_fraction: reports
                .iter()
                .map(|report| {
                    report
                        .latency_error_fraction
                        .max(report.energy_error_fraction)
                })
                .fold(0.0, f64::max)
                .min(1.0),
        });
        self.validate()?;
        Ok(self)
    }

    pub fn validate(&self) -> Result<()> {
        if self.model_version != COST_MODEL_VERSION {
            bail!(
                "unsupported cost model version '{}'; expected '{}'",
                self.model_version,
                COST_MODEL_VERSION
            );
        }
        positive(self.memory_bandwidth_gbps, "memory_bandwidth_gbps")?;
        positive(
            self.digital_accumulation_throughput_gops,
            "digital_accumulation_throughput_gops",
        )?;
        positive(self.signal_to_noise_ratio_db, "signal_to_noise_ratio_db")?;
        positive(
            self.latency_calibration_factor,
            "latency_calibration_factor",
        )?;
        positive(self.energy_calibration_factor, "energy_calibration_factor")?;
        for (value, name) in [
            (self.scheduling_latency_ns, "scheduling_latency_ns"),
            (self.memory_energy_pj_per_byte, "memory_energy_pj_per_byte"),
            (
                self.host_transfer_energy_pj_per_byte,
                "host_transfer_energy_pj_per_byte",
            ),
            (
                self.modulator_energy_pj_per_sample,
                "modulator_energy_pj_per_sample",
            ),
            (
                self.detector_energy_pj_per_sample,
                "detector_energy_pj_per_sample",
            ),
            (
                self.digital_accumulation_energy_pj_per_mac,
                "digital_accumulation_energy_pj_per_mac",
            ),
            (self.optical_propagation_ns, "optical_propagation_ns"),
            (
                self.calibration_amortization_ns,
                "calibration_amortization_ns",
            ),
            (
                self.calibration_amortization_energy_uj,
                "calibration_amortization_energy_uj",
            ),
            (self.support_power_mw, "support_power_mw"),
            (self.insertion_loss_db, "insertion_loss_db"),
            (self.error_calibration_offset, "error_calibration_offset"),
        ] {
            non_negative(value, name)?;
        }
        fraction(self.drift_fraction, "drift_fraction")?;
        fraction(
            self.disabled_component_fraction,
            "disabled_component_fraction",
        )?;
        if self.provenance.is_empty() {
            bail!("cost inputs require provenance");
        }
        for entry in &self.provenance {
            if entry.parameter.trim().is_empty() || entry.reference.trim().is_empty() {
                bail!("cost provenance requires parameter and reference identities");
            }
            if !(0.0..=1.0).contains(&entry.uncertainty_fraction)
                || !entry.uncertainty_fraction.is_finite()
            {
                bail!("cost provenance uncertainty must be finite and within [0, 1]");
            }
        }
        Ok(())
    }

    fn uncertainty(&self) -> f64 {
        self.provenance
            .iter()
            .map(|entry| entry.uncertainty_fraction)
            .fold(0.0, f64::max)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct AutotuneOptions {
    pub graph_fingerprint: u64,
    pub seed: u64,
    pub batch_size: usize,
    pub allow_boundary_fusion: bool,
    pub alternatives: usize,
    pub queue_depth: usize,
    pub overlap_fraction: f64,
    pub resident_input_fraction: f64,
}

impl Default for AutotuneOptions {
    fn default() -> Self {
        Self {
            graph_fingerprint: 0,
            seed: 0,
            batch_size: 1,
            allow_boundary_fusion: false,
            alternatives: 3,
            queue_depth: 0,
            overlap_fraction: 0.0,
            resident_input_fraction: 0.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Observation {
    pub op_id: String,
    pub latency_ns: f64,
    pub energy_uj: f64,
    pub error_fraction: f64,
    pub source: ParameterSource,
    pub artifact_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ObservationSet {
    pub observation_version: String,
    pub observations: Vec<Observation>,
}

impl ObservationSet {
    pub fn validate(&self) -> Result<()> {
        if self.observation_version != OBSERVATION_SET_VERSION {
            bail!(
                "unsupported cost observation version '{}'; expected '{}'",
                self.observation_version,
                OBSERVATION_SET_VERSION
            );
        }
        if self.observations.is_empty() {
            bail!("cost observation sets must contain at least one observation");
        }
        for observation in &self.observations {
            validate_observation(observation)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ModelErrorReport {
    pub fingerprint: String,
    pub predicted: CostEstimate,
    pub observed: Observation,
    pub latency_error_fraction: f64,
    pub energy_error_fraction: f64,
    pub numerical_error_delta: f64,
}

impl ModelErrorReport {
    pub fn compare(
        fingerprint: impl Into<String>,
        predicted: CostEstimate,
        observed: Observation,
    ) -> Result<Self> {
        validate_observation(&observed)?;
        Ok(Self {
            fingerprint: fingerprint.into(),
            latency_error_fraction: relative_error(predicted.latency_ns, observed.latency_ns),
            energy_error_fraction: relative_error(predicted.energy_uj, observed.energy_uj),
            numerical_error_delta: observed.error_fraction - predicted.estimated_error_fraction,
            predicted,
            observed,
        })
    }
}

#[derive(Debug, Default)]
pub struct DecisionCache {
    entries: BTreeMap<String, AutotuneResult>,
}

impl DecisionCache {
    pub fn get(&self, fingerprint: &str) -> Option<&AutotuneResult> {
        self.entries.get(fingerprint)
    }

    pub fn insert(&mut self, result: AutotuneResult) {
        self.entries.insert(result.fingerprint.clone(), result);
    }

    pub fn retain_only(&mut self, fingerprint: &str) {
        self.entries.retain(|key, _| key == fingerprint);
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[allow(clippy::too_many_arguments)]
pub fn decide_placement(
    op_id: &str,
    shape: GemmShape,
    dtype: DType,
    minimum_effective_bits: Option<u8>,
    capabilities: &DeviceCapabilities,
    objective: OptimizationObjective,
    requested_target: TargetBackend,
    digital: DigitalBaseline,
) -> PlacementDecision {
    decide_placement_with_tuning(
        op_id,
        shape,
        dtype,
        minimum_effective_bits,
        capabilities,
        objective,
        requested_target,
        digital,
        AutotuneOptions::default(),
    )
    .expect("validated compiler capability must produce a cost model")
}

#[allow(clippy::too_many_arguments)]
pub fn decide_placement_with_tuning(
    op_id: &str,
    shape: GemmShape,
    dtype: DType,
    minimum_effective_bits: Option<u8>,
    capabilities: &DeviceCapabilities,
    objective: OptimizationObjective,
    requested_target: TargetBackend,
    digital: DigitalBaseline,
    tuning: AutotuneOptions,
) -> Result<PlacementDecision> {
    let source = if capabilities.backend_id.starts_with("reference-") {
        ParameterSource::Simulated
    } else {
        ParameterSource::Assumed
    };
    let model = CostModelInputs::from_capabilities(capabilities, source);
    decide_placement_with_model(
        op_id,
        shape,
        dtype,
        minimum_effective_bits,
        capabilities,
        &model,
        OperationCostProfile::default(),
        objective,
        requested_target,
        digital,
        tuning,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn decide_placement_with_model(
    op_id: &str,
    shape: GemmShape,
    dtype: DType,
    minimum_effective_bits: Option<u8>,
    capabilities: &DeviceCapabilities,
    model: &CostModelInputs,
    profile: OperationCostProfile,
    objective: OptimizationObjective,
    requested_target: TargetBackend,
    digital: DigitalBaseline,
    tuning: AutotuneOptions,
) -> Result<PlacementDecision> {
    let conservative_cpu = estimate_cpu(shape, digital);
    if let Err(error) = model.validate().and_then(|_| profile.validate()) {
        if requested_target == TargetBackend::Photonic {
            bail!("forced photonic placement requires complete cost inputs: {error}");
        }
        return Ok(PlacementDecision {
            op_id: op_id.to_string(),
            selected_backend: TargetBackend::Cpu,
            objective,
            cpu: conservative_cpu,
            photonic: None,
            selected_plan: None,
            alternatives: Vec::new(),
            decision_fingerprint: incomplete_fingerprint(shape, dtype, capabilities, model),
            optical_electrical_boundary_crossings: 0,
            tile_count: 0,
            rationale: format!(
                "conservative CPU fallback because required cost-model inputs are missing or invalid: {error}"
            ),
        });
    }
    let cpu = estimate_cpu_with_context(shape, dtype, digital, model, profile, tuning);
    let autotuned = capabilities.supports(dtype).then(|| {
        autotune_with_profile(
            shape,
            dtype,
            minimum_effective_bits,
            capabilities,
            model,
            profile,
            objective,
            tuning,
        )
    });
    let (autotuned, autotune_diagnostic) = match autotuned {
        Some(Ok(result)) => (Some(result), None),
        Some(Err(error)) => (None, Some(error.to_string())),
        None => (None, None),
    };
    let photonic = autotuned
        .as_ref()
        .map(|result| result.selected.estimate.clone());
    let precision_ok = photonic.as_ref().is_some_and(|estimate| {
        minimum_effective_bits.is_none_or(|bits| estimate.effective_bits >= bits)
            && profile.error_contract_satisfied(estimate)
    });

    let (selected_backend, rationale) = match requested_target {
        TargetBackend::Cpu => (
            TargetBackend::Cpu,
            "CPU placement was explicitly requested".to_string(),
        ),
        TargetBackend::Photonic if photonic.is_none() => (
            TargetBackend::Cpu,
            autotune_diagnostic.clone().map_or_else(
                || format!("photonic placement was requested but dtype {dtype:?} is unsupported"),
                |diagnostic| {
                    format!(
                        "photonic placement was requested but no legal plan exists: {diagnostic}"
                    )
                },
            ),
        ),
        TargetBackend::Photonic if !precision_ok => (
            TargetBackend::Cpu,
            "photonic placement was requested but no tuning plan satisfies the accuracy contract"
                .to_string(),
        ),
        TargetBackend::Photonic => (
            TargetBackend::Photonic,
            autotuned
                .as_ref()
                .map(|result| result.explanation.clone())
                .unwrap_or_default(),
        ),
        TargetBackend::Auto if photonic.is_none() => (
            TargetBackend::Cpu,
            autotune_diagnostic.map_or_else(
                || format!("dtype {dtype:?} is not supported by the photonic backend"),
                |diagnostic| format!("no legal photonic tuning plan exists: {diagnostic}"),
            ),
        ),
        TargetBackend::Auto if !precision_ok => (
            TargetBackend::Cpu,
            "no photonic tuning plan satisfies the operation accuracy contract".to_string(),
        ),
        TargetBackend::Auto => {
            let optical = photonic.as_ref().expect("checked above");
            if wins(objective, optical, &cpu) {
                (
                    TargetBackend::Photonic,
                    format!(
                        "photonic plan wins {objective:?} after accounting for host transfer, memory, optical/electrical conversion, reconfiguration, calibration, laser, support-system, and digital accumulation costs: {}",
                        autotuned
                            .as_ref()
                            .map(|result| result.explanation.as_str())
                            .unwrap_or("autotuned")
                    ),
                )
            } else {
                (
                    TargetBackend::Cpu,
                    format!(
                        "CPU wins {objective:?} after host transfer, memory, conversion, reconfiguration, calibration, laser, support-system, and digital accumulation costs"
                    ),
                )
            }
        }
    };
    let selected_plan = autotuned.as_ref().map(|result| result.selected.plan);
    let alternatives = autotuned
        .as_ref()
        .map(|result| result.alternatives.clone())
        .unwrap_or_default();
    let decision_fingerprint = autotuned
        .as_ref()
        .map(|result| result.fingerprint.clone())
        .unwrap_or_else(|| {
            fingerprint(
                shape,
                dtype,
                capabilities,
                model,
                profile,
                objective,
                tuning,
            )
        });
    let tile_count = selected_plan
        .map(|plan| tile_count_for_plan(shape, plan))
        .unwrap_or_else(|| tile_count(shape, capabilities));
    let crossings = if selected_backend == TargetBackend::Photonic {
        if tuning.allow_boundary_fusion {
            2
        } else {
            2 * tuning.batch_size.max(1) as u32
        }
    } else {
        0
    };
    Ok(PlacementDecision {
        op_id: op_id.to_string(),
        selected_backend,
        objective,
        cpu,
        photonic,
        selected_plan,
        alternatives,
        decision_fingerprint,
        optical_electrical_boundary_crossings: crossings,
        tile_count,
        rationale,
    })
}

pub fn autotune(
    shape: GemmShape,
    dtype: DType,
    minimum_effective_bits: Option<u8>,
    capabilities: &DeviceCapabilities,
    model: &CostModelInputs,
    objective: OptimizationObjective,
    options: AutotuneOptions,
) -> Result<AutotuneResult> {
    autotune_with_profile(
        shape,
        dtype,
        minimum_effective_bits,
        capabilities,
        model,
        OperationCostProfile::default(),
        objective,
        options,
    )
}

#[allow(clippy::too_many_arguments)]
pub fn autotune_with_profile(
    shape: GemmShape,
    dtype: DType,
    minimum_effective_bits: Option<u8>,
    capabilities: &DeviceCapabilities,
    model: &CostModelInputs,
    profile: OperationCostProfile,
    objective: OptimizationObjective,
    options: AutotuneOptions,
) -> Result<AutotuneResult> {
    model.validate()?;
    profile.validate()?;
    if options.batch_size == 0 {
        bail!("autotune batch_size must be non-zero");
    }
    fraction(options.overlap_fraction, "autotune overlap_fraction")?;
    fraction(
        options.resident_input_fraction,
        "autotune resident_input_fraction",
    )?;
    let required_bits = minimum_effective_bits.unwrap_or(capabilities.effective_bits);
    let minimum_slices = required_bits.div_ceil(capabilities.effective_bits).max(1);
    let supports_bit_slicing = capabilities
        .bit_slicing_modes
        .iter()
        .any(|mode| *mode != crate::capability::BitSlicingMode::None);
    if minimum_slices > 1 && !supports_bit_slicing {
        bail!("accuracy contract requires bit slicing but the backend advertises none");
    }
    let maximum_slices = if supports_bit_slicing {
        dtype
            .bits()
            .div_ceil(capabilities.effective_bits)
            .max(minimum_slices)
    } else {
        1
    };
    let slice_options = unique_u8s(&[minimum_slices, maximum_slices]);
    let tile_options = unique_tiles(capabilities);
    let channel_options = unique_usizes(&[
        1,
        capabilities.simultaneous_channels,
        capabilities.simultaneous_channels.min(shape.k),
    ]);
    let fusion_options: &[bool] = if options.allow_boundary_fusion {
        &[false, true]
    } else {
        &[false]
    };
    let mut candidates = Vec::new();
    for (tile_m, tile_n, tile_k) in tile_options {
        for channels in &channel_options {
            for accumulation_mode in &capabilities.accumulation_modes {
                for bit_slices in &slice_options {
                    for fuse_boundaries in fusion_options {
                        let plan = TuningPlan {
                            tile_m,
                            tile_n,
                            tile_k,
                            bit_slices: *bit_slices,
                            wavelength_channels: *channels,
                            accumulation_mode: *accumulation_mode,
                            batch_size: options.batch_size,
                            fuse_boundaries: *fuse_boundaries,
                        };
                        let estimate = estimate_photonic_plan_with_profile(
                            shape,
                            dtype,
                            capabilities,
                            model,
                            profile,
                            plan,
                            options,
                        )?;
                        if profile.error_contract_satisfied(&estimate) {
                            candidates.push(TuningCandidate {
                                objective_score: objective_score(objective, &estimate),
                                plan,
                                estimate,
                            });
                        }
                    }
                }
            }
        }
    }
    candidates.sort_by(|left, right| {
        left.objective_score
            .total_cmp(&right.objective_score)
            .then_with(|| {
                seeded_plan_key(options.seed, left.plan)
                    .cmp(&seeded_plan_key(options.seed, right.plan))
            })
            .then_with(|| plan_key(left.plan).cmp(&plan_key(right.plan)))
    });
    let selected = candidates
        .first()
        .cloned()
        .ok_or_else(|| anyhow::anyhow!("autotuner generated no legal plans"))?;
    let alternatives = candidates
        .iter()
        .skip(1)
        .take(options.alternatives)
        .cloned()
        .collect::<Vec<_>>();
    let fingerprint = fingerprint(
        shape,
        dtype,
        capabilities,
        model,
        profile,
        objective,
        options,
    );
    let explanation = explain_selection(objective, &selected, &alternatives);
    Ok(AutotuneResult {
        cost_model_version: COST_MODEL_VERSION.to_string(),
        seed: options.seed,
        fingerprint,
        objective,
        selected,
        alternatives,
        explanation,
    })
}

pub fn estimate_photonic_plan(
    shape: GemmShape,
    dtype: DType,
    capabilities: &DeviceCapabilities,
    model: &CostModelInputs,
    plan: TuningPlan,
) -> Result<CostEstimate> {
    estimate_photonic_plan_with_profile(
        shape,
        dtype,
        capabilities,
        model,
        OperationCostProfile::default(),
        plan,
        AutotuneOptions {
            batch_size: plan.batch_size,
            allow_boundary_fusion: plan.fuse_boundaries,
            ..AutotuneOptions::default()
        },
    )
}

#[allow(clippy::too_many_arguments)]
pub fn estimate_photonic_plan_with_profile(
    shape: GemmShape,
    dtype: DType,
    capabilities: &DeviceCapabilities,
    model: &CostModelInputs,
    profile: OperationCostProfile,
    plan: TuningPlan,
    options: AutotuneOptions,
) -> Result<CostEstimate> {
    model.validate()?;
    profile.validate()?;
    fraction(options.overlap_fraction, "overlap_fraction")?;
    fraction(options.resident_input_fraction, "resident_input_fraction")?;
    if plan.tile_m == 0
        || plan.tile_n == 0
        || plan.tile_k == 0
        || plan.bit_slices == 0
        || plan.wavelength_channels == 0
        || plan.batch_size == 0
    {
        bail!("tuning plan dimensions, slices, channels, and batch must be non-zero");
    }
    if plan.tile_m > capabilities.matrix_core.m
        || plan.tile_n > capabilities.matrix_core.n
        || plan.tile_k > capabilities.matrix_core.k
        || plan.wavelength_channels > capabilities.simultaneous_channels
    {
        bail!("tuning plan exceeds backend resources");
    }
    if !capabilities
        .accumulation_modes
        .contains(&plan.accumulation_mode)
    {
        bail!("tuning plan requests an unsupported accumulation mode");
    }
    if plan.bit_slices > 1
        && !capabilities
            .bit_slicing_modes
            .iter()
            .any(|mode| *mode != crate::capability::BitSlicingMode::None)
    {
        bail!("tuning plan requests bit slicing but the backend advertises none");
    }
    let batches = plan.batch_size as f64;
    let density = if profile.structured_sparsity {
        1.0 - profile.sparsity_fraction
    } else {
        1.0
    };
    let tiles = tile_count_for_plan(shape, plan) as f64 * batches;
    let input_elements = (shape.m * shape.k + shape.k * shape.n) as f64 * batches;
    let output_elements = (shape.m * shape.n) as f64 * batches;
    let bytes = (input_elements * (1.0 - options.resident_input_fraction) + output_elements)
        * dtype.bits() as f64
        / 8.0
        * density;
    let layout_conversion_bytes = bytes * profile.layout_conversion_count() as f64;
    let macs = shape.m as f64 * shape.n as f64 * shape.k as f64 * batches * density;
    let slices = f64::from(plan.bit_slices);
    let channel_parallelism = plan.wavelength_channels.min(plan.tile_k) as f64;
    let conversion_samples = conversion_samples_for_plan(shape, plan) as f64 * batches * slices;
    let crossings = if plan.fuse_boundaries {
        2.0
    } else {
        2.0 * batches
    };
    let accumulation_macs = if shape.k > plan.tile_k {
        shape.m as f64 * shape.n as f64 * (shape.k.div_ceil(plan.tile_k) - 1) as f64 * batches
    } else {
        0.0
    };
    let digital_accumulation = matches!(
        plan.accumulation_mode,
        AccumulationMode::Digital | AccumulationMode::Hybrid
    );
    let supported_magnitude = capabilities
        .input_dynamic_range
        .minimum
        .abs()
        .max(capabilities.input_dynamic_range.maximum.abs());
    let saturation_error = profile.maximum_input_magnitude.map_or(0.0, |magnitude| {
        if magnitude > supported_magnitude && magnitude > 0.0 {
            (magnitude - supported_magnitude) / magnitude
        } else {
            0.0
        }
    });
    if saturation_error > 0.0 && capabilities.saturation_mode == SaturationMode::Error {
        bail!("operation input exceeds the backend dynamic range");
    }
    let overlappable_fraction = 1.0 - options.overlap_fraction;
    let active_fraction = (1.0 - model.disabled_component_fraction).max(0.01);
    let mut latency_breakdown_ns = LatencyBreakdownNs {
        scheduling: model.scheduling_latency_ns * (options.queue_depth + 1) as f64,
        host_transfer: bytes * 8.0
            / capabilities
                .host_bandwidth_gbps
                .min(capabilities.link_bandwidth_gbps)
            * overlappable_fraction,
        memory: (bytes + layout_conversion_bytes) * 8.0 / model.memory_bandwidth_gbps
            * overlappable_fraction,
        boundary_conversion: crossings * capabilities.boundary_latency_ns * overlappable_fraction,
        reconfiguration: capabilities.reconfiguration_latency_ns,
        calibration_amortization: model.calibration_amortization_ns,
        dac: conversion_samples / capabilities.sample_rate_gsps,
        modulation: tiles * slices / capabilities.modulation_rate_gbaud,
        optical_propagation: tiles * slices * model.optical_propagation_ns
            / (channel_parallelism * active_fraction),
        detection: conversion_samples / capabilities.detector_bandwidth_ghz,
        adc: conversion_samples / capabilities.sample_rate_gsps,
        digital_accumulation: if digital_accumulation {
            accumulation_macs / model.digital_accumulation_throughput_gops
        } else {
            0.0
        },
    };
    latency_breakdown_ns.scale(model.latency_calibration_factor);
    let latency_ns = latency_breakdown_ns.total();
    let optical_loss_factor = 10.0_f64.powf(model.insertion_loss_db / 10.0);
    let mut energy_breakdown_uj = EnergyBreakdownUj {
        host_transfer: bytes * model.host_transfer_energy_pj_per_byte / 1_000_000.0,
        memory: (bytes + layout_conversion_bytes) * model.memory_energy_pj_per_byte / 1_000_000.0,
        dac: conversion_samples * capabilities.dac_energy_pj_per_sample / 1_000_000.0,
        modulation: conversion_samples * model.modulator_energy_pj_per_sample / 1_000_000.0,
        laser: capabilities.laser_power_mw * optical_loss_factor * latency_ns / 1_000_000.0,
        detector: conversion_samples * model.detector_energy_pj_per_sample / 1_000_000.0,
        adc: conversion_samples * capabilities.adc_energy_pj_per_sample / 1_000_000.0,
        digital_accumulation: if digital_accumulation {
            accumulation_macs * model.digital_accumulation_energy_pj_per_mac / 1_000_000.0
        } else {
            0.0
        },
        support_system: model.support_power_mw * latency_ns / 1_000_000.0,
        calibration_amortization: model.calibration_amortization_energy_uj,
    };
    energy_breakdown_uj.scale(model.energy_calibration_factor);
    let energy_uj = energy_breakdown_uj.total();
    let uncertainty = model.uncertainty();
    let calibration_uncertainty = capabilities
        .calibration_profile
        .as_ref()
        .map_or(0.0, |profile| profile.uncertainty);
    let effective_bits = capabilities.effective_bits.saturating_mul(plan.bit_slices);
    let quantization_error = 2.0_f64.powi(-i32::from(effective_bits));
    let effective_snr_db = (model.signal_to_noise_ratio_db - model.insertion_loss_db).max(0.0);
    let snr_error = 10.0_f64.powf(-effective_snr_db / 20.0);
    let k_accumulations = shape.k.div_ceil(plan.tile_k).saturating_sub(1) as f64;
    let accumulation_error = match plan.accumulation_mode {
        AccumulationMode::Optical => quantization_error * k_accumulations * 0.5,
        AccumulationMode::Hybrid => quantization_error * k_accumulations * 0.25,
        AccumulationMode::Digital => 0.0,
    };
    let estimated_error_fraction = (quantization_error.max(snr_error)
        + calibration_uncertainty
        + model.drift_fraction
        + model.error_calibration_offset
        + accumulation_error
        + saturation_error
        + profile.input_error_fraction)
        .min(1.0);
    let operations = 2.0 * macs;
    Ok(CostEstimate {
        latency_ns,
        latency_interval_ns: interval(latency_ns, uncertainty),
        energy_uj,
        energy_interval_uj: interval(energy_uj, uncertainty),
        throughput_gops: operations / latency_ns,
        effective_bits,
        estimated_error_fraction,
        error_interval: interval(estimated_error_fraction, uncertainty),
        latency_breakdown_ns,
        energy_breakdown_uj,
        provenance: model.provenance.clone(),
    })
}

pub fn tile_count(shape: GemmShape, capabilities: &DeviceCapabilities) -> usize {
    shape.m.div_ceil(capabilities.matrix_core.m)
        * shape.n.div_ceil(capabilities.matrix_core.n)
        * shape.k.div_ceil(capabilities.matrix_core.k)
}

fn tile_count_for_plan(shape: GemmShape, plan: TuningPlan) -> usize {
    shape.m.div_ceil(plan.tile_m) * shape.n.div_ceil(plan.tile_n) * shape.k.div_ceil(plan.tile_k)
}

fn conversion_samples_for_plan(shape: GemmShape, plan: TuningPlan) -> usize {
    let mut samples = 0;
    for m_offset in (0..shape.m).step_by(plan.tile_m) {
        for n_offset in (0..shape.n).step_by(plan.tile_n) {
            for k_offset in (0..shape.k).step_by(plan.tile_k) {
                samples += plan.tile_m.min(shape.m - m_offset)
                    + plan.tile_n.min(shape.n - n_offset)
                    + plan.tile_k.min(shape.k - k_offset);
            }
        }
    }
    samples
}

fn estimate_cpu(shape: GemmShape, baseline: DigitalBaseline) -> CostEstimate {
    let macs = shape.m as f64 * shape.n as f64 * shape.k as f64;
    let operations = 2.0 * macs;
    let latency_ns = baseline.launch_latency_ns + operations / (baseline.throughput_tops * 1_000.0);
    let energy_uj = macs * baseline.energy_pj_per_mac / 1_000_000.0;
    let provenance = vec![ParameterProvenance {
        parameter: "digital baseline".to_string(),
        source: ParameterSource::Assumed,
        reference: "compile options".to_string(),
        uncertainty_fraction: 0.1,
    }];
    CostEstimate {
        latency_ns,
        latency_interval_ns: interval(latency_ns, 0.1),
        energy_uj,
        energy_interval_uj: interval(energy_uj, 0.1),
        throughput_gops: operations / latency_ns,
        effective_bits: baseline.effective_bits,
        estimated_error_fraction: 2.0_f64.powi(-i32::from(baseline.effective_bits)),
        error_interval: interval(2.0_f64.powi(-i32::from(baseline.effective_bits)), 0.1),
        latency_breakdown_ns: LatencyBreakdownNs {
            scheduling: baseline.launch_latency_ns,
            digital_accumulation: latency_ns - baseline.launch_latency_ns,
            ..LatencyBreakdownNs::default()
        },
        energy_breakdown_uj: EnergyBreakdownUj {
            digital_accumulation: energy_uj,
            ..EnergyBreakdownUj::default()
        },
        provenance,
    }
}

fn estimate_cpu_with_context(
    shape: GemmShape,
    dtype: DType,
    baseline: DigitalBaseline,
    model: &CostModelInputs,
    profile: OperationCostProfile,
    options: AutotuneOptions,
) -> CostEstimate {
    let batches = options.batch_size.max(1) as f64;
    let density = if profile.structured_sparsity {
        1.0 - profile.sparsity_fraction
    } else {
        1.0
    };
    let macs = shape.m as f64 * shape.n as f64 * shape.k as f64 * batches * density;
    let operations = 2.0 * macs;
    let elements =
        (shape.m * shape.k + shape.k * shape.n + shape.m * shape.n) as f64 * batches * density;
    let bytes = elements * f64::from(dtype.bits()) / 8.0;
    let layout_bytes = bytes * profile.layout_conversion_count() as f64;
    let scheduling = baseline.launch_latency_ns * (options.queue_depth + 1) as f64;
    let memory_latency = (bytes + layout_bytes) * 8.0 / model.memory_bandwidth_gbps
        * (1.0 - options.overlap_fraction);
    let compute_latency = operations / (baseline.throughput_tops * 1_000.0);
    let latency_breakdown_ns = LatencyBreakdownNs {
        scheduling,
        memory: memory_latency,
        digital_accumulation: compute_latency,
        ..LatencyBreakdownNs::default()
    };
    let latency_ns = latency_breakdown_ns.total();
    let compute_energy = macs * baseline.energy_pj_per_mac / 1_000_000.0;
    let memory_energy = (bytes + layout_bytes) * model.memory_energy_pj_per_byte / 1_000_000.0;
    let energy_breakdown_uj = EnergyBreakdownUj {
        memory: memory_energy,
        digital_accumulation: compute_energy,
        ..EnergyBreakdownUj::default()
    };
    let energy_uj = energy_breakdown_uj.total();
    let uncertainty = model.uncertainty().max(0.1);
    let estimated_error_fraction =
        (2.0_f64.powi(-i32::from(baseline.effective_bits)) + profile.input_error_fraction).min(1.0);
    let mut provenance = model.provenance.clone();
    provenance.push(ParameterProvenance {
        parameter: "digital compute baseline".to_string(),
        source: ParameterSource::Assumed,
        reference: "compile options".to_string(),
        uncertainty_fraction: 0.1,
    });
    CostEstimate {
        latency_ns,
        latency_interval_ns: interval(latency_ns, uncertainty),
        energy_uj,
        energy_interval_uj: interval(energy_uj, uncertainty),
        throughput_gops: operations / latency_ns,
        effective_bits: baseline.effective_bits,
        estimated_error_fraction,
        error_interval: interval(estimated_error_fraction, uncertainty),
        latency_breakdown_ns,
        energy_breakdown_uj,
        provenance,
    }
}

fn wins(
    objective: OptimizationObjective,
    candidate: &CostEstimate,
    baseline: &CostEstimate,
) -> bool {
    match objective {
        OptimizationObjective::Latency => candidate.latency_ns < baseline.latency_ns,
        OptimizationObjective::Energy => candidate.energy_uj < baseline.energy_uj,
        OptimizationObjective::Accuracy => {
            candidate.estimated_error_fraction < baseline.estimated_error_fraction
        }
        OptimizationObjective::Throughput => candidate.throughput_gops > baseline.throughput_gops,
    }
}

fn objective_score(objective: OptimizationObjective, estimate: &CostEstimate) -> f64 {
    match objective {
        OptimizationObjective::Latency => estimate.latency_ns,
        OptimizationObjective::Energy => estimate.energy_uj,
        OptimizationObjective::Accuracy => estimate.estimated_error_fraction,
        OptimizationObjective::Throughput => -estimate.throughput_gops,
    }
}

fn unique_tiles(capabilities: &DeviceCapabilities) -> Vec<(usize, usize, usize)> {
    let core = capabilities.matrix_core;
    let mut values = vec![
        (core.m, core.n, core.k),
        (
            (core.m / 2).max(1),
            (core.n / 2).max(1),
            (core.k / 2).max(1),
        ),
    ];
    values.sort_unstable();
    values.dedup();
    values
}

fn unique_usizes(values: &[usize]) -> Vec<usize> {
    let mut result = values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    result.sort_unstable();
    result.dedup();
    result
}

fn unique_u8s(values: &[u8]) -> Vec<u8> {
    let mut result = values
        .iter()
        .copied()
        .filter(|value| *value > 0)
        .collect::<Vec<_>>();
    result.sort_unstable();
    result.dedup();
    result
}

fn plan_key(plan: TuningPlan) -> (usize, usize, usize, u8, usize, u8, usize, bool) {
    (
        plan.tile_m,
        plan.tile_n,
        plan.tile_k,
        plan.bit_slices,
        plan.wavelength_channels,
        accumulation_key(plan.accumulation_mode),
        plan.batch_size,
        plan.fuse_boundaries,
    )
}

fn seeded_plan_key(seed: u64, plan: TuningPlan) -> u64 {
    let input = format!("{seed}|{:?}", plan_key(plan));
    fnv1a64(input.as_bytes())
}

fn accumulation_key(mode: AccumulationMode) -> u8 {
    match mode {
        AccumulationMode::Optical => 0,
        AccumulationMode::Digital => 1,
        AccumulationMode::Hybrid => 2,
    }
}

fn explain_selection(
    objective: OptimizationObjective,
    selected: &TuningCandidate,
    alternatives: &[TuningCandidate],
) -> String {
    let comparison = alternatives.first().map_or_else(
        || "no alternative plan was legal".to_string(),
        |alternative| {
            format!(
                "next plan score {:.6} versus selected {:.6}",
                alternative.objective_score, selected.objective_score
            )
        },
    );
    format!(
        "selected {:?} for {objective:?}; latency={:.3} ns, energy={:.6} uJ, error={:.6}, throughput={:.3} GOPS; {comparison}",
        selected.plan,
        selected.estimate.latency_ns,
        selected.estimate.energy_uj,
        selected.estimate.estimated_error_fraction,
        selected.estimate.throughput_gops,
    )
}

fn fingerprint(
    shape: GemmShape,
    dtype: DType,
    capabilities: &DeviceCapabilities,
    model: &CostModelInputs,
    profile: OperationCostProfile,
    objective: OptimizationObjective,
    options: AutotuneOptions,
) -> String {
    let calibration = capabilities
        .calibration_profile
        .as_ref()
        .map_or("none", |profile| profile.id.as_str());
    let model_json = serde_json::to_string(model).expect("cost model is serializable");
    let profile_json = serde_json::to_string(&profile).expect("cost profile is serializable");
    let input = format!(
        "{COST_MODEL_VERSION}|{}|{}|{}|{}|{}|{dtype:?}|{}|{}|{}|{objective:?}|{}|{}|{}|{}|{}|{}|{}|{}|{}",
        capabilities.backend_id,
        capabilities.capability_version,
        shape.m,
        shape.n,
        shape.k,
        calibration,
        capabilities.effective_bits,
        capabilities.simultaneous_channels,
        options.graph_fingerprint,
        options.seed,
        options.batch_size,
        options.allow_boundary_fusion,
        options.queue_depth,
        options.overlap_fraction,
        options.resident_input_fraction,
        model_json,
        profile_json,
    );
    format!("fnv1a64:{:016x}", fnv1a64(input.as_bytes()))
}

fn incomplete_fingerprint(
    shape: GemmShape,
    dtype: DType,
    capabilities: &DeviceCapabilities,
    model: &CostModelInputs,
) -> String {
    let model_json = serde_json::to_string(model).unwrap_or_else(|_| "unserializable".to_string());
    let input = format!(
        "{COST_MODEL_VERSION}|incomplete|{}|{}|{}|{}|{dtype:?}|{}|{model_json}",
        capabilities.backend_id, shape.m, shape.n, shape.k, capabilities.capability_version
    );
    format!("fnv1a64:{:016x}", fnv1a64(input.as_bytes()))
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut hash = 0xcbf29ce484222325_u64;
    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x100000001b3);
    }
    hash
}

pub fn stable_fingerprint_bytes(bytes: &[u8]) -> u64 {
    fnv1a64(bytes)
}

fn interval(expected: f64, uncertainty: f64) -> EstimateInterval {
    EstimateInterval {
        lower: (expected * (1.0 - uncertainty)).max(0.0),
        expected,
        upper: expected * (1.0 + uncertainty),
    }
}

fn relative_error(predicted: f64, observed: f64) -> f64 {
    (predicted - observed).abs() / observed.abs().max(f64::EPSILON)
}

fn validate_observation(observed: &Observation) -> Result<()> {
    positive(observed.latency_ns, "observed latency")?;
    non_negative(observed.energy_uj, "observed energy")?;
    fraction(observed.error_fraction, "observed error")?;
    if !matches!(
        observed.source,
        ParameterSource::Measured | ParameterSource::Simulated
    ) {
        bail!("observations must have measured or simulated provenance");
    }
    if observed.artifact_id.trim().is_empty() {
        bail!("observations require an immutable artifact identity");
    }
    if observed.op_id.trim().is_empty() {
        bail!("observations require an operation identity");
    }
    Ok(())
}

fn positive(value: f64, name: &str) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        bail!("{name} must be finite and positive");
    }
    Ok(())
}

fn non_negative(value: f64, name: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        bail!("{name} must be finite and non-negative");
    }
    Ok(())
}

fn fraction(value: f64, name: &str) -> Result<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("{name} must be finite and within [0, 1]");
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn capabilities() -> DeviceCapabilities {
        DeviceCapabilities::pace_like_128()
    }

    #[test]
    fn full_system_latency_and_energy_equal_component_sums() {
        let capabilities = capabilities();
        let model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Simulated);
        let plan = TuningPlan {
            tile_m: 128,
            tile_n: 128,
            tile_k: 128,
            bit_slices: 1,
            wavelength_channels: 16,
            accumulation_mode: AccumulationMode::Digital,
            batch_size: 1,
            fuse_boundaries: false,
        };
        let estimate = estimate_photonic_plan(
            GemmShape {
                m: 256,
                n: 256,
                k: 256,
            },
            DType::F16,
            &capabilities,
            &model,
            plan,
        )
        .expect("estimate");
        assert_eq!(estimate.latency_ns, estimate.latency_breakdown_ns.total());
        assert_eq!(estimate.energy_uj, estimate.energy_breakdown_uj.total());
        assert!(estimate.latency_breakdown_ns.host_transfer > 0.0);
        assert!(estimate.latency_breakdown_ns.optical_propagation > 0.0);
        assert!(estimate.energy_breakdown_uj.laser > 0.0);
        assert!(estimate.energy_breakdown_uj.support_system > 0.0);
    }

    #[test]
    fn missing_or_invalid_inputs_are_rejected() {
        let capabilities = capabilities();
        let mut model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Assumed);
        model.provenance.clear();
        assert!(model.validate().is_err());
        let mut model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Assumed);
        model.memory_bandwidth_gbps = f64::NAN;
        assert!(model.validate().is_err());
    }

    #[test]
    fn autotuning_is_deterministic_and_objective_specific() {
        let capabilities = capabilities();
        let model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Simulated);
        let shape = GemmShape {
            m: 256,
            n: 256,
            k: 256,
        };
        let options = AutotuneOptions {
            seed: 42,
            batch_size: 4,
            allow_boundary_fusion: true,
            alternatives: 4,
            ..AutotuneOptions::default()
        };
        let first = autotune(
            shape,
            DType::F16,
            Some(8),
            &capabilities,
            &model,
            OptimizationObjective::Latency,
            options,
        )
        .expect("autotune");
        let second = autotune(
            shape,
            DType::F16,
            Some(8),
            &capabilities,
            &model,
            OptimizationObjective::Latency,
            options,
        )
        .expect("autotune");
        assert_eq!(first, second);
        assert!(first.selected.plan.fuse_boundaries);
        assert_eq!(first.seed, 42);
    }

    #[test]
    fn cache_key_invalidates_on_calibration_or_device_change() {
        let capabilities = capabilities();
        let model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Simulated);
        let shape = GemmShape { m: 4, n: 4, k: 4 };
        let first = autotune(
            shape,
            DType::F16,
            Some(8),
            &capabilities,
            &model,
            OptimizationObjective::Energy,
            AutotuneOptions::default(),
        )
        .expect("autotune");
        let mut changed = capabilities.clone();
        changed.calibration_profile.as_mut().expect("profile").id = "new-calibration".to_string();
        let second = autotune(
            shape,
            DType::F16,
            Some(8),
            &changed,
            &model,
            OptimizationObjective::Energy,
            AutotuneOptions::default(),
        )
        .expect("autotune");
        assert_ne!(first.fingerprint, second.fingerprint);
        let mut cache = DecisionCache::default();
        cache.insert(first);
        cache.insert(second.clone());
        cache.retain_only(&second.fingerprint);
        assert_eq!(cache.len(), 1);
    }

    #[test]
    fn predicted_vs_observed_report_tracks_model_error() {
        let capabilities = capabilities();
        let model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Simulated);
        let result = autotune(
            GemmShape { m: 4, n: 4, k: 4 },
            DType::F16,
            Some(8),
            &capabilities,
            &model,
            OptimizationObjective::Latency,
            AutotuneOptions::default(),
        )
        .expect("autotune");
        let predicted = result.selected.estimate.clone();
        let report = ModelErrorReport::compare(
            result.fingerprint,
            predicted.clone(),
            Observation {
                op_id: "gemm".to_string(),
                latency_ns: predicted.latency_ns * 1.1,
                energy_uj: predicted.energy_uj * 0.9,
                error_fraction: predicted.estimated_error_fraction + 0.001,
                source: ParameterSource::Measured,
                artifact_id: "sha256:benchmark".to_string(),
            },
        )
        .expect("model error report");
        assert!(report.latency_error_fraction > 0.09);
        assert!(report.energy_error_fraction > 0.1);
        assert!((report.numerical_error_delta - 0.001).abs() < 1.0e-12);
    }

    #[test]
    fn physical_unit_conversions_match_declared_dimensions() {
        let capabilities = capabilities();
        let model =
            CostModelInputs::from_capabilities(&capabilities, ParameterSource::VendorSpecified);
        let plan = TuningPlan {
            tile_m: 1,
            tile_n: 1,
            tile_k: 1,
            bit_slices: 1,
            wavelength_channels: 1,
            accumulation_mode: AccumulationMode::Digital,
            batch_size: 1,
            fuse_boundaries: false,
        };
        let estimate = estimate_photonic_plan(
            GemmShape { m: 1, n: 1, k: 1 },
            DType::Int8,
            &capabilities,
            &model,
            plan,
        )
        .expect("dimensioned estimate");

        let bytes = 3.0;
        assert!(
            (estimate.latency_breakdown_ns.host_transfer
                - bytes * 8.0
                    / capabilities
                        .host_bandwidth_gbps
                        .min(capabilities.link_bandwidth_gbps))
            .abs()
                < 1.0e-12
        );
        assert!(
            (estimate.energy_breakdown_uj.host_transfer
                - bytes * model.host_transfer_energy_pj_per_byte / 1_000_000.0)
                .abs()
                < 1.0e-12
        );
        assert!(
            (estimate.energy_breakdown_uj.dac
                - 3.0 * capabilities.dac_energy_pj_per_sample / 1_000_000.0)
                .abs()
                < 1.0e-12
        );
        assert!(estimate.latency_ns > estimate.latency_breakdown_ns.optical_propagation);
    }

    #[test]
    fn all_objectives_are_deterministic_and_accuracy_uses_more_precision() {
        let capabilities = capabilities();
        let model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Simulated);
        let shape = GemmShape {
            m: 256,
            n: 256,
            k: 256,
        };
        let options = AutotuneOptions {
            graph_fingerprint: 0x5eed,
            seed: 7,
            allow_boundary_fusion: true,
            ..AutotuneOptions::default()
        };
        let results = [
            OptimizationObjective::Latency,
            OptimizationObjective::Energy,
            OptimizationObjective::Accuracy,
            OptimizationObjective::Throughput,
        ]
        .map(|objective| {
            autotune(
                shape,
                DType::F16,
                None,
                &capabilities,
                &model,
                objective,
                options,
            )
            .expect("objective tune")
        });

        assert_eq!(results[2].selected.plan.bit_slices, 2);
        assert_eq!(results[0].selected.plan.bit_slices, 1);
        assert_eq!(results[3].selected.plan.bit_slices, 1);
        assert!(results.iter().all(|result| !result.alternatives.is_empty()));
    }

    #[test]
    fn dynamic_range_overflow_is_error_or_clamped_error_by_contract() {
        let mut capabilities = capabilities();
        let model = CostModelInputs::from_capabilities(&capabilities, ParameterSource::Simulated);
        let profile = OperationCostProfile {
            maximum_input_magnitude: Some(2.0),
            ..OperationCostProfile::default()
        };
        let plan = TuningPlan {
            tile_m: 1,
            tile_n: 1,
            tile_k: 1,
            bit_slices: 1,
            wavelength_channels: 1,
            accumulation_mode: AccumulationMode::Digital,
            batch_size: 1,
            fuse_boundaries: false,
        };
        capabilities.saturation_mode = SaturationMode::Clamp;
        let clamped = estimate_photonic_plan_with_profile(
            GemmShape { m: 1, n: 1, k: 1 },
            DType::Int8,
            &capabilities,
            &model,
            profile,
            plan,
            AutotuneOptions::default(),
        )
        .expect("clamped estimate");
        assert!(clamped.estimated_error_fraction >= 0.5);

        capabilities.saturation_mode = SaturationMode::Error;
        let error = estimate_photonic_plan_with_profile(
            GemmShape { m: 1, n: 1, k: 1 },
            DType::Int8,
            &capabilities,
            &model,
            profile,
            plan,
            AutotuneOptions::default(),
        )
        .expect_err("error saturation mode");
        assert!(error.to_string().contains("dynamic range"));
    }
}
