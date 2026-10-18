use anyhow::{bail, ensure, Context, Result};
use awen_compiler::ir::GemmShape;
use awen_compiler::noise::{
    effective_bits_from_sigma, photonic_gemm, Fidelity, GemmSimulationReport, PhotonicNoiseContract,
};
use awen_compiler::{ExecutableCommand, ExecutablePackage};
use serde::{Deserialize, Serialize};

pub const EXECUTION_INPUTS_VERSION: &str = "awen.execution-inputs.v1";
pub const EXECUTION_RESULT_VERSION: &str = "awen.execution-result.v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedExecutable {
    pub backend_id: String,
    pub abi_major: u16,
    pub abi_minor: u16,
    pub dispatches: Vec<PreparedDispatch>,
    pub mlir_bytecode_bytes: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreparedDispatch {
    pub tile: [u32; 3],
    pub minimum_effective_bits: u16,
    pub calibration: String,
    pub layout: String,
    pub result_shape: Vec<i64>,
}

pub fn prepare_executable(bytes: &[u8]) -> Result<PreparedExecutable> {
    let package = ExecutablePackage::decode(bytes).context("invalid AWEN executable artifact")?;
    let dispatches = package
        .commands
        .into_iter()
        .map(|command| match command {
            ExecutableCommand::ExecuteGemm {
                tile_m,
                tile_n,
                tile_k,
                minimum_effective_bits,
                calibration,
                layout,
                result_shape,
            } => PreparedDispatch {
                tile: [tile_m, tile_n, tile_k],
                minimum_effective_bits,
                calibration,
                layout,
                result_shape,
            },
        })
        .collect();
    Ok(PreparedExecutable {
        backend_id: package.backend_id,
        abi_major: package.abi_major,
        abi_minor: package.abi_minor,
        dispatches,
        mlir_bytecode_bytes: package.mlir_bytecode.len(),
    })
}

/// A dense host tensor in the layout named by the dispatch (`row_major` or `column_major`).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct HostTensor {
    pub shape: Vec<usize>,
    pub data: Vec<f64>,
}

impl HostTensor {
    fn validate(&self, name: &str) -> Result<()> {
        ensure!(
            self.shape.len() == 2 || self.shape.len() == 3,
            "{name} must have rank 2 or 3, got shape {:?}",
            self.shape
        );
        ensure!(
            self.shape.iter().all(|d| *d > 0),
            "{name} has a zero dimension"
        );
        let expected: usize = self.shape.iter().product();
        ensure!(
            self.data.len() == expected,
            "{name} has {} values but shape {:?} needs {expected}",
            self.data.len(),
            self.shape
        );
        ensure!(
            self.data.iter().all(|v| v.is_finite()),
            "{name} contains non-finite values"
        );
        Ok(())
    }
}

/// Operands for every GEMM dispatch of an executable, in dispatch order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutionInputs {
    pub version: String,
    pub dispatches: Vec<DispatchInputs>,
}

/// `result = lhs · rhs` with `lhs: [b?, m, k]` and `rhs: [b?, k, n]`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DispatchInputs {
    pub lhs: HostTensor,
    pub rhs: HostTensor,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DispatchExecution {
    pub index: usize,
    pub result: HostTensor,
    /// Exact float64 product of the same inputs, computed on the host.
    pub reference: HostTensor,
    pub max_abs_error: f64,
    pub rms_error: f64,
    /// RMS error relative to the largest reference magnitude.
    pub relative_rms_error: f64,
    /// IEEE 1241 effective bits implied by `relative_rms_error`; `None` when
    /// the simulated result is bit-exact.
    pub measured_effective_bits: Option<f64>,
    pub minimum_effective_bits: u16,
    pub meets_precision_contract: bool,
    pub tile_k: usize,
    /// Simulator report for each batch element.
    pub simulations: Vec<GemmSimulationReport>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExecutionResult {
    pub version: String,
    pub backend_id: String,
    pub fidelity: Fidelity,
    pub evidence: String,
    pub seed: u64,
    pub contract: PhotonicNoiseContract,
    pub dispatches: Vec<DispatchExecution>,
}

/// Run every GEMM dispatch of `executable` on the photonic noise model.
///
/// The arithmetic is simulated (compact-model fidelity); results are never
/// hardware measurements.
pub fn execute_prepared(
    executable: &PreparedExecutable,
    inputs: &ExecutionInputs,
    contract: &PhotonicNoiseContract,
    seed: u64,
) -> Result<ExecutionResult> {
    ensure!(
        inputs.version == EXECUTION_INPUTS_VERSION,
        "execution inputs version must be {EXECUTION_INPUTS_VERSION}, got {}",
        inputs.version
    );
    ensure!(
        inputs.dispatches.len() == executable.dispatches.len(),
        "executable has {} dispatch(es) but inputs provide {}",
        executable.dispatches.len(),
        inputs.dispatches.len()
    );
    contract.validate()?;
    let dispatches = executable
        .dispatches
        .iter()
        .zip(&inputs.dispatches)
        .enumerate()
        .map(|(index, (dispatch, operands))| {
            execute_dispatch(index, dispatch, operands, contract, seed)
                .with_context(|| format!("dispatch {index}"))
        })
        .collect::<Result<Vec<_>>>()?;
    Ok(ExecutionResult {
        version: EXECUTION_RESULT_VERSION.to_string(),
        backend_id: executable.backend_id.clone(),
        fidelity: Fidelity::CompactModel,
        evidence: "simulated with the AWEN photonic noise model; not a hardware measurement"
            .to_string(),
        seed,
        contract: contract.clone(),
        dispatches,
    })
}

/// Batch, rows and columns of a rank-2 or rank-3 shape.
fn batch_dims(shape: &[usize]) -> (usize, usize, usize) {
    match shape {
        [r, c] => (1, *r, *c),
        [b, r, c] => (*b, *r, *c),
        _ => unreachable!("validated rank"),
    }
}

/// Copy matrix `index` of a batch into row-major order.
fn matrix_row_major(t: &HostTensor, index: usize, column_major: bool) -> Vec<f64> {
    let (_, rows, cols) = batch_dims(&t.shape);
    let block = &t.data[index * rows * cols..(index + 1) * rows * cols];
    if !column_major {
        return block.to_vec();
    }
    let mut out = vec![0.0; rows * cols];
    for r in 0..rows {
        for c in 0..cols {
            out[r * cols + c] = block[c * rows + r];
        }
    }
    out
}

fn store_matrix(out: &mut [f64], values: &[f64], rows: usize, cols: usize, column_major: bool) {
    for r in 0..rows {
        for c in 0..cols {
            let target = if column_major {
                c * rows + r
            } else {
                r * cols + c
            };
            out[target] = values[r * cols + c];
        }
    }
}

fn execute_dispatch(
    index: usize,
    dispatch: &PreparedDispatch,
    operands: &DispatchInputs,
    contract: &PhotonicNoiseContract,
    seed: u64,
) -> Result<DispatchExecution> {
    operands.lhs.validate("lhs")?;
    operands.rhs.validate("rhs")?;
    let column_major = match dispatch.layout.as_str() {
        "row_major" => false,
        "column_major" => true,
        other => bail!("unsupported layout '{other}'"),
    };
    let (lb, m, k) = batch_dims(&operands.lhs.shape);
    let (rb, kr, n) = batch_dims(&operands.rhs.shape);
    ensure!(
        operands.lhs.shape.len() == operands.rhs.shape.len() && lb == rb,
        "lhs {:?} and rhs {:?} must have equal rank and batch",
        operands.lhs.shape,
        operands.rhs.shape
    );
    ensure!(
        k == kr,
        "contraction dimensions differ: lhs k={k}, rhs k={kr}"
    );
    let result_shape: Vec<usize> = if operands.lhs.shape.len() == 3 {
        vec![lb, m, n]
    } else {
        vec![m, n]
    };
    ensure!(
        dispatch.result_shape.len() == result_shape.len()
            && dispatch
                .result_shape
                .iter()
                .zip(&result_shape)
                .all(|(declared, actual)| *declared == -1 || *declared as usize == *actual),
        "declared result shape {:?} does not match operands (result {:?})",
        dispatch.result_shape,
        result_shape
    );
    let tile_k = (dispatch.tile[2] as usize).max(1);
    let mut result = vec![0.0; lb * m * n];
    let mut reference = vec![0.0; lb * m * n];
    let mut simulations = Vec::with_capacity(lb);
    for b in 0..lb {
        let x = matrix_row_major(&operands.lhs, b, column_major);
        let rhs = matrix_row_major(&operands.rhs, b, column_major);
        // photonic_gemm takes W as [n, k].
        let mut w = vec![0.0; n * k];
        for kk in 0..k {
            for nn in 0..n {
                w[nn * k + kk] = rhs[kk * n + nn];
            }
        }
        let batch_seed = seed
            .wrapping_add((index as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15))
            .wrapping_add(b as u64);
        let (y, report) =
            photonic_gemm(&x, &w, GemmShape { m, k, n }, contract, tile_k, batch_seed)?;
        let mut exact = vec![0.0; m * n];
        for r in 0..m {
            for c in 0..n {
                exact[r * n + c] = (0..k).map(|i| x[r * k + i] * rhs[i * n + c]).sum();
            }
        }
        let span = m * n;
        store_matrix(
            &mut result[b * span..(b + 1) * span],
            &y,
            m,
            n,
            column_major,
        );
        store_matrix(
            &mut reference[b * span..(b + 1) * span],
            &exact,
            m,
            n,
            column_major,
        );
        simulations.push(report);
    }
    let errors: Vec<f64> = result.iter().zip(&reference).map(|(a, e)| a - e).collect();
    let max_abs_error = errors.iter().fold(0.0f64, |acc, e| acc.max(e.abs()));
    let rms_error = (errors.iter().map(|e| e * e).sum::<f64>() / errors.len() as f64).sqrt();
    let scale = reference.iter().fold(0.0f64, |acc, v| acc.max(v.abs()));
    let relative_rms_error = if scale > 0.0 {
        rms_error / scale
    } else {
        rms_error
    };
    let measured_effective_bits =
        (relative_rms_error > 0.0).then(|| effective_bits_from_sigma(relative_rms_error));
    Ok(DispatchExecution {
        index,
        result: HostTensor {
            shape: result_shape.clone(),
            data: result,
        },
        reference: HostTensor {
            shape: result_shape,
            data: reference,
        },
        max_abs_error,
        rms_error,
        relative_rms_error,
        measured_effective_bits,
        minimum_effective_bits: dispatch.minimum_effective_bits,
        meets_precision_contract: measured_effective_bits
            .is_none_or(|bits| bits >= f64::from(dispatch.minimum_effective_bits)),
        tile_k,
        simulations,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use awen_compiler::executable::{EXECUTABLE_ABI_MAJOR, EXECUTABLE_ABI_MINOR, EXECUTABLE_MAGIC};

    fn rank_three_package() -> Vec<u8> {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(EXECUTABLE_MAGIC);
        bytes.extend_from_slice(&EXECUTABLE_ABI_MAJOR.to_le_bytes());
        bytes.extend_from_slice(&EXECUTABLE_ABI_MINOR.to_le_bytes());
        let backend = b"awen.reference.v1";
        bytes.extend_from_slice(&(backend.len() as u16).to_le_bytes());
        bytes.extend_from_slice(backend);
        bytes.extend_from_slice(&1_u32.to_le_bytes());
        bytes.push(1);
        bytes.extend_from_slice(&128_u32.to_le_bytes());
        bytes.extend_from_slice(&64_u32.to_le_bytes());
        bytes.extend_from_slice(&32_u32.to_le_bytes());
        bytes.extend_from_slice(&8_u16.to_le_bytes());
        let calibration = b"required";
        bytes.extend_from_slice(&(calibration.len() as u16).to_le_bytes());
        bytes.extend_from_slice(calibration);
        let layout = b"row_major";
        bytes.extend_from_slice(&(layout.len() as u16).to_le_bytes());
        bytes.extend_from_slice(layout);
        bytes.push(3);
        for dimension in [2_i64, 16, 8] {
            bytes.extend_from_slice(&dimension.to_le_bytes());
        }
        let bytecode = b"ML\xefRtest";
        bytes.extend_from_slice(&(bytecode.len() as u32).to_le_bytes());
        bytes.extend_from_slice(bytecode);
        bytes
    }

    #[test]
    fn rejects_non_executable_input() {
        let error = prepare_executable(b"not an executable").unwrap_err();
        assert!(error
            .to_string()
            .contains("invalid AWEN executable artifact"));
    }

    #[test]
    fn prepares_equal_batch_rank_three_dispatch() {
        let executable = prepare_executable(&rank_three_package()).unwrap();
        assert_eq!(executable.dispatches.len(), 1);
        assert_eq!(executable.dispatches[0].result_shape, vec![2, 16, 8]);
        assert_eq!(executable.dispatches[0].tile, [128, 64, 32]);
    }

    fn prepared(
        result_shape: Vec<i64>,
        layout: &str,
        tile_k: u32,
        bits: u16,
    ) -> PreparedExecutable {
        PreparedExecutable {
            backend_id: "awen.reference.v1".to_string(),
            abi_major: EXECUTABLE_ABI_MAJOR,
            abi_minor: EXECUTABLE_ABI_MINOR,
            dispatches: vec![PreparedDispatch {
                tile: [8, 8, tile_k],
                minimum_effective_bits: bits,
                calibration: "required".to_string(),
                layout: layout.to_string(),
                result_shape,
            }],
            mlir_bytecode_bytes: 4,
        }
    }

    fn inputs(lhs: HostTensor, rhs: HostTensor) -> ExecutionInputs {
        ExecutionInputs {
            version: EXECUTION_INPUTS_VERSION.to_string(),
            dispatches: vec![DispatchInputs { lhs, rhs }],
        }
    }

    fn ramp(shape: Vec<usize>, offset: f64) -> HostTensor {
        let count: usize = shape.iter().product();
        HostTensor {
            shape,
            data: (0..count)
                .map(|i| (i as f64 * 0.37 + offset).sin())
                .collect(),
        }
    }

    #[test]
    fn ideal_execution_matches_exact_product() {
        let lhs = HostTensor {
            shape: vec![2, 3],
            data: vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0],
        };
        let rhs = HostTensor {
            shape: vec![3, 2],
            data: vec![1.0, 0.0, 0.0, 1.0, 1.0, 1.0],
        };
        let result = execute_prepared(
            &prepared(vec![2, 2], "row_major", 2, 8),
            &inputs(lhs, rhs),
            &PhotonicNoiseContract::ideal(),
            0,
        )
        .unwrap();
        let dispatch = &result.dispatches[0];
        let expected = [4.0, 5.0, 10.0, 11.0];
        for (got, want) in dispatch.result.data.iter().zip(expected) {
            assert!((got - want).abs() < 1e-12);
        }
        assert_eq!(dispatch.reference.data, expected);
        assert!(dispatch.meets_precision_contract);
        assert_eq!(result.fidelity, Fidelity::CompactModel);
    }

    #[test]
    fn batched_and_column_major_layouts_agree_with_reference() {
        for layout in ["row_major", "column_major"] {
            let result = execute_prepared(
                &prepared(vec![2, 3, -1], layout, 4, 4),
                &inputs(ramp(vec![2, 3, 5], 0.1), ramp(vec![2, 5, 4], 0.7)),
                &PhotonicNoiseContract::ideal(),
                1,
            )
            .unwrap();
            let dispatch = &result.dispatches[0];
            assert_eq!(dispatch.result.shape, vec![2, 3, 4]);
            assert!(
                dispatch.max_abs_error < 1e-12,
                "{layout}: {}",
                dispatch.max_abs_error
            );
            assert_eq!(dispatch.simulations.len(), 2);
        }
    }

    #[test]
    fn noisy_execution_reports_error_and_precision_contract() {
        let contract = PhotonicNoiseContract {
            photons_per_mac: Some(10.0),
            ..Default::default()
        };
        let result = execute_prepared(
            &prepared(vec![16, 16], "row_major", 32, 12),
            &inputs(ramp(vec![16, 64], 0.3), ramp(vec![64, 16], 1.1)),
            &contract,
            9,
        )
        .unwrap();
        let dispatch = &result.dispatches[0];
        assert!(dispatch.rms_error > 0.0);
        let bits = dispatch.measured_effective_bits.unwrap();
        assert!(bits < 12.0);
        assert!(!dispatch.meets_precision_contract);
        let again = execute_prepared(
            &prepared(vec![16, 16], "row_major", 32, 12),
            &inputs(ramp(vec![16, 64], 0.3), ramp(vec![64, 16], 1.1)),
            &contract,
            9,
        )
        .unwrap();
        assert_eq!(again.dispatches[0].result, dispatch.result);
        let json = serde_json::to_string(&result).unwrap();
        let back: ExecutionResult = serde_json::from_str(&json).unwrap();
        assert_eq!(back.dispatches[0].result.shape, vec![16, 16]);
        assert_eq!(back.fidelity, Fidelity::CompactModel);
    }

    #[test]
    fn execution_rejects_mismatched_inputs() {
        let exe = prepared(vec![2, 2], "row_major", 2, 8);
        let ideal = PhotonicNoiseContract::ideal();
        let bad_k = inputs(ramp(vec![2, 3], 0.0), ramp(vec![4, 2], 0.0));
        assert!(execute_prepared(&exe, &bad_k, &ideal, 0).is_err());
        let bad_shape = inputs(ramp(vec![3, 3], 0.0), ramp(vec![3, 2], 0.0));
        assert!(execute_prepared(&exe, &bad_shape, &ideal, 0).is_err());
        let mut short = inputs(ramp(vec![2, 3], 0.0), ramp(vec![3, 2], 0.0));
        short.dispatches[0].lhs.data.pop();
        assert!(execute_prepared(&exe, &short, &ideal, 0).is_err());
        let mut wrong_version = inputs(ramp(vec![2, 3], 0.0), ramp(vec![3, 2], 0.0));
        wrong_version.version = "v0".to_string();
        assert!(execute_prepared(&exe, &wrong_version, &ideal, 0).is_err());
        let none = ExecutionInputs {
            version: EXECUTION_INPUTS_VERSION.to_string(),
            dispatches: vec![],
        };
        assert!(execute_prepared(&exe, &none, &ideal, 0).is_err());
    }
}
