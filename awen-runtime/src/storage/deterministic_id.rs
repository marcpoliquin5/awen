//! Deterministic ID generation for artifacts

use anyhow::Result;
use sha2::{Digest, Sha256};
use std::collections::HashMap;

use crate::ir::Graph;

/// Compute deterministic content-addressable ID for an artifact
pub fn compute_deterministic_id(
    ir: &Graph,
    parameters: &HashMap<String, f64>,
    calibration_state: Option<&serde_json::Value>,
    seed: Option<u64>,
    runtime_version: &str,
) -> Result<String> {
    let mut hasher = Sha256::new();

    // IR (canonical JSON - sorted keys, no whitespace)
    let ir_canonical = canonical_json(ir)?;
    hasher.update(ir_canonical.as_bytes());

    // Parameters (sorted by key for determinism)
    let mut param_keys: Vec<_> = parameters.keys().collect();
    param_keys.sort();
    for key in param_keys {
        hasher.update(key.as_bytes());
        hasher.update(parameters[key].to_le_bytes());
    }

    // Calibration state (if present)
    if let Some(calib) = calibration_state {
        let calib_canonical = canonical_json(calib)?;
        hasher.update(calib_canonical.as_bytes());
    }

    // Seed
    if let Some(s) = seed {
        hasher.update(s.to_le_bytes());
    }

    // Runtime version
    hasher.update(runtime_version.as_bytes());

    // Finalize hash
    let hash = hasher.finalize();
    let hex = hex::encode(hash);

    Ok(format!("awen_{}", hex))
}

/// Serialize to canonical JSON (sorted keys, no whitespace)
fn canonical_json<T: serde::Serialize>(value: &T) -> Result<String> {
    // Serialize to serde_json::Value first to ensure key ordering
    let json_value = serde_json::to_value(value)?;
    let canonical = serde_json::to_string(&json_value)?;
    Ok(canonical)
}

/// Get short ID (first 16 hex chars) for citations
pub fn short_id(full_id: &str) -> &str {
    if let Some(hex_part) = full_id.strip_prefix("awen_") {
        let short_len = std::cmp::min(16, hex_part.len());
        &full_id[..(5 + short_len)]
    } else {
        full_id
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_deterministic_id_same_inputs() {
        let ir = Graph {
            nodes: vec![],
            edges: vec![],
            metadata: std::collections::HashMap::new(),
        };
        let mut params = HashMap::new();
        params.insert("a".to_string(), 1.0);
        params.insert("b".to_string(), 2.0);

        let id1 = compute_deterministic_id(&ir, &params, None, Some(42), "0.5.0").unwrap();
        let id2 = compute_deterministic_id(&ir, &params, None, Some(42), "0.5.0").unwrap();

        assert_eq!(id1, id2, "Same inputs should produce same ID");
        assert!(id1.starts_with("awen_"));
        assert_eq!(id1.len(), 5 + 64); // "awen_" + 64 hex chars
    }

    #[test]
    fn test_deterministic_id_different_params() {
        let ir = Graph {
            nodes: vec![],
            edges: vec![],
            metadata: std::collections::HashMap::new(),
        };
        let mut params1 = HashMap::new();
        params1.insert("a".to_string(), 1.0);

        let mut params2 = HashMap::new();
        params2.insert("a".to_string(), 2.0);

        let id1 = compute_deterministic_id(&ir, &params1, None, Some(42), "0.5.0").unwrap();
        let id2 = compute_deterministic_id(&ir, &params2, None, Some(42), "0.5.0").unwrap();

        assert_ne!(id1, id2, "Different params should produce different IDs");
    }

    #[test]
    fn test_deterministic_id_param_order_invariant() {
        let ir = Graph {
            nodes: vec![],
            edges: vec![],
            metadata: std::collections::HashMap::new(),
        };

        let mut params1 = HashMap::new();
        params1.insert("a".to_string(), 1.0);
        params1.insert("b".to_string(), 2.0);

        let mut params2 = HashMap::new();
        params2.insert("b".to_string(), 2.0);
        params2.insert("a".to_string(), 1.0);

        let id1 = compute_deterministic_id(&ir, &params1, None, Some(42), "0.5.0").unwrap();
        let id2 = compute_deterministic_id(&ir, &params2, None, Some(42), "0.5.0").unwrap();

        assert_eq!(id1, id2, "Param insertion order should not affect ID");
    }

    #[test]
    fn test_short_id() {
        let full_id = "awen_0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        let short = short_id(full_id);
        assert_eq!(short, "awen_0123456789abcdef");
    }
}
