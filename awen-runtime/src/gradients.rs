//! Differentiable photonics API stubs and traits
//!
//! This module defines high-level interfaces for gradient computation, adjoint engines,
//! and noise-aware gradient estimators. Implementations may be provided by plugins/backends.

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use once_cell::sync::Lazy;

use crate::ir;
use crate::plugins::run_reference_simulator;
use std::f64::consts::PI;

/// Describes noise model parameters for gradient estimation.
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct NoiseModel {
    pub shot_noise_std: Option<f64>,
    pub thermal_noise_std: Option<f64>,
    pub phase_noise_std: Option<f64>,
    pub loss_variation: Option<f64>,
    pub metadata: Option<HashMap<String, String>>,
}

/// Options for gradient computation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GradientOptions {
    pub strategy: String, // "adjoint", "parameter_shift", "finite_difference", "score_function"
    pub seed: Option<u64>,
    pub samples: Option<u32>,
}

/// Result of gradient computation
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct GradientResult {
    pub gradients: HashMap<String, f64>,
    pub gradient_std: Option<HashMap<String, f64>>, // uncertainty when stochastic
    pub provenance: HashMap<String, String>,
}

/// Trait that gradient-capable backends must implement. Runtime offers a pluggable dispatch to providers.
pub trait GradientProvider: Send + Sync + 'static {
    /// Compute gradients for the given IR snapshot and parameter list.
    /// `ir_json` is a serialized IR snapshot; `params` lists parameter names to differentiate.
    fn compute_gradients(&self, ir_json: &str, params: &[String], noise: &NoiseModel, opts: &GradientOptions) -> Result<GradientResult>;

    /// Optional: compute adjoint analytically when available for higher efficiency.
    fn supports_adjoint(&self) -> bool { false }
}

/// Registry: runtime holds a registry of gradient providers keyed by backend name. Providers are stored as Arc.
pub struct GradientRegistry {
    providers: std::sync::RwLock<HashMap<String, Arc<dyn GradientProvider>>>,
}

impl GradientRegistry {
    pub fn new() -> Self {
        Self { providers: std::sync::RwLock::new(HashMap::new()) }
    }

    pub fn register(&self, name: &str, provider: Arc<dyn GradientProvider>) {
        let mut w = self.providers.write().unwrap();
        w.insert(name.to_string(), provider);
    }

    pub fn get(&self, name: &str) -> Option<Arc<dyn GradientProvider>> {
        let r = self.providers.read().unwrap();
        r.get(name).cloned()
    }
}

/// Global registry accessible to runtime and CLI. Initialized lazily.
pub static GLOBAL_GRADIENT_REGISTRY: Lazy<Arc<GradientRegistry>> = Lazy::new(|| {
    Arc::new(GradientRegistry::new())
});

/// Helper to register default providers into the global registry.
pub fn register_defaults_to_global() {
    register_default_providers(&GLOBAL_GRADIENT_REGISTRY);
}

/// Reference finite-difference GradientProvider for conformance and tests. Computes gradients of a simple scalar
/// cost defined as the output power of the last node in the reference simulator.
pub struct ReferenceGradientProvider {}

impl ReferenceGradientProvider {
    pub fn new() -> Self { Self {} }

    fn evaluate_cost(&self, graph: &ir::Graph, seed: Option<u64>) -> Result<f64> {
        let sim = run_reference_simulator(graph, seed)?;
        // define cost: sum of output power of last node result (real^2 + imag^2)
        if let Some(last) = sim.node_results.last() {
            let (re, im) = last.out_amplitude;
            Ok(re * re + im * im)
        } else {
            Ok(0.0)
        }
    }
}

impl GradientProvider for ReferenceGradientProvider {
    fn compute_gradients(&self, ir_json: &str, params: &[String], _noise: &NoiseModel, opts: &GradientOptions) -> Result<GradientResult> {
        // parse IR
        let mut graph: ir::Graph = serde_json::from_str(ir_json)?;

        // helper to get param value and set it
        let eps = 1e-6_f64;
        let seed_base = opts.seed.unwrap_or(0x1234_5678);

        let mut gradients = HashMap::new();
        let mut stds = HashMap::new();

        // baseline cost
        let _baseline = self.evaluate_cost(&graph, Some(seed_base))?;

        for pname in params {
            // find parameter location: for simplicity, search nodes for param name
            // We support parameters named like "node_id:param" or global param names located in first matching node.
            let (node_idx_opt, key_opt) = if pname.contains(":" ) {
                let mut parts = pname.splitn(2, ':');
                (graph.nodes.iter().position(|n| n.id==parts.next().unwrap().to_string()), Some(parts.next().unwrap().to_string()))
            } else {
                // search first node containing key
                let found = graph.nodes.iter().enumerate()
                    .find(|(_, n)| n.params.contains_key(pname))
                    .map(|(i, _)| (i, pname.clone()));
                (found.as_ref().map(|(i, _)| *i), found.map(|(_, k)| k))
            };

            if node_idx_opt.is_none() || key_opt.is_none() {
                gradients.insert(pname.clone(), 0.0_f64);
                stds.insert(pname.clone(), 0.0_f64);
                continue;
            }

            let node_idx = node_idx_opt.unwrap();
            let key = key_opt.unwrap();

            // central finite difference
            let orig = *graph.nodes[node_idx].params.get(&key).unwrap_or(&0.0_f64);

            // optionally average over samples to reduce noise
            let samples = opts.samples.unwrap_or(1);
            let mut grads_acc = 0.0_f64;
            let mut vals = Vec::new();
            for s in 0..samples {
                let seed1 = seed_base.wrapping_add(s as u64).wrapping_mul(2).wrapping_add(1);
                let seed2 = seed_base.wrapping_add(s as u64).wrapping_mul(2).wrapping_add(2);

                graph.nodes[node_idx].params.insert(key.clone(), orig + eps);
                let f_plus = self.evaluate_cost(&graph, Some(seed1))?;

                graph.nodes[node_idx].params.insert(key.clone(), orig - eps);
                let f_minus = self.evaluate_cost(&graph, Some(seed2))?;

                // reset
                graph.nodes[node_idx].params.insert(key.clone(), orig);

                let g = (f_plus - f_minus) / (2.0 * eps);
                grads_acc += g;
                vals.push(g);
            }

            let avg = grads_acc / (samples as f64);
            // compute std
            let mut var = 0.0_f64;
            for v in &vals { var += (v - avg) * (v - avg); }
            let std = if samples > 1 { (var / (samples as f64 - 1.0)).sqrt() } else { 0.0_f64 };

            gradients.insert(pname.clone(), avg);
            stds.insert(pname.clone(), std);
        }

        let mut provenance = HashMap::new();
        provenance.insert("provider".to_string(), "reference-fd".to_string());

        Ok(GradientResult {
            gradients,
            gradient_std: Some(stds),
            provenance,
        })
    }
}

/// Analytic/adjoint gradient provider (reference). Currently supports analytic adjoint
/// gradients for `mzi` node parameter `phase`. For unsupported parameters it falls back
/// to the finite-difference reference provider.
pub struct ReferenceAdjointProvider {
    fd_fallback: ReferenceGradientProvider,
}

impl ReferenceAdjointProvider {
    pub fn new() -> Self { Self { fd_fallback: ReferenceGradientProvider::new() } }

    /// Compute analytic gradient of final output power with respect to a single parameter
    /// located in node `node_idx` and key `key`. Returns None if analytic unsupported.
    fn analytic_grad_for_param(&self, graph: &ir::Graph, node_idx: usize, key: &str) -> Option<f64> {
        // We support only `mzi` node 'phase' analytic gradients for now.
        if node_idx >= graph.nodes.len() { return None; }
        let target_node = &graph.nodes[node_idx];
        if target_node.node_type.to_lowercase() != "mzi" || key != "phase" { return None; }

        // Forward propagate complex amplitude and sensitivity vector s = d(a)/dparam
        let mut current = (
            graph.metadata
                .get("input_amplitude")
                .and_then(|s| s.parse::<f64>().ok())
                .unwrap_or(1.0_f64),
            0.0_f64,
        );
        let mut s = (0.0_f64, 0.0_f64);

        for (i, node) in graph.nodes.iter().enumerate() {
            let node_type = node.node_type.to_lowercase();
            match node_type.as_str() {
                "mzi" => {
                    let phi = node.params.get("phase").cloned().unwrap_or(0.0_f64);
                    let cos = phi.cos();
                    let sin = phi.sin();
                    // Rotation R(phi)
                    let r00 = cos; let r01 = -sin;
                    let r10 = sin; let r11 = cos;
                    // dR/dphi
                    let dr00 = -sin; let dr01 = -cos;
                    let dr10 = cos;  let dr11 = -sin;

                    // If this is the target node, the parameter appears here; otherwise dr* term is zero.
                    let is_target = i == node_idx;

                    // compute s_new = dR/dp * current + R * s
                    let (cx, cy) = current;
                    let s_new_x = (if is_target { dr00 * cx + dr01 * cy } else { 0.0 }) + (r00 * s.0 + r01 * s.1);
                    let s_new_y = (if is_target { dr10 * cx + dr11 * cy } else { 0.0 }) + (r10 * s.0 + r11 * s.1);

                    // update current = R * current
                    let new_x = r00 * cx + r01 * cy;
                    let new_y = r10 * cx + r11 * cy;

                    current = (new_x, new_y);
                    s = (s_new_x, s_new_y);
                }

                "loss" => {
                    let loss = node.params.get("loss").cloned().unwrap_or(0.0_f64);
                    let factor = 1.0 - loss.clamp(0.0, 1.0);
                    current = (current.0 * factor, current.1 * factor);
                    s = (s.0 * factor, s.1 * factor);
                }

                "ring" => {
                    // treat ring as small rotation by effective phi = coupling/(1+|detuning|)
                    let coupling = node.params.get("coupling").cloned().unwrap_or(0.1);
                    let detuning = node.params.get("detuning").cloned().unwrap_or(0.0);
                    let effective = (coupling * (1.0 / (1.0 + detuning.abs()))).clamp(-PI, PI);
                    let cos = effective.cos();
                    let sin = effective.sin();
                    let r00 = cos; let r01 = -sin; let r10 = sin; let r11 = cos;
                    // For analytic derivatives wrt ring params we don't support yet, set dr=0
                    let dr00 = 0.0_f64; let dr01 = 0.0_f64; let dr10 = 0.0_f64; let dr11 = 0.0_f64;

                    let (cx, cy) = current;
                    let s_new_x = (dr00 * cx + dr01 * cy) + (r00 * s.0 + r01 * s.1);
                    let s_new_y = (dr10 * cx + dr11 * cy) + (r10 * s.0 + r11 * s.1);

                    let new_x = r00 * cx + r01 * cy;
                    let new_y = r10 * cx + r11 * cy;

                    current = (new_x, new_y);
                    s = (s_new_x, s_new_y);
                }

                "detector" => {
                    // detectors are read-only; propagate unchanged
                }

                _ => {
                    // passthrough
                }
            }
        }

        // final power P = x^2 + y^2; dP/dp = 2*(x*dx/dp + y*dy/dp)
        let (x, y) = current;
        let (sx, sy) = s;
        let dp = 2.0 * (x * sx + y * sy);
        Some(dp)
    }
}

impl GradientProvider for ReferenceAdjointProvider {
    fn compute_gradients(&self, ir_json: &str, params: &[String], noise: &NoiseModel, opts: &GradientOptions) -> Result<GradientResult> {
        let graph: ir::Graph = serde_json::from_str(ir_json)?;

        let mut gradients = HashMap::new();
        let mut stds = HashMap::new();

        for pname in params {
            // locate parameter
            let (node_idx_opt, key_opt) = if pname.contains(":" ) {
                let mut parts = pname.splitn(2, ':');
                (graph.nodes.iter().position(|n| n.id==parts.next().unwrap().to_string()), Some(parts.next().unwrap().to_string()))
            } else {
                let found = graph.nodes.iter().enumerate()
                    .find(|(_, n)| n.params.contains_key(pname))
                    .map(|(i, _)| (i, pname.clone()));
                (found.as_ref().map(|(i, _)| *i), found.map(|(_, k)| k))
            };

            if node_idx_opt.is_none() || key_opt.is_none() {
                gradients.insert(pname.clone(), 0.0_f64);
                stds.insert(pname.clone(), 0.0_f64);
                continue;
            }
            let node_idx = node_idx_opt.unwrap();
            let key = key_opt.unwrap();

            // Attempt analytic
            if let Some(g) = self.analytic_grad_for_param(&graph, node_idx, &key) {
                gradients.insert(pname.clone(), g);
                stds.insert(pname.clone(), 0.0_f64);
                continue;
            }

            // fallback to finite-difference provider
            let res = self.fd_fallback.compute_gradients(ir_json, &[pname.clone()], noise, opts)?;
            if let Some(v) = res.gradients.get(pname) { gradients.insert(pname.clone(), *v); }
            if let Some(smap) = &res.gradient_std {
                if let Some(sv) = smap.get(pname) { stds.insert(pname.clone(), *sv); }
            }
        }

        let mut prov = HashMap::new(); prov.insert("provider".to_string(), "reference-adjoint".to_string());
        Ok(GradientResult { gradients, gradient_std: Some(stds), provenance: prov })
    }

    fn supports_adjoint(&self) -> bool { true }
}

// Update register_defaults_to_global to add adjoint provider as well
pub fn register_default_providers(registry: &GradientRegistry) {
    let provider = Arc::new(ReferenceGradientProvider::new());
    registry.register("reference-fd", provider);
    let adj = Arc::new(ReferenceAdjointProvider::new());
    registry.register("reference-adjoint", adj);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn test_reference_gradient_provider() {
        let ir = fs::read_to_string("example_ir.json").expect("read example_ir");
        let provider = ReferenceGradientProvider::new();
        let params = vec!["mzi_0:phase".to_string(), "mzi_1:phase".to_string()];
        let noise = NoiseModel { shot_noise_std: None, thermal_noise_std: None, phase_noise_std: None, loss_variation: None, metadata: None };
        let opts = GradientOptions { strategy: "finite_difference".to_string(), seed: Some(42), samples: Some(1) };
        let res = provider.compute_gradients(&ir, &params, &noise, &opts).expect("compute gradients");
        assert!(res.gradients.contains_key("mzi_0:phase"));
    }

    #[test]
    fn test_adjoint_vs_fd_conformance() {
        // Compare analytic adjoint against finite-difference for MZI phase parameters
        let ir = fs::read_to_string("example_ir.json").expect("read example_ir");
        let params = vec!["mzi_0:phase".to_string(), "mzi_1:phase".to_string()];
        let noise = NoiseModel { shot_noise_std: None, thermal_noise_std: None, phase_noise_std: None, loss_variation: None, metadata: None };
        let fd_opts = GradientOptions { strategy: "finite_difference".to_string(), seed: Some(12345), samples: Some(3) };
        let adj_opts = GradientOptions { strategy: "adjoint".to_string(), seed: Some(12345), samples: Some(1) };

        let fd = ReferenceGradientProvider::new();
        let adj = ReferenceAdjointProvider::new();

        let fd_res = fd.compute_gradients(&ir, &params, &noise, &fd_opts).expect("fd grad");
        let adj_res = adj.compute_gradients(&ir, &params, &noise, &adj_opts).expect("adj grad");

        // Tolerances: allow small absolute error and small relative error
        let abs_tol = 1e-4_f64;
        let rel_tol = 1e-3_f64;

        for p in &params {
            let g_fd = fd_res.gradients.get(p).copied().unwrap_or(0.0);
            let g_adj = adj_res.gradients.get(p).copied().unwrap_or(0.0);
            let abs_err = (g_fd - g_adj).abs();
            let rel_err = if g_fd.abs() > 0.0 { abs_err / g_fd.abs() } else { abs_err };
            assert!(abs_err <= abs_tol || rel_err <= rel_tol, "gradient mismatch for {}: fd={} adj={} abs_err={} rel_err={}", p, g_fd, g_adj, abs_err, rel_err);
        }
    }
}
