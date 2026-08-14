use awen_compiler::{
    compile_with_backend, import_mapping_response, refresh_for_backend, ArtifactRefreshAction,
    BackendSnapshot, CompileOptions, DataClassification, DeviceCapabilities, MappingRequest,
    MappingResponse, PhysicalDesignBinding, TargetBackend, TensorProgram, PHYSICAL_DESIGN_VERSION,
};
use sha2::{Digest, Sha256};

fn program() -> TensorProgram {
    serde_json::from_str(include_str!("../examples/gemm_4x4.json")).expect("GEMM fixture")
}

fn photonic_options() -> CompileOptions {
    CompileOptions {
        target: TargetBackend::Photonic,
        ..CompileOptions::default()
    }
}

fn topology_digest(binding: &PhysicalDesignBinding) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(&binding.topology).expect("serialize topology")
        ))
    )
}

#[test]
fn open_gdsfactory_and_circulax_reference_roundtrips_without_losing_contract_data() {
    let binding: PhysicalDesignBinding = serde_json::from_str(include_str!(
        "../../awen-ecosystem/pdks/example_silicon_pdk.json"
    ))
    .expect("open PDK binding");
    binding.validate().expect("valid immutable binding");
    let serialized = serde_json::to_vec(&binding).expect("serialize binding");
    let reloaded: PhysicalDesignBinding =
        serde_json::from_slice(&serialized).expect("reload binding");
    assert_eq!(reloaded, binding);

    let request: MappingRequest = serde_json::from_str(include_str!(
        "../../awen-spec/fixtures/physical_design_mapping_request.v1.json"
    ))
    .expect("mapping request");
    let response = MappingResponse {
        contract_version: PHYSICAL_DESIGN_VERSION.to_string(),
        request_id: request.request_id.clone(),
        adapter: binding.adapters[0].clone(),
        binding: binding.clone(),
    };
    let imported = import_mapping_response(&request, response).expect("verified mapping import");

    assert_eq!(
        imported.topology.external_ports,
        binding.topology.external_ports
    );
    assert_eq!(
        request.required_ports[0]
            .unit
            .to_micrometers(request.required_ports[0].width),
        imported.topology.external_ports[0]
            .unit
            .to_micrometers(imported.topology.external_ports[0].width)
    );
    assert_eq!(imported.topology.connections, binding.topology.connections);
    assert_eq!(
        imported.circuit_models[0].parameters.get("coupling"),
        Some(&0.5)
    );
    assert_eq!(
        imported.circuit_models[0].framework,
        awen_compiler::CircuitFramework::Circulax
    );

    let mut outside_constraints = binding.clone();
    outside_constraints.layout_constraints.maximum_width = Some(300.0);
    let rejected = MappingResponse {
        contract_version: PHYSICAL_DESIGN_VERSION.to_string(),
        request_id: request.request_id.clone(),
        adapter: outside_constraints.adapters[0].clone(),
        binding: outside_constraints,
    };
    assert!(import_mapping_response(&request, rejected)
        .expect_err("adapter response must stay inside exported constraints")
        .to_string()
        .contains("width constraint"));
}

#[test]
fn immutable_topology_identity_rejects_tampering() {
    let mut binding = PhysicalDesignBinding::reference_open_pdk();
    binding.topology.external_ports[0].width = 0.6;
    let error = binding
        .validate()
        .expect_err("changed topology must fail closed");
    assert!(error.to_string().contains("topology artifact digest"));

    let mut mutable_identity = PhysicalDesignBinding::reference_open_pdk();
    mutable_identity.pdk.manifest.artifact_id = "pdk-latest".to_string();
    let error = mutable_identity
        .validate()
        .expect_err("mutable artifact names are not identities");
    assert!(error.to_string().contains("immutable urn or sha256"));
}

#[test]
fn failed_or_unsupported_verification_cannot_become_a_backend_binding() {
    let mut failed = PhysicalDesignBinding::reference_open_pdk();
    failed.verification[0].status = awen_compiler::EvidenceStatus::Failed;
    assert!(failed
        .validate()
        .expect_err("failed verification")
        .to_string()
        .contains("failed evidence"));

    let mut unsupported = PhysicalDesignBinding::reference_open_pdk();
    unsupported.verification[0].kind = awen_compiler::EvidenceKind::ElectromagneticSimulation;
    assert!(unsupported
        .validate()
        .expect_err("unsupported verification kind")
        .to_string()
        .contains("not supported"));
}

#[test]
fn proprietary_binding_cannot_embed_sources_or_model_and_process_parameters() {
    let mut binding = PhysicalDesignBinding::reference_open_pdk();
    binding.classification = DataClassification::ProprietaryReference;
    binding.pdk.manifest.uri = None;
    binding.component_library.uri = None;
    binding.topology_artifact.uri = None;
    binding.process_corner.parameters.clear();
    for node in &mut binding.topology.nodes {
        node.settings.clear();
    }
    binding.topology.nodes.clear();
    binding.topology.connections.clear();
    binding.topology_artifact.digest = topology_digest(&binding);
    for model in &mut binding.circuit_models {
        model.artifact.uri = None;
        model.parameters.clear();
    }
    for evidence in &mut binding.verification {
        evidence.report.uri = None;
    }
    binding.validate().expect("opaque proprietary references");
    let provenance = binding.provenance().expect("public-safe provenance");
    let public_artifact = serde_json::to_string(&provenance).expect("serialize provenance");
    assert!(!public_artifact.contains("uri"));
    assert!(!public_artifact.contains("parameters"));
    assert!(!public_artifact.contains("settings"));

    binding.circuit_models[0]
        .parameters
        .insert("secret_effective_index".to_string(), 2.4);
    let error = binding
        .validate()
        .expect_err("proprietary parameter leakage must fail closed");
    assert!(error.to_string().contains("must not be embedded"));
}

#[test]
fn pdk_and_process_corner_changes_invalidate_compilation_and_cache_identity() {
    let original_snapshot =
        BackendSnapshot::offline(DeviceCapabilities::pace_like_128()).expect("snapshot");
    let artifact = compile_with_backend(&program(), &original_snapshot, photonic_options())
        .expect("initial compilation");
    assert_eq!(
        artifact.physical_design_provenance.binding_fingerprint,
        original_snapshot
            .capabilities
            .physical_design
            .fingerprint()
            .expect("binding fingerprint")
    );

    let mut pdk_changed = original_snapshot.clone();
    pdk_changed.capabilities.physical_design.pdk.version = "1.0.1".to_string();
    pdk_changed.capabilities.physical_design.pdk.manifest.digest =
        format!("sha256:{}", "b".repeat(64));
    let pdk_topology = pdk_changed.capabilities.topology_fingerprint();
    pdk_changed
        .capabilities
        .calibration_profile
        .as_mut()
        .expect("calibration")
        .topology_fingerprint = pdk_topology;
    let refresh =
        refresh_for_backend(&program(), &artifact, &pdk_changed).expect("PDK change refresh");
    assert_ne!(refresh.action, ArtifactRefreshAction::Reused);
    assert!(refresh.reasons.iter().any(|reason| reason.contains("PDK")));
    assert_ne!(
        artifact.backend_snapshot_fingerprint,
        refresh.artifact.backend_snapshot_fingerprint
    );

    let mut corner_changed = original_snapshot;
    corner_changed
        .capabilities
        .physical_design
        .process_corner
        .corner_id = "hot-85c".to_string();
    corner_changed
        .capabilities
        .physical_design
        .process_corner
        .fingerprint = format!("sha256:{}", "c".repeat(64));
    corner_changed
        .capabilities
        .physical_design
        .process_corner
        .temperature_c = 85.0;
    let corner_topology = corner_changed.capabilities.topology_fingerprint();
    corner_changed
        .capabilities
        .calibration_profile
        .as_mut()
        .expect("calibration")
        .topology_fingerprint = corner_topology;
    let refresh = refresh_for_backend(&program(), &artifact, &corner_changed)
        .expect("process-corner change refresh");
    assert_ne!(refresh.action, ArtifactRefreshAction::Reused);
    assert!(refresh
        .reasons
        .iter()
        .any(|reason| reason.contains("process corner")));
}
