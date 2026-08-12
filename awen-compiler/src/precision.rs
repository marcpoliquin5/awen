use crate::capability::{BitSlicingMode, DynamicRange, SaturationMode};
use crate::ir::{DType, Tensor, TensorOp};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

pub const PRECISION_VERSION: &str = "awen.precision.v1";
pub const ERROR_REPORT_VERSION: &str = "awen.error-report.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum PrecisionEncoding {
    IeeeFloat,
    Bfloat,
    AffineInteger,
    BlockFloatingPoint,
    OpticalEffectiveBits,
    BackendNative,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum ScaleGranularity {
    PerTensor,
    PerChannel,
    PerBlock,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum RoundingMode {
    NearestEven,
    TowardZero,
    Stochastic,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum OverflowMode {
    Saturate,
    Error,
}

impl From<SaturationMode> for OverflowMode {
    fn from(value: SaturationMode) -> Self {
        match value {
            SaturationMode::Clamp => Self::Saturate,
            SaturationMode::Error => Self::Error,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub enum AccumulatorDType {
    F32,
    F64,
    I32,
    I64,
    Optical,
}

impl AccumulatorDType {
    pub fn bits(self) -> u8 {
        match self {
            Self::F32 | Self::I32 => 32,
            Self::F64 | Self::I64 => 64,
            Self::Optical => 0,
        }
    }

    pub fn is_digital(self) -> bool {
        !matches!(self, Self::Optical)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct AnalogNoiseModel {
    pub shot_noise_fraction: f64,
    pub thermal_noise_fraction: f64,
    pub phase_noise_radians: f64,
    pub detector_noise_fraction: f64,
}

impl Default for AnalogNoiseModel {
    fn default() -> Self {
        Self {
            shot_noise_fraction: 0.0,
            thermal_noise_fraction: 0.0,
            phase_noise_radians: 0.0,
            detector_noise_fraction: 0.0,
        }
    }
}

impl AnalogNoiseModel {
    pub fn validate(self) -> Result<()> {
        for (value, name) in [
            (self.shot_noise_fraction, "shot noise"),
            (self.thermal_noise_fraction, "thermal noise"),
            (self.phase_noise_radians, "phase noise"),
            (self.detector_noise_fraction, "detector noise"),
        ] {
            if !value.is_finite() || value < 0.0 {
                bail!("{name} must be finite and non-negative");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct QuantizationSpec {
    pub encoding: PrecisionEncoding,
    pub bits: u8,
    pub signed: bool,
    pub granularity: ScaleGranularity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub axis: Option<usize>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub block_size: Option<usize>,
    pub scales: Vec<f64>,
    pub zero_points: Vec<i64>,
    pub clipping_min: f64,
    pub clipping_max: f64,
    pub rounding: RoundingMode,
    pub overflow: OverflowMode,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub backend_encoding: Option<String>,
}

impl QuantizationSpec {
    pub fn validate(&self, shape: &[usize]) -> Result<()> {
        if self.bits == 0 || self.bits > 63 {
            bail!("quantization bits must be in [1, 63]");
        }
        if shape.is_empty() || shape.contains(&0) {
            bail!("quantization requires a non-empty shape with positive dimensions");
        }
        if !self.clipping_min.is_finite()
            || !self.clipping_max.is_finite()
            || self.clipping_min >= self.clipping_max
        {
            bail!("quantization clipping bounds must be finite and increasing");
        }
        let parameter_count = match self.granularity {
            ScaleGranularity::PerTensor => {
                if self.axis.is_some() || self.block_size.is_some() {
                    bail!("per-tensor quantization cannot declare an axis or block size");
                }
                1
            }
            ScaleGranularity::PerChannel => {
                let axis = self
                    .axis
                    .context("per-channel quantization requires an axis")?;
                if axis >= shape.len() || self.block_size.is_some() {
                    bail!("per-channel quantization axis is invalid");
                }
                shape[axis]
            }
            ScaleGranularity::PerBlock => {
                let block_size = self
                    .block_size
                    .context("per-block quantization requires a block size")?;
                if block_size == 0 || self.axis.is_some() {
                    bail!("per-block quantization block size must be positive and has no axis");
                }
                checked_elements(shape)?.div_ceil(block_size)
            }
        };
        if self.scales.len() != parameter_count || self.zero_points.len() != parameter_count {
            bail!(
                "quantization requires {parameter_count} scales and zero points, got {} and {}",
                self.scales.len(),
                self.zero_points.len()
            );
        }
        if self
            .scales
            .iter()
            .any(|scale| !scale.is_finite() || *scale <= 0.0)
        {
            bail!("quantization scales must be finite and positive");
        }
        let (minimum_code, maximum_code) = integer_range(self.bits, self.signed)?;
        if self
            .zero_points
            .iter()
            .any(|point| *point < minimum_code || *point > maximum_code)
        {
            bail!("quantization zero point is outside the encoded integer range");
        }
        if self.encoding == PrecisionEncoding::BlockFloatingPoint {
            if self.granularity != ScaleGranularity::PerBlock {
                bail!("block floating point requires per-block granularity");
            }
            if self.scales.iter().any(|scale| {
                let exponent = scale.log2();
                !exponent.is_finite() || (exponent - exponent.round()).abs() > 1.0e-12
            }) {
                bail!("block-floating-point scales must be exact powers of two");
            }
        }
        if self.encoding == PrecisionEncoding::IeeeFloat && !matches!(self.bits, 16 | 32) {
            bail!("IEEE floating-point precision must use 16 or 32 bits");
        }
        if self.encoding == PrecisionEncoding::Bfloat && self.bits != 16 {
            bail!("bfloat precision must use 16 bits");
        }
        if matches!(
            self.encoding,
            PrecisionEncoding::IeeeFloat | PrecisionEncoding::Bfloat
        ) && self.rounding != RoundingMode::NearestEven
        {
            bail!("v1 floating-point conversion requires nearest-even rounding");
        }
        if self.encoding == PrecisionEncoding::BackendNative
            && self
                .backend_encoding
                .as_deref()
                .is_none_or(|name| name.trim().is_empty())
        {
            bail!("backend-native precision requires a non-empty encoding identifier");
        }
        if self.encoding != PrecisionEncoding::BackendNative && self.backend_encoding.is_some() {
            bail!("backend_encoding is legal only for backend-native precision");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct TensorPrecisionPolicy {
    pub tensor_id: String,
    pub storage_dtype: DType,
    pub quantization: QuantizationSpec,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct OperationPrecisionPolicy {
    pub op_id: String,
    pub compute_dtype: DType,
    pub output_dtype: DType,
    pub accumulator_dtype: AccumulatorDType,
    pub minimum_accumulator_bits: u8,
    pub allowed_bit_slicing_modes: Vec<BitSlicingMode>,
    pub stochastic_seed: u64,
}

impl OperationPrecisionPolicy {
    pub fn validate(&self) -> Result<()> {
        if self.op_id.trim().is_empty() {
            bail!("precision operation id must not be empty");
        }
        if self.minimum_accumulator_bits == 0 {
            bail!("minimum accumulator bits must be positive");
        }
        if self.accumulator_dtype.is_digital()
            && self.accumulator_dtype.bits() < self.minimum_accumulator_bits
        {
            bail!("selected accumulator cannot satisfy its minimum-bit contract");
        }
        if self.accumulator_dtype == AccumulatorDType::Optical
            && self.minimum_accumulator_bits > self.compute_dtype.bits()
        {
            bail!("optical accumulator minimum bits exceed the compute representation");
        }
        let integer_compute = matches!(self.compute_dtype, DType::Int8 | DType::Int4);
        let integer_accumulator = matches!(
            self.accumulator_dtype,
            AccumulatorDType::I32 | AccumulatorDType::I64
        );
        if self.accumulator_dtype != AccumulatorDType::Optical
            && integer_compute != integer_accumulator
        {
            bail!("compute and digital accumulator numeric classes must match");
        }
        if self.allowed_bit_slicing_modes.is_empty() {
            bail!("precision policy must allow at least one bit-slicing mode");
        }
        let unique = self
            .allowed_bit_slicing_modes
            .iter()
            .copied()
            .collect::<HashSet<_>>();
        if unique.len() != self.allowed_bit_slicing_modes.len() {
            bail!("precision policy bit-slicing modes must be unique");
        }
        if self.compute_dtype.is_complex() != self.output_dtype.is_complex() {
            bail!("precision policy cannot implicitly cross the real/complex boundary");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PrecisionConfiguration {
    pub version: String,
    #[serde(default)]
    pub tensors: Vec<TensorPrecisionPolicy>,
    #[serde(default)]
    pub operations: Vec<OperationPrecisionPolicy>,
}

impl Default for PrecisionConfiguration {
    fn default() -> Self {
        Self {
            version: PRECISION_VERSION.to_string(),
            tensors: Vec::new(),
            operations: Vec::new(),
        }
    }
}

impl PrecisionConfiguration {
    pub fn is_empty(&self) -> bool {
        self.tensors.is_empty() && self.operations.is_empty()
    }

    pub fn validate(&self, tensors: &[Tensor], ops: &[TensorOp]) -> Result<()> {
        if self.version != PRECISION_VERSION {
            bail!(
                "unsupported precision version '{}'; expected '{}'",
                self.version,
                PRECISION_VERSION
            );
        }
        let tensor_by_id = tensors
            .iter()
            .map(|tensor| (tensor.id.as_str(), tensor))
            .collect::<HashMap<_, _>>();
        let op_ids = ops.iter().map(TensorOp::id).collect::<HashSet<_>>();
        let mut precision_tensor_ids = HashSet::new();
        for policy in &self.tensors {
            if !precision_tensor_ids.insert(policy.tensor_id.as_str()) {
                bail!("duplicate tensor precision policy '{}'", policy.tensor_id);
            }
            let tensor = tensor_by_id
                .get(policy.tensor_id.as_str())
                .with_context(|| {
                    format!(
                        "precision policy references unknown tensor '{}'",
                        policy.tensor_id
                    )
                })?;
            if policy.storage_dtype != tensor.dtype {
                bail!(
                    "tensor '{}' precision storage dtype {:?} does not match Tensor IR dtype {:?}",
                    tensor.id,
                    policy.storage_dtype,
                    tensor.dtype
                );
            }
            if tensor.dtype.is_complex()
                && matches!(
                    policy.quantization.encoding,
                    PrecisionEncoding::AffineInteger
                        | PrecisionEncoding::BlockFloatingPoint
                        | PrecisionEncoding::OpticalEffectiveBits
                )
            {
                bail!("complex tensor quantization requires an explicit backend-native encoding");
            }
            policy.quantization.validate(&tensor.shape)?;
        }
        let mut precision_op_ids = HashSet::new();
        for policy in &self.operations {
            policy.validate()?;
            if !precision_op_ids.insert(policy.op_id.as_str()) {
                bail!("duplicate operation precision policy '{}'", policy.op_id);
            }
            if !op_ids.contains(policy.op_id.as_str()) {
                bail!(
                    "precision policy references unknown operation '{}'",
                    policy.op_id
                );
            }
        }
        Ok(())
    }

    pub fn tensor(&self, id: &str) -> Option<&TensorPrecisionPolicy> {
        self.tensors.iter().find(|policy| policy.tensor_id == id)
    }

    pub fn operation(&self, id: &str) -> Option<&OperationPrecisionPolicy> {
        self.operations.iter().find(|policy| policy.op_id == id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct QuantizedTensor {
    pub codes: Vec<i64>,
    pub dequantized: Vec<f64>,
    pub clipped_values: usize,
    pub saturated_values: usize,
    pub maximum_absolute_error: f64,
}

pub fn quantize(
    values: &[f64],
    shape: &[usize],
    spec: &QuantizationSpec,
    seed: u64,
) -> Result<QuantizedTensor> {
    spec.validate(shape)?;
    if values.len() != checked_elements(shape)? {
        bail!("quantization value count does not match tensor shape");
    }
    if values.iter().any(|value| !value.is_finite()) {
        bail!("quantization input values must be finite");
    }
    let (minimum_code, maximum_code) = integer_range(spec.bits, spec.signed)?;
    let mut generator = SplitMix64::new(seed);
    let mut codes = Vec::with_capacity(values.len());
    let mut dequantized = Vec::with_capacity(values.len());
    let mut clipped_values = 0;
    let mut saturated_values = 0;
    let mut maximum_absolute_error = 0.0_f64;
    for (index, value) in values.iter().copied().enumerate() {
        let parameter = parameter_index(index, shape, spec)?;
        let scale = spec.scales[parameter];
        let zero_point = spec.zero_points[parameter];
        let clipped = value.clamp(spec.clipping_min, spec.clipping_max);
        clipped_values += usize::from(clipped != value);
        if clipped != value && spec.overflow == OverflowMode::Error {
            bail!("quantization input exceeds the declared clipping range");
        }
        let (encoded, restored, saturated) = match spec.encoding {
            PrecisionEncoding::IeeeFloat => quantize_ieee(clipped, spec.bits, spec.overflow)?,
            PrecisionEncoding::Bfloat => quantize_bfloat(clipped, spec.overflow)?,
            _ => {
                let unrounded = clipped / scale + zero_point as f64;
                let rounded = round(unrounded, spec.rounding, &mut generator);
                let encoded = if rounded < minimum_code as f64 || rounded > maximum_code as f64 {
                    if spec.overflow == OverflowMode::Error {
                        bail!("quantized value exceeds the encoded integer range");
                    }
                    saturated_values += 1;
                    rounded.clamp(minimum_code as f64, maximum_code as f64) as i64
                } else {
                    rounded as i64
                };
                (encoded, (encoded - zero_point) as f64 * scale, false)
            }
        };
        saturated_values += usize::from(saturated);
        maximum_absolute_error = maximum_absolute_error.max((restored - value).abs());
        codes.push(encoded);
        dequantized.push(restored);
    }
    Ok(QuantizedTensor {
        codes,
        dequantized,
        clipped_values,
        saturated_values,
        maximum_absolute_error,
    })
}

fn quantize_ieee(value: f64, bits: u8, overflow: OverflowMode) -> Result<(i64, f64, bool)> {
    match bits {
        32 => {
            let (converted, saturated) = finite_f32(value, overflow)?;
            Ok((
                i64::from(converted.to_bits()),
                f64::from(converted),
                saturated,
            ))
        }
        16 => {
            let maximum = 65_504.0_f64;
            let (bounded, saturated) = finite_bound(value, maximum, overflow, "IEEE fp16")?;
            let encoded = f32_to_f16_bits(bounded as f32);
            Ok((i64::from(encoded), f16_bits_to_f64(encoded), saturated))
        }
        _ => bail!("IEEE floating-point precision must use 16 or 32 bits"),
    }
}

fn quantize_bfloat(value: f64, overflow: OverflowMode) -> Result<(i64, f64, bool)> {
    let (converted, saturated) = finite_f32(value, overflow)?;
    let source = converted.to_bits();
    let rounding_bias = 0x7fff_u32 + ((source >> 16) & 1);
    let encoded = source.wrapping_add(rounding_bias) >> 16;
    let restored = f32::from_bits(encoded << 16);
    Ok((i64::from(encoded), f64::from(restored), saturated))
}

fn finite_f32(value: f64, overflow: OverflowMode) -> Result<(f32, bool)> {
    let (bounded, saturated) = finite_bound(value, f64::from(f32::MAX), overflow, "fp32")?;
    Ok((bounded as f32, saturated))
}

fn finite_bound(
    value: f64,
    maximum: f64,
    overflow: OverflowMode,
    name: &str,
) -> Result<(f64, bool)> {
    if value.abs() <= maximum {
        return Ok((value, false));
    }
    if overflow == OverflowMode::Error {
        bail!("value exceeds the finite {name} range");
    }
    Ok((value.clamp(-maximum, maximum), true))
}

fn f32_to_f16_bits(value: f32) -> u16 {
    let source = value.to_bits();
    let sign = ((source >> 16) & 0x8000) as u16;
    let exponent = ((source >> 23) & 0xff) as i32;
    let mantissa = source & 0x7f_ffff;
    if exponent == 0xff {
        return sign | 0x7c00 | u16::from(mantissa != 0);
    }
    let mut half_exponent = exponent - 127 + 15;
    if half_exponent >= 31 {
        return sign | 0x7c00;
    }
    if half_exponent <= 0 {
        if half_exponent < -10 {
            return sign;
        }
        let significand = mantissa | 0x80_0000;
        let shift = (14 - half_exponent) as u32;
        let mut half_mantissa = significand >> shift;
        let remainder_mask = (1_u32 << shift) - 1;
        let remainder = significand & remainder_mask;
        let halfway = 1_u32 << (shift - 1);
        if remainder > halfway || (remainder == halfway && half_mantissa & 1 != 0) {
            half_mantissa += 1;
        }
        return sign | half_mantissa as u16;
    }
    let mut half_mantissa = mantissa >> 13;
    let remainder = mantissa & 0x1fff;
    if remainder > 0x1000 || (remainder == 0x1000 && half_mantissa & 1 != 0) {
        half_mantissa += 1;
        if half_mantissa == 0x400 {
            half_mantissa = 0;
            half_exponent += 1;
            if half_exponent >= 31 {
                return sign | 0x7c00;
            }
        }
    }
    sign | ((half_exponent as u16) << 10) | half_mantissa as u16
}

fn f16_bits_to_f64(value: u16) -> f64 {
    let sign = if value & 0x8000 == 0 { 1.0 } else { -1.0 };
    let exponent = (value >> 10) & 0x1f;
    let mantissa = value & 0x03ff;
    match exponent {
        0 => sign * f64::from(mantissa) / 1024.0 * 2.0_f64.powi(-14),
        0x1f if mantissa == 0 => sign * f64::INFINITY,
        0x1f => f64::NAN,
        _ => sign * (1.0 + f64::from(mantissa) / 1024.0) * 2.0_f64.powi(i32::from(exponent) - 15),
    }
}

pub fn default_quantization(
    dtype: DType,
    effective_bits: u8,
    dynamic_range: DynamicRange,
    overflow: OverflowMode,
) -> Result<QuantizationSpec> {
    let bits = effective_bits.min(dtype.bits()).max(1);
    let signed = dynamic_range.minimum < 0.0;
    let (minimum_code, maximum_code) = integer_range(bits, signed)?;
    let maximum_magnitude = dynamic_range
        .minimum
        .abs()
        .max(dynamic_range.maximum.abs())
        .max(f64::EPSILON);
    let level_magnitude = (minimum_code.abs().max(maximum_code.abs())) as f64;
    let scale = maximum_magnitude / level_magnitude.max(1.0);
    let encoding = match dtype {
        DType::F32 | DType::F16 | DType::Bf16 | DType::ComplexF32 if bits < dtype.bits() => {
            PrecisionEncoding::OpticalEffectiveBits
        }
        DType::F32 | DType::F16 | DType::ComplexF32 => PrecisionEncoding::IeeeFloat,
        DType::Bf16 => PrecisionEncoding::Bfloat,
        DType::Int8 | DType::Int4 => PrecisionEncoding::AffineInteger,
    };
    Ok(QuantizationSpec {
        encoding,
        bits,
        signed,
        granularity: ScaleGranularity::PerTensor,
        axis: None,
        block_size: None,
        scales: vec![scale],
        zero_points: vec![0],
        clipping_min: dynamic_range.minimum,
        clipping_max: dynamic_range.maximum,
        rounding: RoundingMode::NearestEven,
        overflow,
        backend_encoding: None,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct BitSlicedValue {
    pub original: i64,
    pub encoded: i64,
    pub slices: Vec<u64>,
    pub negative: bool,
    pub saturated: bool,
}

pub fn bit_slice_signed(
    value: i64,
    total_bits: u8,
    slice_bits: u8,
    mode: BitSlicingMode,
    overflow: OverflowMode,
) -> Result<BitSlicedValue> {
    if !(2..=63).contains(&total_bits) || slice_bits == 0 || slice_bits > total_bits {
        bail!("signed bit slicing requires total bits in [2, 63] and a valid slice width");
    }
    if mode == BitSlicingMode::None && slice_bits != total_bits {
        bail!("bit-slicing mode none requires one full-width slice");
    }
    let maximum = (1_i64 << (total_bits - 1)) - 1;
    let minimum = if mode == BitSlicingMode::SignedMagnitude {
        -maximum
    } else {
        -(1_i64 << (total_bits - 1))
    };
    let encoded_value = if value < minimum || value > maximum {
        if overflow == OverflowMode::Error {
            bail!("signed value {value} overflows {total_bits}-bit encoding");
        }
        value.clamp(minimum, maximum)
    } else {
        value
    };
    let saturated = encoded_value != value;
    let negative = encoded_value < 0;
    let raw = match mode {
        BitSlicingMode::TwosComplement | BitSlicingMode::None => {
            if encoded_value < 0 {
                ((1_i128 << total_bits) + i128::from(encoded_value)) as u64
            } else {
                encoded_value as u64
            }
        }
        BitSlicingMode::SignedMagnitude => encoded_value.unsigned_abs(),
    };
    let slice_count = total_bits.div_ceil(slice_bits);
    let mask = (1_u64 << slice_bits) - 1;
    let slices = (0..slice_count)
        .map(|index| (raw >> (index * slice_bits)) & mask)
        .collect();
    Ok(BitSlicedValue {
        original: value,
        encoded: encoded_value,
        slices,
        negative,
        saturated,
    })
}

pub fn reconstruct_bit_slices(
    value: &BitSlicedValue,
    total_bits: u8,
    slice_bits: u8,
    mode: BitSlicingMode,
) -> Result<i64> {
    if !(2..=63).contains(&total_bits) || slice_bits == 0 || slice_bits > total_bits {
        bail!("invalid bit-slice reconstruction widths");
    }
    let expected = usize::from(total_bits.div_ceil(slice_bits));
    if value.slices.len() != expected {
        bail!("bit-slice count does not match the declared widths");
    }
    let mask = (1_u64 << slice_bits) - 1;
    let mut raw = 0_u64;
    for (index, slice) in value.slices.iter().copied().enumerate() {
        if slice > mask {
            bail!("bit slice exceeds its declared width");
        }
        raw |= slice << (index as u8 * slice_bits);
    }
    let result = match mode {
        BitSlicingMode::SignedMagnitude => {
            let magnitude = i64::try_from(raw).context("signed magnitude overflows i64")?;
            let maximum = (1_i64 << (total_bits - 1)) - 1;
            if magnitude > maximum {
                bail!("signed magnitude exceeds its declared width");
            }
            if value.negative {
                -magnitude
            } else {
                magnitude
            }
        }
        BitSlicingMode::TwosComplement | BitSlicingMode::None => {
            let sign_bit = 1_u64 << (total_bits - 1);
            if raw & sign_bit == 0 {
                raw as i64
            } else {
                (i128::from(raw) - (1_i128 << total_bits)) as i64
            }
        }
    };
    Ok(result)
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct AccumulationResult {
    pub value: i64,
    pub overflowed: bool,
}

pub fn accumulate_integer_products(
    lhs: &[i64],
    rhs: &[i64],
    accumulator: AccumulatorDType,
    overflow: OverflowMode,
) -> Result<AccumulationResult> {
    if lhs.len() != rhs.len() || lhs.is_empty() {
        bail!("integer accumulation requires equal non-empty inputs");
    }
    let (minimum, maximum) = match accumulator {
        AccumulatorDType::I32 => (i128::from(i32::MIN), i128::from(i32::MAX)),
        AccumulatorDType::I64 => (i128::from(i64::MIN), i128::from(i64::MAX)),
        _ => bail!("integer products require an i32 or i64 accumulator"),
    };
    let exact = lhs.iter().zip(rhs).try_fold(0_i128, |sum, (left, right)| {
        sum.checked_add(i128::from(*left) * i128::from(*right))
            .context("integer accumulation overflowed i128")
    })?;
    if exact < minimum || exact > maximum {
        if overflow == OverflowMode::Error {
            bail!("integer accumulation overflowed the declared accumulator");
        }
        return Ok(AccumulationResult {
            value: exact.clamp(minimum, maximum) as i64,
            overflowed: true,
        });
    }
    Ok(AccumulationResult {
        value: exact as i64,
        overflowed: false,
    })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NoiseApplication {
    pub values: Vec<f64>,
    pub shot_noise: Vec<f64>,
    pub thermal_noise: Vec<f64>,
    pub phase_noise: Vec<f64>,
    pub detector_noise: Vec<f64>,
    pub seed: u64,
}

pub fn apply_noise(values: &[f64], model: AnalogNoiseModel, seed: u64) -> Result<NoiseApplication> {
    model.validate()?;
    if values.iter().any(|value| !value.is_finite()) {
        bail!("noise model inputs must be finite");
    }
    let mut generator = SplitMix64::new(seed);
    let mut output = Vec::with_capacity(values.len());
    let mut shot = Vec::with_capacity(values.len());
    let mut thermal = Vec::with_capacity(values.len());
    let mut phase = Vec::with_capacity(values.len());
    let mut detector = Vec::with_capacity(values.len());
    for value in values.iter().copied() {
        let scale = value.abs().max(f64::EPSILON);
        let shot_value = gaussian(&mut generator) * model.shot_noise_fraction * scale.sqrt();
        let thermal_value = gaussian(&mut generator) * model.thermal_noise_fraction;
        let phase_value = gaussian(&mut generator) * model.phase_noise_radians * scale;
        let detector_value = gaussian(&mut generator) * model.detector_noise_fraction * scale;
        let noisy = value + shot_value + thermal_value + phase_value + detector_value;
        if !noisy.is_finite() {
            bail!("noise application produced a non-finite value");
        }
        output.push(noisy);
        shot.push(shot_value);
        thermal.push(thermal_value);
        phase.push(phase_value);
        detector.push(detector_value);
    }
    Ok(NoiseApplication {
        values: output,
        shot_noise: shot,
        thermal_noise: thermal,
        phase_noise: phase,
        detector_noise: detector,
        seed,
    })
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Default)]
pub struct ErrorAttribution {
    pub quantization: f64,
    pub shot_noise: f64,
    pub thermal_noise: f64,
    pub phase_noise: f64,
    pub detector_noise: f64,
    pub calibration_residual: f64,
    pub floating_point_accumulation: f64,
    pub integer_overflow: f64,
    pub clipping: f64,
    pub propagated_input: f64,
    pub total: f64,
}

impl ErrorAttribution {
    pub fn checked(mut self) -> Result<Self> {
        self.total = self.component_sum()?.min(1.0);
        Ok(self)
    }

    pub fn checked_absolute(mut self) -> Result<Self> {
        self.total = self.component_sum()?;
        Ok(self)
    }

    fn component_sum(&self) -> Result<f64> {
        let components = [
            self.quantization,
            self.shot_noise,
            self.thermal_noise,
            self.phase_noise,
            self.detector_noise,
            self.calibration_residual,
            self.floating_point_accumulation,
            self.integer_overflow,
            self.clipping,
            self.propagated_input,
        ];
        if components
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
        {
            bail!("error-attribution components must be finite and non-negative");
        }
        Ok(components.into_iter().sum())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EmpiricalErrorReport {
    pub version: String,
    pub operation_id: String,
    pub seed: u64,
    pub static_fraction: ErrorAttribution,
    pub observed_absolute: ErrorAttribution,
    pub maximum_absolute_error: f64,
    pub maximum_relative_error: f64,
    pub passed: bool,
    pub provenance: Vec<String>,
}

pub fn maximum_absolute(values: &[f64]) -> f64 {
    values
        .iter()
        .fold(0.0_f64, |maximum, value| maximum.max(value.abs()))
}

fn checked_elements(shape: &[usize]) -> Result<usize> {
    shape.iter().try_fold(1_usize, |total, dimension| {
        total
            .checked_mul(*dimension)
            .context("tensor element count overflows usize")
    })
}

fn integer_range(bits: u8, signed: bool) -> Result<(i64, i64)> {
    if bits == 0 || bits > 63 {
        bail!("integer precision must be in [1, 63]");
    }
    if signed {
        Ok((-(1_i64 << (bits - 1)), (1_i64 << (bits - 1)) - 1))
    } else {
        Ok((0, (1_i64 << bits) - 1))
    }
}

fn parameter_index(index: usize, shape: &[usize], spec: &QuantizationSpec) -> Result<usize> {
    match spec.granularity {
        ScaleGranularity::PerTensor => Ok(0),
        ScaleGranularity::PerChannel => {
            let axis = spec.axis.context("per-channel axis is missing")?;
            let trailing = shape[axis + 1..].iter().product::<usize>();
            Ok((index / trailing) % shape[axis])
        }
        ScaleGranularity::PerBlock => {
            Ok(index / spec.block_size.context("per-block block size is missing")?)
        }
    }
}

fn round(value: f64, mode: RoundingMode, generator: &mut SplitMix64) -> f64 {
    match mode {
        RoundingMode::TowardZero => value.trunc(),
        RoundingMode::Stochastic => {
            let floor = value.floor();
            floor
                + if generator.uniform() < value - floor {
                    1.0
                } else {
                    0.0
                }
        }
        RoundingMode::NearestEven => {
            let floor = value.floor();
            let fraction = value - floor;
            if (fraction - 0.5).abs() <= f64::EPSILON {
                if floor as i64 % 2 == 0 {
                    floor
                } else {
                    floor + 1.0
                }
            } else {
                value.round()
            }
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct SplitMix64 {
    state: u64,
}

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self { state: seed }
    }

    fn next(&mut self) -> u64 {
        self.state = self.state.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut value = self.state;
        value = (value ^ (value >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        value ^ (value >> 31)
    }

    fn uniform(&mut self) -> f64 {
        let mantissa = self.next() >> 11;
        mantissa as f64 / (1_u64 << 53) as f64
    }
}

fn gaussian(generator: &mut SplitMix64) -> f64 {
    let first = generator.uniform().max(f64::MIN_POSITIVE);
    let second = generator.uniform();
    (-2.0 * first.ln()).sqrt() * (std::f64::consts::TAU * second).cos()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn per_tensor(bits: u8, overflow: OverflowMode) -> QuantizationSpec {
        QuantizationSpec {
            encoding: PrecisionEncoding::AffineInteger,
            bits,
            signed: true,
            granularity: ScaleGranularity::PerTensor,
            axis: None,
            block_size: None,
            scales: vec![0.25],
            zero_points: vec![0],
            clipping_min: -1.0,
            clipping_max: 1.0,
            rounding: RoundingMode::NearestEven,
            overflow,
            backend_encoding: None,
        }
    }

    #[test]
    fn per_tensor_quantization_handles_signed_extremes_and_clipping() {
        let result = quantize(
            &[-2.0, -1.0, -0.5, 0.5, 1.0, 2.0],
            &[2, 3],
            &per_tensor(4, OverflowMode::Saturate),
            7,
        )
        .unwrap();
        assert_eq!(result.codes, [-4, -4, -2, 2, 4, 4]);
        assert_eq!(result.dequantized, [-1.0, -1.0, -0.5, 0.5, 1.0, 1.0]);
        assert_eq!(result.clipped_values, 2);
        assert_eq!(result.saturated_values, 0);
        assert_eq!(result.maximum_absolute_error, 1.0);
        assert!(quantize(&[2.0], &[1], &per_tensor(4, OverflowMode::Error), 0).is_err());
    }

    #[test]
    fn ieee_fp16_fp32_and_bfloat_use_native_rounding_semantics() {
        let range = DynamicRange {
            minimum: -70_000.0,
            maximum: 70_000.0,
        };
        let fp32 =
            default_quantization(DType::F32, 32, range, OverflowMode::Saturate).expect("fp32 spec");
        let fp32_result = quantize(&[1.000_000_06], &[1], &fp32, 0).expect("fp32 conversion");
        assert_eq!(fp32_result.dequantized[0], f64::from(1.000_000_1_f32));

        let fp16 =
            default_quantization(DType::F16, 16, range, OverflowMode::Saturate).expect("fp16 spec");
        let fp16_result = quantize(&[1.000_4, 70_000.0], &[2], &fp16, 0).expect("fp16 conversion");
        assert_eq!(fp16_result.dequantized[0], 1.0);
        assert_eq!(fp16_result.dequantized[1], 65_504.0);
        assert_eq!(fp16_result.saturated_values, 1);

        let bfloat = default_quantization(DType::Bf16, 16, range, OverflowMode::Saturate)
            .expect("bfloat spec");
        let bfloat_result = quantize(&[1.003], &[1], &bfloat, 0).expect("bfloat conversion");
        assert_eq!(bfloat_result.dequantized[0], 1.0);
    }

    #[test]
    fn per_channel_and_block_floating_point_validate_and_quantize() {
        let channel = QuantizationSpec {
            encoding: PrecisionEncoding::AffineInteger,
            bits: 8,
            signed: true,
            granularity: ScaleGranularity::PerChannel,
            axis: Some(1),
            block_size: None,
            scales: vec![0.5, 0.25],
            zero_points: vec![0, 0],
            clipping_min: -10.0,
            clipping_max: 10.0,
            rounding: RoundingMode::NearestEven,
            overflow: OverflowMode::Error,
            backend_encoding: None,
        };
        let result = quantize(&[1.0, 1.0, 2.0, 2.0], &[2, 2], &channel, 0).unwrap();
        assert_eq!(result.codes, [2, 4, 4, 8]);

        let block = QuantizationSpec {
            encoding: PrecisionEncoding::BlockFloatingPoint,
            bits: 8,
            signed: true,
            granularity: ScaleGranularity::PerBlock,
            axis: None,
            block_size: Some(2),
            scales: vec![0.5, 0.25],
            zero_points: vec![0, 0],
            clipping_min: -16.0,
            clipping_max: 16.0,
            rounding: RoundingMode::NearestEven,
            overflow: OverflowMode::Error,
            backend_encoding: None,
        };
        assert_eq!(
            quantize(&[1.0, 2.0, 1.0, 2.0], &[4], &block, 0)
                .unwrap()
                .codes,
            [2, 4, 4, 8]
        );
    }

    #[test]
    fn twos_complement_and_signed_magnitude_round_trip_every_int4_value() {
        for mode in [
            BitSlicingMode::TwosComplement,
            BitSlicingMode::SignedMagnitude,
        ] {
            for value in -7..=7 {
                let sliced = bit_slice_signed(value, 4, 2, mode, OverflowMode::Error).unwrap();
                assert_eq!(reconstruct_bit_slices(&sliced, 4, 2, mode).unwrap(), value);
            }
        }
        let minimum = bit_slice_signed(
            -8,
            4,
            2,
            BitSlicingMode::TwosComplement,
            OverflowMode::Error,
        )
        .unwrap();
        assert_eq!(
            reconstruct_bit_slices(&minimum, 4, 2, BitSlicingMode::TwosComplement).unwrap(),
            -8
        );
        assert!(bit_slice_signed(
            -8,
            4,
            2,
            BitSlicingMode::SignedMagnitude,
            OverflowMode::Error,
        )
        .is_err());
        let saturated = bit_slice_signed(
            -8,
            4,
            2,
            BitSlicingMode::SignedMagnitude,
            OverflowMode::Saturate,
        )
        .unwrap();
        assert_eq!(saturated.encoded, -7);
        assert!(saturated.saturated);
    }

    #[test]
    fn bit_slicing_saturates_or_rejects_extremes() {
        let saturated = bit_slice_signed(
            99,
            4,
            2,
            BitSlicingMode::TwosComplement,
            OverflowMode::Saturate,
        )
        .unwrap();
        assert_eq!(saturated.encoded, 7);
        assert!(saturated.saturated);
        assert!(bit_slice_signed(
            99,
            4,
            2,
            BitSlicingMode::TwosComplement,
            OverflowMode::Error,
        )
        .is_err());
    }

    #[test]
    fn integer_accumulation_reports_saturation_and_overflow() {
        let values = [i64::from(i32::MAX), i64::from(i32::MAX)];
        let saturated = accumulate_integer_products(
            &values,
            &[2, 2],
            AccumulatorDType::I32,
            OverflowMode::Saturate,
        )
        .unwrap();
        assert_eq!(saturated.value, i64::from(i32::MAX));
        assert!(saturated.overflowed);
        assert!(accumulate_integer_products(
            &values,
            &[2, 2],
            AccumulatorDType::I32,
            OverflowMode::Error,
        )
        .is_err());
    }

    #[test]
    fn deterministic_noise_separates_every_component() {
        let model = AnalogNoiseModel {
            shot_noise_fraction: 0.01,
            thermal_noise_fraction: 0.02,
            phase_noise_radians: 0.03,
            detector_noise_fraction: 0.04,
        };
        let first = apply_noise(&[1.0, 2.0, 3.0], model, 42).unwrap();
        let second = apply_noise(&[1.0, 2.0, 3.0], model, 42).unwrap();
        let different = apply_noise(&[1.0, 2.0, 3.0], model, 43).unwrap();
        assert_eq!(first, second);
        assert_ne!(first.values, different.values);
        assert!(maximum_absolute(&first.shot_noise) > 0.0);
        assert!(maximum_absolute(&first.thermal_noise) > 0.0);
        assert!(maximum_absolute(&first.phase_noise) > 0.0);
        assert!(maximum_absolute(&first.detector_noise) > 0.0);
    }
}
