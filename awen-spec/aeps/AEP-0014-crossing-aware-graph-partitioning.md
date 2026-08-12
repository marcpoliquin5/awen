# AEP-0014: Crossing-aware whole-graph partitioning

Status: Accepted and implemented

## Summary

AWEN defines `awen.partition-graph.v1` and `awen.partition-trace.v1` for
deterministic placement of complete acyclic tensor graphs across CPU, GPU, and
photonic devices. Placement minimizes the selected full-system objective over
regions, tensor residency, deduplicated transfers, optical/electrical
boundaries, numerical error, and live device memory. It does not select each
operation independently.

## Motivation

An optical GEMM can be the fastest local implementation and still make an
application slower after uploads, conversion, downloads, shared-operand
fan-out, nonlinear digital stages, and memory pressure are included. Conversely,
several linear operations can justify one photonic region when their inputs are
reused and intermediate values remain resident. A local greedy decision cannot
distinguish those cases or explain the resulting crossing pattern.

Transformer blocks make the requirement concrete. Q, K, and V projections
reuse one normalized activation; attention GEMMs are linear; layer
normalization, softmax, and GELU remain digital. Scientific pipelines commonly
contain longer linear chains and large resident operators. Both need a graph
contract that represents dependencies, reuse, barriers, residency, and legal
device candidates before lowering.

## Graph contract

A partition request contains a versioned graph and options. Every tensor
declares a stable identifier and byte size, and may declare `initial_device`,
`required_device`, and `persistent` residency. Every node declares a stable
identifier, operation kind, input and output tensor identifiers, dynamic-shape
state, control-flow-barrier state, and one candidate record per considered
device. An eligible candidate carries internal execution latency, energy,
numerical error, operations, and provenance. An ineligible candidate carries an
explicit reason and no cost.

`auto` is a policy request, not a physical device, and is forbidden in tensor
residency, concrete candidates, assignments, transfers, regions, and profiler
records. Photonic eligibility is restricted to GEMM, batched GEMM, and declared
linear transforms. Dynamic, nonlinear, irregular host, and control-flow barrier
nodes cannot advertise an eligible photonic candidate.

The graph must be a directed acyclic graph. Tensor identifiers and node
identifiers are unique, a tensor has at most one producer, every reference
resolves, and every graph input declares initial residency. The partitioner
rejects malformed graphs before search.

## Full-graph objective

For an assignment `A`, AWEN computes:

```text
latency(A) = sum(node_internal_latency) + sum(deduplicated_transfer_latency)
energy(A)  = sum(node_internal_energy)  + sum(deduplicated_transfer_energy)
error(A)   = min(1, sqrt(sum(component_error_fraction^2)))
throughput(A) = sum(operations) / latency(A)
```

The transfer of tensor `t` from device `a` to device `b` costs:

```text
transfer_latency_ns = fixed_transfer_latency_ns
                    + t.bytes * 8 / transfer_bandwidth_gbps
                    + optical_electrical_crossing_penalty_ns

transfer_energy_uJ = t.bytes * transfer_energy_pJ_per_byte / 1,000,000
                   + optical_electrical_crossing_penalty_uJ
```

The crossing terms apply only when either endpoint is photonic. A tensor
produced once and consumed by several nodes on the same target device is
transferred once, with every consumer listed. A second target device requires a
second transfer. Required final residency is represented by a transfer with no
consumer node.

Compiler-derived node costs remove host transfer, generic memory movement, and
photonic boundary conversion from per-operation estimates before graph search.
The graph transfer model reintroduces those terms once, preventing isolated-op
double counting and allowing intermediate residency to eliminate crossings.

The supported objectives are minimum latency, minimum energy, minimum numerical
error, and maximum throughput. An individually faster photonic operation must
not be selected when its complete graph assignment has a worse objective score.

## Residency and memory pressure

AWEN derives a live interval for every tensor residency on every device. The
producer residency begins when the tensor is produced, graph inputs begin live
at graph entry, target residency begins at its first consumer, and residency
ends at the last consumer. Persistent tensors remain live through graph exit.
Required output residency is live at graph exit.

The trace reports peak live bytes and the configured budget for CPU, GPU, and
photonics. An assignment whose peak exceeds any budget is illegal and is not
reported as selected or alternative. If all assignments exceed a budget,
partitioning fails instead of silently oversubscribing memory.

## Regions and fusion

Connected same-device producer/consumer nodes form a region. Same-device
consumers of one shared operand may also join one region so Q/K/V projection
fan-out is represented as one reusable placement group. Control-flow barriers
never fuse with adjacent nodes.

Each region records its device, ordered node membership, external inputs,
external outputs, fusion state, and the sum of internal node costs. Fusion never
makes an unsupported operation photonic and never removes a real boundary.

## Deterministic search

Candidate devices and ready DAG nodes are ordered by stable identifiers. If the
candidate cross-product does not exceed `max_search_states`, the partitioner
evaluates every assignment exactly. Larger spaces use a deterministic bounded
beam ordered by partial objective score, a seed-dependent stable FNV-1a tie key,
and a final lexicographic assignment key.

Complete legal assignments are ordered by full objective score, the same seeded
tie key, and the same lexicographic key. For a fixed graph, candidates, device
and calibration snapshot, options, objective, and seed, serialized output is
byte deterministic. The trace fingerprint covers the complete request.

## Trace, profiling, and visualization

The selected trace contains:

- the complete node-to-device assignment and ranked legal alternatives;
- full-system totals and per-device memory peaks;
- each node's selected device, node-local winner, candidates, and rationale;
- every deduplicated transfer and its consumers, bytes, cost, error, and
  optical/electrical-boundary flag;
- fused regions and their external tensors;
- explicit tensor-transfer, optical/electrical-boundary, and region-execution
  profiler events;
- producer/consumer visualization edges annotated with source, target, and
  whether the edge crosses devices;
- search strategy, stable fingerprint, and a whole-graph rationale.

Compilation artifacts embed this trace. Digital fallback lowering names CPU or
GPU explicitly. The reference simulator executes both digital targets with the
numerical reference path while retaining their distinct modeled costs.

## Numerical contracts and fallback

Graph placement consumes only candidates admitted by capability and numerical
contract negotiation. If photonic precision, calibration, health, shape,
layout, dtype, or another required capability is insufficient, the photonic
candidate is ineligible. Auto placement deterministically considers the
remaining CPU and GPU candidates. Forced photonic compilation fails if any
required operation has no legal photonic plan.

## Backwards compatibility

The graph and trace schemas are independently versioned additions. Existing
Tensor IR inputs remain valid. `CompileOptions` uses serde defaults for all new
GPU, transfer, crossing, memory, alternative, and search controls. Compilation
artifacts add `partition_trace`; Device IR and Classical Photonic IR host
fallback records add a concrete `target`. Consumers validating those v1
artifacts must accept the added graph trace and required digital target.

## Test plan

- Prove that a locally faster isolated photonic operation remains digital when
  graph crossings make it slower.
- Golden-test a transformer block with Q/K/V shared-operand fan-out, attention
  GEMMs, GPU layer normalization, GPU softmax, GPU GELU, and photonic regions.
- Golden-test a fused scientific linear pipeline with resident operators.
- Verify one shared tensor transfer serves every consumer on the same device.
- Verify fan-out, required output residency, and visualization edges.
- Verify unsupported, dynamic, nonlinear, irregular, and barrier operations
  never select photonics; reject illegally advertised photonic candidates.
- Verify memory pressure rejects an otherwise fastest assignment.
- Verify selected and alternative assignments always satisfy memory budgets.
- Verify explicit transfer, boundary, and region profiler events.
- Verify byte-identical traces for a fixed request and seed.
- Verify compiler artifacts embed the trace and lowering retains CPU/GPU target
  identity.
