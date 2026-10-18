use awen_runtime::ffi::AwenStatus;
use awen_runtime::ffi_kernels::{
    awen_cgemm_f64, awen_kernel_execute_json, awen_kernel_kinds, kernel_kind_names,
};
use std::ffi::{CStr, CString};
use std::ptr;

fn execute(request: &str, mode: u32, bits: u8, noise: f64, seed: u64) -> (AwenStatus, String) {
    let request = CString::new(request).unwrap();
    let mut required = 0usize;
    let status = unsafe {
        awen_kernel_execute_json(
            request.as_ptr(),
            mode,
            bits,
            noise,
            seed,
            ptr::null_mut(),
            0,
            &mut required,
        )
    };
    if status != AwenStatus::BufferTooSmall {
        return (status, String::new());
    }
    let mut buffer = vec![0 as std::ffi::c_char; required];
    let status = unsafe {
        awen_kernel_execute_json(
            request.as_ptr(),
            mode,
            bits,
            noise,
            seed,
            buffer.as_mut_ptr(),
            buffer.len(),
            &mut required,
        )
    };
    let text = unsafe { CStr::from_ptr(buffer.as_ptr()) }
        .to_string_lossy()
        .into_owned();
    (status, text)
}

const GEMM: &str = r#"{"version":"awen.blas.v1","id":"t","kind":"gemm","inputs":[
 {"id":"a","shape":[2,2],"dtype":"f32","layout":"row_major","data":{"representation":"real","values":[1,2,3,4]}},
 {"id":"b","shape":[2,2],"dtype":"f32","layout":"row_major","data":{"representation":"real","values":[5,6,7,8]}}]}"#;

#[test]
fn kernel_json_reference_and_simulator_paths() {
    let (status, json) = execute(GEMM, 0, 8, 0.0, 0);
    assert_eq!(status, AwenStatus::Ok);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["simulated"], false);
    assert_eq!(
        value["outputs"][0]["data"]["values"],
        serde_json::json!([19.0, 22.0, 43.0, 50.0])
    );
    let (status, json) = execute(GEMM, 1, 12, 0.01, 3);
    assert_eq!(status, AwenStatus::Ok);
    let value: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(value["simulated"], true);
    let (again_status, again) = execute(GEMM, 1, 12, 0.01, 3);
    assert_eq!(again_status, AwenStatus::Ok);
    assert_eq!(json, again, "same seed must be reproducible across the ABI");

    assert_eq!(execute(GEMM, 9, 8, 0.0, 0).0, AwenStatus::Unsupported);
    assert_eq!(execute("{", 0, 8, 0.0, 0).0, AwenStatus::InvalidArgument);
    assert_eq!(
        execute(&GEMM.replace("[5,6,7,8]", "[5,6,7]"), 0, 8, 0.0, 0).0,
        AwenStatus::InvalidArgument
    );
    let null_status = unsafe {
        awen_kernel_execute_json(
            ptr::null(),
            0,
            8,
            0.0,
            0,
            ptr::null_mut(),
            0,
            ptr::null_mut(),
        )
    };
    assert_eq!(null_status, AwenStatus::InvalidArgument);
}

#[test]
fn complex_gemm_abi_matches_hand_computed_products_and_validates_lengths() {
    // [[1+2i, 3]] x [[1-1i], [2i]] = (1+2i)(1-i) + 3(2i) = 3+1i + 6i = 3+7i
    let lhs = [1.0, 2.0, 3.0, 0.0];
    let rhs = [1.0, -1.0, 0.0, 2.0];
    let mut out = [0.0; 2];
    let status = unsafe {
        awen_cgemm_f64(
            lhs.as_ptr(),
            4,
            rhs.as_ptr(),
            4,
            out.as_mut_ptr(),
            2,
            1,
            1,
            2,
        )
    };
    assert_eq!(status, AwenStatus::Ok);
    assert_eq!(out, [3.0, 7.0]);
    let status = unsafe {
        awen_cgemm_f64(
            lhs.as_ptr(),
            4,
            rhs.as_ptr(),
            4,
            out.as_mut_ptr(),
            1,
            1,
            1,
            2,
        )
    };
    assert_eq!(status, AwenStatus::BufferTooSmall);
    let status = unsafe {
        awen_cgemm_f64(
            ptr::null(),
            4,
            rhs.as_ptr(),
            4,
            out.as_mut_ptr(),
            2,
            1,
            1,
            2,
        )
    };
    assert_eq!(status, AwenStatus::InvalidArgument);
    let status = unsafe {
        awen_cgemm_f64(
            lhs.as_ptr(),
            4,
            rhs.as_ptr(),
            4,
            out.as_mut_ptr(),
            2,
            usize::MAX,
            2,
            usize::MAX,
        )
    };
    assert_eq!(status, AwenStatus::InvalidArgument);
}

#[test]
fn kernel_kind_listing_round_trips_and_sizes_buffers() {
    let names = kernel_kind_names();
    assert!(names.len() >= 22);
    assert!(names.iter().all(|name| !name.is_empty()));
    for name in &names {
        let kind: awen_compiler::KernelKind =
            serde_json::from_value(serde_json::Value::String(name.clone())).unwrap();
        assert_eq!(serde_json::to_value(kind).unwrap(), name.as_str());
    }
    let mut required = 0usize;
    let status = unsafe { awen_kernel_kinds(ptr::null_mut(), 0, &mut required) };
    assert_eq!(status, AwenStatus::BufferTooSmall);
    assert_eq!(required, names.join(",").len() + 1);
    let mut small = vec![0 as std::ffi::c_char; required - 1];
    let status = unsafe { awen_kernel_kinds(small.as_mut_ptr(), small.len(), ptr::null_mut()) };
    assert_eq!(status, AwenStatus::BufferTooSmall);
    let mut buffer = vec![0 as std::ffi::c_char; required];
    let status = unsafe { awen_kernel_kinds(buffer.as_mut_ptr(), buffer.len(), ptr::null_mut()) };
    assert_eq!(status, AwenStatus::Ok);
    let listed = unsafe { CStr::from_ptr(buffer.as_ptr()) }.to_str().unwrap();
    assert_eq!(listed, names.join(","));
}
