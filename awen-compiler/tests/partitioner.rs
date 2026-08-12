use awen_compiler::{
    partition_graph, GraphNode, GraphOpKind, GraphTensor, NodeCandidate, ParameterSource,
    PartitionCost, PartitionGraph, PartitionOptions, PartitionRequest, ProfilerEventKind,
    TargetBackend, PARTITION_GRAPH_VERSION,
};
use std::collections::BTreeMap;

fn cost(latency_ns: f64) -> PartitionCost {
    PartitionCost {
        latency_ns,
        energy_uj: latency_ns / 100.0,
        error_fraction: 0.0001,
        operations: 1_000_000.0,
        source: ParameterSource::Simulated,
    }
}

fn candidate(device: TargetBackend, latency_ns: f64) -> NodeCandidate {
    NodeCandidate {
        device,
        eligible: true,
        cost: Some(cost(latency_ns)),
        reason: format!("{device:?} test candidate"),
    }
}

fn tensor(id: &str, bytes: u64) -> GraphTensor {
    GraphTensor {
        id: id.to_string(),
        bytes,
        initial_device: None,
        required_device: None,
        persistent: false,
    }
}

fn request(tensors: Vec<GraphTensor>, nodes: Vec<GraphNode>) -> PartitionRequest {
    PartitionRequest {
        graph: PartitionGraph {
            graph_version: PARTITION_GRAPH_VERSION.to_string(),
            tensors,
            nodes,
        },
        options: PartitionOptions::default(),
    }
}

#[test]
fn individually_faster_photonic_op_is_rejected_when_boundaries_make_region_slower() {
    let mut input = tensor("input", 8_192);
    input.initial_device = Some(TargetBackend::Cpu);
    let mut output = tensor("output", 8_192);
    output.required_device = Some(TargetBackend::Cpu);
    let graph = request(
        vec![input, output],
        vec![GraphNode {
            id: "isolated".to_string(),
            kind: GraphOpKind::Gemm,
            inputs: vec!["input".to_string()],
            outputs: vec!["output".to_string()],
            dynamic_shape: false,
            control_flow_barrier: false,
            candidates: vec![
                candidate(TargetBackend::Cpu, 1_200.0),
                candidate(TargetBackend::Photonic, 1_000.0),
            ],
        }],
    );

    let trace = partition_graph(&graph).expect("graph must partition");
    assert_eq!(trace.selected.assignments["isolated"], TargetBackend::Cpu);
    assert_eq!(trace.nodes[0].local_best_device, TargetBackend::Photonic);
    assert!(trace.nodes[0].rationale.contains("whole-region"));
}

#[test]
fn transformer_golden_groups_linear_attention_and_keeps_nonlinear_ops_digital() {
    let request: PartitionRequest =
        serde_json::from_str(include_str!("fixtures/partition_transformer.json"))
            .expect("transformer fixture must parse");
    let expected: BTreeMap<String, TargetBackend> =
        serde_json::from_str(include_str!("fixtures/partition_transformer.golden.json"))
            .expect("transformer golden must parse");

    let trace = partition_graph(&request).expect("transformer graph must partition");
    assert_eq!(trace.selected.assignments, expected);
    let shared = trace
        .transfers
        .iter()
        .find(|transfer| {
            transfer.tensor == "norm"
                && transfer.from == TargetBackend::Gpu
                && transfer.to == TargetBackend::Photonic
        })
        .expect("normalized activation should be uploaded once");
    assert_eq!(
        shared.consumer_nodes,
        vec![
            "k_proj".to_string(),
            "q_proj".to_string(),
            "v_proj".to_string()
        ]
    );
    assert!(trace.regions.iter().any(|region| {
        region.device == TargetBackend::Photonic
            && ["q_proj", "k_proj", "v_proj", "attention_scores"]
                .iter()
                .all(|node| region.nodes.iter().any(|member| member == node))
    }));
    assert!(trace
        .profiler_events
        .iter()
        .any(|event| event.kind == ProfilerEventKind::OpticalElectricalBoundary));
    assert!(trace
        .visualization_edges
        .iter()
        .any(|edge| edge.crosses_device));
}

#[test]
fn scientific_golden_fuses_the_linear_pipeline() {
    let request: PartitionRequest =
        serde_json::from_str(include_str!("fixtures/partition_scientific.json"))
            .expect("scientific fixture must parse");
    let expected: BTreeMap<String, TargetBackend> =
        serde_json::from_str(include_str!("fixtures/partition_scientific.golden.json"))
            .expect("scientific golden must parse");

    let trace = partition_graph(&request).expect("scientific graph must partition");
    assert_eq!(trace.selected.assignments, expected);
    let region = trace
        .regions
        .iter()
        .find(|region| region.device == TargetBackend::Photonic)
        .expect("photonic region");
    assert_eq!(region.nodes.len(), 3);
    assert!(region.fused);
}

#[test]
fn fixed_snapshot_and_seed_are_byte_deterministic() {
    let request: PartitionRequest =
        serde_json::from_str(include_str!("fixtures/partition_transformer.json"))
            .expect("transformer fixture must parse");
    let first = partition_graph(&request).expect("first partition");
    let second = partition_graph(&request).expect("second partition");
    assert_eq!(
        serde_json::to_vec(&first).expect("serialize first"),
        serde_json::to_vec(&second).expect("serialize second")
    );
}

#[test]
fn fan_out_transfer_is_deduplicated_per_tensor_and_target_device() {
    let mut input = tensor("shared", 4_096);
    input.initial_device = Some(TargetBackend::Cpu);
    let mut left = tensor("left", 4_096);
    left.required_device = Some(TargetBackend::Photonic);
    let mut right = tensor("right", 4_096);
    right.required_device = Some(TargetBackend::Photonic);
    let nodes = ["left_op", "right_op"]
        .iter()
        .enumerate()
        .map(|(index, id)| GraphNode {
            id: (*id).to_string(),
            kind: GraphOpKind::Gemm,
            inputs: vec!["shared".to_string()],
            outputs: vec![if index == 0 { "left" } else { "right" }.to_string()],
            dynamic_shape: false,
            control_flow_barrier: false,
            candidates: vec![candidate(TargetBackend::Photonic, 10.0)],
        })
        .collect();
    let trace = partition_graph(&request(vec![input, left, right], nodes)).expect("partition");
    let shared_transfers = trace
        .transfers
        .iter()
        .filter(|transfer| transfer.tensor == "shared")
        .collect::<Vec<_>>();
    assert_eq!(shared_transfers.len(), 1);
    assert_eq!(shared_transfers[0].consumer_nodes.len(), 2);
}

#[test]
fn unsupported_dynamic_and_barrier_nodes_never_select_photonic() {
    let mut input = tensor("input", 1_024);
    input.initial_device = Some(TargetBackend::Cpu);
    let mut output = tensor("output", 1_024);
    output.required_device = Some(TargetBackend::Cpu);
    let node = GraphNode {
        id: "dynamic_softmax".to_string(),
        kind: GraphOpKind::Softmax,
        inputs: vec!["input".to_string()],
        outputs: vec!["output".to_string()],
        dynamic_shape: true,
        control_flow_barrier: true,
        candidates: vec![
            candidate(TargetBackend::Cpu, 100.0),
            candidate(TargetBackend::Gpu, 20.0),
            NodeCandidate {
                device: TargetBackend::Photonic,
                eligible: false,
                cost: None,
                reason: "dynamic nonlinear barrier is unsupported".to_string(),
            },
        ],
    };
    let trace = partition_graph(&request(vec![input, output], vec![node])).expect("partition");
    assert_eq!(
        trace.selected.assignments["dynamic_softmax"],
        TargetBackend::Gpu
    );
}

#[test]
fn memory_pressure_rejects_the_otherwise_fastest_device() {
    let mut input = tensor("input", 1_024);
    input.initial_device = Some(TargetBackend::Cpu);
    let mut output = tensor("output", 1_024);
    output.required_device = Some(TargetBackend::Cpu);
    let node = GraphNode {
        id: "memory_bound".to_string(),
        kind: GraphOpKind::Gemm,
        inputs: vec!["input".to_string()],
        outputs: vec!["output".to_string()],
        dynamic_shape: false,
        control_flow_barrier: false,
        candidates: vec![
            candidate(TargetBackend::Cpu, 100.0),
            candidate(TargetBackend::Gpu, 10.0),
        ],
    };
    let mut request = request(vec![input, output], vec![node]);
    request.options.transfer_latency_ns = 0.0;
    request.options.gpu_memory_budget_bytes = 1_500;
    let trace = partition_graph(&request).expect("partition");
    assert_eq!(
        trace.selected.assignments["memory_bound"],
        TargetBackend::Cpu
    );
    assert!(trace.alternatives.iter().all(|alternative| alternative
        .memory_peaks
        .iter()
        .all(|peak| peak.bytes <= peak.budget_bytes)));
}

#[test]
fn illegal_photonic_candidate_on_unsupported_node_is_rejected() {
    let mut input = tensor("input", 64);
    input.initial_device = Some(TargetBackend::Cpu);
    let output = tensor("output", 64);
    let node = GraphNode {
        id: "bad".to_string(),
        kind: GraphOpKind::HostIrregular,
        inputs: vec!["input".to_string()],
        outputs: vec!["output".to_string()],
        dynamic_shape: false,
        control_flow_barrier: false,
        candidates: vec![candidate(TargetBackend::Photonic, 1.0)],
    };
    let error = partition_graph(&request(vec![input, output], vec![node]))
        .expect_err("unsupported photonic candidate must fail validation");
    assert!(error.to_string().contains("illegally enables photonics"));
}
