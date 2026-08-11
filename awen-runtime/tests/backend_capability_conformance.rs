use awen_compiler::ir::{DType, GemmShape};
use awen_compiler::{BackendHealth, BackendSnapshot, DeviceCapabilities, HealthStatus};

fn capabilities() -> DeviceCapabilities {
    serde_json::from_str(include_str!(
        "../../awen-compiler/capabilities/pace_like_128.json"
    ))
    .expect("reference capability must parse")
}

fn health() -> BackendHealth {
    serde_json::from_str(include_str!(
        "../../awen-compiler/capabilities/pace_like_128.health.json"
    ))
    .expect("reference health must parse")
}

fn shape() -> GemmShape {
    GemmShape {
        m: 128,
        n: 128,
        k: 128,
    }
}

#[test]
fn reference_capability_and_health_conform_to_published_json_schemas() {
    let capability_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/schemas/awen_device_capability.v1.json"
    ))
    .expect("capability schema JSON");
    let health_schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/schemas/awen_backend_health.v1.json"
    ))
    .expect("health schema JSON");
    let capability_value: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-compiler/capabilities/pace_like_128.json"
    ))
    .expect("capability JSON");
    let health_value: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-compiler/capabilities/pace_like_128.health.json"
    ))
    .expect("health JSON");

    let capability_validator =
        jsonschema::JSONSchema::compile(&capability_schema).expect("compile capability schema");
    let health_validator =
        jsonschema::JSONSchema::compile(&health_schema).expect("compile health schema");
    assert!(capability_validator.is_valid(&capability_value));
    assert!(health_validator.is_valid(&health_value));
}

#[test]
fn missing_precision_timing_power_and_calibration_fields_are_not_accepted() {
    for field in [
        "effective_bits",
        "boundary_latency_ns",
        "total_power_budget_mw",
        "calibration_requirements",
    ] {
        let mut value: serde_json::Value = serde_json::from_str(include_str!(
            "../../awen-compiler/capabilities/pace_like_128.json"
        ))
        .expect("fixture JSON");
        value
            .as_object_mut()
            .expect("capability object")
            .remove(field);
        let error = serde_json::from_value::<DeviceCapabilities>(value)
            .expect_err("critical field must be required");
        assert!(error.to_string().contains(field));
    }
}

#[test]
fn invalid_profile_and_version_skew_are_rejected() {
    let mut invalid_profile = capabilities();
    invalid_profile
        .calibration_profile
        .as_mut()
        .expect("reference profile")
        .backend_id = "wrong-backend".to_string();
    assert!(invalid_profile.validate().is_err());

    let mut version_skew = capabilities();
    version_skew.plugin_abi_version = "awen.backend-plugin.v2".to_string();
    let error = version_skew
        .validate()
        .expect_err("future plugin ABI must fail");
    assert!(error.to_string().contains("plugin ABI"));
}

#[test]
fn complex_support_is_explicit_and_cross_field_consistent() {
    let mut contradictory = capabilities();
    contradictory.supported_dtypes.push(DType::ComplexF32);
    assert!(contradictory.validate().is_err());

    let mut complex = capabilities();
    complex.supported_dtypes.push(DType::ComplexF32);
    complex.supports_complex = true;
    let snapshot = BackendSnapshot::new(complex, health()).expect("consistent complex profile");
    let negotiation = snapshot.negotiate_gemm(shape(), DType::ComplexF32, Some(8), false, false);
    assert!(negotiation.eligible);
}

#[test]
fn calibration_expiry_partial_tiles_and_unavailable_resources_force_fallback() {
    let mut expired_health = health();
    expired_health.observed_at = "2026-08-12T00:00:01Z".to_string();
    let expired = BackendSnapshot::new(capabilities(), expired_health)
        .expect("expired is an availability state, not malformed data")
        .negotiate_gemm(shape(), DType::F16, Some(8), false, false);
    assert!(expired
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "calibration_expired"));

    let mut no_partial = capabilities();
    no_partial.supported_operations[0].supports_partial_n = false;
    let partial = BackendSnapshot::new(no_partial, health())
        .expect("partial-tile policy is valid")
        .negotiate_gemm(
            GemmShape {
                m: 128,
                n: 129,
                k: 128,
            },
            DType::F16,
            Some(8),
            false,
            false,
        );
    assert!(partial
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == "partial_n_unsupported"));

    let mut unavailable_health = health();
    unavailable_health.status = HealthStatus::Unavailable;
    unavailable_health.available_channels = 0;
    unavailable_health
        .unavailable_resources
        .push("matrix_core".to_string());
    let unavailable = BackendSnapshot::new(capabilities(), unavailable_health)
        .expect("unavailable health is structurally valid")
        .negotiate_gemm(shape(), DType::F16, Some(8), false, false);
    let codes: Vec<_> = unavailable
        .diagnostics
        .iter()
        .map(|diagnostic| diagnostic.code.as_str())
        .collect();
    assert!(codes.contains(&"backend_unavailable"));
    assert!(codes.contains(&"no_channels"));
    assert!(codes.contains(&"matrix_core_unavailable"));
}
