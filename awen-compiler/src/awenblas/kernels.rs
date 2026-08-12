use crate::capability::AccumulationMode;
use crate::cost::{
    stable_fingerprint_bytes, OptimizationObjective, ParameterSource, TargetBackend,
};
use crate::ir::{AccuracyContract, DType, Layout};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use std::f64::consts::PI;
use std::time::Instant;

pub const AWENBLAS_VERSION: &str = "awen.blas.v1";
pub const AWENBLAS_BENCHMARK_VERSION: &str = "awen.blas-benchmark.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum KernelKind {
    Gemm,
    BatchedGemm,
    ComplexGemm,
    Linear,
    TransformerQkv,
    AttentionScores,
    AttentionValue,
    MlpProjection,
    Convolution1d,
    Correlation1d,
    Dft,
    Fft,
    FourierFilter,
    LowRankGemm,
    RandomProjection,
    Toeplitz,
    Circulant,
    BlockCirculant,
    Beamforming,
    RfFir,
    ReservoirStep,
    Propagation,
}

impl KernelKind {
    pub fn is_complex(self) -> bool {
        matches!(
            self,
            Self::ComplexGemm
                | Self::Dft
                | Self::Fft
                | Self::FourierFilter
                | Self::Beamforming
                | Self::Propagation
        )
    }

    pub fn natural_structure(self) -> Option<KernelStructure> {
        match self {
            Self::LowRankGemm => Some(KernelStructure::LowRank),
            Self::RandomProjection => Some(KernelStructure::RandomProjection),
            Self::Toeplitz => Some(KernelStructure::Toeplitz),
            Self::Circulant => Some(KernelStructure::Circulant),
            Self::BlockCirculant => Some(KernelStructure::BlockCirculant),
            Self::Convolution1d | Self::Correlation1d | Self::RfFir => {
                Some(KernelStructure::Convolutional)
            }
            Self::Dft | Self::Fft | Self::FourierFilter => Some(KernelStructure::Fourier),
            Self::Beamforming => Some(KernelStructure::Beamforming),
            Self::ReservoirStep => Some(KernelStructure::Reservoir),
            Self::Propagation => Some(KernelStructure::Propagation),
            _ => Some(KernelStructure::Dense),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum KernelStructure {
    Dense,
    LowRank,
    RandomProjection,
    Toeplitz,
    Circulant,
    BlockCirculant,
    Convolutional,
    Fourier,
    Beamforming,
    Reservoir,
    Propagation,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PhaseConvention {
    #[default]
    NegativeExponent,
    PositiveExponent,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ComplexValue {
    pub real: f64,
    pub imaginary: f64,
}

impl ComplexValue {
    pub const ZERO: Self = Self {
        real: 0.0,
        imaginary: 0.0,
    };

    pub fn new(real: f64, imaginary: f64) -> Self {
        Self { real, imaginary }
    }

    fn add(self, other: Self) -> Self {
        Self::new(self.real + other.real, self.imaginary + other.imaginary)
    }

    fn multiply(self, other: Self) -> Self {
        Self::new(
            self.real * other.real - self.imaginary * other.imaginary,
            self.real * other.imaginary + self.imaginary * other.real,
        )
    }

    fn scale(self, factor: f64) -> Self {
        Self::new(self.real * factor, self.imaginary * factor)
    }

    fn magnitude(self) -> f64 {
        self.real.hypot(self.imaginary)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "representation", content = "values", rename_all = "snake_case")]
pub enum KernelData {
    Real(Vec<f64>),
    Complex(Vec<ComplexValue>),
}

impl KernelData {
    fn len(&self) -> usize {
        match self {
            Self::Real(values) => values.len(),
            Self::Complex(values) => values.len(),
        }
    }

    fn validate(&self) -> Result<()> {
        let valid = match self {
            Self::Real(values) => values.iter().all(|value| value.is_finite()),
            Self::Complex(values) => values
                .iter()
                .all(|value| value.real.is_finite() && value.imaginary.is_finite()),
        };
        if !valid {
            bail!("kernel tensor data must contain only finite values");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelTensor {
    pub id: String,
    pub shape: Vec<usize>,
    pub dtype: DType,
    pub layout: Layout,
    pub data: KernelData,
}

impl KernelTensor {
    pub fn real(id: impl Into<String>, shape: Vec<usize>, data: Vec<f64>) -> Self {
        Self {
            id: id.into(),
            shape,
            dtype: DType::F32,
            layout: Layout::RowMajor,
            data: KernelData::Real(data),
        }
    }

    pub fn complex(id: impl Into<String>, shape: Vec<usize>, data: Vec<ComplexValue>) -> Self {
        Self {
            id: id.into(),
            shape,
            dtype: DType::ComplexF32,
            layout: Layout::RowMajor,
            data: KernelData::Complex(data),
        }
    }

    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty() || self.shape.is_empty() || self.shape.contains(&0) {
            bail!("kernel tensors require an id and non-zero shape");
        }
        let elements = element_count(&self.shape)?;
        if elements != self.data.len() {
            bail!(
                "kernel tensor '{}' has {} values but shape {:?} requires {elements}",
                self.id,
                self.data.len(),
                self.shape
            );
        }
        match (&self.data, self.dtype) {
            (KernelData::Complex(_), DType::ComplexF32)
            | (
                KernelData::Real(_),
                DType::F32 | DType::F16 | DType::Bf16 | DType::Int8 | DType::Int4,
            ) => {}
            _ => bail!(
                "kernel tensor '{}' data representation does not match its dtype",
                self.id
            ),
        }
        self.data.validate()
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct KernelAttributes {
    pub transpose_lhs: bool,
    pub transpose_rhs: bool,
    pub stride: usize,
    pub dilation: usize,
    pub padding: usize,
    pub block_size: usize,
    pub rank: usize,
    pub output_size: usize,
    pub seed: u64,
    pub scale: f64,
    pub leakage: f64,
    pub inverse: bool,
    pub phase_convention: PhaseConvention,
    pub accumulation_mode: AccumulationMode,
}

impl Default for KernelAttributes {
    fn default() -> Self {
        Self {
            transpose_lhs: false,
            transpose_rhs: false,
            stride: 1,
            dilation: 1,
            padding: 0,
            block_size: 1,
            rank: 0,
            output_size: 0,
            seed: 0,
            scale: 1.0,
            leakage: 1.0,
            inverse: false,
            phase_convention: PhaseConvention::NegativeExponent,
            accumulation_mode: AccumulationMode::Digital,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibrationInput {
    pub id: String,
    pub gain: f64,
    pub bias: f64,
    pub uncertainty_fraction: f64,
}

impl CalibrationInput {
    fn validate(&self) -> Result<()> {
        if self.id.trim().is_empty()
            || !self.gain.is_finite()
            || self.gain == 0.0
            || !self.bias.is_finite()
            || !self.uncertainty_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.uncertainty_fraction)
        {
            bail!("kernel calibration inputs require finite gain/bias and uncertainty in [0, 1]");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelRequest {
    pub version: String,
    pub id: String,
    pub kind: KernelKind,
    pub inputs: Vec<KernelTensor>,
    #[serde(default)]
    pub attributes: KernelAttributes,
    #[serde(default)]
    pub accuracy: AccuracyContract,
    #[serde(default)]
    pub calibration_inputs: Vec<CalibrationInput>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelDescriptor {
    pub id: String,
    pub kind: KernelKind,
    pub input_shapes: Vec<Vec<usize>>,
    pub output_shapes: Vec<Vec<usize>>,
    pub dtypes: Vec<DType>,
    pub layouts: Vec<Layout>,
    pub structure: KernelStructure,
    pub phase_convention: Option<PhaseConvention>,
    pub accumulation_mode: AccumulationMode,
    pub minimum_effective_bits: Option<u8>,
    pub maximum_absolute_error: f64,
    pub maximum_relative_error: f64,
    pub calibration_inputs: Vec<String>,
    pub operations: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelResult {
    pub version: String,
    pub request_id: String,
    pub kind: KernelKind,
    pub outputs: Vec<KernelTensor>,
    pub descriptor: KernelDescriptor,
    pub execution_target: TargetBackend,
    pub simulated: bool,
    pub execution_fingerprint: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelCostEstimate {
    pub latency_ns: f64,
    pub energy_uj: f64,
    pub error_fraction: f64,
    pub throughput_gops: f64,
    pub source: ParameterSource,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelBackendProfile {
    pub version: String,
    pub backend_id: String,
    pub target: TargetBackend,
    pub supported_kinds: BTreeSet<KernelKind>,
    pub supported_dtypes: BTreeSet<DType>,
    pub supported_structures: BTreeSet<KernelStructure>,
    pub supports_complex: bool,
    pub maximum_tensor_elements: usize,
    pub effective_bits: u8,
    pub requires_calibration: bool,
    pub launch_latency_ns: f64,
    pub throughput_tops: f64,
    pub energy_pj_per_operation: f64,
    pub estimated_error_fraction: f64,
    pub source: ParameterSource,
}

impl KernelBackendProfile {
    pub fn cpu_reference() -> Self {
        Self {
            version: AWENBLAS_VERSION.to_string(),
            backend_id: "awenblas-cpu-reference".to_string(),
            target: TargetBackend::Cpu,
            supported_kinds: all_kernel_kinds(),
            supported_dtypes: [
                DType::F32,
                DType::F16,
                DType::Bf16,
                DType::Int8,
                DType::Int4,
                DType::ComplexF32,
            ]
            .into_iter()
            .collect(),
            supported_structures: all_kernel_structures(),
            supports_complex: true,
            maximum_tensor_elements: usize::MAX,
            effective_bits: 64,
            requires_calibration: false,
            launch_latency_ns: 2_500.0,
            throughput_tops: 0.25,
            energy_pj_per_operation: 20.0,
            estimated_error_fraction: 0.0,
            source: ParameterSource::Assumed,
        }
    }

    pub fn validate(&self) -> Result<()> {
        if self.version != AWENBLAS_VERSION
            || self.backend_id.trim().is_empty()
            || self.target == TargetBackend::Auto
            || self.supported_kinds.is_empty()
            || self.supported_dtypes.is_empty()
            || self.supported_structures.is_empty()
            || self.maximum_tensor_elements == 0
            || self.effective_bits == 0
            || !self.launch_latency_ns.is_finite()
            || self.launch_latency_ns < 0.0
            || !self.throughput_tops.is_finite()
            || self.throughput_tops <= 0.0
            || !self.energy_pj_per_operation.is_finite()
            || self.energy_pj_per_operation < 0.0
            || !self.estimated_error_fraction.is_finite()
            || !(0.0..=1.0).contains(&self.estimated_error_fraction)
        {
            bail!("kernel backend profile contains invalid or incomplete values");
        }
        let advertises_complex_kind = self.supported_kinds.iter().any(|kind| kind.is_complex());
        let advertises_complex_dtype = self.supported_dtypes.contains(&DType::ComplexF32);
        if advertises_complex_kind && (!self.supports_complex || !advertises_complex_dtype)
            || self.supports_complex && !advertises_complex_dtype
            || !self.supports_complex && advertises_complex_dtype
        {
            bail!("kernel backend complex capability fields are contradictory");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelCandidateTrace {
    pub backend_id: String,
    pub target: TargetBackend,
    pub eligible: bool,
    pub estimate: Option<KernelCostEstimate>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelExecutionPlan {
    pub version: String,
    pub request_id: String,
    pub descriptor: KernelDescriptor,
    pub objective: OptimizationObjective,
    pub selected_backend_id: String,
    pub selected_target: TargetBackend,
    pub selected_estimate: KernelCostEstimate,
    pub candidates: Vec<KernelCandidateTrace>,
    pub fallback: bool,
    pub fingerprint: String,
    pub rationale: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct KernelSimulatorOptions {
    pub target: TargetBackend,
    pub effective_bits: u8,
    pub noise_fraction: f64,
    pub seed: u64,
}

impl Default for KernelSimulatorOptions {
    fn default() -> Self {
        Self {
            target: TargetBackend::Photonic,
            effective_bits: 8,
            noise_fraction: 0.0,
            seed: 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct KernelBenchmarkReport {
    pub version: String,
    pub request_id: String,
    pub kind: KernelKind,
    pub repetitions: usize,
    pub reference_wall_clock_ns: u128,
    pub simulator_wall_clock_ns: u128,
    pub maximum_absolute_error: f64,
    pub maximum_relative_error: f64,
    pub within_contract: bool,
    pub source: ParameterSource,
    pub measurement_boundary: Vec<String>,
    pub output_checksum: String,
}

impl KernelRequest {
    pub fn validate(&self) -> Result<()> {
        if self.version != AWENBLAS_VERSION {
            bail!(
                "unsupported awenBLAS version '{}'; expected '{}'",
                self.version,
                AWENBLAS_VERSION
            );
        }
        if self.id.trim().is_empty() || self.inputs.is_empty() {
            bail!("awenBLAS requests require an id and at least one input");
        }
        for input in &self.inputs {
            input.validate()?;
        }
        for calibration in &self.calibration_inputs {
            calibration.validate()?;
        }
        let (combined_gain, combined_bias) =
            self.calibration_inputs
                .iter()
                .fold((1.0, 0.0), |(gain, bias), calibration| {
                    (
                        gain * calibration.gain,
                        bias * calibration.gain + calibration.bias,
                    )
                });
        if !combined_gain.is_finite() || combined_gain == 0.0 || !combined_bias.is_finite() {
            bail!("composed kernel calibration transfer must remain finite and invertible");
        }
        if !self.accuracy.max_abs_error.is_finite()
            || self.accuracy.max_abs_error < 0.0
            || !self.accuracy.max_rel_error.is_finite()
            || self.accuracy.max_rel_error < 0.0
            || self.attributes.stride == 0
            || self.attributes.dilation == 0
            || self.attributes.block_size == 0
            || !self.attributes.scale.is_finite()
            || !self.attributes.leakage.is_finite()
            || !(0.0..=1.0).contains(&self.attributes.leakage)
        {
            bail!("awenBLAS request attributes or numerical contract are invalid");
        }
        if self.accuracy.minimum_effective_bits.is_some_and(|bits| {
            self.inputs
                .iter()
                .map(|input| input.dtype.bits())
                .min()
                .is_some_and(|dtype_bits| bits > dtype_bits)
        }) {
            bail!("awenBLAS minimum effective bits exceed an input dtype's precision");
        }
        let expected_inputs = match self.kind {
            KernelKind::Gemm
            | KernelKind::BatchedGemm
            | KernelKind::ComplexGemm
            | KernelKind::AttentionScores
            | KernelKind::AttentionValue
            | KernelKind::Convolution1d
            | KernelKind::Correlation1d
            | KernelKind::FourierFilter
            | KernelKind::Circulant
            | KernelKind::BlockCirculant
            | KernelKind::Beamforming
            | KernelKind::RfFir
            | KernelKind::Propagation => 2,
            KernelKind::Linear | KernelKind::MlpProjection => 2,
            KernelKind::TransformerQkv => 4,
            KernelKind::Dft | KernelKind::Fft | KernelKind::RandomProjection => 1,
            KernelKind::LowRankGemm => 3,
            KernelKind::Toeplitz | KernelKind::ReservoirStep => 3,
        };
        if self.inputs.len() != expected_inputs
            && !matches!(self.kind, KernelKind::Linear | KernelKind::MlpProjection)
        {
            bail!(
                "kernel {:?} requires {expected_inputs} input tensor(s), got {}",
                self.kind,
                self.inputs.len()
            );
        }
        if matches!(self.kind, KernelKind::Linear | KernelKind::MlpProjection)
            && !(2..=3).contains(&self.inputs.len())
        {
            bail!("linear and MLP kernels require matrix, weight, and optional bias inputs");
        }
        if self.kind.is_complex()
            && self
                .inputs
                .iter()
                .any(|input| input.dtype != DType::ComplexF32)
        {
            bail!("complex kernel {:?} requires complex_f32 inputs", self.kind);
        }
        if !self.kind.is_complex()
            && self
                .inputs
                .iter()
                .any(|input| input.dtype != self.inputs[0].dtype)
        {
            bail!("real awenBLAS v1 kernels require matching input dtypes");
        }
        let _ = reference_outputs(self)?;
        Ok(())
    }

    pub fn descriptor(&self) -> Result<KernelDescriptor> {
        let outputs = reference_outputs(self)?;
        let output_shapes = outputs.iter().map(|output| output.shape.clone()).collect();
        let operations = estimate_operations(self, &outputs);
        Ok(KernelDescriptor {
            id: self.id.clone(),
            kind: self.kind,
            input_shapes: self
                .inputs
                .iter()
                .map(|input| input.shape.clone())
                .collect(),
            output_shapes,
            dtypes: self.inputs.iter().map(|input| input.dtype).collect(),
            layouts: self.inputs.iter().map(|input| input.layout).collect(),
            structure: self
                .kind
                .natural_structure()
                .unwrap_or(KernelStructure::Dense),
            phase_convention: self
                .kind
                .is_complex()
                .then_some(self.attributes.phase_convention),
            accumulation_mode: self.attributes.accumulation_mode,
            minimum_effective_bits: self.accuracy.minimum_effective_bits,
            maximum_absolute_error: self.accuracy.max_abs_error,
            maximum_relative_error: self.accuracy.max_rel_error,
            calibration_inputs: self
                .calibration_inputs
                .iter()
                .map(|input| input.id.clone())
                .collect(),
            operations,
        })
    }
}

pub fn execute_reference(request: &KernelRequest) -> Result<KernelResult> {
    request.validate()?;
    let descriptor = request.descriptor()?;
    let outputs = reference_outputs(request)?;
    kernel_result(request, descriptor, outputs, TargetBackend::Cpu, false)
}

pub fn execute_simulator(
    request: &KernelRequest,
    options: KernelSimulatorOptions,
) -> Result<KernelResult> {
    request.validate()?;
    if options.target == TargetBackend::Auto
        || options.effective_bits == 0
        || !options.noise_fraction.is_finite()
        || options.noise_fraction < 0.0
    {
        bail!("kernel simulator options require a concrete target, bits, and non-negative noise");
    }
    let mut quantized = request.clone();
    for input in &mut quantized.inputs {
        quantize_data(&mut input.data, options.effective_bits);
    }
    let descriptor = request.descriptor()?;
    let mut outputs = reference_outputs(&quantized)?;
    let mut state = options.seed;
    for output in &mut outputs {
        apply_simulation_effects(
            &mut output.data,
            &request.calibration_inputs,
            options.noise_fraction,
            &mut state,
        );
    }
    kernel_result(request, descriptor, outputs, options.target, true)
}

pub fn select_kernel(
    request: &KernelRequest,
    profiles: &[KernelBackendProfile],
    objective: OptimizationObjective,
) -> Result<KernelExecutionPlan> {
    request.validate()?;
    let descriptor = request.descriptor()?;
    let mut candidates = Vec::new();
    let mut considered = vec![KernelBackendProfile::cpu_reference()];
    considered.extend_from_slice(profiles);
    let mut seen = BTreeSet::new();
    for profile in considered {
        profile.validate()?;
        if !seen.insert(profile.backend_id.clone()) {
            bail!("duplicate kernel backend id '{}'", profile.backend_id);
        }
        let reason = ineligibility_reason(request, &descriptor, &profile);
        let eligible = reason.is_none();
        let estimate = eligible.then(|| estimate_kernel_cost(&descriptor, &profile));
        candidates.push(KernelCandidateTrace {
            backend_id: profile.backend_id,
            target: profile.target,
            eligible,
            estimate,
            reason: reason.unwrap_or_else(|| {
                "kernel, structure, precision, calibration, and size are supported".to_string()
            }),
        });
    }
    let mut legal = candidates
        .iter()
        .filter_map(|candidate| {
            candidate
                .estimate
                .as_ref()
                .map(|estimate| (candidate, estimate))
        })
        .collect::<Vec<_>>();
    legal.sort_by(|(left_candidate, left), (right_candidate, right)| {
        kernel_objective_score(objective, left)
            .total_cmp(&kernel_objective_score(objective, right))
            .then_with(|| left_candidate.backend_id.cmp(&right_candidate.backend_id))
    });
    let (winner, selected_estimate) = legal
        .first()
        .copied()
        .context("awenBLAS selector found no legal backend")?;
    let fallback = winner.target == TargetBackend::Cpu
        && profiles
            .iter()
            .any(|profile| profile.target != TargetBackend::Cpu);
    let fingerprint = format!(
        "fnv1a64:{:016x}",
        stable_fingerprint_bytes(&serde_json::to_vec(&(request, profiles, objective))?)
    );
    let rationale = if fallback {
        format!(
            "selected CPU reference fallback for {:?} under {:?}; accelerator candidates were unsupported, inaccurate, uncalibrated, oversized, or more expensive",
            request.kind, objective
        )
    } else {
        format!(
            "selected '{}' on {:?} for {:?} under {:?} from {} legal candidate(s)",
            winner.backend_id,
            winner.target,
            request.kind,
            objective,
            legal.len()
        )
    };
    Ok(KernelExecutionPlan {
        version: AWENBLAS_VERSION.to_string(),
        request_id: request.id.clone(),
        descriptor,
        objective,
        selected_backend_id: winner.backend_id.clone(),
        selected_target: winner.target,
        selected_estimate: selected_estimate.clone(),
        candidates,
        fallback,
        fingerprint,
        rationale,
    })
}

pub fn benchmark_kernel(
    request: &KernelRequest,
    simulator: KernelSimulatorOptions,
    repetitions: usize,
) -> Result<KernelBenchmarkReport> {
    if repetitions == 0 {
        bail!("awenBLAS benchmark repetitions must be non-zero");
    }
    let reference_start = Instant::now();
    let mut reference = None;
    for _ in 0..repetitions {
        reference = Some(execute_reference(request)?);
    }
    let reference_wall_clock_ns = reference_start.elapsed().as_nanos();
    let simulator_start = Instant::now();
    let mut simulated = None;
    for _ in 0..repetitions {
        simulated = Some(execute_simulator(request, simulator)?);
    }
    let simulator_wall_clock_ns = simulator_start.elapsed().as_nanos();
    let reference = reference.expect("non-zero repetitions");
    let simulated = simulated.expect("non-zero repetitions");
    let (maximum_absolute_error, maximum_relative_error) =
        compare_outputs(&reference.outputs, &simulated.outputs)?;
    let within_contract = maximum_absolute_error <= request.accuracy.max_abs_error
        || maximum_relative_error <= request.accuracy.max_rel_error;
    let output_bytes = serde_json::to_vec(&simulated.outputs)?;
    Ok(KernelBenchmarkReport {
        version: AWENBLAS_BENCHMARK_VERSION.to_string(),
        request_id: request.id.clone(),
        kind: request.kind,
        repetitions,
        reference_wall_clock_ns,
        simulator_wall_clock_ns,
        maximum_absolute_error,
        maximum_relative_error,
        within_contract,
        source: ParameterSource::Measured,
        measurement_boundary: vec![
            "request validation".to_string(),
            "input quantization".to_string(),
            "kernel execution".to_string(),
            "calibration transfer".to_string(),
            "deterministic simulator noise".to_string(),
            "output materialization".to_string(),
        ],
        output_checksum: format!("fnv1a64:{:016x}", stable_fingerprint_bytes(&output_bytes)),
    })
}

fn reference_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    match request.kind {
        KernelKind::Gemm | KernelKind::BatchedGemm => {
            real_gemm_outputs(request, request.kind == KernelKind::BatchedGemm)
        }
        KernelKind::ComplexGemm => complex_gemm_outputs(request),
        KernelKind::Linear | KernelKind::MlpProjection => linear_outputs(request),
        KernelKind::TransformerQkv => transformer_qkv_outputs(request),
        KernelKind::AttentionScores => attention_scores_outputs(request),
        KernelKind::AttentionValue => {
            real_gemm_outputs(request, request.inputs[0].shape.len() == 3)
        }
        KernelKind::Convolution1d | KernelKind::RfFir => convolution_outputs(request, false),
        KernelKind::Correlation1d => convolution_outputs(request, true),
        KernelKind::Dft => dft_outputs(request, false),
        KernelKind::Fft => dft_outputs(request, true),
        KernelKind::FourierFilter => fourier_filter_outputs(request),
        KernelKind::LowRankGemm => low_rank_outputs(request),
        KernelKind::RandomProjection => random_projection_outputs(request),
        KernelKind::Toeplitz => toeplitz_outputs(request),
        KernelKind::Circulant => circulant_outputs(request),
        KernelKind::BlockCirculant => block_circulant_outputs(request),
        KernelKind::Beamforming => complex_gemm_outputs(request),
        KernelKind::ReservoirStep => reservoir_outputs(request),
        KernelKind::Propagation => complex_gemm_outputs(request),
    }
}

fn real_gemm_outputs(request: &KernelRequest, batched: bool) -> Result<Vec<KernelTensor>> {
    let lhs = real_values(&request.inputs[0])?;
    let rhs = real_values(&request.inputs[1])?;
    let lhs_shape = &request.inputs[0].shape;
    let rhs_shape = &request.inputs[1].shape;
    let batch = if batched {
        require_rank(&request.inputs[0], 3)?;
        require_rank(&request.inputs[1], 3)?;
        if lhs_shape[0] != rhs_shape[0] {
            bail!("batched GEMM requires equal batch dimensions");
        }
        lhs_shape[0]
    } else {
        require_rank(&request.inputs[0], 2)?;
        require_rank(&request.inputs[1], 2)?;
        1
    };
    let lhs_rows = lhs_shape[lhs_shape.len() - 2];
    let lhs_cols = lhs_shape[lhs_shape.len() - 1];
    let rhs_rows = rhs_shape[rhs_shape.len() - 2];
    let rhs_cols = rhs_shape[rhs_shape.len() - 1];
    let (m, k_left) = if request.attributes.transpose_lhs {
        (lhs_cols, lhs_rows)
    } else {
        (lhs_rows, lhs_cols)
    };
    let (k_right, n) = if request.attributes.transpose_rhs {
        (rhs_cols, rhs_rows)
    } else {
        (rhs_rows, rhs_cols)
    };
    if k_left != k_right {
        bail!("GEMM inner dimensions {k_left} and {k_right} do not match");
    }
    let mut output = vec![0.0; batch * m * n];
    for b in 0..batch {
        for row in 0..m {
            for column in 0..n {
                for inner in 0..k_left {
                    let lhs_value = matrix_element(
                        &request.inputs[0],
                        lhs,
                        b,
                        row,
                        inner,
                        request.attributes.transpose_lhs,
                    )?;
                    let rhs_value = matrix_element(
                        &request.inputs[1],
                        rhs,
                        b,
                        inner,
                        column,
                        request.attributes.transpose_rhs,
                    )?;
                    output[b * m * n + row * n + column] += lhs_value * rhs_value;
                }
            }
        }
    }
    let shape = if batched {
        vec![batch, m, n]
    } else {
        vec![m, n]
    };
    Ok(vec![KernelTensor {
        id: format!("{}.output", request.id),
        shape,
        dtype: request.inputs[0].dtype,
        layout: Layout::RowMajor,
        data: KernelData::Real(output),
    }])
}

fn complex_gemm_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 2)?;
    require_rank(&request.inputs[1], 2)?;
    let lhs = complex_values(&request.inputs[0])?;
    let rhs = complex_values(&request.inputs[1])?;
    let lhs_rows = request.inputs[0].shape[0];
    let lhs_cols = request.inputs[0].shape[1];
    let rhs_rows = request.inputs[1].shape[0];
    let rhs_cols = request.inputs[1].shape[1];
    let (m, k_left) = if request.attributes.transpose_lhs {
        (lhs_cols, lhs_rows)
    } else {
        (lhs_rows, lhs_cols)
    };
    let (k_right, n) = if request.attributes.transpose_rhs {
        (rhs_cols, rhs_rows)
    } else {
        (rhs_rows, rhs_cols)
    };
    if k_left != k_right {
        bail!("complex GEMM inner dimensions do not match");
    }
    let mut output = vec![ComplexValue::ZERO; m * n];
    for row in 0..m {
        for column in 0..n {
            for inner in 0..k_left {
                let left = complex_matrix_element(
                    &request.inputs[0],
                    lhs,
                    row,
                    inner,
                    request.attributes.transpose_lhs,
                )?;
                let right = complex_matrix_element(
                    &request.inputs[1],
                    rhs,
                    inner,
                    column,
                    request.attributes.transpose_rhs,
                )?;
                output[row * n + column] = output[row * n + column].add(left.multiply(right));
            }
        }
    }
    Ok(vec![KernelTensor::complex(
        format!("{}.output", request.id),
        vec![m, n],
        output,
    )])
}

fn linear_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    let mut output = real_gemm_outputs(request, false)?;
    if let Some(bias) = request.inputs.get(2) {
        require_rank(bias, 1)?;
        let bias = real_values(bias)?;
        let columns = output[0].shape[1];
        if bias.len() != columns {
            bail!("linear bias length must equal output columns");
        }
        if let KernelData::Real(values) = &mut output[0].data {
            for row in values.chunks_exact_mut(columns) {
                for (value, bias) in row.iter_mut().zip(bias) {
                    *value += bias;
                }
            }
        }
    }
    Ok(output)
}

fn transformer_qkv_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    let mut outputs = Vec::new();
    for (name, weight) in ["q", "k", "v"].into_iter().zip(&request.inputs[1..]) {
        let subrequest = KernelRequest {
            version: request.version.clone(),
            id: format!("{}.{}", request.id, name),
            kind: KernelKind::Gemm,
            inputs: vec![request.inputs[0].clone(), weight.clone()],
            attributes: request.attributes.clone(),
            accuracy: request.accuracy.clone(),
            calibration_inputs: request.calibration_inputs.clone(),
        };
        outputs.push(real_gemm_outputs(&subrequest, false)?.remove(0));
    }
    Ok(outputs)
}

fn attention_scores_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    let mut adjusted = request.clone();
    adjusted.attributes.transpose_rhs = true;
    let mut outputs = real_gemm_outputs(&adjusted, request.inputs[0].shape.len() == 3)?;
    if let KernelData::Real(values) = &mut outputs[0].data {
        for value in values {
            *value *= request.attributes.scale;
        }
    }
    Ok(outputs)
}

fn convolution_outputs(request: &KernelRequest, correlation: bool) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 1)?;
    require_rank(&request.inputs[1], 1)?;
    let signal = real_values(&request.inputs[0])?;
    let kernel = real_values(&request.inputs[1])?;
    let effective_kernel = request.attributes.dilation * (kernel.len() - 1) + 1;
    let padded = signal.len() + 2 * request.attributes.padding;
    if effective_kernel > padded {
        bail!("effective convolution kernel exceeds padded signal");
    }
    let output_length = (padded - effective_kernel) / request.attributes.stride + 1;
    let mut output = vec![0.0; output_length];
    for (index, value) in output.iter_mut().enumerate() {
        for tap in 0..kernel.len() {
            let padded_index =
                index * request.attributes.stride + tap * request.attributes.dilation;
            if let Some(signal_index) = padded_index.checked_sub(request.attributes.padding) {
                if signal_index < signal.len() {
                    let kernel_index = if correlation {
                        tap
                    } else {
                        kernel.len() - 1 - tap
                    };
                    *value += signal[signal_index] * kernel[kernel_index];
                }
            }
        }
    }
    Ok(vec![KernelTensor::real(
        format!("{}.output", request.id),
        vec![output_length],
        output,
    )])
}

fn dft_outputs(request: &KernelRequest, use_fft: bool) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 1)?;
    let input = complex_values(&request.inputs[0])?;
    let output = if use_fft {
        fft(
            input,
            request.attributes.inverse,
            request.attributes.phase_convention,
        )
    } else {
        dft(
            input,
            request.attributes.inverse,
            request.attributes.phase_convention,
        )
    };
    Ok(vec![KernelTensor::complex(
        format!("{}.output", request.id),
        vec![output.len()],
        output,
    )])
}

fn fourier_filter_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 1)?;
    require_rank(&request.inputs[1], 1)?;
    let input = complex_values(&request.inputs[0])?;
    let response = complex_values(&request.inputs[1])?;
    if input.len() != response.len() {
        bail!("Fourier filter response length must match the input");
    }
    let spectrum = dft(input, false, request.attributes.phase_convention);
    let filtered = spectrum
        .into_iter()
        .zip(response)
        .map(|(value, response)| value.multiply(*response))
        .collect::<Vec<_>>();
    let output = dft(&filtered, true, request.attributes.phase_convention);
    Ok(vec![KernelTensor::complex(
        format!("{}.output", request.id),
        vec![output.len()],
        output,
    )])
}

fn low_rank_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 2)?;
    require_rank(&request.inputs[1], 2)?;
    require_rank(&request.inputs[2], 2)?;
    let a = real_values(&request.inputs[0])?;
    let u = real_values(&request.inputs[1])?;
    let v = real_values(&request.inputs[2])?;
    let (m, k) = (request.inputs[0].shape[0], request.inputs[0].shape[1]);
    let rank = request.inputs[1].shape[1];
    let n = request.inputs[2].shape[0];
    if request.inputs[1].shape != [k, rank] || request.inputs[2].shape != [n, rank] {
        bail!("low-rank GEMM requires A[m,k], U[k,r], and V[n,r]");
    }
    if request.attributes.rank != 0 && request.attributes.rank != rank {
        bail!("declared low-rank value does not match factor shapes");
    }
    let mut intermediate = vec![0.0; m * rank];
    for row in 0..m {
        for component in 0..rank {
            for inner in 0..k {
                intermediate[row * rank + component] +=
                    matrix_element(&request.inputs[0], a, 0, row, inner, false)?
                        * matrix_element(&request.inputs[1], u, 0, inner, component, false)?;
            }
        }
    }
    let mut output = vec![0.0; m * n];
    for row in 0..m {
        for column in 0..n {
            for component in 0..rank {
                output[row * n + column] += intermediate[row * rank + component]
                    * matrix_element(&request.inputs[2], v, 0, column, component, false)?;
            }
        }
    }
    Ok(vec![KernelTensor::real(
        format!("{}.output", request.id),
        vec![m, n],
        output,
    )])
}

fn random_projection_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 2)?;
    let input = real_values(&request.inputs[0])?;
    let (rows, columns) = (request.inputs[0].shape[0], request.inputs[0].shape[1]);
    let output_size = request.attributes.output_size;
    if output_size == 0 {
        bail!("random projection requires a non-zero output_size");
    }
    let normalization = (output_size as f64).sqrt().recip();
    let mut state = request.attributes.seed;
    let projection = (0..columns * output_size)
        .map(|_| {
            if next_unit(&mut state) < 0.5 {
                -normalization
            } else {
                normalization
            }
        })
        .collect::<Vec<_>>();
    let mut output = vec![0.0; rows * output_size];
    for row in 0..rows {
        for column in 0..output_size {
            for inner in 0..columns {
                output[row * output_size + column] +=
                    matrix_element(&request.inputs[0], input, 0, row, inner, false)?
                        * projection[inner * output_size + column];
            }
        }
    }
    Ok(vec![KernelTensor::real(
        format!("{}.output", request.id),
        vec![rows, output_size],
        output,
    )])
}

fn toeplitz_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    for input in &request.inputs {
        require_rank(input, 1)?;
    }
    let column = real_values(&request.inputs[0])?;
    let row = real_values(&request.inputs[1])?;
    let vector = real_values(&request.inputs[2])?;
    if row.len() != vector.len()
        || (column[0] - row[0]).abs() > request.accuracy.max_abs_error.max(f64::EPSILON)
    {
        bail!("Toeplitz kernel requires matching first element and vector/row lengths");
    }
    let mut output = vec![0.0; column.len()];
    for i in 0..column.len() {
        for j in 0..row.len() {
            let coefficient = if i >= j { column[i - j] } else { row[j - i] };
            output[i] += coefficient * vector[j];
        }
    }
    Ok(vec![KernelTensor::real(
        format!("{}.output", request.id),
        vec![column.len()],
        output,
    )])
}

fn circulant_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 1)?;
    require_rank(&request.inputs[1], 1)?;
    let generator = real_values(&request.inputs[0])?;
    let vector = real_values(&request.inputs[1])?;
    if generator.len() != vector.len() {
        bail!("circulant generator and vector lengths must match");
    }
    let n = generator.len();
    let mut output = vec![0.0; n];
    for (i, value) in output.iter_mut().enumerate() {
        for j in 0..n {
            *value += generator[(j + n - i) % n] * vector[j];
        }
    }
    Ok(vec![KernelTensor::real(
        format!("{}.output", request.id),
        vec![n],
        output,
    )])
}

fn block_circulant_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 3)?;
    require_rank(&request.inputs[1], 1)?;
    let blocks = request.inputs[0].shape[0];
    let block_size = request.inputs[0].shape[1];
    if request.inputs[0].shape[2] != block_size
        || request.attributes.block_size != block_size
        || request.inputs[1].shape[0] != blocks * block_size
    {
        bail!("block-circulant input requires [blocks,b,b] generators and a blocks*b vector");
    }
    let generators = real_values(&request.inputs[0])?;
    let vector = real_values(&request.inputs[1])?;
    let mut output = vec![0.0; vector.len()];
    for output_block in 0..blocks {
        for input_block in 0..blocks {
            let generator_block = (input_block + blocks - output_block) % blocks;
            for row in 0..block_size {
                for column in 0..block_size {
                    output[output_block * block_size + row] += matrix_element(
                        &request.inputs[0],
                        generators,
                        generator_block,
                        row,
                        column,
                        false,
                    )? * vector
                        [input_block * block_size + column];
                }
            }
        }
    }
    Ok(vec![KernelTensor::real(
        format!("{}.output", request.id),
        vec![vector.len()],
        output,
    )])
}

fn reservoir_outputs(request: &KernelRequest) -> Result<Vec<KernelTensor>> {
    require_rank(&request.inputs[0], 1)?;
    require_rank(&request.inputs[1], 2)?;
    require_rank(&request.inputs[2], 2)?;
    let state = real_values(&request.inputs[0])?;
    let recurrent = real_values(&request.inputs[1])?;
    let input_matrix = real_values(&request.inputs[2])?;
    let size = state.len();
    if request.inputs[1].shape != [size, size] || request.inputs[2].shape[0] != size {
        bail!("reservoir step requires state[n], recurrent[n,n], and input_matrix[n,m]");
    }
    let external = request.attributes.scale;
    let mut output = vec![0.0; size];
    for row in 0..size {
        let mut activation =
            matrix_element(&request.inputs[2], input_matrix, 0, row, 0, false)? * external;
        for (column, state_value) in state.iter().enumerate() {
            activation +=
                matrix_element(&request.inputs[1], recurrent, 0, row, column, false)? * state_value;
        }
        output[row] = (1.0 - request.attributes.leakage) * state[row]
            + request.attributes.leakage * activation.tanh();
    }
    Ok(vec![KernelTensor::real(
        format!("{}.output", request.id),
        vec![size],
        output,
    )])
}

fn dft(input: &[ComplexValue], inverse: bool, convention: PhaseConvention) -> Vec<ComplexValue> {
    let n = input.len();
    let convention_sign = match convention {
        PhaseConvention::NegativeExponent => -1.0,
        PhaseConvention::PositiveExponent => 1.0,
    };
    let sign = if inverse {
        -convention_sign
    } else {
        convention_sign
    };
    (0..n)
        .map(|frequency| {
            let mut value = ComplexValue::ZERO;
            for (sample, input) in input.iter().enumerate() {
                let angle = sign * 2.0 * PI * frequency as f64 * sample as f64 / n as f64;
                value = value.add(input.multiply(ComplexValue::new(angle.cos(), angle.sin())));
            }
            if inverse {
                value.scale(1.0 / n as f64)
            } else {
                value
            }
        })
        .collect()
}

fn fft(input: &[ComplexValue], inverse: bool, convention: PhaseConvention) -> Vec<ComplexValue> {
    if !input.len().is_power_of_two() {
        return dft(input, inverse, convention);
    }
    if input.len() == 1 {
        return input.to_vec();
    }
    let mut output = input.to_vec();
    let bits = input.len().trailing_zeros();
    for index in 0..input.len() {
        let reversed = index.reverse_bits() >> (usize::BITS - bits);
        if reversed > index {
            output.swap(index, reversed);
        }
    }
    let convention_sign = match convention {
        PhaseConvention::NegativeExponent => -1.0,
        PhaseConvention::PositiveExponent => 1.0,
    };
    let sign = if inverse {
        -convention_sign
    } else {
        convention_sign
    };
    let mut width = 2;
    while width <= input.len() {
        let angle = sign * 2.0 * PI / width as f64;
        let root = ComplexValue::new(angle.cos(), angle.sin());
        for start in (0..input.len()).step_by(width) {
            let mut twiddle = ComplexValue::new(1.0, 0.0);
            for offset in 0..width / 2 {
                let even = output[start + offset];
                let odd = output[start + offset + width / 2].multiply(twiddle);
                output[start + offset] = even.add(odd);
                output[start + offset + width / 2] = even.add(odd.scale(-1.0));
                twiddle = twiddle.multiply(root);
            }
        }
        width *= 2;
    }
    if inverse {
        for value in &mut output {
            *value = value.scale(1.0 / input.len() as f64);
        }
    }
    output
}

fn kernel_result(
    request: &KernelRequest,
    descriptor: KernelDescriptor,
    outputs: Vec<KernelTensor>,
    execution_target: TargetBackend,
    simulated: bool,
) -> Result<KernelResult> {
    let fingerprint = format!(
        "fnv1a64:{:016x}",
        stable_fingerprint_bytes(&serde_json::to_vec(&(
            request,
            execution_target,
            simulated,
            &outputs
        ))?)
    );
    Ok(KernelResult {
        version: AWENBLAS_VERSION.to_string(),
        request_id: request.id.clone(),
        kind: request.kind,
        outputs,
        descriptor,
        execution_target,
        simulated,
        execution_fingerprint: fingerprint,
    })
}

fn ineligibility_reason(
    request: &KernelRequest,
    descriptor: &KernelDescriptor,
    profile: &KernelBackendProfile,
) -> Option<String> {
    if !profile.supported_kinds.contains(&request.kind) {
        return Some("kernel kind is unsupported".to_string());
    }
    if !profile.supported_structures.contains(&descriptor.structure) {
        return Some(
            "kernel structure is unsupported and must not be densified silently".to_string(),
        );
    }
    if descriptor
        .dtypes
        .iter()
        .any(|dtype| !profile.supported_dtypes.contains(dtype))
    {
        return Some("one or more input dtypes are unsupported".to_string());
    }
    if request.kind.is_complex() && !profile.supports_complex {
        return Some("backend does not implement explicit complex phase semantics".to_string());
    }
    if request
        .inputs
        .iter()
        .any(|input| input.data.len() > profile.maximum_tensor_elements)
        || descriptor
            .output_shapes
            .iter()
            .any(|shape| shape.iter().copied().product::<usize>() > profile.maximum_tensor_elements)
    {
        return Some("an input or output tensor exceeds backend capacity".to_string());
    }
    if descriptor
        .minimum_effective_bits
        .is_some_and(|bits| profile.effective_bits < bits)
    {
        return Some("backend effective precision is below the numerical contract".to_string());
    }
    if profile.estimated_error_fraction
        > request
            .accuracy
            .max_abs_error
            .max(request.accuracy.max_rel_error)
    {
        return Some("backend error estimate exceeds the numerical contract".to_string());
    }
    if profile.requires_calibration && request.calibration_inputs.is_empty() {
        return Some("backend requires a calibration input".to_string());
    }
    None
}

fn estimate_kernel_cost(
    descriptor: &KernelDescriptor,
    profile: &KernelBackendProfile,
) -> KernelCostEstimate {
    let latency_ns =
        profile.launch_latency_ns + descriptor.operations / (profile.throughput_tops * 1_000.0);
    let energy_uj = descriptor.operations * profile.energy_pj_per_operation / 1_000_000.0;
    KernelCostEstimate {
        latency_ns,
        energy_uj,
        error_fraction: profile.estimated_error_fraction,
        throughput_gops: descriptor.operations / latency_ns.max(f64::EPSILON),
        source: profile.source,
    }
}

fn kernel_objective_score(objective: OptimizationObjective, estimate: &KernelCostEstimate) -> f64 {
    match objective {
        OptimizationObjective::Latency => estimate.latency_ns,
        OptimizationObjective::Energy => estimate.energy_uj,
        OptimizationObjective::Accuracy => estimate.error_fraction,
        OptimizationObjective::Throughput => -estimate.throughput_gops,
    }
}

fn estimate_operations(request: &KernelRequest, outputs: &[KernelTensor]) -> f64 {
    let output_elements = outputs
        .iter()
        .map(|output| output.data.len() as f64)
        .sum::<f64>();
    match request.kind {
        KernelKind::Gemm
        | KernelKind::BatchedGemm
        | KernelKind::ComplexGemm
        | KernelKind::Linear
        | KernelKind::AttentionScores
        | KernelKind::AttentionValue
        | KernelKind::MlpProjection
        | KernelKind::Beamforming
        | KernelKind::Propagation => {
            let inner = request.inputs[0].shape.last().copied().unwrap_or(1) as f64;
            2.0 * output_elements * inner
        }
        KernelKind::TransformerQkv => {
            6.0 * output_elements / 3.0 * request.inputs[0].shape[1] as f64
        }
        KernelKind::Dft | KernelKind::Fft | KernelKind::FourierFilter => {
            8.0 * output_elements * output_elements.max(1.0).log2().max(1.0)
        }
        KernelKind::LowRankGemm => {
            let rank = request.inputs[1].shape[1] as f64;
            2.0 * rank * (request.inputs[0].data.len() + output_elements as usize) as f64
        }
        KernelKind::Convolution1d | KernelKind::Correlation1d | KernelKind::RfFir => {
            2.0 * output_elements * request.inputs[1].data.len() as f64
        }
        KernelKind::RandomProjection => 2.0 * output_elements * request.inputs[0].shape[1] as f64,
        KernelKind::Toeplitz | KernelKind::Circulant | KernelKind::BlockCirculant => {
            2.0 * output_elements * output_elements
        }
        KernelKind::ReservoirStep => 2.0 * output_elements * output_elements + output_elements,
    }
}

fn matrix_element(
    tensor: &KernelTensor,
    values: &[f64],
    batch: usize,
    row: usize,
    column: usize,
    transpose: bool,
) -> Result<f64> {
    let rows = tensor.shape[tensor.shape.len() - 2];
    let columns = tensor.shape[tensor.shape.len() - 1];
    let (physical_row, physical_column) = if transpose {
        (column, row)
    } else {
        (row, column)
    };
    if physical_row >= rows || physical_column >= columns {
        bail!("matrix index exceeds tensor '{}' shape", tensor.id);
    }
    let matrix_size = rows * columns;
    let local = match tensor.layout {
        Layout::RowMajor => physical_row * columns + physical_column,
        Layout::ColumnMajor => physical_column * rows + physical_row,
    };
    values
        .get(batch * matrix_size + local)
        .copied()
        .context("matrix batch index exceeds tensor data")
}

fn complex_matrix_element(
    tensor: &KernelTensor,
    values: &[ComplexValue],
    row: usize,
    column: usize,
    transpose: bool,
) -> Result<ComplexValue> {
    let rows = tensor.shape[0];
    let columns = tensor.shape[1];
    let (physical_row, physical_column) = if transpose {
        (column, row)
    } else {
        (row, column)
    };
    if physical_row >= rows || physical_column >= columns {
        bail!("complex matrix index exceeds tensor '{}' shape", tensor.id);
    }
    let index = match tensor.layout {
        Layout::RowMajor => physical_row * columns + physical_column,
        Layout::ColumnMajor => physical_column * rows + physical_row,
    };
    Ok(values[index])
}

fn real_values(tensor: &KernelTensor) -> Result<&[f64]> {
    match &tensor.data {
        KernelData::Real(values) => Ok(values),
        KernelData::Complex(_) => bail!("kernel tensor '{}' must contain real data", tensor.id),
    }
}

fn complex_values(tensor: &KernelTensor) -> Result<&[ComplexValue]> {
    match &tensor.data {
        KernelData::Complex(values) => Ok(values),
        KernelData::Real(_) => bail!("kernel tensor '{}' must contain complex data", tensor.id),
    }
}

fn require_rank(tensor: &KernelTensor, rank: usize) -> Result<()> {
    if tensor.shape.len() != rank {
        bail!(
            "kernel tensor '{}' requires rank {rank}, got {:?}",
            tensor.id,
            tensor.shape
        );
    }
    Ok(())
}

fn element_count(shape: &[usize]) -> Result<usize> {
    shape
        .iter()
        .try_fold(1_usize, |total, dimension| total.checked_mul(*dimension))
        .context("kernel tensor element count overflows usize")
}

fn quantize_data(data: &mut KernelData, bits: u8) {
    let levels = 2.0_f64.powi(i32::from(bits.min(24))) - 1.0;
    match data {
        KernelData::Real(values) => quantize_components(values, levels),
        KernelData::Complex(values) => {
            let maximum = values
                .iter()
                .map(|value| value.real.abs().max(value.imaginary.abs()))
                .fold(0.0_f64, f64::max);
            if maximum > 0.0 {
                for value in values {
                    value.real = (value.real / maximum * levels).round() / levels * maximum;
                    value.imaginary =
                        (value.imaginary / maximum * levels).round() / levels * maximum;
                }
            }
        }
    }
}

fn quantize_components(values: &mut [f64], levels: f64) {
    let maximum = values
        .iter()
        .map(|value| value.abs())
        .fold(0.0_f64, f64::max);
    if maximum > 0.0 {
        for value in values {
            *value = (*value / maximum * levels).round() / levels * maximum;
        }
    }
}

fn apply_simulation_effects(
    data: &mut KernelData,
    calibrations: &[CalibrationInput],
    noise_fraction: f64,
    state: &mut u64,
) {
    let (gain, bias) = calibrations.iter().fold((1.0, 0.0), |(gain, bias), input| {
        (gain * input.gain, bias * input.gain + input.bias)
    });
    match data {
        KernelData::Real(values) => {
            let maximum = values
                .iter()
                .map(|value| value.abs())
                .fold(0.0_f64, f64::max);
            for value in values {
                let noise = (next_unit(state) * 2.0 - 1.0) * maximum * noise_fraction;
                let measured = *value * gain + bias + noise;
                *value = (measured - bias) / gain;
            }
        }
        KernelData::Complex(values) => {
            let maximum = values
                .iter()
                .map(|value| value.magnitude())
                .fold(0.0_f64, f64::max);
            for value in values {
                let real_noise = (next_unit(state) * 2.0 - 1.0) * maximum * noise_fraction;
                let imaginary_noise = (next_unit(state) * 2.0 - 1.0) * maximum * noise_fraction;
                let measured_real = value.real * gain + bias + real_noise;
                let measured_imaginary = value.imaginary * gain + imaginary_noise;
                value.real = (measured_real - bias) / gain;
                value.imaginary = measured_imaginary / gain;
            }
        }
    }
}

fn next_unit(state: &mut u64) -> f64 {
    *state = state
        .wrapping_mul(6_364_136_223_846_793_005)
        .wrapping_add(1_442_695_040_888_963_407);
    ((*state >> 11) as f64) / ((1_u64 << 53) as f64)
}

fn compare_outputs(expected: &[KernelTensor], actual: &[KernelTensor]) -> Result<(f64, f64)> {
    if expected.len() != actual.len() {
        bail!("kernel output counts differ");
    }
    let mut maximum_absolute = 0.0_f64;
    let mut maximum_relative = 0.0_f64;
    for (expected, actual) in expected.iter().zip(actual) {
        if expected.shape != actual.shape {
            bail!("kernel output shapes differ");
        }
        match (&expected.data, &actual.data) {
            (KernelData::Real(expected), KernelData::Real(actual)) => {
                for (expected, actual) in expected.iter().zip(actual) {
                    let difference = (expected - actual).abs();
                    maximum_absolute = maximum_absolute.max(difference);
                    maximum_relative =
                        maximum_relative.max(difference / expected.abs().max(f64::EPSILON));
                }
            }
            (KernelData::Complex(expected), KernelData::Complex(actual)) => {
                for (expected, actual) in expected.iter().zip(actual) {
                    let difference = ComplexValue::new(
                        expected.real - actual.real,
                        expected.imaginary - actual.imaginary,
                    )
                    .magnitude();
                    maximum_absolute = maximum_absolute.max(difference);
                    maximum_relative =
                        maximum_relative.max(difference / expected.magnitude().max(f64::EPSILON));
                }
            }
            _ => bail!("kernel output representations differ"),
        }
    }
    Ok((maximum_absolute, maximum_relative))
}

fn all_kernel_kinds() -> BTreeSet<KernelKind> {
    [
        KernelKind::Gemm,
        KernelKind::BatchedGemm,
        KernelKind::ComplexGemm,
        KernelKind::Linear,
        KernelKind::TransformerQkv,
        KernelKind::AttentionScores,
        KernelKind::AttentionValue,
        KernelKind::MlpProjection,
        KernelKind::Convolution1d,
        KernelKind::Correlation1d,
        KernelKind::Dft,
        KernelKind::Fft,
        KernelKind::FourierFilter,
        KernelKind::LowRankGemm,
        KernelKind::RandomProjection,
        KernelKind::Toeplitz,
        KernelKind::Circulant,
        KernelKind::BlockCirculant,
        KernelKind::Beamforming,
        KernelKind::RfFir,
        KernelKind::ReservoirStep,
        KernelKind::Propagation,
    ]
    .into_iter()
    .collect()
}

fn all_kernel_structures() -> BTreeSet<KernelStructure> {
    [
        KernelStructure::Dense,
        KernelStructure::LowRank,
        KernelStructure::RandomProjection,
        KernelStructure::Toeplitz,
        KernelStructure::Circulant,
        KernelStructure::BlockCirculant,
        KernelStructure::Convolutional,
        KernelStructure::Fourier,
        KernelStructure::Beamforming,
        KernelStructure::Reservoir,
        KernelStructure::Propagation,
    ]
    .into_iter()
    .collect()
}
