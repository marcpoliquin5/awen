// Engine skeleton

use crate::ir::Graph;
use crate::plugins::run_reference_simulator;
use crate::storage::save_artifact;
use anyhow::Result;
use uuid::Uuid;
use std::path::PathBuf;
use crate::hal::{self, LabDevice};
use std::collections::HashMap;
use crate::observability;
use std::path::Path;
use chrono::Utc;
use crate::state::{StateEvolver, CoherenceManager, ReferenceStateEvolver, ReferenceCoherenceManager, QuantumState, QuantumMode};

pub struct Engine {}

impl Engine {
    pub fn new() -> Self { Self {} }

    /// Run the provided IR graph, optionally with a seed for deterministic replay.
    pub fn run_graph(&self, graph: &Graph, seed: Option<u64>) -> Result<PathBuf> {
        // Validate IR: check conditional branches reference valid nodes
        crate::ir::validate_graph(graph).map_err(|e| anyhow::anyhow!(e))?;

        let run_seed = seed.unwrap_or(42);

        // Initialize coherence window and quantum state evolver for quantum-capable graphs
        let coherence_mgr = ReferenceCoherenceManager;
        let state_evolver = ReferenceStateEvolver;

        // Create coherence window for this execution
        // Assume graph execution takes ~1 microsecond per node (realistic for photonic systems)
        let execution_duration_ns = (graph.nodes.len() as u64) * 1_000; // 1µs per node
        let coherence_window = coherence_mgr.create_window(0, 10_000_000, "gaussian")?; // 10ms coherence

        // Initialize quantum state: one mode per node (simplified; real systems track physical modes)
        let initial_modes: Vec<QuantumMode> = graph.nodes.iter().enumerate()
            .map(|(i, n)| QuantumMode {
                mode_id: format!("mode_{}", i),
                mode_type: "quantum_fock".to_string(),
                photon_numbers: Some(vec![0, 1, 2]),
                amplitudes: Some(vec![1.0, 0.0, 0.0]), // |0⟩ state
                phases: Some(vec![0.0, 0.0, 0.0]),
            })
            .collect();

        let mut quantum_state = QuantumState {
            id: format!("qstate-{}", run_seed),
            modes: initial_modes,
            coherence_window: coherence_window.clone(),
            seed: Some(run_seed),
            provenance: {
                let mut p = HashMap::new();
                p.insert("origin".to_string(), "engine.run_graph".to_string());
                p
            },
        };

        // Track quantum state evolution through simulation
        let mut state_history: Vec<QuantumState> = vec![quantum_state.clone()];
        let mut measurement_outcomes: HashMap<String, crate::state::MeasurementOutcome> = HashMap::new();

        // Run reference simulator for classical simulation
        let sim = run_reference_simulator(graph, Some(run_seed))?;

        // Simulate quantum gate operations on each node (demonstration)
        // Build a set of nodes to execute, starting with root nodes
        let mut nodes_to_execute: Vec<String> = graph.nodes.iter().map(|n| n.id.clone()).collect();
        let mut executed_nodes = std::collections::HashSet::new();
        let mut idx = 0usize;

        while idx < nodes_to_execute.len() {
            let node_id = &nodes_to_execute[idx];
            idx += 1;
            
            if executed_nodes.contains(node_id) {
                continue; // skip already executed nodes
            }
            executed_nodes.insert(node_id.clone());

            let node = graph.nodes.iter().find(|n| &n.id == node_id)
                .ok_or_else(|| anyhow::anyhow!("node {} not found", node_id))?;

            // Validate coherence before processing this node
            let current_time_ns = (idx as u64) * 1_000; // increment time by 1µs per node
            coherence_mgr.validate_coherence(&quantum_state, current_time_ns)?;

            // Apply gate evolution based on node type
            if !node.params.is_empty() {
                let params = &node.params;
                match node.node_type.as_str() {
                    "MZI" => {
                        // MZI acts as a beam splitter; couple modes
                        let mut gate_params = params.clone();
                        gate_params.insert("mode1".to_string(), 0.0);
                        gate_params.insert("mode2".to_string(), 1.0);
                        gate_params.insert("theta".to_string(), params.get("phase").copied().unwrap_or(0.785)); // π/4 default

                        quantum_state = state_evolver.evolve_state(&quantum_state, "BS", &gate_params)?;
                        state_history.push(quantum_state.clone());
                    }
                    "PS" => {
                        // Phase shifter applies phase shift
                        let mut gate_params = params.clone();
                        gate_params.insert("mode_id".to_string(), 0.0);
                        if !gate_params.contains_key("phase") {
                            gate_params.insert("phase".to_string(), 0.1);
                        }
                        quantum_state = state_evolver.evolve_state(&quantum_state, "PS", &gate_params)?;
                        state_history.push(quantum_state.clone());
                    }
                    "DETECTOR" => {
                        // Measurement: destructive measurement on mode specified in measure_mode or default to mode_0
                        let measure_mode = node.measure_mode.as_deref().unwrap_or("mode_0");
                        let outcome = state_evolver.measure(&quantum_state, measure_mode, Some(run_seed + idx as u64))?;
                        measurement_outcomes.insert(node_id.clone(), outcome.clone());
                        quantum_state = outcome.collapsed_state.ok_or_else(|| anyhow::anyhow!("measurement failed"))?;
                        state_history.push(quantum_state.clone());

                        // Handle measurement-conditioned branches
                        if let Some(branches) = &node.conditional_branches {
                            for branch in branches {
                                if outcome.outcome_index == branch.outcome_index {
                                    // Execute then_nodes
                                    for then_id in &branch.then_nodes {
                                        if !executed_nodes.contains(then_id) {
                                            nodes_to_execute.push(then_id.clone());
                                        }
                                    }
                                } else if let Some(else_nodes) = &branch.else_nodes {
                                    // Execute else_nodes
                                    for else_id in else_nodes {
                                        if !executed_nodes.contains(else_id) {
                                            nodes_to_execute.push(else_id.clone());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    _ => {
                        // Other node types: skip quantum evolution for now
                    }
                }
            }
        }

        // Create artifact bundle directory
        let run_id = Uuid::new_v4().to_string();
        let out_dir = std::env::current_dir()?.join(format!("awen_run_{}", run_id));
        std::fs::create_dir_all(&out_dir)?;

        // Save IR
        let ir_path = out_dir.join("ir.json");
        let ir_data = serde_json::to_string_pretty(graph)?;
        std::fs::write(&ir_path, ir_data)?;

        // Save simulation results
        let results_path = out_dir.join("results.json");
        let results_data = serde_json::to_string_pretty(&sim)?;
        std::fs::write(&results_path, results_data)?;

        // Save quantum state history (new artifact)
        let state_history_path = out_dir.join("quantum_states.json");
        let state_history_data = serde_json::to_string_pretty(&state_history)?;
        std::fs::write(&state_history_path, state_history_data)?;

        // Save measurement outcomes (new artifact)
        let measurements_path = out_dir.join("measurements.json");
        let measurements_data = serde_json::to_string_pretty(&measurement_outcomes)?;
        std::fs::write(&measurements_path, measurements_data)?;

        // Save a simple trace (reuse results for now)
        let trace_path = out_dir.join("trace.json");
        std::fs::write(&trace_path, serde_json::to_string_pretty(&sim)?)?;

        // Build and write basic observability artifacts (traces.jsonl, timeline.json, metrics.json)
        // Create simple node id list
        let node_ids: Vec<String> = graph.nodes.iter().map(|n| n.id.clone()).collect();
        let (spans, events, metrics) = observability::build_basic_observability(&run_id, &node_ids, Some(run_seed));
        observability::write_traces(&out_dir, &spans)?;
        observability::write_timeline(&out_dir, &events)?;
        observability::write_metrics(&out_dir, &metrics)?;

        // More structured spans/events: IR validate, scheduling, per-node execution and measurement
        let mut extra_spans: Vec<observability::Span> = Vec::new();
        let mut extra_events: Vec<observability::TimelineEvent> = Vec::new();

        // IR validate span
        extra_spans.push(observability::Span { id: format!("{}-ir-validate", run_id), parent: None, name: "ir_validate".to_string(), start_iso: Utc::now().to_rfc3339(), end_iso: Utc::now().to_rfc3339(), attributes: HashMap::new() });
        // scheduling span
        extra_spans.push(observability::Span { id: format!("{}-schedule", run_id), parent: None, name: "scheduling".to_string(), start_iso: Utc::now().to_rfc3339(), end_iso: Utc::now().to_rfc3339(), attributes: HashMap::new() });

        // coherence window span
        let mut coh_attrs = HashMap::new();
        coh_attrs.insert("coherence_start_ns".to_string(), coherence_window.start_ns.to_string());
        coh_attrs.insert("coherence_end_ns".to_string(), coherence_window.end_ns.to_string());
        extra_spans.push(observability::Span { id: format!("{}-coherence", run_id), parent: None, name: "coherence_window".to_string(), start_iso: Utc::now().to_rfc3339(), end_iso: Utc::now().to_rfc3339(), attributes: coh_attrs });

        for nr in &sim.node_results {
            let mut attrs = HashMap::new();
            attrs.insert("node_id".to_string(), nr.node_id.clone());
            attrs.insert("phase_noise".to_string(), format!("{}", nr.phase_noise));
            let span = observability::Span { id: format!("{}-node-{}", run_id, nr.node_id), parent: None, name: format!("exec:{}", nr.node_id), start_iso: Utc::now().to_rfc3339(), end_iso: Utc::now().to_rfc3339(), attributes: attrs.clone() };
            extra_spans.push(span);

            let ev = observability::TimelineEvent { lane: "kernel".to_string(), name: format!("exec:{}", nr.node_id), start_ms: Utc::now().timestamp_millis() as u128, end_ms: (Utc::now().timestamp_millis()+1) as u128, attributes: attrs };
            extra_events.push(ev);
        }

        // merge previous and extra
        let mut all_spans = spans.clone();
        all_spans.extend(extra_spans);
        let mut all_events = events.clone();
        all_events.extend(extra_events);

        // rewrite observability artifacts including detailed spans/events
        observability::write_traces(&out_dir, &all_spans)?;
        observability::write_timeline(&out_dir, &all_events)?;
        observability::write_metrics(&out_dir, &metrics)?;

        // Save metadata using storage helper
        save_artifact(&out_dir);

        Ok(out_dir)
    }

    /// Apply a calibration mapping through the HAL, enforcing safety limits if provided.
    pub fn apply_calibration(&self, mapping: &HashMap<String, f64>, safety: Option<&hal::SafetyLimits>) -> Result<hal::CalibrationResult> {
        // In a realistic runtime this would select a real device from a registry. For now use the simulated device.
        let dev = hal::SimulatedDevice::new();
        let res = dev.apply_calibration(mapping, safety).map_err(|e: String| anyhow::anyhow!(e))?;
        Ok(res)
    }
}

// Ensure gradient providers and other pluggable subsystems are registered during runtime initialization.
impl Default for Engine {
    fn default() -> Self {
        // register default gradient providers into global registry so CLI and runtime can discover them
        crate::gradients::register_defaults_to_global();
        Engine::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir;

    #[test]
    fn integration_run_example_ir() {
        // Load example IR shipped with the crate
        let graph = ir::load_from_json("example_ir.json").expect("failed to load example_ir.json");
        let engine = Engine::new();
        let out = engine.run_graph(&graph, Some(42)).expect("engine run failed");
        assert!(out.exists(), "output directory does not exist");
        assert!(out.join("results.json").exists(), "results.json missing");
        assert!(out.join("ir.json").exists(), "ir.json missing");
        // Observability artifacts
        assert!(out.join("traces.jsonl").exists(), "traces.jsonl missing");
        assert!(out.join("timeline.json").exists(), "timeline.json missing");
        assert!(out.join("metrics.json").exists(), "metrics.json missing");
    }

    #[test]
    fn test_timeline_contains_kernel_events() {
        let graph = ir::load_from_json("example_ir.json").expect("failed to load example_ir.json");
        let engine = Engine::new();
        let out = engine.run_graph(&graph, Some(42)).expect("engine run failed");
        let timeline_path = out.join("timeline.json");
        let data = std::fs::read_to_string(&timeline_path).expect("read timeline");
        let events: Vec<observability::TimelineEvent> = serde_json::from_str(&data).expect("parse timeline");
        assert!(events.iter().any(|e| e.lane == "kernel"), "no kernel events in timeline");
    }

    #[test]
    fn test_apply_calibration_enforces_safety() {
        let engine = Engine::new();
        let mut mapping = HashMap::new();
        mapping.insert("mzi_0:phase".to_string(), 5.0_f64);
        let safety = hal::SafetyLimits { max_voltage: Some(1.0), min_voltage: Some(-1.0), max_temperature: None, notes: None };
        let res = engine.apply_calibration(&mapping, Some(&safety)).expect("apply calibration");
        assert!(res.applied.get("mzi_0:phase").is_some());
        assert!(*res.applied.get("mzi_0:phase").unwrap() <= 1.0_f64, "value should be clamped to max_voltage");
        assert!(res.warnings.len() >= 1, "should emit a warning when clamping");
    }

    #[test]
    fn test_quantum_state_artifact_created() {
        let graph = ir::load_from_json("example_ir.json").expect("failed to load example_ir.json");
        let engine = Engine::new();
        let out = engine.run_graph(&graph, Some(42)).expect("engine run failed");
        // Verify quantum state artifact is created
        assert!(out.join("quantum_states.json").exists(), "quantum_states.json missing");
        let state_data = std::fs::read_to_string(out.join("quantum_states.json")).expect("read quantum states");
        let states: Vec<crate::state::QuantumState> = serde_json::from_str(&state_data).expect("parse quantum states");
        assert!(!states.is_empty(), "quantum state history should not be empty");
        // First state should have initial modes
        assert!(!states[0].modes.is_empty(), "initial quantum state should have modes");
    }

    #[test]
    fn test_measurements_artifact_created() {
        let graph = ir::load_from_json("example_ir.json").expect("failed to load example_ir.json");
        let engine = Engine::new();
        let out = engine.run_graph(&graph, Some(42)).expect("engine run failed");
        // Verify measurements artifact is created
        assert!(out.join("measurements.json").exists(), "measurements.json missing");
        let measures_data = std::fs::read_to_string(out.join("measurements.json")).expect("read measurements");
        let _measures: std::collections::HashMap<String, crate::state::MeasurementOutcome> = serde_json::from_str(&measures_data).expect("parse measurements");
        // Measurements may be empty if no DETECTOR nodes in graph, which is fine
    }

    #[test]
    fn test_ir_validation_fails_on_invalid_branches() {
        let mut graph = ir::Graph {
            nodes: vec![
                ir::Node {
                    id: "m0".to_string(),
                    node_type: "DETECTOR".to_string(),
                    params: Default::default(),
                    measure_mode: None,
                    conditional_branches: Some(vec![
                        ir::ConditionalBranch {
                            outcome_index: 0,
                            then_nodes: vec!["nonexistent".to_string()], // references non-existent node
                            else_nodes: None,
                        }
                    ]),
                }
            ],
            edges: vec![],
            metadata: Default::default(),
        };

        let result = ir::validate_graph(&graph);
        assert!(result.is_err(), "validation should reject invalid branch references");
    }

    #[test]
    fn test_ir_validation_passes_on_valid_branches() {
        let graph = ir::Graph {
            nodes: vec![
                ir::Node {
                    id: "m0".to_string(),
                    node_type: "DETECTOR".to_string(),
                    params: Default::default(),
                    measure_mode: None,
                    conditional_branches: Some(vec![
                        ir::ConditionalBranch {
                            outcome_index: 0,
                            then_nodes: vec!["mzi1".to_string()],
                            else_nodes: None,
                        }
                    ]),
                },
                ir::Node {
                    id: "mzi1".to_string(),
                    node_type: "MZI".to_string(),
                    params: Default::default(),
                    measure_mode: None,
                    conditional_branches: None,
                }
            ],
            edges: vec![],
            metadata: Default::default(),
        };

        let result = ir::validate_graph(&graph);
        assert!(result.is_ok(), "validation should accept valid branch references");
    }
}
