use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

use crate::precision::{OperationPrecisionPolicy, PrecisionConfiguration};

pub const TENSOR_IR_VERSION: &str = "awen.tensor.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum DType {
    F32,
    F16,
    Bf16,
    Int8,
    Int4,
    ComplexF32,
}

impl DType {
    pub fn bits(self) -> u8 {
        match self {
            Self::F32 | Self::ComplexF32 => 32,
            Self::F16 | Self::Bf16 => 16,
            Self::Int8 => 8,
            Self::Int4 => 4,
        }
    }

    pub fn is_complex(self) -> bool {
        matches!(self, Self::ComplexF32)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    RowMajor,
    ColumnMajor,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Tensor {
    pub id: String,
    pub shape: Vec<usize>,
    pub dtype: DType,
    pub layout: Layout,
    /// Optional literal data is accepted by the reference benchmark path. It
    /// is intentionally omitted from compiler output artifacts.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub data: Option<Vec<f64>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AccuracyContract {
    #[serde(default = "default_max_abs_error")]
    pub max_abs_error: f64,
    #[serde(default = "default_max_rel_error")]
    pub max_rel_error: f64,
    #[serde(default)]
    pub minimum_effective_bits: Option<u8>,
}

fn default_max_abs_error() -> f64 {
    1.0e-2
}

fn default_max_rel_error() -> f64 {
    1.0e-2
}

impl Default for AccuracyContract {
    fn default() -> Self {
        Self {
            max_abs_error: default_max_abs_error(),
            max_rel_error: default_max_rel_error(),
            minimum_effective_bits: None,
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct CostHints {
    #[serde(default)]
    pub sparsity_fraction: f64,
    #[serde(default)]
    pub structured_sparsity: bool,
    #[serde(default)]
    pub input_error_fraction: f64,
    #[serde(default)]
    pub maximum_input_magnitude: Option<f64>,
}

impl Default for CostHints {
    fn default() -> Self {
        Self {
            sparsity_fraction: 0.0,
            structured_sparsity: false,
            input_error_fraction: 0.0,
            maximum_input_magnitude: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum TensorOp {
    Gemm {
        id: String,
        lhs: String,
        rhs: String,
        output: String,
        #[serde(default)]
        transpose_lhs: bool,
        #[serde(default)]
        transpose_rhs: bool,
        #[serde(default)]
        accuracy: AccuracyContract,
        #[serde(default)]
        cost_hints: CostHints,
    },
}

impl TensorOp {
    pub fn id(&self) -> &str {
        match self {
            Self::Gemm { id, .. } => id,
        }
    }

    pub fn accuracy(&self) -> &AccuracyContract {
        match self {
            Self::Gemm { accuracy, .. } => accuracy,
        }
    }

    pub fn cost_hints(&self) -> CostHints {
        match self {
            Self::Gemm { cost_hints, .. } => *cost_hints,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TensorProgram {
    pub ir_version: String,
    pub tensors: Vec<Tensor>,
    pub ops: Vec<TensorOp>,
    #[serde(default, skip_serializing_if = "PrecisionConfiguration::is_empty")]
    pub precision: PrecisionConfiguration,
    #[serde(default)]
    pub metadata: BTreeMap<String, String>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct GemmShape {
    pub m: usize,
    pub n: usize,
    pub k: usize,
}

#[derive(Debug, Clone)]
pub struct ValidatedGemm<'a> {
    pub op: &'a TensorOp,
    pub lhs: &'a Tensor,
    pub rhs: &'a Tensor,
    pub output: &'a Tensor,
    pub shape: GemmShape,
    pub precision_policy: Option<&'a OperationPrecisionPolicy>,
}

impl ValidatedGemm<'_> {
    pub fn compute_dtype(&self) -> DType {
        self.precision_policy
            .map_or(self.lhs.dtype, |policy| policy.compute_dtype)
    }

    pub fn output_dtype(&self) -> DType {
        self.precision_policy
            .map_or(self.output.dtype, |policy| policy.output_dtype)
    }
}

pub fn validate_program(program: &TensorProgram) -> Result<Vec<ValidatedGemm<'_>>> {
    if program.ir_version != TENSOR_IR_VERSION {
        bail!(
            "unsupported tensor IR version '{}'; expected '{}'",
            program.ir_version,
            TENSOR_IR_VERSION
        );
    }
    if program.ops.is_empty() {
        bail!("tensor program must contain at least one operation");
    }

    let mut tensors = HashMap::new();
    for tensor in &program.tensors {
        if tensor.id.trim().is_empty() {
            bail!("tensor id must not be empty");
        }
        if tensor.shape.len() != 2 || tensor.shape.contains(&0) {
            bail!(
                "tensor '{}' must have a non-zero rank-2 shape, got {:?}",
                tensor.id,
                tensor.shape
            );
        }
        if let Some(data) = &tensor.data {
            let expected = tensor.shape.iter().product::<usize>();
            if data.len() != expected {
                bail!(
                    "tensor '{}' has {} values but shape {:?} requires {}",
                    tensor.id,
                    data.len(),
                    tensor.shape,
                    expected
                );
            }
        }
        if tensors.insert(tensor.id.as_str(), tensor).is_some() {
            bail!("duplicate tensor id '{}'", tensor.id);
        }
    }
    program.precision.validate(&program.tensors, &program.ops)?;

    let mut op_ids = HashMap::new();
    let mut validated = Vec::with_capacity(program.ops.len());
    for op in &program.ops {
        if op.id().trim().is_empty() {
            bail!("operation id must not be empty");
        }
        if op_ids.insert(op.id(), ()).is_some() {
            bail!("duplicate operation id '{}'", op.id());
        }

        match op {
            TensorOp::Gemm {
                lhs,
                rhs,
                output,
                transpose_lhs,
                transpose_rhs,
                accuracy,
                cost_hints,
                ..
            } => {
                if !accuracy.max_abs_error.is_finite()
                    || !accuracy.max_rel_error.is_finite()
                    || accuracy.max_abs_error < 0.0
                    || accuracy.max_rel_error < 0.0
                {
                    bail!("operation '{}' has an invalid error tolerance", op.id());
                }
                if !cost_hints.sparsity_fraction.is_finite()
                    || !(0.0..=1.0).contains(&cost_hints.sparsity_fraction)
                    || !cost_hints.input_error_fraction.is_finite()
                    || !(0.0..=1.0).contains(&cost_hints.input_error_fraction)
                    || cost_hints
                        .maximum_input_magnitude
                        .is_some_and(|value| !value.is_finite() || value < 0.0)
                {
                    bail!("operation '{}' has invalid cost hints", op.id());
                }
                let lhs_tensor = tensors.get(lhs.as_str()).copied().with_context(|| {
                    format!("operation '{}' references missing lhs '{lhs}'", op.id())
                })?;
                let rhs_tensor = tensors.get(rhs.as_str()).copied().with_context(|| {
                    format!("operation '{}' references missing rhs '{rhs}'", op.id())
                })?;
                let output_tensor = tensors.get(output.as_str()).copied().with_context(|| {
                    format!(
                        "operation '{}' references missing output '{output}'",
                        op.id()
                    )
                })?;
                let precision_policy = program.precision.operation(op.id());
                if let Some(policy) = precision_policy {
                    if output_tensor.dtype != policy.output_dtype {
                        bail!(
                            "operation '{}' precision output dtype {:?} does not match tensor '{}' dtype {:?}",
                            op.id(),
                            policy.output_dtype,
                            output_tensor.id,
                            output_tensor.dtype
                        );
                    }
                    if accuracy
                        .minimum_effective_bits
                        .is_some_and(|bits| bits > policy.compute_dtype.bits())
                    {
                        bail!(
                            "operation '{}' minimum effective bits exceed its compute dtype {:?}",
                            op.id(),
                            policy.compute_dtype
                        );
                    }
                } else if lhs_tensor.dtype != rhs_tensor.dtype
                    || lhs_tensor.dtype != output_tensor.dtype
                {
                    bail!(
                        "operation '{}' requires matching operand/output dtypes without an explicit precision policy, got {:?}, {:?}, {:?}",
                        op.id(),
                        lhs_tensor.dtype,
                        rhs_tensor.dtype,
                        output_tensor.dtype
                    );
                }

                let (m, lhs_k) = logical_shape(lhs_tensor, *transpose_lhs);
                let (rhs_k, n) = logical_shape(rhs_tensor, *transpose_rhs);
                if lhs_k != rhs_k {
                    bail!(
                        "operation '{}' has incompatible inner dimensions {} and {}",
                        op.id(),
                        lhs_k,
                        rhs_k
                    );
                }
                if output_tensor.shape != [m, n] {
                    bail!(
                        "operation '{}' output '{}' has shape {:?}; expected [{}, {}]",
                        op.id(),
                        output_tensor.id,
                        output_tensor.shape,
                        m,
                        n
                    );
                }

                validated.push(ValidatedGemm {
                    op,
                    lhs: lhs_tensor,
                    rhs: rhs_tensor,
                    output: output_tensor,
                    shape: GemmShape { m, n, k: lhs_k },
                    precision_policy,
                });
            }
        }
    }
    Ok(validated)
}

pub fn logical_shape(tensor: &Tensor, transpose: bool) -> (usize, usize) {
    if transpose {
        (tensor.shape[1], tensor.shape[0])
    } else {
        (tensor.shape[0], tensor.shape[1])
    }
}
