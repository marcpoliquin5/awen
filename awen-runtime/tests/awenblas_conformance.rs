use awen_compiler::{
    benchmark_kernel, execute_kernel_reference, execute_kernel_simulator, select_kernel,
    KernelBackendProfile, KernelRequest, KernelSimulatorOptions, OptimizationObjective,
    TargetBackend,
};
use jsonschema::JSONSchema;
use serde_json::Value;

const REQUEST_SCHEMA_ID: &str = "https://awen.dev/schemas/awen_blas.v1.json";
const BACKEND_SCHEMA_ID: &str = "https://awen.dev/schemas/awen_blas_backend.v1.json";
const RESULT_SCHEMA_ID: &str = "https://awen.dev/schemas/awen_blas_result.v1.json";
const PLAN_SCHEMA_ID: &str = "https://awen.dev/schemas/awen_blas_plan.v1.json";
const BENCHMARK_SCHEMA_ID: &str = "https://awen.dev/schemas/awen_blas_benchmark.v1.json";

fn schema(source: &str) -> Value {
    serde_json::from_str(source).expect("published awenBLAS schema must be JSON")
}

fn schemas() -> Vec<(&'static str, Value)> {
    vec![
        (
            REQUEST_SCHEMA_ID,
            schema(include_str!("../../awen-spec/schemas/awen_blas.v1.json")),
        ),
        (
            BACKEND_SCHEMA_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_blas_backend.v1.json"
            )),
        ),
        (
            RESULT_SCHEMA_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_blas_result.v1.json"
            )),
        ),
        (
            PLAN_SCHEMA_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_blas_plan.v1.json"
            )),
        ),
        (
            BENCHMARK_SCHEMA_ID,
            schema(include_str!(
                "../../awen-spec/schemas/awen_blas_benchmark.v1.json"
            )),
        ),
    ]
}

fn compile_schema(schema_id: &str) -> JSONSchema {
    let documents = schemas();
    let selected = documents
        .iter()
        .find_map(|(id, schema)| (*id == schema_id).then_some(schema))
        .expect("requested schema must be registered");
    let mut options = JSONSchema::options();
    for (id, document) in documents.iter() {
        options.with_document((*id).to_string(), document.clone());
    }
    options
        .compile(selected)
        .expect("published awenBLAS schema must compile")
}

fn assert_valid(schema_id: &str, value: &Value) {
    let validator = compile_schema(schema_id);
    if let Err(errors) = validator.validate(value) {
        let messages = errors.map(|error| error.to_string()).collect::<Vec<_>>();
        panic!("{schema_id} rejected instance: {messages:?}");
    };
}

#[test]
fn request_backend_result_plan_and_benchmark_conform_to_published_schemas() {
    let request_value: Value = serde_json::from_str(include_str!(
        "../../awen-compiler/kernels/transformer_qkv.json"
    ))
    .expect("request fixture JSON");
    let backend_values: Value = serde_json::from_str(include_str!(
        "../../awen-compiler/kernels/reference_kernel_backends.json"
    ))
    .expect("backend fixture JSON");

    assert_valid(REQUEST_SCHEMA_ID, &request_value);
    for backend in backend_values
        .as_array()
        .expect("backend fixture must be an array")
    {
        assert_valid(BACKEND_SCHEMA_ID, backend);
    }

    let request: KernelRequest =
        serde_json::from_value(request_value).expect("request Rust contract");
    request.validate().expect("request semantic validation");
    let profiles: Vec<KernelBackendProfile> =
        serde_json::from_value(backend_values).expect("backend Rust contract");
    for profile in &profiles {
        profile.validate().expect("backend semantic validation");
    }

    let reference = execute_kernel_reference(&request).expect("CPU reference execution");
    assert_valid(
        RESULT_SCHEMA_ID,
        &serde_json::to_value(reference).expect("reference result JSON"),
    );

    let simulator = KernelSimulatorOptions {
        target: TargetBackend::Photonic,
        effective_bits: 12,
        noise_fraction: 0.0,
        seed: 17,
    };
    let simulated = execute_kernel_simulator(&request, simulator).expect("simulator execution");
    assert_valid(
        RESULT_SCHEMA_ID,
        &serde_json::to_value(simulated).expect("simulator result JSON"),
    );

    let plan = select_kernel(&request, &profiles, OptimizationObjective::Latency)
        .expect("kernel selection");
    assert_valid(
        PLAN_SCHEMA_ID,
        &serde_json::to_value(plan).expect("selection plan JSON"),
    );

    let benchmark = benchmark_kernel(&request, simulator, 10).expect("kernel benchmark");
    assert_valid(
        BENCHMARK_SCHEMA_ID,
        &serde_json::to_value(benchmark).expect("benchmark JSON"),
    );
}

#[test]
fn published_schemas_reject_missing_identity_and_unknown_fields() {
    let mut request: Value = serde_json::from_str(include_str!(
        "../../awen-compiler/kernels/transformer_qkv.json"
    ))
    .expect("request fixture JSON");
    request
        .as_object_mut()
        .expect("request object")
        .remove("id");
    assert!(!compile_schema(REQUEST_SCHEMA_ID).is_valid(&request));

    let mut backend: Value = serde_json::from_str::<Value>(include_str!(
        "../../awen-compiler/kernels/reference_kernel_backends.json"
    ))
    .expect("backend fixture JSON")[0]
        .clone();
    backend
        .as_object_mut()
        .expect("backend object")
        .insert("unversioned_extension".to_string(), Value::Bool(true));
    assert!(!compile_schema(BACKEND_SCHEMA_ID).is_valid(&backend));
}
