use awen_compiler::{
    benchmark, compile, CompileOptions, DeviceCapabilities, TargetBackend, TensorProgram,
};
use jsonschema::JSONSchema;
use serde_json::{json, Value};

const TENSOR_ID: &str = "https://awen.dev/schemas/awen_tensor_ir.v1.json";
const PRECISION_ID: &str = "https://awen.dev/schemas/awen_precision.v1.json";
const CAPABILITY_ID: &str = "https://awen.dev/schemas/awen_device_capability.v1.json";
const PHOTONIC_ID: &str = "https://awen.dev/schemas/awen_photonic_ir.classical.v1.json";
const DEVICE_ID: &str = "https://awen.dev/schemas/awen_device_ir.v1.json";
const ERROR_ID: &str = "https://awen.dev/schemas/awen_error_report.v1.json";

fn schema(source: &str) -> Value {
    serde_json::from_str(source).expect("published precision schema must be JSON")
}

fn schemas() -> Vec<(&'static str, Value)> {
    vec![
        (
            TENSOR_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_tensor_ir.v1.json"
            )),
        ),
        (
            PRECISION_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_precision.v1.json"
            )),
        ),
        (
            CAPABILITY_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_device_capability.v1.json"
            )),
        ),
        (
            PHOTONIC_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_photonic_ir.classical.v1.json"
            )),
        ),
        (
            DEVICE_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_device_ir.v1.json"
            )),
        ),
        (
            ERROR_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_error_report.v1.json"
            )),
        ),
    ]
}

fn validator(schema_id: &str) -> JSONSchema {
    let documents = schemas();
    let selected = documents
        .iter()
        .find_map(|(id, document)| (*id == schema_id).then_some(document))
        .expect("registered precision schema");
    let mut options = JSONSchema::options();
    for (id, document) in &documents {
        options.with_document((*id).to_string(), document.clone());
    }
    options
        .compile(selected)
        .expect("published precision schema must compile")
}

fn assert_valid(schema_id: &str, value: &Value) {
    let compiled = validator(schema_id);
    if let Err(errors) = compiled.validate(value) {
        let messages = errors.map(|error| error.to_string()).collect::<Vec<_>>();
        panic!("{schema_id} rejected precision artifact: {messages:?}");
    };
}

fn program() -> TensorProgram {
    serde_json::from_value(json!({
        "ir_version": "awen.tensor.v1",
        "tensors": [
            {
                "id": "lhs", "shape": [2, 2], "dtype": "f32", "layout": "row_major",
                "data": [0.25, -0.5, 0.75, 1.0]
            },
            {
                "id": "rhs", "shape": [2, 2], "dtype": "bf16", "layout": "row_major",
                "data": [1.0, 0.5, -0.25, 0.75]
            },
            {"id": "out", "shape": [2, 2], "dtype": "f32", "layout": "row_major"}
        ],
        "ops": [{
            "op": "gemm", "id": "mixed", "lhs": "lhs", "rhs": "rhs", "output": "out",
            "accuracy": {
                "max_abs_error": 0.2,
                "max_rel_error": 0.2,
                "minimum_effective_bits": 12
            }
        }],
        "precision": {
            "version": "awen.precision.v1",
            "tensors": [],
            "operations": [{
                "op_id": "mixed",
                "compute_dtype": "f16",
                "output_dtype": "f32",
                "accumulator_dtype": "f32",
                "minimum_accumulator_bits": 32,
                "allowed_bit_slicing_modes": ["twos_complement"],
                "stochastic_seed": 2026
            }]
        }
    }))
    .expect("precision Tensor IR")
}

#[test]
fn source_lowered_device_and_error_artifacts_match_published_precision_schemas() {
    let program = program();
    let capabilities: DeviceCapabilities = serde_json::from_str(include_str!(
        "../../awen-compiler/capabilities/reference_2x2.json"
    ))
    .expect("reference capabilities");
    let artifact = compile(
        &program,
        &capabilities,
        CompileOptions {
            target: TargetBackend::Photonic,
            ..CompileOptions::default()
        },
    )
    .expect("precision compilation");
    let report = benchmark(&program, &artifact).expect("precision benchmark");

    assert_valid(
        TENSOR_ID,
        &serde_json::to_value(&program).expect("Tensor IR value"),
    );
    assert_valid(
        PRECISION_ID,
        &serde_json::to_value(&program.precision).expect("precision value"),
    );
    assert_valid(
        CAPABILITY_ID,
        &serde_json::to_value(&capabilities).expect("capability value"),
    );
    assert_valid(
        PHOTONIC_ID,
        &serde_json::to_value(&artifact.photonic_ir).expect("Photonic IR value"),
    );
    assert_valid(
        DEVICE_ID,
        &serde_json::to_value(&artifact.device_ir).expect("Device IR value"),
    );
    assert_valid(
        ERROR_ID,
        &serde_json::to_value(&report.outputs[0].error_report).expect("error report value"),
    );
}

#[test]
fn schemas_reject_missing_precision_identity_and_unattributed_reports() {
    let mut precision = serde_json::to_value(program().precision).expect("precision value");
    precision
        .as_object_mut()
        .expect("precision object")
        .remove("version");
    assert!(!validator(PRECISION_ID).is_valid(&precision));

    let report = json!({
        "version": "awen.error-report.v1",
        "operation_id": "mixed",
        "seed": 1,
        "maximum_absolute_error": 0.0,
        "maximum_relative_error": 0.0,
        "passed": true,
        "provenance": []
    });
    assert!(!validator(ERROR_ID).is_valid(&report));

    let implicit_conversion = json!({
        "ir_version": "awen.device.v1",
        "backend_id": "reference",
        "commands": [{"command": "convert_tensor", "tensor": "lhs"}]
    });
    assert!(!validator(DEVICE_ID).is_valid(&implicit_conversion));
}
