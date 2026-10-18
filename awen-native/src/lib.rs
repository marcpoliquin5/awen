//! Python bindings for the AWEN compiler and the compact photonic noise model.
//!
//! Everything exposed here is simulation. Results carry the fidelity label of
//! the model that produced them and are never hardware measurements.

use awen_compiler::ir::GemmShape;
use awen_compiler::noise::{self, PhotonicNoiseContract};
use awen_compiler::{CompileOptions, DeviceCapabilities, TensorProgram};
use numpy::{PyArray2, PyReadonlyArray2, PyUntypedArrayMethods};
use pyo3::exceptions::PyValueError;
use pyo3::prelude::*;

fn value_error(error: impl std::fmt::Display) -> PyErr {
    PyValueError::new_err(error.to_string())
}

fn parse_contract(contract_json: Option<&str>) -> PyResult<PhotonicNoiseContract> {
    let contract = match contract_json {
        Some(text) => serde_json::from_str::<PhotonicNoiseContract>(text).map_err(value_error)?,
        None => PhotonicNoiseContract::default(),
    };
    contract.validate().map_err(value_error)?;
    Ok(contract)
}

/// The default (illustrative, not measured) noise contract as JSON.
#[pyfunction]
fn default_contract_json() -> PyResult<String> {
    serde_json::to_string(&PhotonicNoiseContract::default()).map_err(value_error)
}

/// A noise-free contract as JSON: every optional term disabled.
#[pyfunction]
fn ideal_contract_json() -> PyResult<String> {
    serde_json::to_string(&PhotonicNoiseContract::ideal()).map_err(value_error)
}

/// Validate a contract and return it normalized as JSON.
#[pyfunction]
fn validate_contract(contract_json: &str) -> PyResult<String> {
    serde_json::to_string(&parse_contract(Some(contract_json))?).map_err(value_error)
}

/// Analytic per-term noise budget for a contraction tile of length `k`.
#[pyfunction]
#[pyo3(signature = (k, signal = 1.0, contract_json = None))]
fn predicted_noise(k: usize, signal: f64, contract_json: Option<&str>) -> PyResult<String> {
    if k == 0 {
        return Err(value_error("k must be positive"));
    }
    let contract = parse_contract(contract_json)?;
    serde_json::to_string(&contract.predicted_sigmas(k, signal)).map_err(value_error)
}

/// IEEE 1241 effective bits for a full-scale sine with additive noise `sigma`.
#[pyfunction]
fn effective_bits_from_sigma(sigma: f64) -> f64 {
    noise::effective_bits_from_sigma(sigma)
}

/// Shot-noise-limited effective bits for `full_scale_photoelectrons`.
#[pyfunction]
fn shot_limited_effective_bits(full_scale_photoelectrons: f64) -> f64 {
    noise::shot_limited_effective_bits(full_scale_photoelectrons)
}

/// Simulate `Y = X Wᵀ` (`X: [m, k]`, `W: [n, k]`, float64) on the photonic GEMM model.
///
/// Returns `(Y, report_json)`.
#[pyfunction]
#[pyo3(signature = (x, w, contract_json = None, tile_k = 64, seed = 0))]
fn photonic_gemm<'py>(
    py: Python<'py>,
    x: PyReadonlyArray2<'py, f64>,
    w: PyReadonlyArray2<'py, f64>,
    contract_json: Option<&str>,
    tile_k: usize,
    seed: u64,
) -> PyResult<(Bound<'py, PyArray2<f64>>, String)> {
    let contract = parse_contract(contract_json)?;
    let (m, k) = (x.shape()[0], x.shape()[1]);
    let (n, kw) = (w.shape()[0], w.shape()[1]);
    if k != kw {
        return Err(value_error(format!(
            "inner dimensions differ: x has {k} columns, w has {kw}"
        )));
    }
    let xs = x
        .as_slice()
        .map(<[f64]>::to_vec)
        .unwrap_or_else(|_| x.as_array().iter().copied().collect());
    let ws = w
        .as_slice()
        .map(<[f64]>::to_vec)
        .unwrap_or_else(|_| w.as_array().iter().copied().collect());
    let shape = GemmShape { m, k, n };
    let (y, report) = py
        .detach(|| noise::photonic_gemm(&xs, &ws, shape, &contract, tile_k, seed))
        .map_err(value_error)?;
    let rows: Vec<Vec<f64>> = y.chunks(n).map(<[f64]>::to_vec).collect();
    let array = PyArray2::from_vec2(py, &rows).map_err(value_error)?;
    Ok((array, serde_json::to_string(&report).map_err(value_error)?))
}

/// Compile a `TensorProgram` (JSON) for `DeviceCapabilities` (JSON).
///
/// `options_json` overrides `CompileOptions` fields. Returns the
/// compilation artifact as JSON.
#[pyfunction]
#[pyo3(signature = (program_json, capabilities_json, options_json = None))]
fn compile_program(
    py: Python<'_>,
    program_json: &str,
    capabilities_json: &str,
    options_json: Option<&str>,
) -> PyResult<String> {
    let program: TensorProgram = serde_json::from_str(program_json).map_err(value_error)?;
    let capabilities: DeviceCapabilities =
        serde_json::from_str(capabilities_json).map_err(value_error)?;
    let options: CompileOptions = match options_json {
        Some(text) => serde_json::from_str(text).map_err(value_error)?,
        None => CompileOptions::default(),
    };
    let artifact = py
        .detach(|| awen_compiler::compile(&program, &capabilities, options))
        .map_err(value_error)?;
    serde_json::to_string(&artifact).map_err(value_error)
}

#[pymodule]
fn _native(m: &Bound<'_, PyModule>) -> PyResult<()> {
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    m.add("NOISE_CONTRACT_VERSION", noise::NOISE_CONTRACT_VERSION)?;
    m.add_function(wrap_pyfunction!(default_contract_json, m)?)?;
    m.add_function(wrap_pyfunction!(ideal_contract_json, m)?)?;
    m.add_function(wrap_pyfunction!(validate_contract, m)?)?;
    m.add_function(wrap_pyfunction!(predicted_noise, m)?)?;
    m.add_function(wrap_pyfunction!(effective_bits_from_sigma, m)?)?;
    m.add_function(wrap_pyfunction!(shot_limited_effective_bits, m)?)?;
    m.add_function(wrap_pyfunction!(photonic_gemm, m)?)?;
    m.add_function(wrap_pyfunction!(compile_program, m)?)?;
    Ok(())
}
