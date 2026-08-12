use awen_compiler::lowering::DeviceCommand;
use awen_compiler::{
    benchmark, compile_with_backend, refresh_for_backend, ArtifactRefreshAction, BackendHealth,
    BackendSnapshot, CalibrationSpareCell, CompileOptions, DeviceCapabilities, TargetBackend,
    TensorProgram,
};

fn program() -> TensorProgram {
    serde_json::from_str(include_str!("../examples/gemm_4x4.json"))
        .expect("GEMM fixture must parse")
}

fn snapshot(name: &str) -> BackendSnapshot {
    let capabilities: DeviceCapabilities = serde_json::from_str(match name {
        "2x2" => include_str!("../capabilities/reference_2x2.json"),
        "128" => include_str!("../capabilities/pace_like_128.json"),
        _ => panic!("unknown snapshot fixture"),
    })
    .expect("capability fixture must parse");
    let health: BackendHealth = serde_json::from_str(match name {
        "2x2" => include_str!("../capabilities/reference_2x2.health.json"),
        "128" => include_str!("../capabilities/pace_like_128.health.json"),
        _ => panic!("unknown snapshot fixture"),
    })
    .expect("health fixture must parse");
    BackendSnapshot::new(capabilities, health).expect("snapshot must validate")
}

fn photonic_options() -> CompileOptions {
    CompileOptions {
        target: TargetBackend::Photonic,
        ..CompileOptions::default()
    }
}

#[test]
fn identical_graphs_compile_differently_for_measured_snapshots() {
    let mut first = snapshot("128");
    first.health.disabled_components = vec!["cell-0-0".to_string()];
    let first_artifact = compile_with_backend(&program(), &first, photonic_options())
        .expect("first calibrated compile");
    assert_eq!(
        first_artifact.photonic_ir.ops[0].cell_remaps[0].replacement_cell,
        "spare-cell-a"
    );

    let mut second = first.clone();
    let profile = second
        .capabilities
        .calibration_profile
        .as_mut()
        .expect("calibration snapshot");
    profile.id = "reference-room-temperature-v2".to_string();
    profile.parent_id = Some("reference-room-temperature-v1".to_string());
    profile.fingerprint =
        "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".to_string();
    profile.spare_cells[0].uncertainty = 0.5;
    second.health.calibration_profile_id = Some(profile.id.clone());
    second.health.calibration_fingerprint = Some(profile.fingerprint.clone());
    let second_artifact = compile_with_backend(&program(), &second, photonic_options())
        .expect("second calibrated compile");

    assert_eq!(
        second_artifact.photonic_ir.ops[0].cell_remaps[0].replacement_cell,
        "spare-cell-b"
    );
    assert_ne!(
        first_artifact.backend_snapshot_fingerprint,
        second_artifact.backend_snapshot_fingerprint
    );
    assert_ne!(first_artifact.device_ir, second_artifact.device_ir);
}

#[test]
fn disabled_cell_is_remapped_without_changing_gemm_semantics() {
    let mut measured = snapshot("2x2");
    measured.health.disabled_components = vec!["cell-0-0".to_string(), "channel-0".to_string()];
    measured.health.available_channels = 1;
    let artifact = compile_with_backend(&program(), &measured, photonic_options())
        .expect("healthy spare should preserve photonic placement");
    let report = benchmark(&program(), &artifact).expect("calibrated benchmark");

    assert!(
        report.all_outputs_within_tolerance,
        "{:#?}",
        report.outputs[0]
    );
    assert!(artifact.device_ir.commands.iter().any(|command| matches!(
        command,
        DeviceCommand::RemapCell {
            disabled_cell,
            replacement_cell,
            ..
        } if disabled_cell == "cell-0-0" && replacement_cell == "spare-cell-a"
    )));
    let calibration = artifact
        .calibration_record
        .as_ref()
        .expect("calibration record");
    assert_eq!(calibration.snapshot_id, "reference-2x2-calibration-v1");
    assert_eq!(calibration.environment.temperature_c, 22.0);
    assert_eq!(calibration.decision_impacts[0].cell_remaps.len(), 1);
    assert_eq!(
        calibration.decision_impacts[0].selected_channels,
        ["channel-1"]
    );
    assert_eq!(calibration.decision_impacts[0].capacity_loss_fraction, 0.5);
    assert_eq!(
        calibration.decision_impacts[0].selected_tile_shape,
        [1, 1, 1]
    );
    assert_eq!(artifact.photonic_ir.ops.len(), 64);
    assert!(calibration.decision_impacts[0].estimated_error_fraction > 0.0);
}

#[test]
fn exhausted_remap_capacity_falls_back_or_rejects_safely() {
    let mut measured = snapshot("2x2");
    measured.health.disabled_components = vec!["cell-0-0".to_string(), "spare-cell-a".to_string()];
    let mut automatic = photonic_options();
    automatic.target = TargetBackend::Auto;
    let artifact = compile_with_backend(&program(), &measured, automatic)
        .expect("automatic placement must fall back");
    assert!(artifact.photonic_ir.ops.is_empty());
    assert!(artifact.placement.iter().all(|decision| {
        decision.selected_backend != TargetBackend::Photonic
            && decision
                .rationale
                .contains("calibration_remap_capacity_exhausted")
    }));

    let error = compile_with_backend(&program(), &measured, photonic_options())
        .expect_err("forced photonic placement must reject exhausted remapping");
    assert!(error
        .to_string()
        .contains("calibration_remap_capacity_exhausted"));
}

#[test]
fn drift_invalidates_and_refreshes_to_safe_fallback() {
    let original_snapshot = snapshot("2x2");
    let artifact = compile_with_backend(&program(), &original_snapshot, photonic_options())
        .expect("initial compile");
    let mut drifted = original_snapshot;
    drifted.health.observed_at = "2026-08-11T22:31:00Z".to_string();
    drifted.health.drift = 0.02;

    let refresh = refresh_for_backend(&program(), &artifact, &drifted)
        .expect("drift refresh must be safe and deterministic");
    assert_eq!(refresh.refresh_version, "awen.artifact-refresh.v1");
    assert_eq!(refresh.action, ArtifactRefreshAction::FellBack);
    assert!(refresh
        .reasons
        .iter()
        .any(|reason| reason.contains("drift")));
    assert!(refresh.artifact.photonic_ir.ops.is_empty());
    assert!(refresh
        .artifact
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.contains("artifact invalidated")));
}

#[test]
fn new_calibration_lineage_recompiles_and_exact_snapshot_reuses() {
    let original_snapshot = snapshot("2x2");
    let artifact = compile_with_backend(&program(), &original_snapshot, photonic_options())
        .expect("initial compile");
    let reused = refresh_for_backend(&program(), &artifact, &original_snapshot)
        .expect("unchanged snapshot should reuse");
    assert_eq!(reused.action, ArtifactRefreshAction::Reused);
    assert_eq!(reused.artifact, artifact);

    let mut recalibrated = original_snapshot;
    let profile = recalibrated
        .capabilities
        .calibration_profile
        .as_mut()
        .expect("calibration snapshot");
    profile.parent_id = Some(profile.id.clone());
    profile.id = "reference-2x2-calibration-v2".to_string();
    profile.fingerprint =
        "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc".to_string();
    profile.measured_at = "2026-08-11T22:30:00Z".to_string();
    profile.environment.temperature_c = 22.1;
    recalibrated.health.observed_at = "2026-08-11T22:31:00Z".to_string();
    recalibrated.health.calibration_profile_id = Some(profile.id.clone());
    recalibrated.health.calibration_fingerprint = Some(profile.fingerprint.clone());

    let refresh = refresh_for_backend(&program(), &artifact, &recalibrated)
        .expect("new valid calibration should trigger recompilation");
    assert_eq!(refresh.action, ArtifactRefreshAction::Recompiled);
    let record = refresh
        .artifact
        .calibration_record
        .expect("refreshed calibration record");
    assert_eq!(record.snapshot_id, "reference-2x2-calibration-v2");
    assert_eq!(
        record.parent_id.as_deref(),
        Some("reference-2x2-calibration-v1")
    );
}

#[test]
fn cross_topology_snapshot_is_rejected_before_compilation() {
    let mut measured = snapshot("2x2");
    measured
        .capabilities
        .calibration_profile
        .as_mut()
        .expect("profile")
        .topology_fingerprint = "fnv1a64:0000000000000000".to_string();
    let error = BackendSnapshot::new(measured.capabilities, measured.health)
        .expect_err("cross-topology calibration must fail closed");
    assert!(error.to_string().contains("topology fingerprint"));
}

#[test]
fn disabling_more_cells_than_spares_is_detected_deterministically() {
    let mut measured = snapshot("2x2");
    let profile = measured
        .capabilities
        .calibration_profile
        .as_mut()
        .expect("profile");
    profile.cells.push(awen_compiler::CalibrationCell {
        id: "cell-0-1".to_string(),
        row: 0,
        column: 1,
        gain: 0.99,
        offset: 0.001,
        phase_error_radians: 0.002,
        insertion_loss_db: 0.9,
        uncertainty: 0.003,
    });
    profile.spare_cells = vec![CalibrationSpareCell {
        id: "only-spare".to_string(),
        gain: 0.99,
        offset: 0.001,
        phase_error_radians: 0.002,
        insertion_loss_db: 0.9,
        uncertainty: 0.003,
    }];
    measured.health.disabled_components = vec!["cell-0-0".into(), "cell-0-1".into()];
    let negotiation = measured.negotiate_gemm(
        awen_compiler::GemmShape { m: 4, n: 4, k: 4 },
        awen_compiler::DType::F16,
        Some(8),
        false,
        false,
    );
    assert_eq!(
        negotiation
            .diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.code == "calibration_remap_capacity_exhausted")
            .count(),
        1
    );
}
