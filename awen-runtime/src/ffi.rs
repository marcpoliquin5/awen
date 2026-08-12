//! Stable, allocation-free C ABI for dense framework buffers.

use std::cell::RefCell;
use std::ffi::{c_char, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::ptr;
use std::slice;

pub const FRAMEWORK_C_ABI_VERSION: &str = "awen.framework-c.v1";

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AwenStatus {
    Ok = 0,
    InvalidArgument = 1,
    BufferTooSmall = 2,
    Unsupported = 3,
    InternalError = 4,
}

thread_local! {
    static LAST_ERROR: RefCell<CString> = RefCell::new(CString::new("").expect("empty CString"));
}

/// Return the immutable, process-lifetime ABI version string.
#[no_mangle]
pub extern "C" fn awen_framework_abi_version() -> *const c_char {
    static VERSION: &[u8] = b"awen.framework-c.v1\0";
    VERSION.as_ptr().cast()
}

/// Copy the calling thread's last AWEN error into a caller-owned UTF-8 buffer.
///
/// # Safety
///
/// When `output` is non-null, it must be writable for `output_length` bytes.
#[no_mangle]
pub unsafe extern "C" fn awen_last_error_message(
    output: *mut c_char,
    output_length: usize,
) -> usize {
    LAST_ERROR.with(|slot| {
        let message = slot.borrow();
        let bytes = message.as_bytes_with_nul();
        if !output.is_null() && output_length > 0 {
            let copied = bytes.len().min(output_length);
            // SAFETY: The caller promises `output` is writable for `output_length` bytes.
            unsafe { ptr::copy_nonoverlapping(bytes.as_ptr(), output.cast(), copied) };
            if copied == output_length {
                // SAFETY: output_length is non-zero and the last element is in bounds.
                unsafe { *output.add(output_length - 1) = 0 };
            }
        }
        bytes.len()
    })
}

/// Row-major, contiguous `f64` GEMM. All buffers remain caller-owned.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the associated element count
/// for the duration of this call. `output` must be exclusively writable and
/// must not alias either input buffer.
#[no_mangle]
pub unsafe extern "C" fn awen_gemm_f64(
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
        validate_gemm_buffers(
            (lhs, rhs, output),
            [lhs_length, rhs_length, output_length],
            [m, n, k],
        )?;
        // SAFETY: Buffer validation proves the pointers and lengths needed below.
        let lhs = unsafe { slice::from_raw_parts(lhs, lhs_length) };
        // SAFETY: Buffer validation proves the pointers and lengths needed below.
        let rhs = unsafe { slice::from_raw_parts(rhs, rhs_length) };
        // SAFETY: Buffer validation proves the pointer and output length needed below.
        let output = unsafe { slice::from_raw_parts_mut(output, output_length) };
        gemm(lhs, rhs, output, m, n, k);
        Ok(())
    })
}

/// Row-major, contiguous `f32` GEMM. All buffers remain caller-owned.
///
/// # Safety
///
/// Every non-null pointer must remain valid for the associated element count
/// for the duration of this call. `output` must be exclusively writable and
/// must not alias either input buffer.
#[no_mangle]
pub unsafe extern "C" fn awen_gemm_f32(
    lhs: *const f32,
    lhs_length: usize,
    rhs: *const f32,
    rhs_length: usize,
    output: *mut f32,
    output_length: usize,
    m: usize,
    n: usize,
    k: usize,
) -> AwenStatus {
    ffi_boundary(|| {
        validate_gemm_buffers(
            (lhs, rhs, output),
            [lhs_length, rhs_length, output_length],
            [m, n, k],
        )?;
        // SAFETY: Buffer validation proves the pointers and lengths needed below.
        let lhs = unsafe { slice::from_raw_parts(lhs, lhs_length) };
        // SAFETY: Buffer validation proves the pointers and lengths needed below.
        let rhs = unsafe { slice::from_raw_parts(rhs, rhs_length) };
        // SAFETY: Buffer validation proves the pointer and output length needed below.
        let output = unsafe { slice::from_raw_parts_mut(output, output_length) };
        gemm(lhs, rhs, output, m, n, k);
        Ok(())
    })
}

fn validate_gemm_buffers<T>(
    buffers: (*const T, *const T, *mut T),
    lengths: [usize; 3],
    dimensions: [usize; 3],
) -> Result<(), (AwenStatus, String)> {
    let (lhs, rhs, output) = buffers;
    let [lhs_length, rhs_length, output_length] = lengths;
    let [m, n, k] = dimensions;
    if lhs.is_null() || rhs.is_null() || output.is_null() {
        return Err((
            AwenStatus::InvalidArgument,
            "GEMM buffers must not be null".into(),
        ));
    }
    if m == 0 || n == 0 || k == 0 {
        return Err((
            AwenStatus::InvalidArgument,
            "GEMM dimensions must be positive".into(),
        ));
    }
    let lhs_required = m.checked_mul(k).ok_or_else(|| overflow("lhs"))?;
    let rhs_required = k.checked_mul(n).ok_or_else(|| overflow("rhs"))?;
    let output_required = m.checked_mul(n).ok_or_else(|| overflow("output"))?;
    if lhs_length < lhs_required || rhs_length < rhs_required || output_length < output_required {
        return Err((
            AwenStatus::BufferTooSmall,
            format!(
                "GEMM requires lhs={lhs_required}, rhs={rhs_required}, output={output_required}; received lhs={lhs_length}, rhs={rhs_length}, output={output_length}"
            ),
        ));
    }
    Ok(())
}

fn overflow(name: &str) -> (AwenStatus, String) {
    (
        AwenStatus::InvalidArgument,
        format!("{name} dimensions overflow usize"),
    )
}

fn gemm<T>(lhs: &[T], rhs: &[T], output: &mut [T], m: usize, n: usize, k: usize)
where
    T: Copy + Default + std::ops::AddAssign + std::ops::Mul<Output = T>,
{
    output[..m * n].fill(T::default());
    for row in 0..m {
        for inner in 0..k {
            let lhs_value = lhs[row * k + inner];
            for column in 0..n {
                output[row * n + column] += lhs_value * rhs[inner * n + column];
            }
        }
    }
}

fn ffi_boundary(function: impl FnOnce() -> Result<(), (AwenStatus, String)>) -> AwenStatus {
    clear_error();
    match catch_unwind(AssertUnwindSafe(function)) {
        Ok(Ok(())) => AwenStatus::Ok,
        Ok(Err((status, message))) => {
            set_error(&message);
            status
        }
        Err(_) => {
            set_error("AWEN caught a panic at the C ABI boundary");
            AwenStatus::InternalError
        }
    }
}

fn clear_error() {
    set_error("");
}

fn set_error(message: &str) {
    let sanitized = message.replace('\0', "?");
    LAST_ERROR
        .with(|slot| *slot.borrow_mut() = CString::new(sanitized).expect("NUL bytes were removed"));
}

#[allow(dead_code)]
unsafe fn _c_string(pointer: *const c_char) -> Result<String, (AwenStatus, String)> {
    if pointer.is_null() {
        return Err((AwenStatus::InvalidArgument, "string pointer is null".into()));
    }
    // SAFETY: This private helper documents the conventional NUL-terminated input contract.
    unsafe { CStr::from_ptr(pointer) }
        .to_str()
        .map(str::to_owned)
        .map_err(|_| {
            (
                AwenStatus::InvalidArgument,
                "string is not valid UTF-8".into(),
            )
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn c_abi_gemm_and_error_contract() {
        let lhs = [1.0_f64, 2.0, 3.0, 4.0];
        let rhs = [5.0_f64, 6.0, 7.0, 8.0];
        let mut output = [0.0_f64; 4];
        let status = unsafe {
            awen_gemm_f64(
                lhs.as_ptr(),
                lhs.len(),
                rhs.as_ptr(),
                rhs.len(),
                output.as_mut_ptr(),
                output.len(),
                2,
                2,
                2,
            )
        };
        assert_eq!(status, AwenStatus::Ok);
        assert_eq!(output, [19.0, 22.0, 43.0, 50.0]);

        let status = unsafe {
            awen_gemm_f64(
                lhs.as_ptr(),
                1,
                rhs.as_ptr(),
                rhs.len(),
                output.as_mut_ptr(),
                output.len(),
                2,
                2,
                2,
            )
        };
        assert_eq!(status, AwenStatus::BufferTooSmall);
        let mut message = [0_i8; 256];
        let required = unsafe { awen_last_error_message(message.as_mut_ptr(), message.len()) };
        assert!(required > 1);
        let message = unsafe { CStr::from_ptr(message.as_ptr()) }
            .to_str()
            .unwrap();
        assert!(message.contains("requires lhs=4"));
    }
}
