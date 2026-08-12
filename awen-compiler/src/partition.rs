use crate::cost::{
    stable_fingerprint_bytes, OptimizationObjective, ParameterSource, TargetBackend,
};
use anyhow::{bail, Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub const PARTITION_GRAPH_VERSION: &str = "awen.partition-graph.v1";
pub const PARTITION_TRACE_VERSION: &str = "awen.partition-trace.v1";

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[serde(rename_all = "snake_case")]
pub enum GraphOpKind {
    Gemm,
    BatchedGemm,
    LayerNorm,
    Softmax,
    Gelu,
    LinearTransform,
    HostIrregular,
    ControlFlowBarrier,
}

impl GraphOpKind {
    pub fn is_photonic_linear(self) -> bool {
        matches!(self, Self::Gemm | Self::BatchedGemm | Self::LinearTransform)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PartitionCost {
    pub latency_ns: f64,
    pub energy_uj: f64,
    pub error_fraction: f64,
    pub operations: f64,
    pub source: ParameterSource,
}

impl PartitionCost {
    pub fn validate(self) -> Result<()> {
        positive(self.latency_ns, "candidate latency_ns")?;
        non_negative(self.energy_uj, "candidate energy_uj")?;
        fraction(self.error_fraction, "candidate error_fraction")?;
        non_negative(self.operations, "candidate operations")?;
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct NodeCandidate {
    pub device: TargetBackend,
    pub eligible: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cost: Option<PartitionCost>,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct GraphTensor {
    pub id: String,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub initial_device: Option<TargetBackend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub required_device: Option<TargetBackend>,
    #[serde(default)]
    pub persistent: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct GraphNode {
    pub id: String,
    pub kind: GraphOpKind,
    pub inputs: Vec<String>,
    pub outputs: Vec<String>,
    #[serde(default)]
    pub dynamic_shape: bool,
    #[serde(default)]
    pub control_flow_barrier: bool,
    pub candidates: Vec<NodeCandidate>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PartitionGraph {
    pub graph_version: String,
    pub tensors: Vec<GraphTensor>,
    pub nodes: Vec<GraphNode>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
#[serde(default, deny_unknown_fields)]
pub struct PartitionOptions {
    pub objective: OptimizationObjective,
    pub seed: u64,
    pub transfer_bandwidth_gbps: f64,
    pub transfer_latency_ns: f64,
    pub transfer_energy_pj_per_byte: f64,
    pub crossing_penalty_ns: f64,
    pub crossing_penalty_uj: f64,
    pub crossing_error_fraction: f64,
    pub cpu_memory_budget_bytes: u64,
    pub gpu_memory_budget_bytes: u64,
    pub photonic_memory_budget_bytes: u64,
    pub alternatives: usize,
    pub max_search_states: usize,
}

impl Default for PartitionOptions {
    fn default() -> Self {
        Self {
            objective: OptimizationObjective::Latency,
            seed: 0,
            transfer_bandwidth_gbps: 128.0,
            transfer_latency_ns: 100.0,
            transfer_energy_pj_per_byte: 1.0,
            crossing_penalty_ns: 500.0,
            crossing_penalty_uj: 0.001,
            crossing_error_fraction: 0.0,
            cpu_memory_budget_bytes: u64::MAX,
            gpu_memory_budget_bytes: u64::MAX,
            photonic_memory_budget_bytes: u64::MAX,
            alternatives: 3,
            max_search_states: 1_000_000,
        }
    }
}

impl PartitionOptions {
    pub fn validate(self) -> Result<()> {
        positive(
            self.transfer_bandwidth_gbps,
            "partition transfer_bandwidth_gbps",
        )?;
        for (value, name) in [
            (self.transfer_latency_ns, "partition transfer_latency_ns"),
            (
                self.transfer_energy_pj_per_byte,
                "partition transfer_energy_pj_per_byte",
            ),
            (self.crossing_penalty_ns, "partition crossing_penalty_ns"),
            (self.crossing_penalty_uj, "partition crossing_penalty_uj"),
        ] {
            non_negative(value, name)?;
        }
        fraction(
            self.crossing_error_fraction,
            "partition crossing_error_fraction",
        )?;
        if self.cpu_memory_budget_bytes == 0
            || self.gpu_memory_budget_bytes == 0
            || self.photonic_memory_budget_bytes == 0
        {
            bail!("partition memory budgets must be non-zero");
        }
        if self.max_search_states == 0 {
            bail!("partition max_search_states must be non-zero");
        }
        Ok(())
    }

    fn memory_budget(self, device: TargetBackend) -> u64 {
        match device {
            TargetBackend::Cpu => self.cpu_memory_budget_bytes,
            TargetBackend::Gpu => self.gpu_memory_budget_bytes,
            TargetBackend::Photonic => self.photonic_memory_budget_bytes,
            TargetBackend::Auto => 0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct PartitionRequest {
    pub graph: PartitionGraph,
    #[serde(default)]
    pub options: PartitionOptions,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartitionTotals {
    pub latency_ns: f64,
    pub energy_uj: f64,
    pub error_fraction: f64,
    pub operations: f64,
    pub throughput_gops: f64,
    pub optical_electrical_boundary_crossings: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TransferRecord {
    pub id: String,
    pub tensor: String,
    pub from: TargetBackend,
    pub to: TargetBackend,
    pub bytes: u64,
    pub latency_ns: f64,
    pub energy_uj: f64,
    pub error_fraction: f64,
    pub optical_electrical_boundary: bool,
    pub consumer_nodes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MemoryPeak {
    pub device: TargetBackend,
    pub bytes: u64,
    pub budget_bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartitionAlternative {
    pub assignments: BTreeMap<String, TargetBackend>,
    pub totals: PartitionTotals,
    pub objective_score: f64,
    pub memory_peaks: Vec<MemoryPeak>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct NodePlacementTrace {
    pub node_id: String,
    pub kind: GraphOpKind,
    pub selected_device: TargetBackend,
    pub local_best_device: TargetBackend,
    pub candidates: Vec<NodeCandidate>,
    pub rationale: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartitionRegion {
    pub id: String,
    pub device: TargetBackend,
    pub nodes: Vec<String>,
    pub external_inputs: Vec<String>,
    pub external_outputs: Vec<String>,
    pub fused: bool,
    pub node_cost: PartitionCost,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProfilerEventKind {
    TensorTransfer,
    OpticalElectricalBoundary,
    RegionExecute,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartitionProfilerEvent {
    pub sequence: usize,
    pub kind: ProfilerEventKind,
    pub name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tensor: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub region: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub from: Option<TargetBackend>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<TargetBackend>,
    pub bytes: u64,
    pub estimated_latency_ns: f64,
    pub estimated_energy_uj: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct VisualizationEdge {
    pub tensor: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub producer: Option<String>,
    pub consumer: String,
    pub from: TargetBackend,
    pub to: TargetBackend,
    pub crosses_device: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PartitionTrace {
    pub trace_version: String,
    pub graph_version: String,
    pub graph_fingerprint: String,
    pub objective: OptimizationObjective,
    pub search_strategy: String,
    pub selected: PartitionAlternative,
    pub alternatives: Vec<PartitionAlternative>,
    pub nodes: Vec<NodePlacementTrace>,
    pub transfers: Vec<TransferRecord>,
    pub regions: Vec<PartitionRegion>,
    pub profiler_events: Vec<PartitionProfilerEvent>,
    pub visualization_edges: Vec<VisualizationEdge>,
    pub rationale: String,
}

#[derive(Debug, Clone)]
struct GraphIndex {
    order: Vec<usize>,
    tensor_by_id: HashMap<String, usize>,
    node_by_id: HashMap<String, usize>,
    producer: Vec<Option<usize>>,
    consumers: Vec<Vec<usize>>,
}

#[derive(Debug, Clone)]
struct EvaluatedAssignment {
    assignment: Vec<TargetBackend>,
    alternative: PartitionAlternative,
    transfers: Vec<TransferRecord>,
}

pub fn partition_graph(request: &PartitionRequest) -> Result<PartitionTrace> {
    request.options.validate()?;
    let index = validate_graph(&request.graph)?;
    let choices = legal_choices(&request.graph, &index)?;
    let combination_count = choices.iter().fold(1_usize, |product, candidates| {
        product.saturating_mul(candidates.len())
    });
    let (mut evaluated, search_strategy) = if combination_count <= request.options.max_search_states
    {
        (
            exhaustive_search(&request.graph, &index, &choices, request.options)?,
            format!("exact_exhaustive:{combination_count}"),
        )
    } else {
        (
            beam_search(&request.graph, &index, &choices, request.options)?,
            format!(
                "deterministic_beam:{}:{}",
                request.options.max_search_states, combination_count
            ),
        )
    };
    if evaluated.is_empty() {
        bail!("graph partitioner found no assignment within device memory budgets");
    }
    evaluated.sort_by(|left, right| {
        left.alternative
            .objective_score
            .total_cmp(&right.alternative.objective_score)
            .then_with(|| {
                assignment_tie_key(request.options.seed, &left.assignment)
                    .cmp(&assignment_tie_key(request.options.seed, &right.assignment))
            })
            .then_with(|| left.assignment.cmp(&right.assignment))
    });
    let winner = evaluated.remove(0);
    let alternatives = evaluated
        .iter()
        .take(request.options.alternatives)
        .map(|candidate| candidate.alternative.clone())
        .collect::<Vec<_>>();
    let nodes = node_traces(
        &request.graph,
        &index,
        &winner.assignment,
        request.options.objective,
    );
    let regions = build_regions(&request.graph, &index, &winner.assignment)?;
    let profiler_events = profiler_events(&winner.transfers, &regions);
    let visualization_edges = visualization_edges(&request.graph, &index, &winner.assignment);
    let graph_fingerprint = graph_fingerprint(request)?;
    let next = alternatives.first().map_or_else(
        || "no other legal assignment".to_string(),
        |alternative| {
            format!(
                "next assignment score {:.6} versus selected {:.6}",
                alternative.objective_score, winner.alternative.objective_score
            )
        },
    );
    let rationale = format!(
        "selected a whole-graph {:?} partition with {} region(s), {} deduplicated transfer(s), {} optical/electrical crossing(s), and {}; {next}",
        request.options.objective,
        regions.len(),
        winner.transfers.len(),
        winner
            .alternative
            .totals
            .optical_electrical_boundary_crossings,
        search_strategy,
    );
    Ok(PartitionTrace {
        trace_version: PARTITION_TRACE_VERSION.to_string(),
        graph_version: request.graph.graph_version.clone(),
        graph_fingerprint,
        objective: request.options.objective,
        search_strategy,
        selected: winner.alternative,
        alternatives,
        nodes,
        transfers: winner.transfers,
        regions,
        profiler_events,
        visualization_edges,
        rationale,
    })
}

fn validate_graph(graph: &PartitionGraph) -> Result<GraphIndex> {
    if graph.graph_version != PARTITION_GRAPH_VERSION {
        bail!(
            "unsupported partition graph version '{}'; expected '{}'",
            graph.graph_version,
            PARTITION_GRAPH_VERSION
        );
    }
    if graph.nodes.is_empty() {
        bail!("partition graph must contain at least one node");
    }
    let mut tensor_by_id = HashMap::new();
    for (index, tensor) in graph.tensors.iter().enumerate() {
        if tensor.id.trim().is_empty() || tensor.bytes == 0 {
            bail!("partition tensors require a non-empty id and non-zero byte size");
        }
        if tensor
            .initial_device
            .is_some_and(|device| device == TargetBackend::Auto)
            || tensor
                .required_device
                .is_some_and(|device| device == TargetBackend::Auto)
        {
            bail!("tensor residency cannot use the auto pseudo-device");
        }
        if tensor_by_id.insert(tensor.id.clone(), index).is_some() {
            bail!("duplicate partition tensor id '{}'", tensor.id);
        }
    }
    let mut node_by_id = HashMap::new();
    for (index, node) in graph.nodes.iter().enumerate() {
        if node.id.trim().is_empty() {
            bail!("partition nodes require a non-empty id");
        }
        if node_by_id.insert(node.id.clone(), index).is_some() {
            bail!("duplicate partition node id '{}'", node.id);
        }
        if node.inputs.is_empty() || node.outputs.is_empty() {
            bail!("partition node '{}' requires inputs and outputs", node.id);
        }
        for tensor in node.inputs.iter().chain(&node.outputs) {
            if !tensor_by_id.contains_key(tensor) {
                bail!(
                    "partition node '{}' references unknown tensor '{tensor}'",
                    node.id
                );
            }
        }
        let mut devices = BTreeSet::new();
        let mut eligible = 0;
        for candidate in &node.candidates {
            if candidate.device == TargetBackend::Auto {
                bail!("node '{}' cannot use auto as a concrete candidate", node.id);
            }
            if !devices.insert(candidate.device) {
                bail!(
                    "node '{}' has duplicate {:?} candidates",
                    node.id,
                    candidate.device
                );
            }
            if candidate.reason.trim().is_empty() {
                bail!("node '{}' candidates require an explanation", node.id);
            }
            if candidate.eligible {
                candidate
                    .cost
                    .with_context(|| {
                        format!("eligible candidate on node '{}' requires a cost", node.id)
                    })?
                    .validate()?;
                eligible += 1;
            } else if candidate.cost.is_some() {
                bail!(
                    "ineligible candidate on node '{}' must not carry a cost",
                    node.id
                );
            }
            if candidate.device == TargetBackend::Photonic
                && candidate.eligible
                && (!node.kind.is_photonic_linear()
                    || node.dynamic_shape
                    || node.control_flow_barrier)
            {
                bail!(
                    "node '{}' illegally enables photonics for an unsupported, dynamic, or barrier operation",
                    node.id
                );
            }
        }
        if eligible == 0 {
            bail!("partition node '{}' has no eligible candidate", node.id);
        }
    }

    let mut producer = vec![None; graph.tensors.len()];
    let mut consumers = vec![Vec::new(); graph.tensors.len()];
    for (node_index, node) in graph.nodes.iter().enumerate() {
        for output in &node.outputs {
            let tensor_index = tensor_by_id[output];
            if producer[tensor_index].replace(node_index).is_some() {
                bail!("tensor '{output}' has multiple producers");
            }
        }
        for input in &node.inputs {
            consumers[tensor_by_id[input]].push(node_index);
        }
    }
    for (tensor_index, tensor) in graph.tensors.iter().enumerate() {
        if producer[tensor_index].is_none() && tensor.initial_device.is_none() {
            bail!(
                "graph input tensor '{}' requires an initial_device",
                tensor.id
            );
        }
    }

    let mut indegree = vec![0_usize; graph.nodes.len()];
    let mut outgoing = vec![Vec::new(); graph.nodes.len()];
    for (tensor_index, source) in producer.iter().enumerate() {
        if let Some(source) = source {
            for consumer in &consumers[tensor_index] {
                if source != consumer {
                    outgoing[*source].push(*consumer);
                    indegree[*consumer] += 1;
                }
            }
        }
    }
    let mut ready = BTreeSet::new();
    for (index, degree) in indegree.iter().enumerate() {
        if *degree == 0 {
            ready.insert((graph.nodes[index].id.clone(), index));
        }
    }
    let mut order = Vec::with_capacity(graph.nodes.len());
    while let Some((id, node_index)) = ready.pop_first() {
        let _ = id;
        order.push(node_index);
        for next in &outgoing[node_index] {
            indegree[*next] -= 1;
            if indegree[*next] == 0 {
                ready.insert((graph.nodes[*next].id.clone(), *next));
            }
        }
    }
    if order.len() != graph.nodes.len() {
        bail!("partition graph contains a dependency cycle");
    }
    Ok(GraphIndex {
        order,
        tensor_by_id,
        node_by_id,
        producer,
        consumers,
    })
}

fn legal_choices(graph: &PartitionGraph, index: &GraphIndex) -> Result<Vec<Vec<TargetBackend>>> {
    index
        .order
        .iter()
        .map(|node_index| {
            let mut choices = graph.nodes[*node_index]
                .candidates
                .iter()
                .filter(|candidate| candidate.eligible)
                .map(|candidate| candidate.device)
                .collect::<Vec<_>>();
            choices.sort_unstable();
            choices.dedup();
            if choices.is_empty() {
                bail!(
                    "node '{}' has no legal placement",
                    graph.nodes[*node_index].id
                );
            }
            Ok(choices)
        })
        .collect()
}

fn exhaustive_search(
    graph: &PartitionGraph,
    index: &GraphIndex,
    choices: &[Vec<TargetBackend>],
    options: PartitionOptions,
) -> Result<Vec<EvaluatedAssignment>> {
    fn visit(
        depth: usize,
        graph: &PartitionGraph,
        index: &GraphIndex,
        choices: &[Vec<TargetBackend>],
        options: PartitionOptions,
        assignment: &mut Vec<TargetBackend>,
        results: &mut Vec<EvaluatedAssignment>,
    ) -> Result<()> {
        if depth == choices.len() {
            if let Some(evaluated) = evaluate_assignment(graph, index, assignment, options)? {
                results.push(evaluated);
            }
            return Ok(());
        }
        for device in &choices[depth] {
            assignment.push(*device);
            visit(
                depth + 1,
                graph,
                index,
                choices,
                options,
                assignment,
                results,
            )?;
            assignment.pop();
        }
        Ok(())
    }

    let mut results = Vec::new();
    visit(
        0,
        graph,
        index,
        choices,
        options,
        &mut Vec::with_capacity(choices.len()),
        &mut results,
    )?;
    Ok(results)
}

fn beam_search(
    graph: &PartitionGraph,
    index: &GraphIndex,
    choices: &[Vec<TargetBackend>],
    options: PartitionOptions,
) -> Result<Vec<EvaluatedAssignment>> {
    let beam_width = options.max_search_states.min(4096).max(1);
    let mut states = vec![Vec::new()];
    for (depth, node_choices) in choices.iter().enumerate() {
        let mut expanded = Vec::with_capacity(states.len() * node_choices.len());
        for state in states {
            for device in node_choices {
                let mut next = state.clone();
                next.push(*device);
                expanded.push(next);
            }
        }
        expanded.sort_by(|left, right| {
            partial_score(graph, index, left, options.objective)
                .total_cmp(&partial_score(graph, index, right, options.objective))
                .then_with(|| {
                    assignment_tie_key(options.seed, left)
                        .cmp(&assignment_tie_key(options.seed, right))
                })
                .then_with(|| left.cmp(right))
        });
        expanded.truncate(beam_width);
        states = expanded;
        if depth + 1 == choices.len() {
            break;
        }
    }
    let mut results = Vec::new();
    for assignment in states {
        if let Some(evaluated) = evaluate_assignment(graph, index, &assignment, options)? {
            results.push(evaluated);
        }
    }
    Ok(results)
}

fn partial_score(
    graph: &PartitionGraph,
    index: &GraphIndex,
    assignment: &[TargetBackend],
    objective: OptimizationObjective,
) -> f64 {
    let mut totals = PartitionTotals {
        latency_ns: 0.0,
        energy_uj: 0.0,
        error_fraction: 0.0,
        operations: 0.0,
        throughput_gops: 0.0,
        optical_electrical_boundary_crossings: 0,
    };
    let mut error_squared = 0.0;
    for (order_index, device) in assignment.iter().enumerate() {
        let node = &graph.nodes[index.order[order_index]];
        if let Some(cost) = candidate_cost(node, *device) {
            totals.latency_ns += cost.latency_ns;
            totals.energy_uj += cost.energy_uj;
            totals.operations += cost.operations;
            error_squared += cost.error_fraction * cost.error_fraction;
        }
    }
    totals.error_fraction = error_squared.sqrt().min(1.0);
    totals.throughput_gops = totals.operations / totals.latency_ns.max(f64::EPSILON);
    objective_score(objective, &totals)
}

fn evaluate_assignment(
    graph: &PartitionGraph,
    index: &GraphIndex,
    assignment: &[TargetBackend],
    options: PartitionOptions,
) -> Result<Option<EvaluatedAssignment>> {
    if assignment.len() != index.order.len() {
        bail!("partition assignment length does not match graph nodes");
    }
    let mut totals = PartitionTotals {
        latency_ns: 0.0,
        energy_uj: 0.0,
        error_fraction: 0.0,
        operations: 0.0,
        throughput_gops: 0.0,
        optical_electrical_boundary_crossings: 0,
    };
    let mut error_squared = 0.0;
    for (order_index, device) in assignment.iter().enumerate() {
        let node = &graph.nodes[index.order[order_index]];
        let cost = candidate_cost(node, *device)
            .with_context(|| format!("assignment chose illegal {:?} for '{}'", device, node.id))?;
        totals.latency_ns += cost.latency_ns;
        totals.energy_uj += cost.energy_uj;
        totals.operations += cost.operations;
        error_squared += cost.error_fraction * cost.error_fraction;
    }
    let transfers = build_transfers(graph, index, assignment, options)?;
    for transfer in &transfers {
        totals.latency_ns += transfer.latency_ns;
        totals.energy_uj += transfer.energy_uj;
        error_squared += transfer.error_fraction * transfer.error_fraction;
        if transfer.optical_electrical_boundary {
            totals.optical_electrical_boundary_crossings += 1;
        }
    }
    totals.error_fraction = error_squared.sqrt().min(1.0);
    totals.throughput_gops = totals.operations / totals.latency_ns.max(f64::EPSILON);
    let memory_peaks = memory_peaks(graph, index, assignment, options)?;
    if memory_peaks
        .iter()
        .any(|peak| peak.bytes > peak.budget_bytes)
    {
        return Ok(None);
    }
    let assignments = index
        .order
        .iter()
        .enumerate()
        .map(|(order_index, node_index)| {
            (graph.nodes[*node_index].id.clone(), assignment[order_index])
        })
        .collect();
    let objective_score = objective_score(options.objective, &totals);
    Ok(Some(EvaluatedAssignment {
        assignment: assignment.to_vec(),
        alternative: PartitionAlternative {
            assignments,
            totals,
            objective_score,
            memory_peaks,
        },
        transfers,
    }))
}

fn candidate_cost(node: &GraphNode, device: TargetBackend) -> Option<PartitionCost> {
    node.candidates
        .iter()
        .find(|candidate| candidate.device == device && candidate.eligible)
        .and_then(|candidate| candidate.cost)
}

fn build_transfers(
    graph: &PartitionGraph,
    index: &GraphIndex,
    assignment: &[TargetBackend],
    options: PartitionOptions,
) -> Result<Vec<TransferRecord>> {
    let order_position = order_positions(index);
    let device_for_node = |node_index: usize| assignment[order_position[&node_index]];
    let mut transfers = Vec::new();
    for (tensor_index, tensor) in graph.tensors.iter().enumerate() {
        let source_device = index.producer[tensor_index]
            .map(&device_for_node)
            .or(tensor.initial_device)
            .with_context(|| format!("tensor '{}' has no source residency", tensor.id))?;
        let mut targets: BTreeMap<TargetBackend, Vec<String>> = BTreeMap::new();
        for consumer in &index.consumers[tensor_index] {
            let target = device_for_node(*consumer);
            if target != source_device {
                targets
                    .entry(target)
                    .or_default()
                    .push(graph.nodes[*consumer].id.clone());
            }
        }
        if let Some(required) = tensor.required_device {
            if required != source_device {
                targets.entry(required).or_default();
            }
        }
        for (target, mut consumers) in targets {
            consumers.sort();
            let boundary =
                source_device == TargetBackend::Photonic || target == TargetBackend::Photonic;
            let latency_ns = options.transfer_latency_ns
                + tensor.bytes as f64 * 8.0 / options.transfer_bandwidth_gbps
                + if boundary {
                    options.crossing_penalty_ns
                } else {
                    0.0
                };
            let energy_uj = tensor.bytes as f64 * options.transfer_energy_pj_per_byte / 1_000_000.0
                + if boundary {
                    options.crossing_penalty_uj
                } else {
                    0.0
                };
            transfers.push(TransferRecord {
                id: format!("transfer:{}:{:?}:{:?}", tensor.id, source_device, target)
                    .to_ascii_lowercase(),
                tensor: tensor.id.clone(),
                from: source_device,
                to: target,
                bytes: tensor.bytes,
                latency_ns,
                energy_uj,
                error_fraction: if boundary {
                    options.crossing_error_fraction
                } else {
                    0.0
                },
                optical_electrical_boundary: boundary,
                consumer_nodes: consumers,
            });
        }
    }
    transfers.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(transfers)
}

fn memory_peaks(
    graph: &PartitionGraph,
    index: &GraphIndex,
    assignment: &[TargetBackend],
    options: PartitionOptions,
) -> Result<Vec<MemoryPeak>> {
    let positions = order_positions(index);
    let device_for_node = |node_index: usize| assignment[positions[&node_index]];
    let mut intervals: BTreeMap<TargetBackend, Vec<(usize, usize, u64)>> = BTreeMap::new();
    for (tensor_index, tensor) in graph.tensors.iter().enumerate() {
        let source_device = index.producer[tensor_index]
            .map(&device_for_node)
            .or(tensor.initial_device)
            .with_context(|| format!("tensor '{}' has no source residency", tensor.id))?;
        let source_start = index.producer[tensor_index]
            .map(|node| positions[&node])
            .unwrap_or(0);
        let source_end = index.consumers[tensor_index]
            .iter()
            .map(|node| positions[node])
            .max()
            .unwrap_or(source_start)
            .max(if tensor.persistent {
                graph.nodes.len()
            } else {
                0
            });
        intervals
            .entry(source_device)
            .or_default()
            .push((source_start, source_end, tensor.bytes));

        let mut target_positions: BTreeMap<TargetBackend, Vec<usize>> = BTreeMap::new();
        for consumer in &index.consumers[tensor_index] {
            let target = device_for_node(*consumer);
            if target != source_device {
                target_positions
                    .entry(target)
                    .or_default()
                    .push(positions[consumer]);
            }
        }
        if let Some(required) = tensor.required_device {
            if required != source_device {
                target_positions
                    .entry(required)
                    .or_default()
                    .push(graph.nodes.len());
            }
        }
        for (device, positions) in target_positions {
            let start = positions.iter().copied().min().unwrap_or(0);
            let mut end = positions.iter().copied().max().unwrap_or(start);
            if tensor.persistent {
                end = graph.nodes.len();
            }
            intervals
                .entry(device)
                .or_default()
                .push((start, end, tensor.bytes));
        }
    }
    let mut result = Vec::new();
    for device in [
        TargetBackend::Cpu,
        TargetBackend::Gpu,
        TargetBackend::Photonic,
    ] {
        let peak = (0..=graph.nodes.len())
            .map(|time| {
                intervals
                    .get(&device)
                    .into_iter()
                    .flatten()
                    .filter(|(start, end, _)| *start <= time && time <= *end)
                    .map(|(_, _, bytes)| *bytes)
                    .sum::<u64>()
            })
            .max()
            .unwrap_or(0);
        result.push(MemoryPeak {
            device,
            bytes: peak,
            budget_bytes: options.memory_budget(device),
        });
    }
    Ok(result)
}

fn node_traces(
    graph: &PartitionGraph,
    index: &GraphIndex,
    assignment: &[TargetBackend],
    objective: OptimizationObjective,
) -> Vec<NodePlacementTrace> {
    index
        .order
        .iter()
        .enumerate()
        .map(|(position, node_index)| {
            let node = &graph.nodes[*node_index];
            let mut legal = node
                .candidates
                .iter()
                .filter_map(|candidate| {
                    candidate
                        .cost
                        .filter(|_| candidate.eligible)
                        .map(|cost| (candidate.device, node_objective_score(objective, cost)))
                })
                .collect::<Vec<_>>();
            legal.sort_by(|left, right| {
                left.1
                    .total_cmp(&right.1)
                    .then_with(|| left.0.cmp(&right.0))
            });
            let local_best = legal[0].0;
            let selected = assignment[position];
            let rationale = if selected == local_best {
                format!(
                    "selected {:?}; it is also the node-local {:?} winner",
                    selected, objective
                )
            } else {
                format!(
                    "selected {:?} instead of node-local {:?} winner {:?} because whole-region transfer, reuse, crossing, or memory cost is lower",
                    selected, objective, local_best
                )
            };
            NodePlacementTrace {
                node_id: node.id.clone(),
                kind: node.kind,
                selected_device: selected,
                local_best_device: local_best,
                candidates: node.candidates.clone(),
                rationale,
            }
        })
        .collect()
}

fn build_regions(
    graph: &PartitionGraph,
    index: &GraphIndex,
    assignment: &[TargetBackend],
) -> Result<Vec<PartitionRegion>> {
    let positions = order_positions(index);
    let mut parent = (0..graph.nodes.len()).collect::<Vec<_>>();
    let selected = |node_index: usize| assignment[positions[&node_index]];
    for (tensor_index, consumers) in index.consumers.iter().enumerate() {
        if let Some(producer) = index.producer[tensor_index] {
            for consumer in consumers {
                if selected(producer) == selected(*consumer)
                    && !is_barrier(&graph.nodes[producer])
                    && !is_barrier(&graph.nodes[*consumer])
                {
                    union(&mut parent, producer, *consumer);
                }
            }
        }
        for left in 0..consumers.len() {
            for right in left + 1..consumers.len() {
                let left_node = consumers[left];
                let right_node = consumers[right];
                if selected(left_node) == selected(right_node)
                    && !is_barrier(&graph.nodes[left_node])
                    && !is_barrier(&graph.nodes[right_node])
                {
                    union(&mut parent, left_node, right_node);
                }
            }
        }
    }
    let mut groups: BTreeMap<(TargetBackend, usize), Vec<usize>> = BTreeMap::new();
    for node_index in &index.order {
        let root = find(&mut parent, *node_index);
        groups
            .entry((selected(*node_index), root))
            .or_default()
            .push(*node_index);
    }
    let mut regions = Vec::new();
    for (region_index, ((device, _), mut members)) in groups.into_iter().enumerate() {
        members.sort_by_key(|member| positions[member]);
        let member_set = members.iter().copied().collect::<BTreeSet<_>>();
        let mut external_inputs = BTreeSet::new();
        let mut external_outputs = BTreeSet::new();
        let mut cost = PartitionCost {
            latency_ns: 0.0,
            energy_uj: 0.0,
            error_fraction: 0.0,
            operations: 0.0,
            source: ParameterSource::Assumed,
        };
        let mut error_squared = 0.0;
        let mut source_rank = 0;
        for member in &members {
            let node = &graph.nodes[*member];
            let node_cost = candidate_cost(node, device).with_context(|| {
                format!("region contains illegal {:?} node '{}'", device, node.id)
            })?;
            cost.latency_ns += node_cost.latency_ns;
            cost.energy_uj += node_cost.energy_uj;
            cost.operations += node_cost.operations;
            error_squared += node_cost.error_fraction * node_cost.error_fraction;
            let rank = parameter_source_rank(node_cost.source);
            if rank >= source_rank {
                source_rank = rank;
                cost.source = node_cost.source;
            }
            for input in &node.inputs {
                let tensor_index = index.tensor_by_id[input];
                if index.producer[tensor_index]
                    .is_none_or(|producer| !member_set.contains(&producer))
                {
                    external_inputs.insert(input.clone());
                }
            }
            for output in &node.outputs {
                let tensor_index = index.tensor_by_id[output];
                if index.consumers[tensor_index]
                    .iter()
                    .any(|consumer| !member_set.contains(consumer))
                    || graph.tensors[tensor_index].required_device.is_some()
                {
                    external_outputs.insert(output.clone());
                }
            }
        }
        cost.error_fraction = error_squared.sqrt().min(1.0);
        regions.push(PartitionRegion {
            id: format!("region-{region_index:03}"),
            device,
            nodes: members
                .iter()
                .map(|member| graph.nodes[*member].id.clone())
                .collect(),
            external_inputs: external_inputs.into_iter().collect(),
            external_outputs: external_outputs.into_iter().collect(),
            fused: members.len() > 1,
            node_cost: cost,
        });
    }
    regions.sort_by(|left, right| {
        let left_position = positions[&index.node_by_id[&left.nodes[0]]];
        let right_position = positions[&index.node_by_id[&right.nodes[0]]];
        left_position.cmp(&right_position)
    });
    for (index, region) in regions.iter_mut().enumerate() {
        region.id = format!("region-{index:03}");
    }
    Ok(regions)
}

fn profiler_events(
    transfers: &[TransferRecord],
    regions: &[PartitionRegion],
) -> Vec<PartitionProfilerEvent> {
    let mut events = Vec::new();
    for transfer in transfers {
        events.push(PartitionProfilerEvent {
            sequence: events.len(),
            kind: ProfilerEventKind::TensorTransfer,
            name: transfer.id.clone(),
            tensor: Some(transfer.tensor.clone()),
            region: None,
            from: Some(transfer.from),
            to: Some(transfer.to),
            bytes: transfer.bytes,
            estimated_latency_ns: transfer.latency_ns,
            estimated_energy_uj: transfer.energy_uj,
        });
        if transfer.optical_electrical_boundary {
            events.push(PartitionProfilerEvent {
                sequence: events.len(),
                kind: ProfilerEventKind::OpticalElectricalBoundary,
                name: format!("boundary:{}", transfer.id),
                tensor: Some(transfer.tensor.clone()),
                region: None,
                from: Some(transfer.from),
                to: Some(transfer.to),
                bytes: transfer.bytes,
                estimated_latency_ns: transfer.latency_ns,
                estimated_energy_uj: transfer.energy_uj,
            });
        }
    }
    for region in regions {
        events.push(PartitionProfilerEvent {
            sequence: events.len(),
            kind: ProfilerEventKind::RegionExecute,
            name: format!("execute:{}", region.id),
            tensor: None,
            region: Some(region.id.clone()),
            from: None,
            to: Some(region.device),
            bytes: 0,
            estimated_latency_ns: region.node_cost.latency_ns,
            estimated_energy_uj: region.node_cost.energy_uj,
        });
    }
    events
}

fn visualization_edges(
    graph: &PartitionGraph,
    index: &GraphIndex,
    assignment: &[TargetBackend],
) -> Vec<VisualizationEdge> {
    let positions = order_positions(index);
    let device_for_node = |node_index: usize| assignment[positions[&node_index]];
    let mut edges = Vec::new();
    for (tensor_index, tensor) in graph.tensors.iter().enumerate() {
        let producer = index.producer[tensor_index];
        let from = producer
            .map(&device_for_node)
            .or(tensor.initial_device)
            .unwrap_or(TargetBackend::Cpu);
        for consumer in &index.consumers[tensor_index] {
            let to = device_for_node(*consumer);
            edges.push(VisualizationEdge {
                tensor: tensor.id.clone(),
                producer: producer.map(|node| graph.nodes[node].id.clone()),
                consumer: graph.nodes[*consumer].id.clone(),
                from,
                to,
                crosses_device: from != to,
            });
        }
    }
    edges.sort_by(|left, right| {
        left.consumer
            .cmp(&right.consumer)
            .then_with(|| left.tensor.cmp(&right.tensor))
    });
    edges
}

fn graph_fingerprint(request: &PartitionRequest) -> Result<String> {
    let bytes = serde_json::to_vec(request)?;
    Ok(format!("fnv1a64:{:016x}", stable_fingerprint_bytes(&bytes)))
}

fn assignment_tie_key(seed: u64, assignment: &[TargetBackend]) -> u64 {
    let text = format!("{seed}|{assignment:?}");
    stable_fingerprint_bytes(text.as_bytes())
}

fn order_positions(index: &GraphIndex) -> HashMap<usize, usize> {
    index
        .order
        .iter()
        .enumerate()
        .map(|(position, node)| (*node, position))
        .collect()
}

fn objective_score(objective: OptimizationObjective, totals: &PartitionTotals) -> f64 {
    match objective {
        OptimizationObjective::Latency => totals.latency_ns,
        OptimizationObjective::Energy => totals.energy_uj,
        OptimizationObjective::Accuracy => totals.error_fraction,
        OptimizationObjective::Throughput => -totals.throughput_gops,
    }
}

fn node_objective_score(objective: OptimizationObjective, cost: PartitionCost) -> f64 {
    match objective {
        OptimizationObjective::Latency => cost.latency_ns,
        OptimizationObjective::Energy => cost.energy_uj,
        OptimizationObjective::Accuracy => cost.error_fraction,
        OptimizationObjective::Throughput => -cost.operations / cost.latency_ns,
    }
}

fn parameter_source_rank(source: ParameterSource) -> u8 {
    match source {
        ParameterSource::Measured => 0,
        ParameterSource::VendorSpecified => 1,
        ParameterSource::Simulated => 2,
        ParameterSource::Assumed => 3,
    }
}

fn is_barrier(node: &GraphNode) -> bool {
    node.control_flow_barrier || node.kind == GraphOpKind::ControlFlowBarrier
}

fn find(parent: &mut [usize], node: usize) -> usize {
    if parent[node] != node {
        parent[node] = find(parent, parent[node]);
    }
    parent[node]
}

fn union(parent: &mut [usize], left: usize, right: usize) {
    let left_root = find(parent, left);
    let right_root = find(parent, right);
    if left_root != right_root {
        let (minimum, maximum) = if left_root < right_root {
            (left_root, right_root)
        } else {
            (right_root, left_root)
        };
        parent[maximum] = minimum;
    }
}

fn positive(value: f64, name: &str) -> Result<()> {
    if !value.is_finite() || value <= 0.0 {
        bail!("{name} must be finite and positive");
    }
    Ok(())
}

fn non_negative(value: f64, name: &str) -> Result<()> {
    if !value.is_finite() || value < 0.0 {
        bail!("{name} must be finite and non-negative");
    }
    Ok(())
}

fn fraction(value: f64, name: &str) -> Result<()> {
    if !value.is_finite() || !(0.0..=1.0).contains(&value) {
        bail!("{name} must be finite and within [0, 1]");
    }
    Ok(())
}
