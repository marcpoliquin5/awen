use crate::ir::Graph;
use anyhow::Result;
use rand::{rngs::StdRng, Rng, SeedableRng};
use serde::Serialize;
use std::collections::HashMap;

/// Extended node simulation result includes measurement outcomes and loss tracking.
#[derive(Serialize)]
pub struct NodeResult {
    pub node_id: String,
    pub out_amplitude: (f64, f64), // (real, imag)
    pub phase_noise: f64,
    pub power_loss: f64,
    pub measurement: Option<MeasurementResult>,
}

#[derive(Serialize)]
pub struct MeasurementResult {
    pub detector_id: String,
    pub outcome: Option<u64>, // None for non-quantum detectors in CF mode
    pub analog_value: Option<f64>,
}

#[derive(Serialize)]
pub struct SimulationResult {
    pub run_seed: u64,
    pub node_results: Vec<NodeResult>,
}

/// Reference simulator: supports node types: MZI, RING, DETECTOR, LOSS, DELAY.
/// - MZI: applies a phase shift parameter `phase` and optional `loss` param
/// - RING: applies frequency-dependent transfer approximation via `coupling` and `loss`
/// - DETECTOR: produces measurement outcomes (analog & optional digital probabilistic outcome)
/// - LOSS: multiply amplitude by (1 - loss)
pub fn run_reference_simulator(graph: &Graph, seed: Option<u64>) -> Result<SimulationResult> {
    let seed = seed.unwrap_or(0xDEADBEEF_u64);
    let mut rng = StdRng::seed_from_u64(seed);

    let input_amp = graph
        .metadata
        .get("input_amplitude")
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(1.0);

    let mut results = Vec::new();
    let mut current = (input_amp, 0.0_f64); // real, imag
    let mut accumulated_loss = 0.0_f64;

    for node in &graph.nodes {
        let node_type = node.node_type.to_lowercase();
        let mut phase_noise = 0.0_f64;
        let mut power_loss = 0.0_f64;
        let mut measurement = None;

        match node_type.as_str() {
            "mzi" => {
                let phi = node.params.get("phase").cloned().unwrap_or(0.0_f64);
                phase_noise = rng.gen_range(-1e-3..1e-3);
                let total_phase = phi + phase_noise;
                let (re, im) = current;
                let cos = total_phase.cos();
                let sin = total_phase.sin();
                let new_re = re * cos - im * sin;
                let new_im = re * sin + im * cos;
                current = (new_re, new_im);
                power_loss = node.params.get("loss").cloned().unwrap_or(0.0_f64);
                accumulated_loss += power_loss;
            }

            "ring" => {
                // Very small approximation: apply an extra phase shift scaled by `coupling` and `detuning`
                let coupling = node.params.get("coupling").cloned().unwrap_or(0.1);
                let detuning = node.params.get("detuning").cloned().unwrap_or(0.0);
                phase_noise = rng.gen_range(-2e-3..2e-3);
                let effective = (coupling * (1.0 / (1.0 + detuning.abs()))) + phase_noise;
                let (re, im) = current;
                let cos = effective.cos();
                let sin = effective.sin();
                current = (re * cos - im * sin, re * sin + im * cos);
                power_loss = node.params.get("loss").cloned().unwrap_or(0.0_f64);
                accumulated_loss += power_loss;
            }

            "loss" => {
                let loss = node.params.get("loss").cloned().unwrap_or(0.0_f64);
                let factor = 1.0 - loss.clamp(0.0, 1.0);
                current = (current.0 * factor, current.1 * factor);
                power_loss = loss;
                accumulated_loss += power_loss;
            }

            "detector" => {
                // Analog readout: power = re^2 + im^2; digital outcome (photon-count) sampled probabilistically
                let power = current.0 * current.0 + current.1 * current.1;
                let analog = Some(power);
                // For CF mode, optionally provide a Poisson-ish count when `quantum` param set
                let outcome = if node.params.get("quantum").cloned().unwrap_or(0.0) > 0.5 {
                    // sample 0/1 with probability proportional to power (clamped)
                    let p = power.min(1.0);
                    if rng.gen_bool(p) { Some(1u64) } else { Some(0u64) }
                } else {
                    None
                };
                measurement = Some(MeasurementResult {
                    detector_id: node.id.clone(),
                    outcome,
                    analog_value: analog,
                });
            }

            _ => {
                // passthrough
            }
        }

        results.push(NodeResult {
            node_id: node.id.clone(),
            out_amplitude: current,
            phase_noise,
            power_loss,
            measurement,
        });
    }

    Ok(SimulationResult {
        run_seed: seed,
        node_results: results,
    })
}
