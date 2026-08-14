//! Independent classical- and quantum-photonic runtime contracts.

use anyhow::{bail, Context, Result};
use jsonschema::JSONSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::{BTreeMap, BTreeSet};

pub const CLASSICAL_PROGRAM_VERSION: &str = "awen.photonic.program.v1";
pub const QUANTUM_PROGRAM_VERSION: &str = "awen.qphotonic.program.v1";
pub const QUANTUM_RESULT_VERSION: &str = "awen.qphotonic.result.v1";
pub const INTEROP_PROGRAM_VERSION: &str = "awen.photonic-interop.v1";
pub const V5_MIGRATION_VERSION: &str = "awen.photonic-v5-migration.v1";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "dialect", content = "program", rename_all = "snake_case")]
pub enum PhotonicProgram {
    Classical(Box<ClassicalProgram>),
    Quantum(Box<QuantumProgram>),
    Interop(Box<InteropProgram>),
}

impl PhotonicProgram {
    pub fn validate(&self) -> Result<()> {
        match self {
            Self::Classical(program) => program.validate(),
            Self::Quantum(program) => program.validate(),
            Self::Interop(program) => program.validate(),
        }
    }

    pub fn dialect_name(&self) -> &'static str {
        match self {
            Self::Classical(_) => "awen.photonic",
            Self::Quantum(_) => "awen.qphotonic",
            Self::Interop(_) => "awen.photonic-interop",
        }
    }

    pub fn program_id(&self) -> &str {
        match self {
            Self::Classical(program) => &program.program_id,
            Self::Quantum(program) => &program.program_id,
            Self::Interop(program) => &program.program_id,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum ClassicalOperationFamily {
    Gemm,
    AnalogTransform,
    Modulate,
    Detect,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClassicalEncoding {
    RealAmplitude,
    ComplexField,
    Intensity,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClassicalSignal {
    pub id: String,
    pub shape: Vec<usize>,
    pub encoding: ClassicalEncoding,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClassicalPrecisionContract {
    pub input_bits: u8,
    pub optical_effective_bits: u8,
    pub dac_bits: u8,
    pub adc_bits: u8,
    pub accumulator_bits: u8,
    pub maximum_absolute_error: f64,
    pub maximum_relative_error: f64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClassicalNoiseKind {
    Shot,
    Thermal,
    Phase,
    Detector,
    Composite,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClassicalNoiseContract {
    pub kind: ClassicalNoiseKind,
    pub rms_fraction: f64,
    pub seed: u64,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TransferModel {
    Affine,
    Matrix,
    DetectorResponse,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CalibratedTransferFunction {
    pub model: TransferModel,
    pub coefficients: Vec<f64>,
    pub calibration_snapshot_id: String,
    pub calibration_fingerprint: String,
    pub maximum_residual_error: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationTiming {
    pub start_ns: u64,
    pub duration_ns: u64,
    pub dependencies: Vec<String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AnalogTransformKind {
    Splitter,
    PhaseShift,
    Interferometer,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModulationKind {
    Amplitude,
    Phase,
    InPhaseQuadrature,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DetectionKind {
    Direct,
    Homodyne,
    Heterodyne,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum ClassicalOperationKind {
    Gemm {
        m: usize,
        n: usize,
        k: usize,
        transpose_lhs: bool,
        transpose_rhs: bool,
    },
    AnalogTransform {
        transform: AnalogTransformKind,
        phase_radians: Option<f64>,
        power_ratio: Option<f64>,
    },
    Modulate {
        modulation: ModulationKind,
        carrier_wavelength_nm: f64,
    },
    Detect {
        detection: DetectionKind,
        integration_time_ns: u64,
    },
}

impl ClassicalOperationKind {
    fn family(&self) -> ClassicalOperationFamily {
        match self {
            Self::Gemm { .. } => ClassicalOperationFamily::Gemm,
            Self::AnalogTransform { .. } => ClassicalOperationFamily::AnalogTransform,
            Self::Modulate { .. } => ClassicalOperationFamily::Modulate,
            Self::Detect { .. } => ClassicalOperationFamily::Detect,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct ClassicalOperation {
    pub op_id: String,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(flatten)]
    pub kind: ClassicalOperationKind,
    pub precision: ClassicalPrecisionContract,
    pub noise: ClassicalNoiseContract,
    pub transfer: CalibratedTransferFunction,
    pub timing: OperationTiming,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClassicalCapabilityRequirements {
    pub operations: BTreeSet<ClassicalOperationFamily>,
    pub minimum_optical_bits: u8,
    pub minimum_dac_bits: u8,
    pub minimum_adc_bits: u8,
    pub calibrated_transfer_required: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct ClassicalProgram {
    pub version: String,
    pub program_id: String,
    pub signals: Vec<ClassicalSignal>,
    pub operations: Vec<ClassicalOperation>,
    pub outputs: Vec<String>,
    pub capabilities: ClassicalCapabilityRequirements,
}

impl ClassicalProgram {
    pub fn validate(&self) -> Result<()> {
        if self.version != CLASSICAL_PROGRAM_VERSION {
            bail!("unsupported classical photonic program version");
        }
        identifier(&self.program_id, "classical program id")?;
        if self.signals.is_empty() || self.operations.is_empty() || self.outputs.is_empty() {
            bail!("classical programs require signals, operations, and outputs");
        }
        if self.outputs.iter().collect::<BTreeSet<_>>().len() != self.outputs.len() {
            bail!("classical program outputs must be unique");
        }
        if !self.capabilities.calibrated_transfer_required
            || self.capabilities.operations.is_empty()
            || self.capabilities.minimum_optical_bits == 0
            || self.capabilities.minimum_dac_bits == 0
            || self.capabilities.minimum_adc_bits == 0
        {
            bail!("classical capability requirements are incomplete");
        }
        let mut signal_ids = BTreeSet::new();
        for signal in &self.signals {
            identifier(&signal.id, "classical signal id")?;
            if signal.shape.is_empty()
                || signal.shape.contains(&0)
                || !signal_ids.insert(&signal.id)
            {
                bail!("classical signals require unique ids and non-zero shapes");
            }
        }
        let internal_signals = self
            .operations
            .iter()
            .flat_map(|operation| operation.outputs.iter())
            .collect::<BTreeSet<_>>();
        let output_reference_count = self
            .operations
            .iter()
            .map(|operation| operation.outputs.len())
            .sum::<usize>();
        if internal_signals.len() != output_reference_count {
            bail!("classical signals may be produced by only one operation");
        }
        let mut available_signals = signal_ids
            .iter()
            .filter(|signal| !internal_signals.contains(*signal))
            .copied()
            .collect::<BTreeSet<_>>();
        let mut operation_ids = BTreeSet::new();
        let mut operation_end_times = BTreeMap::new();
        for operation in &self.operations {
            identifier(&operation.op_id, "classical operation id")?;
            if !operation_ids.insert(&operation.op_id)
                || operation.inputs.is_empty()
                || operation.outputs.is_empty()
                || operation
                    .inputs
                    .iter()
                    .any(|id| !available_signals.contains(id))
                || operation.outputs.iter().any(|id| !signal_ids.contains(id))
            {
                bail!(
                    "classical operation ids and dataflow references must be complete, ordered, and unique"
                );
            }
            validate_classical_operation(operation, &self.signals)?;
            if !self
                .capabilities
                .operations
                .contains(&operation.kind.family())
                || operation.precision.optical_effective_bits
                    < self.capabilities.minimum_optical_bits
                || operation.precision.dac_bits < self.capabilities.minimum_dac_bits
                || operation.precision.adc_bits < self.capabilities.minimum_adc_bits
            {
                bail!("classical operation does not satisfy its declared capability requirements");
            }
            for dependency in &operation.timing.dependencies {
                if operation
                    .timing
                    .dependencies
                    .iter()
                    .collect::<BTreeSet<_>>()
                    .len()
                    != operation.timing.dependencies.len()
                {
                    bail!("classical timing dependencies must be unique");
                }
                if dependency == &operation.op_id || !operation_end_times.contains_key(dependency) {
                    bail!("classical timing dependencies must refer to preceding operations");
                }
                if operation_end_times
                    .get(dependency)
                    .is_some_and(|end_ns| *end_ns > operation.timing.start_ns)
                {
                    bail!("classical operations must start after every declared dependency");
                }
            }
            let end_ns = operation
                .timing
                .start_ns
                .checked_add(operation.timing.duration_ns)
                .context("classical operation timing overflow")?;
            operation_end_times.insert(&operation.op_id, end_ns);
            available_signals.extend(operation.outputs.iter());
        }
        if self
            .outputs
            .iter()
            .any(|id| !available_signals.contains(id))
        {
            bail!("classical program outputs must refer to available produced or input signals");
        }
        Ok(())
    }
}

fn validate_classical_operation(
    operation: &ClassicalOperation,
    signals: &[ClassicalSignal],
) -> Result<()> {
    let precision = &operation.precision;
    if [
        precision.input_bits,
        precision.optical_effective_bits,
        precision.dac_bits,
        precision.adc_bits,
        precision.accumulator_bits,
    ]
    .contains(&0)
        || precision.accumulator_bits < precision.optical_effective_bits
        || !finite_non_negative(precision.maximum_absolute_error)
        || !finite_non_negative(precision.maximum_relative_error)
    {
        bail!("classical precision contract is invalid");
    }
    if !finite_non_negative(operation.noise.rms_fraction)
        || operation.transfer.coefficients.is_empty()
        || operation
            .transfer
            .coefficients
            .iter()
            .any(|value| !value.is_finite())
        || !finite_non_negative(operation.transfer.maximum_residual_error)
        || !valid_fingerprint(&operation.transfer.calibration_fingerprint)
        || operation.transfer.calibration_snapshot_id.trim().is_empty()
        || operation.timing.duration_ns == 0
    {
        bail!("classical noise, transfer, calibration, or timing contract is invalid");
    }
    match &operation.kind {
        ClassicalOperationKind::Gemm {
            m,
            n,
            k,
            transpose_lhs,
            transpose_rhs,
        } => {
            if *m == 0
                || *n == 0
                || *k == 0
                || operation.inputs.len() != 2
                || operation.outputs.len() != 1
                || operation.transfer.model != TransferModel::Matrix
            {
                bail!("classical GEMM requires two inputs and non-zero dimensions");
            }
            let lhs = classical_signal(signals, &operation.inputs[0]);
            let rhs = classical_signal(signals, &operation.inputs[1]);
            let output = classical_signal(signals, &operation.outputs[0]);
            if lhs.shape.len() != 2 || rhs.shape.len() != 2 || output.shape != [*m, *n] {
                bail!("classical GEMM signal shapes must match its M, N, and K contract");
            }
            let (lhs_m, lhs_k) = if *transpose_lhs {
                (lhs.shape[1], lhs.shape[0])
            } else {
                (lhs.shape[0], lhs.shape[1])
            };
            let (rhs_k, rhs_n) = if *transpose_rhs {
                (rhs.shape[1], rhs.shape[0])
            } else {
                (rhs.shape[0], rhs.shape[1])
            };
            if (lhs_m, lhs_k, rhs_k, rhs_n) != (*m, *k, *k, *n) {
                bail!("classical GEMM signal shapes must match its M, N, and K contract");
            }
        }
        ClassicalOperationKind::AnalogTransform {
            transform,
            phase_radians,
            power_ratio,
        } => match transform {
            AnalogTransformKind::Splitter => {
                if operation.inputs.len() != 1
                    || operation.outputs.len() != 2
                    || !operation_shapes_match(signals, operation)
                    || !power_ratio
                        .is_some_and(|value| value.is_finite() && (0.0..=1.0).contains(&value))
                    || phase_radians.is_some()
                {
                    bail!("classical splitter requires two outputs and a [0,1] power ratio");
                }
            }
            AnalogTransformKind::PhaseShift => {
                if operation.inputs.len() != 1
                    || operation.outputs.len() != 1
                    || !operation_shapes_match(signals, operation)
                    || !phase_radians.is_some_and(f64::is_finite)
                    || power_ratio.is_some()
                {
                    bail!("classical phase shift requires a finite phase only");
                }
            }
            AnalogTransformKind::Interferometer => {
                if operation.inputs.len() != 2
                    || operation.outputs.len() != 2
                    || !operation_shapes_match(signals, operation)
                    || phase_radians.is_some_and(|value| !value.is_finite())
                    || power_ratio.is_some_and(|value| !value.is_finite())
                {
                    bail!("classical interferometer parameters must be finite");
                }
            }
        },
        ClassicalOperationKind::Modulate {
            carrier_wavelength_nm,
            ..
        } => {
            if !carrier_wavelength_nm.is_finite()
                || *carrier_wavelength_nm <= 0.0
                || operation.inputs.len() != 1
                || operation.outputs.len() != 1
                || !operation_shapes_match(signals, operation)
            {
                bail!("classical modulation requires a positive carrier wavelength");
            }
        }
        ClassicalOperationKind::Detect {
            integration_time_ns,
            ..
        } => {
            if *integration_time_ns == 0
                || operation.inputs.len() != 1
                || operation.outputs.len() != 1
                || !operation_shapes_match(signals, operation)
                || operation.transfer.model != TransferModel::DetectorResponse
            {
                bail!("classical detection requires a positive integration time");
            }
        }
    }
    Ok(())
}

fn classical_signal<'a>(signals: &'a [ClassicalSignal], id: &str) -> &'a ClassicalSignal {
    signals
        .iter()
        .find(|signal| signal.id == id)
        .expect("classical signal existence checked before operation validation")
}

fn operation_shapes_match(signals: &[ClassicalSignal], operation: &ClassicalOperation) -> bool {
    let Some(reference) = operation.inputs.first() else {
        return false;
    };
    let reference_shape = &classical_signal(signals, reference).shape;
    operation
        .inputs
        .iter()
        .chain(&operation.outputs)
        .all(|id| &classical_signal(signals, id).shape == reference_shape)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum QuantumStateSpace {
    Fock,
    GaussianCv,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "state_space", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuantumModeSpace {
    Fock { cutoff: usize },
    GaussianCv,
}

impl QuantumModeSpace {
    fn family(&self) -> QuantumStateSpace {
        match self {
            Self::Fock { .. } => QuantumStateSpace::Fock,
            Self::GaussianCv => QuantumStateSpace::GaussianCv,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantumMode {
    pub id: String,
    #[serde(flatten)]
    pub space: QuantumModeSpace,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "representation", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuantumInitialState {
    Fock {
        occupations: Vec<usize>,
    },
    Gaussian {
        displacement: Vec<f64>,
        covariance: Vec<Vec<f64>>,
    },
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum QuantumGateFamily {
    BeamSplitter,
    PhaseShift,
    Squeeze,
    Displace,
    ControlledX,
    Fourier,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "gate", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuantumGate {
    BeamSplitter {
        theta_radians: f64,
        phi_radians: f64,
    },
    PhaseShift {
        radians: f64,
    },
    Squeeze {
        magnitude: f64,
        angle_radians: f64,
    },
    Displace {
        q: f64,
        p: f64,
    },
    ControlledX {
        dimension: usize,
    },
    Fourier,
}

impl QuantumGate {
    fn family(&self) -> QuantumGateFamily {
        match self {
            Self::BeamSplitter { .. } => QuantumGateFamily::BeamSplitter,
            Self::PhaseShift { .. } => QuantumGateFamily::PhaseShift,
            Self::Squeeze { .. } => QuantumGateFamily::Squeeze,
            Self::Displace { .. } => QuantumGateFamily::Displace,
            Self::ControlledX { .. } => QuantumGateFamily::ControlledX,
            Self::Fourier => QuantumGateFamily::Fourier,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum QuantumMeasurementFamily {
    PhotonCounting,
    HomodyneQ,
    HomodyneP,
    Heterodyne,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum QuantumControlParameter {
    PhaseRadians,
    DisplacementQ,
    DisplacementP,
    SqueezingMagnitude,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum QuantumOperationKind {
    Gate {
        gate_spec: QuantumGate,
    },
    Measure {
        measurement_id: String,
        basis: QuantumMeasurementFamily,
        destructive: bool,
    },
    FeedForward {
        source_measurement_id: String,
        target_operation_id: String,
        parameter: QuantumControlParameter,
        scale: f64,
        offset: f64,
        maximum_latency_ns: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantumOperation {
    pub op_id: String,
    pub modes: Vec<String>,
    pub coherence_cost_ns: u64,
    #[serde(flatten)]
    pub kind: QuantumOperationKind,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuantumExecutionContract {
    pub shots: u64,
    pub seed: u64,
    pub deterministic_replay: bool,
    pub coherence_budget_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct StatisticalCorrectnessContract {
    pub expected_distribution: BTreeMap<String, f64>,
    pub expected_means: BTreeMap<String, f64>,
    pub maximum_total_variation_distance: f64,
    pub maximum_mean_error: f64,
    pub minimum_fidelity: f64,
    pub confidence_level: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuantumCapabilityRequirements {
    pub state_spaces: BTreeSet<QuantumStateSpace>,
    pub gates: BTreeSet<QuantumGateFamily>,
    pub measurements: BTreeSet<QuantumMeasurementFamily>,
    pub feed_forward: bool,
    pub minimum_modes: usize,
    pub maximum_fock_cutoff: Option<usize>,
    pub minimum_coherence_ns: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuantumProgram {
    pub version: String,
    pub program_id: String,
    pub modes: Vec<QuantumMode>,
    pub initial_state: QuantumInitialState,
    pub operations: Vec<QuantumOperation>,
    pub execution: QuantumExecutionContract,
    pub correctness: StatisticalCorrectnessContract,
    pub capabilities: QuantumCapabilityRequirements,
}

impl QuantumProgram {
    pub fn validate(&self) -> Result<()> {
        if self.version != QUANTUM_PROGRAM_VERSION {
            bail!("unsupported quantum-photonic program version");
        }
        identifier(&self.program_id, "quantum program id")?;
        if self.modes.is_empty() || self.operations.is_empty() {
            bail!("quantum programs require modes and operations");
        }
        let mut mode_ids = BTreeSet::new();
        let mut state_spaces = BTreeSet::new();
        for mode in &self.modes {
            identifier(&mode.id, "quantum mode id")?;
            if !mode_ids.insert(&mode.id) {
                bail!("quantum mode ids must be unique");
            }
            if let QuantumModeSpace::Fock { cutoff } = mode.space {
                if cutoff < 2 {
                    bail!("Fock modes require cutoff >= 2");
                }
                if self
                    .capabilities
                    .maximum_fock_cutoff
                    .is_some_and(|maximum| cutoff > maximum)
                {
                    bail!("Fock mode exceeds the declared capability cutoff");
                }
            }
            state_spaces.insert(mode.space.family());
        }
        validate_initial_state(&self.initial_state, &self.modes, &state_spaces)?;
        if state_spaces.contains(&QuantumStateSpace::Fock)
            != self.capabilities.maximum_fock_cutoff.is_some()
        {
            bail!("quantum Fock capabilities require an explicit maximum cutoff");
        }
        if self.execution.shots == 0
            || self.execution.coherence_budget_ns == 0
            || !self.execution.deterministic_replay
        {
            bail!("quantum execution requires shots, coherence, and deterministic replay");
        }
        validate_statistical_contract(&self.correctness)?;
        if self.capabilities.minimum_modes > self.modes.len()
            || self.capabilities.minimum_coherence_ns > self.execution.coherence_budget_ns
            || !state_spaces.is_subset(&self.capabilities.state_spaces)
        {
            bail!("quantum state does not satisfy its capability requirements");
        }
        let operation_map = self
            .operations
            .iter()
            .enumerate()
            .map(|(index, operation)| (operation.op_id.as_str(), (index, operation)))
            .collect::<BTreeMap<_, _>>();
        if operation_map.len() != self.operations.len() {
            bail!("quantum operation ids must be unique");
        }
        let mut measurements = BTreeMap::new();
        let mut destructively_measured_modes = BTreeSet::new();
        let mut consumed_coherence = 0_u64;
        for (operation_index, operation) in self.operations.iter().enumerate() {
            identifier(&operation.op_id, "quantum operation id")?;
            if operation.modes.is_empty()
                || operation.modes.iter().collect::<BTreeSet<_>>().len() != operation.modes.len()
                || operation.modes.iter().any(|mode| !mode_ids.contains(mode))
                || operation
                    .modes
                    .iter()
                    .any(|mode| destructively_measured_modes.contains(mode))
            {
                bail!("quantum operations require unique, declared, live modes");
            }
            consumed_coherence = consumed_coherence
                .checked_add(operation.coherence_cost_ns)
                .context("quantum coherence cost overflow")?;
            validate_quantum_operation(
                operation,
                &self.modes,
                &self.capabilities,
                &operation_map,
                &measurements,
                operation_index,
            )?;
            if let QuantumOperationKind::Measure {
                measurement_id,
                basis,
                destructive,
            } = &operation.kind
            {
                identifier(measurement_id, "quantum measurement id")?;
                if measurements.insert(measurement_id, *basis).is_some() {
                    bail!("quantum measurement ids must be unique");
                }
                if *destructive {
                    destructively_measured_modes.extend(operation.modes.iter());
                }
            }
        }
        if consumed_coherence > self.execution.coherence_budget_ns {
            bail!("quantum operations exceed the coherence budget");
        }
        if measurements.is_empty()
            || self
                .correctness
                .expected_means
                .keys()
                .any(|measurement| !measurements.contains_key(measurement))
        {
            bail!("quantum correctness evidence must reference declared measurements");
        }
        Ok(())
    }
}

fn validate_initial_state(
    state: &QuantumInitialState,
    modes: &[QuantumMode],
    state_spaces: &BTreeSet<QuantumStateSpace>,
) -> Result<()> {
    match state {
        QuantumInitialState::Fock { occupations } => {
            if state_spaces != &BTreeSet::from([QuantumStateSpace::Fock])
                || occupations.len() != modes.len()
                || occupations.iter().zip(modes).any(|(occupation, mode)| {
                    matches!(mode.space, QuantumModeSpace::Fock { cutoff } if *occupation >= cutoff)
                })
            {
                bail!("Fock initial state must match every Fock mode and cutoff");
            }
        }
        QuantumInitialState::Gaussian {
            displacement,
            covariance,
        } => {
            let dimensions = modes.len() * 2;
            if state_spaces != &BTreeSet::from([QuantumStateSpace::GaussianCv])
                || displacement.len() != dimensions
                || covariance.len() != dimensions
                || covariance.iter().any(|row| row.len() != dimensions)
                || displacement.iter().any(|value| !value.is_finite())
                || covariance.iter().flatten().any(|value| !value.is_finite())
            {
                bail!("Gaussian initial state requires finite 2N means and a 2N by 2N covariance");
            }
            for (row, values) in covariance.iter().enumerate() {
                if values[row] <= 0.0
                    || (0..dimensions).any(|column| {
                        (values[column] - covariance[column][row]).abs() > 1e-12
                    })
                {
                    bail!("Gaussian covariance must be symmetric with positive diagonal");
                }
            }
            if !positive_semidefinite(covariance) {
                bail!("Gaussian covariance must be positive semidefinite");
            }
        }
    }
    Ok(())
}

fn positive_semidefinite(matrix: &[Vec<f64>]) -> bool {
    const TOLERANCE: f64 = 1e-12;
    let dimensions = matrix.len();
    let mut factor = vec![vec![0.0; dimensions]; dimensions];
    for row in 0..dimensions {
        for column in 0..=row {
            let residual = matrix[row][column]
                - (0..column)
                    .map(|index| factor[row][index] * factor[column][index])
                    .sum::<f64>();
            if row == column {
                if residual < -TOLERANCE {
                    return false;
                }
                factor[row][column] = residual.max(0.0).sqrt();
            } else if factor[column][column] > TOLERANCE {
                factor[row][column] = residual / factor[column][column];
            } else if residual.abs() > TOLERANCE {
                return false;
            }
        }
    }
    true
}

fn validate_statistical_contract(contract: &StatisticalCorrectnessContract) -> Result<()> {
    let probability_sum = contract.expected_distribution.values().sum::<f64>();
    if contract.expected_distribution.is_empty()
        || contract
            .expected_distribution
            .iter()
            .any(|(outcome, probability)| {
                outcome.trim().is_empty() || !finite_non_negative(*probability)
            })
        || (probability_sum - 1.0).abs() > 1e-12
        || contract
            .expected_means
            .iter()
            .any(|(name, value)| name.trim().is_empty() || !value.is_finite())
        || !unit_interval(contract.maximum_total_variation_distance)
        || !finite_non_negative(contract.maximum_mean_error)
        || !unit_interval(contract.minimum_fidelity)
        || !unit_interval(contract.confidence_level)
        || contract.confidence_level == 0.0
    {
        bail!("quantum statistical correctness contract is invalid");
    }
    Ok(())
}

fn validate_quantum_operation(
    operation: &QuantumOperation,
    modes: &[QuantumMode],
    capabilities: &QuantumCapabilityRequirements,
    operation_map: &BTreeMap<&str, (usize, &QuantumOperation)>,
    measurements: &BTreeMap<&String, QuantumMeasurementFamily>,
    operation_index: usize,
) -> Result<()> {
    let selected_spaces = operation
        .modes
        .iter()
        .map(|id| {
            modes
                .iter()
                .find(|mode| mode.id == *id)
                .expect("mode existence checked")
                .space
                .family()
        })
        .collect::<BTreeSet<_>>();
    match &operation.kind {
        QuantumOperationKind::Gate { gate_spec } => {
            validate_gate(gate_spec, &selected_spaces, operation.modes.len())?;
            if let QuantumGate::ControlledX { dimension } = gate_spec {
                let cutoffs_satisfy_dimension = operation.modes.iter().all(|id| {
                    modes.iter().find(|mode| mode.id == *id).is_some_and(|mode| {
                        matches!(mode.space, QuantumModeSpace::Fock { cutoff } if cutoff >= *dimension)
                    })
                });
                if !cutoffs_satisfy_dimension {
                    bail!("controlled-X dimension exceeds a selected Fock cutoff");
                }
            }
            if !capabilities.gates.contains(&gate_spec.family()) {
                bail!("quantum gate is absent from capability requirements");
            }
        }
        QuantumOperationKind::Measure { basis, .. } => {
            let compatible = match basis {
                QuantumMeasurementFamily::PhotonCounting => {
                    selected_spaces == BTreeSet::from([QuantumStateSpace::Fock])
                }
                QuantumMeasurementFamily::HomodyneQ
                | QuantumMeasurementFamily::HomodyneP
                | QuantumMeasurementFamily::Heterodyne => {
                    selected_spaces == BTreeSet::from([QuantumStateSpace::GaussianCv])
                }
            };
            if !compatible || !capabilities.measurements.contains(basis) {
                bail!(
                    "quantum measurement is incompatible with its mode state space or capabilities"
                );
            }
        }
        QuantumOperationKind::FeedForward {
            source_measurement_id,
            target_operation_id,
            parameter,
            scale,
            offset,
            maximum_latency_ns,
        } => {
            if !capabilities.feed_forward
                || !measurements.contains_key(source_measurement_id)
                || !operation_map.contains_key(target_operation_id.as_str())
                || target_operation_id == &operation.op_id
                || !scale.is_finite()
                || !offset.is_finite()
                || *maximum_latency_ns == 0
            {
                bail!(
                    "quantum feed-forward contract is invalid or references unavailable evidence"
                );
            }
            let (target_index, target_operation) = operation_map
                .get(target_operation_id.as_str())
                .context("feed-forward target operation")?;
            let QuantumOperationKind::Gate { gate_spec } = &target_operation.kind else {
                bail!("quantum feed-forward must target a gate operation");
            };
            let compatible_parameter = matches!(
                (parameter, gate_spec),
                (
                    QuantumControlParameter::PhaseRadians,
                    QuantumGate::PhaseShift { .. } | QuantumGate::BeamSplitter { .. }
                ) | (
                    QuantumControlParameter::DisplacementQ | QuantumControlParameter::DisplacementP,
                    QuantumGate::Displace { .. }
                ) | (
                    QuantumControlParameter::SqueezingMagnitude,
                    QuantumGate::Squeeze { .. }
                )
            );
            if *target_index <= operation_index
                || !compatible_parameter
                || operation.modes != target_operation.modes
            {
                bail!(
                    "quantum feed-forward must target the same modes on a later compatible gate parameter"
                );
            }
        }
    }
    Ok(())
}

fn validate_gate(
    gate: &QuantumGate,
    spaces: &BTreeSet<QuantumStateSpace>,
    mode_count: usize,
) -> Result<()> {
    let finite = |values: &[f64]| values.iter().all(|value| value.is_finite());
    match gate {
        QuantumGate::BeamSplitter {
            theta_radians,
            phi_radians,
        } if mode_count == 2 && spaces.len() == 1 && finite(&[*theta_radians, *phi_radians]) => {}
        QuantumGate::PhaseShift { radians } if mode_count == 1 && radians.is_finite() => {}
        QuantumGate::Squeeze {
            magnitude,
            angle_radians,
        } if mode_count == 1
            && spaces == &BTreeSet::from([QuantumStateSpace::GaussianCv])
            && finite(&[*magnitude, *angle_radians])
            && *magnitude >= 0.0 => {}
        QuantumGate::Displace { q, p }
            if mode_count == 1
                && spaces == &BTreeSet::from([QuantumStateSpace::GaussianCv])
                && finite(&[*q, *p]) => {}
        QuantumGate::ControlledX { dimension }
            if mode_count == 2
                && spaces == &BTreeSet::from([QuantumStateSpace::Fock])
                && *dimension >= 2 => {}
        QuantumGate::Fourier
            if mode_count == 1 && spaces == &BTreeSet::from([QuantumStateSpace::Fock]) => {}
        _ => bail!("quantum gate parameters, arity, or state space are invalid"),
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuantumResult {
    pub version: String,
    pub program_id: String,
    pub program_fingerprint: String,
    pub seed: u64,
    pub shots: u64,
    pub outcome_counts: BTreeMap<String, u64>,
    pub measured_means: BTreeMap<String, f64>,
    pub fidelity_estimate: f64,
    pub confidence_level: f64,
    pub coherence_elapsed_ns: u64,
    pub replay_fingerprint: String,
}

impl QuantumResult {
    pub fn seal_replay(&mut self, program: &QuantumProgram) -> Result<()> {
        if self.program_id != program.program_id
            || self.seed != program.execution.seed
            || self.shots != program.execution.shots
        {
            bail!("quantum result identity does not match the program execution contract");
        }
        self.program_fingerprint = quantum_program_fingerprint(program)?;
        self.replay_fingerprint = quantum_replay_fingerprint(self)?;
        Ok(())
    }

    pub fn validate_against(&self, program: &QuantumProgram) -> Result<()> {
        program.validate()?;
        let expected_program_fingerprint = quantum_program_fingerprint(program)?;
        if self.version != QUANTUM_RESULT_VERSION
            || self.program_id != program.program_id
            || self.program_fingerprint != expected_program_fingerprint
            || self.seed != program.execution.seed
            || self.shots != program.execution.shots
            || self
                .outcome_counts
                .values()
                .try_fold(0_u64, |sum, count| sum.checked_add(*count))
                != Some(self.shots)
            || self
                .outcome_counts
                .keys()
                .any(|outcome| outcome.trim().is_empty())
            || self
                .measured_means
                .iter()
                .any(|(name, value)| name.trim().is_empty() || !value.is_finite())
            || !unit_interval(self.fidelity_estimate)
            || !unit_interval(self.confidence_level)
            || self.coherence_elapsed_ns > program.execution.coherence_budget_ns
        {
            bail!("quantum result identity, sampling, or coherence evidence is invalid");
        }
        let total_variation_distance = program
            .correctness
            .expected_distribution
            .keys()
            .chain(self.outcome_counts.keys())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .map(|outcome| {
                let expected = program
                    .correctness
                    .expected_distribution
                    .get(outcome)
                    .copied()
                    .unwrap_or(0.0);
                let observed = self.outcome_counts.get(outcome).copied().unwrap_or(0) as f64
                    / self.shots as f64;
                (expected - observed).abs()
            })
            .sum::<f64>()
            * 0.5;
        if total_variation_distance
            > program.correctness.maximum_total_variation_distance + f64::EPSILON
            || self.fidelity_estimate + f64::EPSILON < program.correctness.minimum_fidelity
            || self.confidence_level + f64::EPSILON < program.correctness.confidence_level
        {
            bail!("quantum result fails its statistical distribution or fidelity contract");
        }
        for (name, expected) in &program.correctness.expected_means {
            let observed = self
                .measured_means
                .get(name)
                .with_context(|| format!("quantum result is missing expected mean '{name}'"))?;
            if (observed - expected).abs() > program.correctness.maximum_mean_error {
                bail!("quantum result fails mean correctness contract for '{name}'");
            }
        }
        let expected_fingerprint = quantum_replay_fingerprint(self)?;
        if self.replay_fingerprint != expected_fingerprint {
            bail!("quantum result replay fingerprint does not match its seeded evidence");
        }
        Ok(())
    }
}

pub fn quantum_program_fingerprint(program: &QuantumProgram) -> Result<String> {
    program.validate()?;
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(serde_json::to_vec(program)?))
    ))
}

pub fn quantum_replay_fingerprint(result: &QuantumResult) -> Result<String> {
    let evidence = (
        &result.program_id,
        &result.program_fingerprint,
        result.seed,
        result.shots,
        &result.outcome_counts,
        &result.measured_means,
        result.fidelity_estimate,
        result.confidence_level,
        result.coherence_elapsed_ns,
    );
    Ok(format!(
        "sha256:{}",
        hex::encode(Sha256::digest(serde_json::to_vec(&evidence)?))
    ))
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum InteropOperationKind {
    MeasurementReadout,
    ClassicalControl,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
pub enum InteropOperation {
    MeasurementReadout {
        op_id: String,
        quantum_program_id: String,
        measurement_id: String,
        classical_output: String,
    },
    ClassicalControl {
        op_id: String,
        classical_input: String,
        quantum_program_id: String,
        target_operation_id: String,
        parameter: QuantumControlParameter,
        scale: f64,
        offset: f64,
        maximum_latency_ns: u64,
    },
}

impl InteropOperation {
    fn id(&self) -> &str {
        match self {
            Self::MeasurementReadout { op_id, .. } | Self::ClassicalControl { op_id, .. } => op_id,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct InteropProgram {
    pub version: String,
    pub program_id: String,
    pub operations: Vec<InteropOperation>,
}

impl InteropProgram {
    pub fn validate(&self) -> Result<()> {
        if self.version != INTEROP_PROGRAM_VERSION {
            bail!("unsupported photonic interoperability version");
        }
        identifier(&self.program_id, "interop program id")?;
        if self.operations.is_empty() {
            bail!("interop programs require operations");
        }
        let mut ids = BTreeSet::new();
        for operation in &self.operations {
            identifier(operation.id(), "interop operation id")?;
            if !ids.insert(operation.id()) {
                bail!("interop operation ids must be unique");
            }
            match operation {
                InteropOperation::MeasurementReadout {
                    quantum_program_id,
                    measurement_id,
                    classical_output,
                    ..
                } => {
                    identifier(quantum_program_id, "quantum program reference")?;
                    identifier(measurement_id, "measurement reference")?;
                    identifier(classical_output, "classical output")?;
                }
                InteropOperation::ClassicalControl {
                    classical_input,
                    quantum_program_id,
                    target_operation_id,
                    scale,
                    offset,
                    maximum_latency_ns,
                    ..
                } => {
                    identifier(classical_input, "classical control input")?;
                    identifier(quantum_program_id, "quantum program reference")?;
                    identifier(target_operation_id, "quantum target operation")?;
                    if !scale.is_finite() || !offset.is_finite() || *maximum_latency_ns == 0 {
                        bail!("classical control interoperability parameters are invalid");
                    }
                }
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationSeverity {
    Warning,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MigrationDiagnostic {
    pub op_id: Option<String>,
    pub severity: MigrationSeverity,
    pub code: String,
    pub message: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MigrationStatus {
    Migrated,
    Rejected,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MigratedV5Operation {
    pub op_id: String,
    pub targets: Vec<String>,
    #[serde(flatten)]
    pub operation: MigratedOperationKind,
    pub legacy_operation: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "dialect", rename_all = "snake_case", deny_unknown_fields)]
pub enum MigratedOperationKind {
    Classical { operation: ClassicalOperationFamily },
    QuantumGate { gate: QuantumGateFamily },
    QuantumMeasurement { basis: QuantumMeasurementFamily },
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct V5MigrationReport {
    pub version: String,
    pub source_version: String,
    pub status: MigrationStatus,
    pub operations: Vec<MigratedV5Operation>,
    pub diagnostics: Vec<MigrationDiagnostic>,
}

pub fn migrate_v5_document(document: &Value) -> Result<V5MigrationReport> {
    let legacy_schema: Value =
        serde_json::from_str(include_str!("../../awen-spec/schemas/photonic_ir.v5.json"))?;
    let validator = JSONSchema::options()
        .compile(&legacy_schema)
        .map_err(|error| anyhow::anyhow!("compile legacy Photonic IR V5 schema: {error}"))?;
    if let Err(errors) = validator.validate(document) {
        bail!(
            "legacy Photonic IR V5 structure is invalid: {}",
            errors
                .map(|error| error.to_string())
                .collect::<Vec<_>>()
                .join("; ")
        );
    }
    let source_version = document
        .get("ir_version")
        .and_then(Value::as_str)
        .context("legacy V5 document requires ir_version")?;
    if !valid_v5_version(source_version) {
        bail!("migration accepts only Photonic IR V5 documents");
    }
    let operations = document
        .get("ops")
        .and_then(Value::as_array)
        .context("legacy V5 document requires an ops array")?;
    let mut migrated = Vec::new();
    let mut diagnostics = Vec::new();
    let mut ids = BTreeSet::new();
    for operation in operations {
        let op_id = operation
            .get("op_id")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let op_type = operation
            .get("type")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let targets = operation
            .get("targets")
            .and_then(Value::as_array)
            .map(|values| {
                values
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_string)
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let valid_targets = operation
            .get("targets")
            .and_then(Value::as_array)
            .is_some_and(|values| {
                !values.is_empty()
                    && values
                        .iter()
                        .all(|value| value.as_str().is_some_and(portable_identifier))
                    && targets.iter().collect::<BTreeSet<_>>().len() == targets.len()
            });
        if !portable_identifier(op_id) || !ids.insert(op_id.to_string()) {
            diagnostics.push(migration_error(
                optional_id(op_id),
                "invalid_operation_id",
                "legacy operation ids must be present and unique",
            ));
            continue;
        }
        if !valid_targets {
            diagnostics.push(migration_error(
                Some(op_id.to_string()),
                "invalid_targets",
                "legacy operation targets must be a non-empty string array",
            ));
            continue;
        }
        let classified = classify_v5_operation(op_type);
        match classified {
            Ok(operation_kind) => migrated.push(MigratedV5Operation {
                op_id: op_id.to_string(),
                targets,
                operation: operation_kind,
                legacy_operation: operation.clone(),
            }),
            Err(message) => diagnostics.push(migration_error(
                Some(op_id.to_string()),
                "ambiguous_or_unsupported_operation",
                &message,
            )),
        }
    }
    let dialects = migrated
        .iter()
        .map(|operation| match operation.operation {
            MigratedOperationKind::Classical { .. } => "awen.photonic",
            MigratedOperationKind::QuantumGate { .. }
            | MigratedOperationKind::QuantumMeasurement { .. } => "awen.qphotonic",
        })
        .collect::<BTreeSet<_>>();
    if dialects.len() > 1 {
        diagnostics.push(MigrationDiagnostic {
            op_id: None,
            severity: MigrationSeverity::Warning,
            code: "explicit_interop_required".to_string(),
            message: "classical and quantum operations were classified independently; add explicit awen.photonic-interop operations instead of inferring cross-dialect conversion".to_string(),
        });
    }
    let status = if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == MigrationSeverity::Error)
    {
        MigrationStatus::Rejected
    } else {
        MigrationStatus::Migrated
    };
    Ok(V5MigrationReport {
        version: V5_MIGRATION_VERSION.to_string(),
        source_version: source_version.to_string(),
        status,
        operations: migrated,
        diagnostics,
    })
}

fn classify_v5_operation(op_type: &str) -> std::result::Result<MigratedOperationKind, String> {
    let operation = match op_type {
        "classical:gemm" => MigratedOperationKind::Classical {
            operation: ClassicalOperationFamily::Gemm,
        },
        "classical:splitter" | "classical:phase_shift" | "classical:interferometer" => {
            MigratedOperationKind::Classical {
                operation: ClassicalOperationFamily::AnalogTransform,
            }
        }
        "classical:modulate" => MigratedOperationKind::Classical {
            operation: ClassicalOperationFamily::Modulate,
        },
        "classical:detect" => MigratedOperationKind::Classical {
            operation: ClassicalOperationFamily::Detect,
        },
        "quantum:beam_splitter" => MigratedOperationKind::QuantumGate {
            gate: QuantumGateFamily::BeamSplitter,
        },
        "quantum:phase_shift" => MigratedOperationKind::QuantumGate {
            gate: QuantumGateFamily::PhaseShift,
        },
        "quantum:squeeze" => MigratedOperationKind::QuantumGate {
            gate: QuantumGateFamily::Squeeze,
        },
        "quantum:displace" => MigratedOperationKind::QuantumGate {
            gate: QuantumGateFamily::Displace,
        },
        "quantum:controlled_x" => MigratedOperationKind::QuantumGate {
            gate: QuantumGateFamily::ControlledX,
        },
        "quantum:fourier" => MigratedOperationKind::QuantumGate {
            gate: QuantumGateFamily::Fourier,
        },
        "quantum:photon_count" => MigratedOperationKind::QuantumMeasurement {
            basis: QuantumMeasurementFamily::PhotonCounting,
        },
        "quantum:homodyne_q" => MigratedOperationKind::QuantumMeasurement {
            basis: QuantumMeasurementFamily::HomodyneQ,
        },
        "quantum:homodyne_p" => MigratedOperationKind::QuantumMeasurement {
            basis: QuantumMeasurementFamily::HomodyneP,
        },
        "quantum:heterodyne" => MigratedOperationKind::QuantumMeasurement {
            basis: QuantumMeasurementFamily::Heterodyne,
        },
        "measurement" | "splitter" | "phase_shift" | "beam_splitter" => {
            return Err(format!(
                "legacy operation type '{op_type}' has no dialect prefix and is ambiguous"
            ));
        }
        "" => return Err("legacy operation is missing its type".to_string()),
        _ => return Err(format!("legacy operation type '{op_type}' is unsupported")),
    };
    Ok(operation)
}

fn migration_error(op_id: Option<String>, code: &str, message: &str) -> MigrationDiagnostic {
    MigrationDiagnostic {
        op_id,
        severity: MigrationSeverity::Error,
        code: code.to_string(),
        message: message.to_string(),
    }
}

fn optional_id(value: &str) -> Option<String> {
    portable_identifier(value).then(|| value.to_string())
}

fn identifier(value: &str, label: &str) -> Result<()> {
    if !portable_identifier(value) {
        bail!("{label} must be a non-empty portable identifier");
    }
    Ok(())
}

fn portable_identifier(value: &str) -> bool {
    !value.trim().is_empty()
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn valid_v5_version(value: &str) -> bool {
    value == "v5"
        || value.strip_prefix("v5.").is_some_and(|minor| {
            !minor.is_empty() && minor.bytes().all(|byte| byte.is_ascii_digit())
        })
}

fn finite_non_negative(value: f64) -> bool {
    value.is_finite() && value >= 0.0
}

fn unit_interval(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn valid_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn quantum_program() -> QuantumProgram {
        QuantumProgram {
            version: QUANTUM_PROGRAM_VERSION.to_string(),
            program_id: "bell-sampling".to_string(),
            modes: vec![
                QuantumMode {
                    id: "q0".to_string(),
                    space: QuantumModeSpace::Fock { cutoff: 2 },
                },
                QuantumMode {
                    id: "q1".to_string(),
                    space: QuantumModeSpace::Fock { cutoff: 2 },
                },
            ],
            initial_state: QuantumInitialState::Fock {
                occupations: vec![0, 0],
            },
            operations: vec![
                QuantumOperation {
                    op_id: "fourier".to_string(),
                    modes: vec!["q0".to_string()],
                    coherence_cost_ns: 10,
                    kind: QuantumOperationKind::Gate {
                        gate_spec: QuantumGate::Fourier,
                    },
                },
                QuantumOperation {
                    op_id: "controlled-x".to_string(),
                    modes: vec!["q0".to_string(), "q1".to_string()],
                    coherence_cost_ns: 10,
                    kind: QuantumOperationKind::Gate {
                        gate_spec: QuantumGate::ControlledX { dimension: 2 },
                    },
                },
                QuantumOperation {
                    op_id: "measure".to_string(),
                    modes: vec!["q0".to_string(), "q1".to_string()],
                    coherence_cost_ns: 10,
                    kind: QuantumOperationKind::Measure {
                        measurement_id: "bell-outcome".to_string(),
                        basis: QuantumMeasurementFamily::PhotonCounting,
                        destructive: true,
                    },
                },
            ],
            execution: QuantumExecutionContract {
                shots: 100,
                seed: 17,
                deterministic_replay: true,
                coherence_budget_ns: 100,
            },
            correctness: StatisticalCorrectnessContract {
                expected_distribution: BTreeMap::from([
                    ("00".to_string(), 0.5),
                    ("11".to_string(), 0.5),
                ]),
                expected_means: BTreeMap::new(),
                maximum_total_variation_distance: 0.1,
                maximum_mean_error: 0.01,
                minimum_fidelity: 0.9,
                confidence_level: 0.95,
            },
            capabilities: QuantumCapabilityRequirements {
                state_spaces: BTreeSet::from([QuantumStateSpace::Fock]),
                gates: BTreeSet::from([QuantumGateFamily::Fourier, QuantumGateFamily::ControlledX]),
                measurements: BTreeSet::from([QuantumMeasurementFamily::PhotonCounting]),
                feed_forward: false,
                minimum_modes: 2,
                maximum_fock_cutoff: Some(2),
                minimum_coherence_ns: 100,
            },
        }
    }

    #[test]
    fn seeded_quantum_result_verifies_statistical_and_replay_contracts() {
        let program = quantum_program();
        let counts = BTreeMap::from([("00".to_string(), 50), ("11".to_string(), 50)]);
        let means = BTreeMap::new();
        let mut result = QuantumResult {
            version: QUANTUM_RESULT_VERSION.to_string(),
            program_id: program.program_id.clone(),
            program_fingerprint: String::new(),
            seed: 17,
            shots: 100,
            outcome_counts: counts.clone(),
            measured_means: means.clone(),
            fidelity_estimate: 0.99,
            confidence_level: 0.95,
            coherence_elapsed_ns: 30,
            replay_fingerprint: String::new(),
        };
        result.seal_replay(&program).expect("seal replay evidence");
        result.validate_against(&program).expect("quantum result");
    }

    #[test]
    fn v5_migration_rejects_ambiguous_measurement_without_inventing_semantics() {
        let report = migrate_v5_document(&serde_json::json!({
            "ir_version": "v5",
            "metadata": {},
            "ops": [{"op_id":"ambiguous","type":"measurement","targets":["m0"]}]
        }))
        .expect("migration report");
        assert_eq!(report.status, MigrationStatus::Rejected);
        assert!(report.operations.is_empty());
        assert_eq!(
            report.diagnostics[0].code,
            "ambiguous_or_unsupported_operation"
        );
    }
}
