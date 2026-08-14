use awen_compiler::{
    AdapterKind, DataClassification, MappingRequest, PhysicalDesignBinding, PHYSICAL_DESIGN_VERSION,
};
use awen_runtime::plugins::{PluginManifest, PluginRegistry, PLUGIN_MANIFEST_VERSION};
use jsonschema::JSONSchema;

fn validator() -> JSONSchema {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/schemas/awen_physical_design.v1.json"
    ))
    .expect("physical-design schema JSON");
    JSONSchema::options()
        .compile(&schema)
        .expect("compile physical-design schema")
}

fn plugin_validator() -> JSONSchema {
    let physical: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/schemas/awen_physical_design.v1.json"
    ))
    .expect("physical-design schema JSON");
    let plugin: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/schemas/awen_plugin_manifest.v1.json"
    ))
    .expect("plugin schema JSON");
    JSONSchema::options()
        .with_document(
            "https://awen.dev/schemas/awen_physical_design.v1.json".to_string(),
            physical,
        )
        .compile(&plugin)
        .expect("compile plugin schema with physical-design adapter reference")
}

fn binding() -> PhysicalDesignBinding {
    serde_json::from_str(include_str!(
        "../../awen-ecosystem/pdks/example_silicon_pdk.json"
    ))
    .expect("open-PDK fixture")
}

#[test]
fn open_pdk_and_mapping_request_conform_to_the_closed_schema() {
    let validator = validator();
    let binding: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-ecosystem/pdks/example_silicon_pdk.json"
    ))
    .expect("binding JSON");
    let request: serde_json::Value = serde_json::from_str(include_str!(
        "../../awen-spec/fixtures/physical_design_mapping_request.v1.json"
    ))
    .expect("request JSON");
    assert!(validator.is_valid(&binding));
    assert!(validator.is_valid(&request));
    serde_json::from_value::<PhysicalDesignBinding>(binding)
        .expect("schema binding matches Rust contract")
        .validate()
        .expect("binding semantics");
    serde_json::from_value::<MappingRequest>(request)
        .expect("schema request matches Rust contract")
        .validate()
        .expect("request semantics");
}

#[test]
fn geometry_and_foundry_payload_fields_are_unrepresentable() {
    let validator = validator();
    let mut value = serde_json::to_value(binding()).expect("binding value");
    value
        .as_object_mut()
        .expect("binding object")
        .insert("gds_polygons".to_string(), serde_json::json!([]));
    assert!(!validator.is_valid(&value));
    assert!(serde_json::from_value::<PhysicalDesignBinding>(value).is_err());
}

#[test]
fn plugin_registry_accepts_closed_circuit_and_em_adapter_boundaries() {
    let binding = binding();
    let mut circuit = binding
        .adapters
        .iter()
        .find(|adapter| adapter.kind == AdapterKind::CircuitSimulator)
        .expect("Circulax adapter")
        .clone();
    let mut electromagnetic = circuit.clone();
    electromagnetic.kind = AdapterKind::ElectromagneticSimulator;
    electromagnetic.tool.name = "open-em-reference".to_string();
    electromagnetic.supported_evidence =
        vec![awen_compiler::EvidenceKind::ElectromagneticSimulation];
    let manifest = PluginManifest {
        manifest_version: PLUGIN_MANIFEST_VERSION.to_string(),
        id: "physical-design-adapters".to_string(),
        version: "1.0.0".to_string(),
        capabilities: vec!["physical_design".to_string()],
        signature: None,
        public_key: None,
        path: None,
        backend: None,
        physical_design_adapters: vec![circuit.clone(), electromagnetic],
    };
    PluginRegistry::new()
        .validate_manifest(&manifest)
        .expect("typed simulator adapter boundaries");
    assert!(plugin_validator()
        .is_valid(&serde_json::to_value(&manifest).expect("plugin manifest JSON")));

    circuit.request_version = "awen.physical-design.v2".to_string();
    let mut invalid = manifest;
    invalid.physical_design_adapters = vec![circuit];
    assert!(PluginRegistry::new().validate_manifest(&invalid).is_err());
}

#[test]
fn proprietary_classification_is_present_and_versioned() {
    let mut binding = binding();
    binding.classification = DataClassification::ProprietaryReference;
    assert_eq!(binding.contract_version, PHYSICAL_DESIGN_VERSION);
    assert!(binding.validate().is_err());
    assert!(!validator().is_valid(&serde_json::to_value(binding).expect("binding JSON")));
}
