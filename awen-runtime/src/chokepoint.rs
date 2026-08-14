//! Non-bypassable typed classical/quantum photonic execution boundary.

use crate::ir::{Graph, Node};
use crate::observability;
use crate::photonic::PhotonicProgram;
use crate::plugins::registry::PluginRegistry;
use crate::plugins::PluginLoader;
use crate::storage::{save_artifact, ArtifactType, BundleBuilder};
use jsonschema::JSONSchema;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fs;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExecContext {
    pub run_id: String,
    pub timestamp_ns: u64,
}

pub struct ExecutionResult {
    pub ok: bool,
    pub details: Option<String>,
}

pub trait ExecutionChokepoint: Send + Sync {
    fn execute(&self, program: &PhotonicProgram, ctx: &ExecContext) -> ExecutionResult;
}

pub struct NonBypassableGateway {
    classical_schema: Option<JSONSchema>,
    quantum_schema: Option<JSONSchema>,
    interop_schema: Option<JSONSchema>,
}

impl NonBypassableGateway {
    pub fn new() -> Self {
        let classical_schema = compile_schema(include_str!(
            "../../awen-spec/schemas/awen_photonic_program.v1.json"
        ));
        let quantum_schema = compile_schema(include_str!(
            "../../awen-spec/schemas/awen_qphotonic_program.v1.json"
        ));
        let interop_schema = compile_schema(include_str!(
            "../../awen-spec/schemas/awen_photonic_interop.v1.json"
        ));
        if classical_schema.is_none() || quantum_schema.is_none() || interop_schema.is_none() {
            warn!("failed to compile a typed photonic schema; that dialect will fail closed");
        }
        Self {
            classical_schema,
            quantum_schema,
            interop_schema,
        }
    }

    fn validate_program(&self, program: &PhotonicProgram) -> Result<(), String> {
        program.validate().map_err(|error| error.to_string())?;
        match program {
            PhotonicProgram::Classical(program) => {
                validate_schema(self.classical_schema.as_ref(), program, "awen.photonic")
            }
            PhotonicProgram::Quantum(program) => {
                validate_schema(self.quantum_schema.as_ref(), program, "awen.qphotonic")
            }
            PhotonicProgram::Interop(program) => validate_schema(
                self.interop_schema.as_ref(),
                program,
                "awen.photonic-interop",
            ),
        }
    }
}

impl Default for NonBypassableGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionChokepoint for NonBypassableGateway {
    fn execute(&self, program: &PhotonicProgram, ctx: &ExecContext) -> ExecutionResult {
        if !portable_path_component(&ctx.run_id) {
            return failed("execution context requires a portable run id");
        }
        if let Err(error) = self.validate_program(program) {
            return failed(&format!("typed dialect validation failed: {error}"));
        }

        let mut output_dir = std::env::temp_dir();
        output_dir.push("awen_runtime_artifacts");
        output_dir.push(&ctx.run_id);
        output_dir.push(ctx.timestamp_ns.to_string());
        if let Err(error) = fs::create_dir_all(&output_dir) {
            return failed(&format!("failed to create artifact directory: {error}"));
        }

        let program_bytes = match serde_json::to_vec_pretty(program) {
            Ok(bytes) => bytes,
            Err(error) => return failed(&format!("failed to serialize typed program: {error}")),
        };
        let fingerprint = format!("sha256:{}", hex::encode(Sha256::digest(&program_bytes)));
        if let Err(error) = fs::write(output_dir.join("typed_program.json"), &program_bytes) {
            return failed(&format!("failed to write typed program: {error}"));
        }

        let targets = program_targets(program);
        let (spans, events, metrics) =
            observability::build_basic_observability(&ctx.run_id, &targets, None);
        if let Err(error) = observability::write_traces(&output_dir, &spans) {
            return failed(&format!("failed to write traces: {error}"));
        }
        if let Err(error) = observability::write_timeline(&output_dir, &events) {
            return failed(&format!("failed to write timeline: {error}"));
        }
        if let Err(error) = observability::write_metrics(&output_dir, &metrics) {
            return failed(&format!("failed to write metrics: {error}"));
        }

        let graph = Graph {
            nodes: vec![Node {
                id: program.program_id().to_string(),
                node_type: program.dialect_name().to_string(),
                params: HashMap::new(),
                measure_mode: None,
                conditional_branches: None,
            }],
            edges: Vec::new(),
            metadata: HashMap::from([
                ("dialect".to_string(), program.dialect_name().to_string()),
                ("typed_program_fingerprint".to_string(), fingerprint.clone()),
            ]),
        };
        let mut builder = BundleBuilder::new(graph, ArtifactType::Run)
            .with_initial_parameters(HashMap::new())
            .with_results(serde_json::json!({
                "status": "accepted",
                "dialect": program.dialect_name(),
                "program_id": program.program_id(),
                "typed_program_fingerprint": fingerprint,
                "dialect_contract": program
            }))
            .with_seed(program_seed(program))
            .with_observability_dir(output_dir.clone());
        if let Some(calibration) = classical_calibration_record(program) {
            builder = builder.with_calibration_state(calibration, None);
        }
        match builder.build() {
            Ok(bundle) => {
                let root = output_dir.parent().unwrap_or(&output_dir);
                if let Err(error) = save_artifact(&bundle, root) {
                    return failed(&format!("failed to save typed artifact bundle: {error}"));
                }
            }
            Err(error) => {
                return failed(&format!("failed to build typed artifact bundle: {error}"))
            }
        }

        let plugin_dir = std::env::var("AWEN_PLUGIN_DIR").unwrap_or_else(|_| "plugins".to_string());
        let registry = match PluginRegistry::discover_from_dir(std::path::Path::new(&plugin_dir)) {
            Ok(registry) => registry,
            Err(error) => {
                warn!("plugin discovery failed: {error}");
                PluginRegistry::new()
            }
        };
        let capability = format!("execute:{}", program.dialect_name());
        if let Some(plugin) = registry.find_by_capability(&capability) {
            match registry.verify_manifest(&plugin) {
                Ok(true) => {
                    let Some(path) = plugin.path.clone() else {
                        return failed("typed dialect plugin manifest has no executable path");
                    };
                    let payload = serde_json::json!({"program": program, "context": ctx});
                    let payload = match serde_json::to_string(&payload) {
                        Ok(payload) => payload,
                        Err(error) => {
                            return failed(&format!("failed to serialize plugin payload: {error}"))
                        }
                    };
                    return match PluginLoader::invoke(path, &payload) {
                        Ok(Some(stdout)) => ExecutionResult {
                            ok: true,
                            details: Some(format!("typed plugin response: {stdout}")),
                        },
                        Ok(None) => failed("typed dialect plugin produced no response"),
                        Err(error) => {
                            failed(&format!("typed dialect plugin invocation failed: {error}"))
                        }
                    };
                }
                Ok(false) => return failed("typed dialect plugin signature verification failed"),
                Err(error) => {
                    return failed(&format!(
                        "typed dialect plugin verification failed: {error}"
                    ))
                }
            }
        }

        info!(
            "accepted typed {} program '{}'",
            program.dialect_name(),
            program.program_id()
        );
        ExecutionResult {
            ok: true,
            details: Some(format!(
                "typed {} program '{}' accepted at {}",
                program.dialect_name(),
                program.program_id(),
                ctx.timestamp_ns
            )),
        }
    }
}

fn compile_schema(source: &str) -> Option<JSONSchema> {
    serde_json::from_str::<Value>(source)
        .ok()
        .and_then(|schema| JSONSchema::options().compile(&schema).ok())
}

fn validate_schema<T: Serialize>(
    schema: Option<&JSONSchema>,
    value: &T,
    dialect: &str,
) -> Result<(), String> {
    let schema = schema.ok_or_else(|| format!("{dialect} schema is unavailable"))?;
    let instance = serde_json::to_value(value).map_err(|error| error.to_string())?;
    schema.validate(&instance).map_err(|errors| {
        errors
            .map(|error| error.to_string())
            .collect::<Vec<_>>()
            .join("; ")
    })
}

fn failed(message: &str) -> ExecutionResult {
    ExecutionResult {
        ok: false,
        details: Some(message.to_string()),
    }
}

fn portable_path_component(value: &str) -> bool {
    !value.is_empty()
        && value != "."
        && value != ".."
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
}

fn program_targets(program: &PhotonicProgram) -> Vec<String> {
    match program {
        PhotonicProgram::Classical(program) => program
            .signals
            .iter()
            .map(|signal| signal.id.clone())
            .collect(),
        PhotonicProgram::Quantum(program) => {
            program.modes.iter().map(|mode| mode.id.clone()).collect()
        }
        PhotonicProgram::Interop(program) => program
            .operations
            .iter()
            .enumerate()
            .map(|(index, _)| format!("interop-{index}"))
            .collect(),
    }
}

fn program_seed(program: &PhotonicProgram) -> u64 {
    match program {
        PhotonicProgram::Classical(program) => program
            .operations
            .first()
            .map(|operation| operation.noise.seed)
            .unwrap_or(0),
        PhotonicProgram::Quantum(program) => program.execution.seed,
        PhotonicProgram::Interop(_) => 0,
    }
}

fn classical_calibration_record(program: &PhotonicProgram) -> Option<Value> {
    let PhotonicProgram::Classical(program) = program else {
        return None;
    };
    Some(serde_json::json!({
        "dialect": "awen.photonic",
        "calibrated_transfers": program.operations.iter().map(|operation| serde_json::json!({
            "operation_id": operation.op_id,
            "snapshot_id": operation.transfer.calibration_snapshot_id,
            "snapshot_fingerprint": operation.transfer.calibration_fingerprint,
            "model": operation.transfer.model
        })).collect::<Vec<_>>()
    }))
}
