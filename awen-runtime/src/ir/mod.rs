//! IR loader and validator (v0.1)

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Node {
    pub id: String,
    #[serde(rename = "type")]
    pub node_type: String,
    #[serde(default)]
    pub params: HashMap<String, f64>,
    /// Optional: for measurement nodes, specify which mode to measure
    #[serde(default)]
    pub measure_mode: Option<String>,
    /// Optional: conditional branches based on measurement outcome
    #[serde(default)]
    pub conditional_branches: Option<Vec<ConditionalBranch>>,
}

/// A measurement-conditioned feedback branch: if outcome matches condition, execute subgraph
#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ConditionalBranch {
    pub outcome_index: u32,
    /// Nodes to execute if this outcome is measured
    pub then_nodes: Vec<String>,
    /// Nodes to execute if outcome does not match (optional)
    #[serde(default)]
    pub else_nodes: Option<Vec<String>>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Edge {
    pub src_node: String,
    pub src_port: Option<String>,
    pub dst_node: String,
    pub dst_port: Option<String>,
    pub delay: Option<f64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Graph {
    pub nodes: Vec<Node>,
    #[serde(default)]
    pub edges: Vec<Edge>,
    #[serde(default)]
    pub metadata: HashMap<String, String>,
}

pub fn load_from_json(path: &str) -> Result<Graph, String> {
    let data = std::fs::read_to_string(path).map_err(|e| format!("read error: {}", e))?;
    serde_json::from_str::<Graph>(&data).map_err(|e| format!("parse error: {}", e))
}

/// Validate IR: check that conditional branches reference existing nodes
pub fn validate_graph(graph: &Graph) -> Result<(), String> {
    let node_ids: std::collections::HashSet<&str> =
        graph.nodes.iter().map(|n| n.id.as_str()).collect();

    for node in &graph.nodes {
        if let Some(branches) = &node.conditional_branches {
            for branch in branches {
                for then_id in &branch.then_nodes {
                    if !node_ids.contains(then_id.as_str()) {
                        return Err(format!(
                            "conditional branch references non-existent node: {}",
                            then_id
                        ));
                    }
                }
                if let Some(else_nodes) = &branch.else_nodes {
                    for else_id in else_nodes {
                        if !node_ids.contains(else_id.as_str()) {
                            return Err(format!(
                                "else branch references non-existent node: {}",
                                else_id
                            ));
                        }
                    }
                }
            }
        }
    }

    Ok(())
}
