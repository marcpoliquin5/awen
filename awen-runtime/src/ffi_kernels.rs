//! C ABI for the awenBLAS kernel library (directive item C10), extending the
//! dense GEMM ABI in [`crate::ffi`]. Header: `include/awen/awen.h`.
//!
//! Conventions match the existing ABI: caller-owned buffers, a status code
//! per call, the thread-local last-error message, and no panics across the
//! boundary. Variable-length outputs use a "required length" out-parameter so
//! callers can size buffers with a first call.

use crate::ffi::{c_string, ffi_boundary, AwenStatus};
use awen_compiler::{
    all_kernel_kinds, execute_kernel_reference, execute_kernel_simulator, KernelRequest,
    KernelSimulatorOptions, TargetBackend,
};
use std::ffi::c_char;
use std::ptr;
use std::slice;

pub const KERNEL_C_ABI_VERSION: &str = "awen.kernel-c.v1";

/// Every kernel kind accepted by `awen_kernel_execute_json`.
pub fn kernel_kind_names() -> Vec<String> {
    all_kernel_kinds()
        .into_iter()
        .map(|kind| {
            serde_json::to_value(kind)
                .ok()
                .and_then(|value| value.as_str().map(str::to_owned))
                .unwrap_or_default()
        })
        .collect()
}

/// Copy `bytes` plus a NUL into `output` when it fits; always report the
/// required size (including NUL) through `required`.
unsafe fn write_c_string(
    bytes: &[u8],
    output: *mut c_char,
    output_length: usize,
    required: *mut usize,
) -> Result<(), (AwenStatus, String)> {
    let needed = bytes.len() + 1;
    if !required.is_null() {
        // SAFETY: the caller promises `required` is writable when non-null.
        unsafe { *required = needed };
    }
    if output.is_null() || output_length < needed {
        return Err((
            AwenStatus::BufferTooSmall,
            format!("output buffer needs {needed} bytes"),
        ));
    }
    // SAFETY: `output` is writable for `output_length >= needed` bytes.
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), output.cast::<u8>(), bytes.len());
        *output.add(bytes.len()) = 0;
    }
    Ok(())
}

/// Return the immutable kernel ABI version string.
#[no_mangle]
pub extern "C" fn awen_kernel_abi_version() -> *const c_char {
    static VERSION: &[u8] = b"awen.kernel-c.v1\0";
    VERSION.as_ptr().cast()
}

/// Write the comma-separated kernel kind names.
///
/// # Safety
///
/// `output` must be null or writable for `output_length` bytes; `required`
/// must be null or writable.
#[no_mangle]
pub unsafe extern "C" fn awen_kernel_kinds(
    output: *mut c_char,
    output_length: usize,
    required: *mut usize,
) -> AwenStatus {
    ffi_boundary(|| {
        let joined = kernel_kind_names().join(",");
        // SAFETY: forwarded caller contract.
        unsafe { write_c_string(joined.as_bytes(), output, output_length, required) }
    })
}

/// Row-major complex GEMM on interleaved (re, im) `f64` buffers. Lengths are
/// in doubles: `lhs_length == 2*m*k`, `rhs_length == 2*k*n`,
/// `output_length == 2*m*n`.
///
/// # Safety
///
/// Pointers must be valid for their lengths for the duration of the call;
/// `output` must not alias the inputs.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn awen_cgemm_f64(
    lhs: *const f64,
    lhs_length: usize,
    rhs: *const f64,
    rhs_length: usize,
    output: *mut f64,
    output_length: usize,
    m: usize,
    n: usize,
    k: usize,
) -> AwenStatus {
    ffi_boundary(|| {
        if lhs.is_null() || rhs.is_null() || output.is_null() {
            return Err((AwenStatus::InvalidArgument, "null buffer".into()));
        }
        let expect = |a: usize, b: usize| a.checked_mul(b).and_then(|v| v.checked_mul(2));
        let (Some(l), Some(r), Some(o)) = (expect(m, k), expect(k, n), expect(m, n)) else {
            return Err((AwenStatus::InvalidArgument, "dimension overflow".into()));
        };
        if lhs_length != l || rhs_length != r {
            return Err((
                AwenStatus::InvalidArgument,
                format!("expected lhs {l} and rhs {r} doubles"),
            ));
        }
        if output_length < o {
            return Err((
                AwenStatus::BufferTooSmall,
                format!("output needs {o} doubles"),
            ));
        }
        // SAFETY: lengths validated against the caller's declared sizes.
        let (a, b, c) = unsafe {
            (
                slice::from_raw_parts(lhs, l),
                slice::from_raw_parts(rhs, r),
                slice::from_raw_parts_mut(output, o),
            )
        };
        for row in 0..m {
            for column in 0..n {
                let (mut re, mut im) = (0.0, 0.0);
                for inner in 0..k {
                    let (ar, ai) = (a[2 * (row * k + inner)], a[2 * (row * k + inner) + 1]);
                    let (br, bi) = (b[2 * (inner * n + column)], b[2 * (inner * n + column) + 1]);
                    re += ar * br - ai * bi;
                    im += ar * bi + ai * br;
                }
                c[2 * (row * n + column)] = re;
                c[2 * (row * n + column) + 1] = im;
            }
        }
        Ok(())
    })
}

/// Execute an `awen.blas.v1` request given as JSON and write the
/// `KernelResult` JSON. `mode` 0 = exact CPU reference, 1 = photonic
/// simulator with `effective_bits`, `noise_fraction` and `seed`.
///
/// # Safety
///
/// `request_json` must be a NUL-terminated UTF-8 string; `output` must be
/// null or writable for `output_length` bytes; `required` null or writable.
#[no_mangle]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn awen_kernel_execute_json(
    request_json: *const c_char,
    mode: u32,
    effective_bits: u8,
    noise_fraction: f64,
    seed: u64,
    output: *mut c_char,
    output_length: usize,
    required: *mut usize,
) -> AwenStatus {
    ffi_boundary(|| {
        // SAFETY: forwarded caller contract.
        let text = unsafe { c_string(request_json) }?;
        let request: KernelRequest = serde_json::from_str(&text).map_err(|error| {
            (
                AwenStatus::InvalidArgument,
                format!("invalid request JSON: {error}"),
            )
        })?;
        let result = match mode {
            0 => execute_kernel_reference(&request),
            1 => execute_kernel_simulator(
                &request,
                KernelSimulatorOptions {
                    target: TargetBackend::Photonic,
                    effective_bits,
                    noise_fraction,
                    seed,
                },
            ),
            other => {
                return Err((
                    AwenStatus::Unsupported,
                    format!("unknown execution mode {other}"),
                ))
            }
        }
        .map_err(|error| (AwenStatus::InvalidArgument, error.to_string()))?;
        let json = serde_json::to_string(&result)
            .map_err(|error| (AwenStatus::InternalError, error.to_string()))?;
        // SAFETY: forwarded caller contract.
        unsafe { write_c_string(json.as_bytes(), output, output_length, required) }
    })
}
