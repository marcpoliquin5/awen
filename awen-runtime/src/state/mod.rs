// Quantum State & Coherence Window model (v0.1)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use anyhow::Result;
use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

/// A photonic quantum state mode: classical or quantum (Fock/mixed).
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuantumMode {
    pub mode_id: String,
    pub mode_type: String, // "classical", "quantum_fock", "mixed"
    pub photon_numbers: Option<Vec<u32>>,
    pub amplitudes: Option<Vec<f64>>,
    pub phases: Option<Vec<f64>>,
}

/// Coherence window: temporal validity of quantum state before decoherence.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct CoherenceWindow {
    pub id: String,
    pub start_ns: u64,
    pub end_ns: u64,
    pub decoherence_timescale_ns: Option<f64>,
    pub cross_mode_decoherence_ns: Option<f64>,
    pub idle_time_budget_ns: Option<u64>,
    pub notes: Option<String>,
}

/// A quantum state snapshot: modes + coherence window + provenance.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct QuantumState {
    pub id: String,
    pub modes: Vec<QuantumMode>,
    pub coherence_window: CoherenceWindow,
    pub seed: Option<u64>,
    pub provenance: HashMap<String, String>,
}

/// Measurement outcome on a mode.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct MeasurementOutcome {
    pub outcome_index: u32,
    pub photon_count: u32,
    pub probability: f64,
    pub collapsed_state: Option<QuantumState>,
    pub seed_used: Option<u64>,
}

/// Trait for quantum state evolution and measurement.
pub trait StateEvolver: Send + Sync {
    /// Evolve a quantum state via a gate (parametric: BS, PS, SQUEEZING, PDC).
    fn evolve_state(&self, state: &QuantumState, gate: &str, params: &HashMap<String, f64>) -> Result<QuantumState>;

    /// Measure a mode, collapse the state, return outcome.
    fn measure(&self, state: &QuantumState, mode_id: &str, seed: Option<u64>) -> Result<MeasurementOutcome>;

    /// Check if state is still coherent (within time window and budgets).
    fn is_coherent(&self, state: &QuantumState, current_time_ns: u64) -> bool;
}

/// Trait for coherence window tracking.
pub trait CoherenceManager: Send + Sync {
    /// Create a coherence window for a subgraph execution.
    fn create_window(&self, start_ns: u64, duration_ns: u64, decoherence_model: &str) -> Result<CoherenceWindow>;

    /// Validate state is within coherence window.
    fn validate_coherence(&self, state: &QuantumState, current_time_ns: u64) -> Result<()>;
}

/// Reference (stateless) quantum state evolver for testing and reference.
pub struct ReferenceStateEvolver;

impl StateEvolver for ReferenceStateEvolver {
    fn evolve_state(&self, state: &QuantumState, gate: &str, params: &HashMap<String, f64>) -> Result<QuantumState> {
        // Implement unitary gate evolution on quantum modes.
        // Each gate applies a unitary transformation to mode amplitudes/phases.
        let mut out = state.clone();

        match gate {
            "PS" => {
                // Phase Shift: modifies phase of selected mode
                let mode_id = params.get("mode_id")
                    .and_then(|v| Some((*v as usize).to_string()))
                    .ok_or_else(|| anyhow::anyhow!("PS gate requires mode_id parameter"))?;
                let phase_shift = params.get("phase")
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("PS gate requires phase parameter"))?;

                if let Some(mode) = out.modes.iter_mut().find(|m| m.mode_id == mode_id) {
                    if let Some(ref mut phases) = mode.phases {
                        for p in phases.iter_mut() {
                            *p += phase_shift;
                        }
                    }
                }
                out.provenance.insert("last_gate".to_string(), format!("PS(phase={})", phase_shift));
            }
            "BS" => {
                // Beam Splitter: couples two modes via unitary transformation
                let mode1 = params.get("mode1")
                    .and_then(|v| Some((*v as usize).to_string()))
                    .ok_or_else(|| anyhow::anyhow!("BS gate requires mode1 parameter"))?;
                let mode2 = params.get("mode2")
                    .and_then(|v| Some((*v as usize).to_string()))
                    .ok_or_else(|| anyhow::anyhow!("BS gate requires mode2 parameter"))?;
                let theta = params.get("theta")
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("BS gate requires theta parameter"))?;

                // BS unitary: [cos(θ), -sin(θ); sin(θ), cos(θ)] applied to mode amplitudes
                let idx1 = out.modes.iter().position(|m| m.mode_id == mode1);
                let idx2 = out.modes.iter().position(|m| m.mode_id == mode2);

                if let (Some(i1), Some(i2)) = (idx1, idx2) {
                    if let (Some(amp1), Some(amp2)) = 
                        (out.modes[i1].amplitudes.as_mut(), out.modes[i2].amplitudes.as_mut()) {
                        if amp1.len() > 0 && amp2.len() > 0 {
                            let a1 = amp1[0] * theta.cos() - amp2[0] * theta.sin();
                            let a2 = amp1[0] * theta.sin() + amp2[0] * theta.cos();
                            amp1[0] = a1;
                            amp2[0] = a2;
                        }
                    }
                }
                out.provenance.insert("last_gate".to_string(), format!("BS(theta={})", theta));
            }
            "SQUEEZING" => {
                // Squeezing: modifies variance of mode
                let mode_id = params.get("mode_id")
                    .and_then(|v| Some((*v as usize).to_string()))
                    .ok_or_else(|| anyhow::anyhow!("SQUEEZING gate requires mode_id parameter"))?;
                let r = params.get("r")
                    .copied()
                    .ok_or_else(|| anyhow::anyhow!("SQUEEZING gate requires r parameter"))?;

                if let Some(mode) = out.modes.iter_mut().find(|m| m.mode_id == mode_id) {
                    if let Some(ref mut amps) = mode.amplitudes {
                        for a in amps.iter_mut() {
                            *a *= r.exp(); // squeeze amplitude
                        }
                    }
                }
                out.provenance.insert("last_gate".to_string(), format!("SQUEEZING(r={})", r));
            }
            "PDC" => {
                // Parametric Down-Conversion: creates entangled pairs (simplified)
                let pump_id = params.get("pump_id")
                    .and_then(|v| Some((*v as usize).to_string()))
                    .ok_or_else(|| anyhow::anyhow!("PDC gate requires pump_id parameter"))?;
                let nonlinearity = params.get("nonlinearity")
                    .copied()
                    .unwrap_or(0.1);

                if let Some(mode) = out.modes.iter_mut().find(|m| m.mode_id == pump_id) {
                    if let Some(ref mut amps) = mode.amplitudes {
                        for a in amps.iter_mut() {
                            *a *= (1.0 + nonlinearity); // simplified entanglement generation
                        }
                    }
                }
                out.provenance.insert("last_gate".to_string(), format!("PDC(nonlinearity={})", nonlinearity));
            }
            _ => {
                return Err(anyhow::anyhow!("unknown gate: {}", gate));
            }
        }

        Ok(out)
    }

    fn measure(&self, state: &QuantumState, mode_id: &str, seed: Option<u64>) -> Result<MeasurementOutcome> {
        // Implement destructive measurement via seeded RNG sampling.
        let seed_val = seed.unwrap_or(0xDEADBEEF);
        let mut rng = StdRng::seed_from_u64(seed_val);

        // Find the target mode
        let mode = state.modes.iter()
            .find(|m| m.mode_id == mode_id)
            .ok_or_else(|| anyhow::anyhow!("mode {} not found", mode_id))?;

        // Sample outcome based on amplitudes (probabilities ~ |amplitude|^2)
        let amplitudes = mode.amplitudes.as_ref()
            .ok_or_else(|| anyhow::anyhow!("mode {} has no amplitudes", mode_id))?;

        // Compute outcome probabilities (normalized)
        let probs: Vec<f64> = amplitudes.iter()
            .map(|a| a.abs().powi(2))
            .collect();
        let total: f64 = probs.iter().sum();
        if total <= 0.0 {
            return Err(anyhow::anyhow!("invalid probability distribution for measurement"));
        }

        let normalized: Vec<f64> = probs.iter().map(|p| p / total).collect();

        // Sample outcome index via categorical distribution
        let r = rng.gen::<f64>();
        let mut cum_prob = 0.0;
        let mut outcome_idx = 0u32;
        for (i, &p) in normalized.iter().enumerate() {
            cum_prob += p;
            if r < cum_prob {
                outcome_idx = i as u32;
                break;
            }
        }

        // Build collapsed state (projection onto measured outcome)
        let mut collapsed_modes = state.modes.clone();
        if let Some(m) = collapsed_modes.iter_mut().find(|m| m.mode_id == mode_id) {
            if let Some(ref mut amps) = m.amplitudes {
                amps.iter_mut().enumerate().for_each(|(i, a)| {
                    if i != outcome_idx as usize {
                        *a = 0.0;
                    }
                });
            }
        }

        let collapsed_state = QuantumState {
            id: format!("{}-measured-{}", state.id, outcome_idx),
            modes: collapsed_modes,
            coherence_window: state.coherence_window.clone(),
            seed: Some(seed_val),
            provenance: {
                let mut p = state.provenance.clone();
                p.insert("measurement".to_string(), format!("mode:{} outcome:{}", mode_id, outcome_idx));
                p
            },
        };

        Ok(MeasurementOutcome {
            outcome_index: outcome_idx,
            photon_count: outcome_idx, // simplified: outcome_idx maps to photon count
            probability: normalized[outcome_idx as usize],
            collapsed_state: Some(collapsed_state),
            seed_used: Some(seed_val),
        })
    }

    fn is_coherent(&self, state: &QuantumState, current_time_ns: u64) -> bool {
        current_time_ns < state.coherence_window.end_ns
    }
}

/// Reference coherence manager.
pub struct ReferenceCoherenceManager;

impl CoherenceManager for ReferenceCoherenceManager {
    fn create_window(&self, start_ns: u64, duration_ns: u64, decoherence_model: &str) -> Result<CoherenceWindow> {
        Ok(CoherenceWindow {
            id: format!("coh-{}-{}", start_ns, decoherence_model),
            start_ns,
            end_ns: start_ns + duration_ns,
            decoherence_timescale_ns: Some(500.0),
            cross_mode_decoherence_ns: Some(750.0),
            idle_time_budget_ns: Some(200),
            notes: Some(format!("model:{}", decoherence_model)),
        })
    }

    fn validate_coherence(&self, state: &QuantumState, current_time_ns: u64) -> Result<()> {
        if current_time_ns > state.coherence_window.end_ns {
            Err(anyhow::anyhow!("state exceeds coherence window end time"))
        } else {
            Ok(())
        }
    }
}
