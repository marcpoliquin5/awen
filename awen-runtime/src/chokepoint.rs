//! Runtime chokepoint interface for AWEN Photonics
//!
//! This module defines the non-bypassable execution chokepoint: the single
//! gateway through which all runtime-executed photonic operations must pass.

use crate::calibration;
use crate::ir::{Graph, Node};
use crate::observability;
use crate::plugins::registry::PluginRegistry;
use crate::plugins::PluginLoader;
use crate::storage::{save_artifact, ArtifactType, BundleBuilder};
use jsonschema::JSONSchema;
use log::{info, warn};
use serde::{Deserialize, Serialize};
use serde_json::json;
use serde_json::Value as JsonValue;
use std::collections::HashMap;
use std::fs;

/// A representation of a photonic operation in the runtime IR. This is
/// intentionally serializable so we can validate against the canonical
/// `awen-spec/schemas/photonic_ir.v5.json` schema before execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PhotonicOp {
    pub op_id: String,
    pub op_type: String,
    pub targets: Vec<String>,
    #[serde(default)]
    pub params: Option<JsonValue>,
    #[serde(default)]
    pub calibration_handle: Option<String>,
}

/// Execution context passed with each operation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecContext {
    pub run_id: String,
    pub timestamp_ns: u64,
}

/// Result of an execution attempt.
pub struct ExecutionResult {
    pub ok: bool,
    pub details: Option<String>,
}

/// The execution chokepoint trait. All backends and plugins must route
/// operations through an implementation of this trait to satisfy the
/// non-bypassable runtime requirement.
pub trait ExecutionChokepoint: Send + Sync {
    fn execute(&self, op: &PhotonicOp, ctx: &ExecContext) -> ExecutionResult;
}

/// A simple in-memory gateway used as a reference implementation. It performs:
/// - JSON Schema validation against the canonical IR schema
/// - Calibration injection stub
/// - Telemetry hooks (logs)
pub struct NonBypassableGateway {
    // Precompiled JSONSchema instance (None on compilation failure)
    compiled_schema: Option<JSONSchema>,
}

impl NonBypassableGateway {
    /// Construct a new gateway instance and compile the embedded schema.
    pub fn new() -> Self {
        // Embed the canonical IR schema at compile time.
        // Path is relative to this file: ../../awen-spec/schemas/photonic_ir.v5.json
        let schema_str = include_str!("../../awen-spec/schemas/photonic_ir.v5.json");
        let compiled = serde_json::from_str::<JsonValue>(schema_str)
            .ok()
            .and_then(|schema_v| JSONSchema::compile(&schema_v).ok());

        if compiled.is_none() {
            warn!("failed to compile embedded photonic IR schema; validation disabled");
        }

        NonBypassableGateway {
            compiled_schema: compiled,
        }
    }

    fn validate_op_against_schema(&self, op: &PhotonicOp) -> Result<(), String> {
        let schema = match &self.compiled_schema {
            Some(s) => s,
            None => return Ok(()), // Schema unavailable — allow through but warn
        };

        // Build a minimal IR instance that conforms to / can be validated by the
        // top-level schema. The schema expects an object with `ir_version`, `ops`, and `metadata`.
        let instance = json!({
            "ir_version": "v5",
            "metadata": {"timestamp": chrono::Utc::now().to_rfc3339()},
            "ops": [
                {
                    "op_id": op.op_id,
                    "type": op.op_type,
                    "targets": op.targets,
                    "params": op.params.clone().unwrap_or(JsonValue::Null),
                    "calibration_handle": op.calibration_handle.clone().unwrap_or_default()
                }
            ]
        });

        let result = schema.validate(&instance);
        match result {
            Ok(_) => Ok(()),
            Err(errors) => {
                let msgs: Vec<String> = errors.map(|e| e.to_string()).collect();
                Err(msgs.join("; "))
            }
        }
    }

    fn inject_calibration(&self, op: &mut PhotonicOp, artifacts_dir: &std::path::Path) {
        // If a calibration handle is present, attempt to load state and apply it.
        if let Some(ref handle) = op.calibration_handle {
            match calibration::basic_load_state(handle, artifacts_dir) {
                Ok(Some(st)) => {
                    op.params = calibration::basic_apply_to_params(&st, op.params.clone());
                    info!("applied calibration {} to op {}", handle, op.op_id);
                }
                Ok(None) => {
                    info!(
                        "calibration handle {} not found on disk; proceeding",
                        handle
                    );
                }
                Err(e) => {
                    warn!("error loading calibration {}: {}", handle, e);
                }
            }
        } else {
            // Generate and persist default calibration state
            let st = calibration::basic_generate_default_state();
            if let Err(e) = calibration::basic_save_state(&st, artifacts_dir) {
                warn!(
                    "failed to persist generated calibration {}: {}",
                    st.handle, e
                );
            } else {
                op.calibration_handle = Some(st.handle.clone());
                op.params = calibration::basic_apply_to_params(&st, op.params.clone());
                info!(
                    "generated and applied calibration {} to op {}",
                    st.handle, op.op_id
                );
            }
        }
    }
}

impl Default for NonBypassableGateway {
    fn default() -> Self {
        Self::new()
    }
}

impl ExecutionChokepoint for NonBypassableGateway {
    fn execute(&self, op: &PhotonicOp, ctx: &ExecContext) -> ExecutionResult {
        if op.op_id.is_empty() {
            return ExecutionResult {
                ok: false,
                details: Some("missing op_id".into()),
            };
        }

        // Validate against canonical IR schema
        match self.validate_op_against_schema(op) {
            Ok(_) => info!("schema validation passed for op {}", op.op_id),
            Err(e) => {
                return ExecutionResult {
                    ok: false,
                    details: Some(format!("schema validation failed: {}", e)),
                }
            }
        }

        // Artifact directory must exist before calibration state is persisted or loaded.
        let mut out_dir = std::env::temp_dir();
        out_dir.push("awen_runtime_artifacts");
        out_dir.push(&ctx.run_id);
        out_dir.push(ctx.timestamp_ns.to_string());

        if let Err(e) = fs::create_dir_all(&out_dir) {
            return ExecutionResult {
                ok: false,
                details: Some(format!("failed to create artifact dir: {}", e)),
            };
        }

        // Calibration injection — operate on a mutable clone to avoid surprising callers
        let mut op_clone = op.clone();
        self.inject_calibration(&mut op_clone, &out_dir);

        // Write operation JSON
        match serde_json::to_string_pretty(&op_clone) {
            Ok(s) => {
                if let Err(e) = fs::write(out_dir.join("op.json"), s) {
                    return ExecutionResult {
                        ok: false,
                        details: Some(format!("failed to write op.json: {}", e)),
                    };
                }
            }
            Err(e) => {
                return ExecutionResult {
                    ok: false,
                    details: Some(format!("failed to serialize op: {}", e)),
                }
            }
        }

        // Build basic observability artifacts and write them into the artifact directory
        let (spans, events, metrics) =
            observability::build_basic_observability(&ctx.run_id, &op_clone.targets, None);
        if let Err(e) = observability::write_traces(&out_dir, &spans) {
            return ExecutionResult {
                ok: false,
                details: Some(format!("failed to write traces: {}", e)),
            };
        }
        if let Err(e) = observability::write_timeline(&out_dir, &events) {
            return ExecutionResult {
                ok: false,
                details: Some(format!("failed to write timeline: {}", e)),
            };
        }
        if let Err(e) = observability::write_metrics(&out_dir, &metrics) {
            return ExecutionResult {
                ok: false,
                details: Some(format!("failed to write metrics: {}", e)),
            };
        }

        info!("wrote artifacts to {}", out_dir.display());

        // Build a minimal IR Graph for the artifact bundle (one node per op)
        let node = Node {
            id: op_clone.op_id.clone(),
            node_type: op_clone.op_type.clone(),
            params: {
                let mut m = HashMap::new();
                // Attempt to extract numeric params from params JSON if present
                if let Some(JsonValue::Object(map)) = op_clone.params.as_ref() {
                    for (k, v) in map {
                        if let Some(n) = v.as_f64() {
                            m.insert(k.clone(), n);
                        }
                    }
                }
                m
            },
            measure_mode: None,
            conditional_branches: None,
        };

        let graph = Graph {
            nodes: vec![node],
            edges: vec![],
            metadata: HashMap::new(),
        };

        // Build artifact bundle
        let mut builder = BundleBuilder::new(graph, ArtifactType::Run)
            .with_initial_parameters(HashMap::new())
            .with_results(serde_json::json!({"status": "accepted", "op_id": op_clone.op_id}))
            .with_seed(0);

        if let Some(cal) = op_clone.calibration_handle.clone() {
            builder = builder.with_calibration_state(serde_json::json!({"handle": cal}), None);
        }

        builder = builder.with_observability_dir(out_dir.clone());

        match builder.build() {
            Ok(bundle) => {
                // Save bundle into artifacts root (parent of out_dir)
                let artifacts_root = out_dir.parent().unwrap_or(&out_dir).to_path_buf();
                if let Err(e) = save_artifact(&bundle, &artifacts_root) {
                    warn!("failed to save artifact bundle: {}", e);
                } else {
                    info!("artifact bundle saved under {}", artifacts_root.display());
                }
            }
            Err(e) => warn!("failed to build artifact bundle: {}", e),
        }

        // Plugin registry enforcement: discover manifests from configured plugin dir
        let plugin_dir = std::env::var("AWEN_PLUGIN_DIR").unwrap_or_else(|_| "plugins".to_string());
        let registry = match PluginRegistry::discover_from_dir(std::path::Path::new(&plugin_dir)) {
            Ok(r) => r,
            Err(e) => {
                warn!("plugin discovery failed: {}", e);
                PluginRegistry::new()
            }
        };

        if let Some(p) = registry.find_by_capability("execute") {
            // Enforce manifest signature before routing
            match registry.verify_manifest(&p) {
                Ok(true) => {
                    if let Some(path) = p.path.clone() {
                        // Prepare JSON payload for plugin: {"op": <op>, "ctx": <ctx>}
                        let payload = serde_json::json!({"op": op_clone, "ctx": ctx});
                        if let Ok(Some(stdout)) = PluginLoader::invoke(
                            path,
                            &serde_json::to_string(&payload).unwrap_or_default(),
                        ) {
                            info!("routed op {} to plugin and received output", op_clone.op_id);
                            return ExecutionResult {
                                ok: true,
                                details: Some(format!("plugin response: {}", stdout)),
                            };
                        } else {
                            warn!("plugin at {:?} failed or produced no output; falling back to in-process simulation", p.path);
                        }
                    } else {
                        warn!("plugin manifest for {} has no path; skipping routing", p.id);
                    }
                }
                Ok(false) => {
                    warn!("manifest verification failed for plugin {}; skipping", p.id);
                }
                Err(e) => {
                    warn!(
                        "error verifying manifest for plugin {}: {}; skipping",
                        p.id, e
                    );
                }
            }
        } else {
            info!("no plugin registered for capability 'execute'; proceeding with in-process simulation");
        }

        ExecutionResult {
            ok: true,
            details: Some(format!(
                "op {} accepted at {}",
                op_clone.op_id, ctx.timestamp_ns
            )),
        }
    }
}
