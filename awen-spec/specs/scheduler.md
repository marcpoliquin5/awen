# Scheduler Conformance

## Scope

The shipped scheduler is `awen_runtime::scheduler::StaticScheduler`. It converts an
AWEN IR graph and `SchedulingConstraints` into an `ExecutionPlan`. The public
`Scheduler` trait also validates an existing plan against a `ResourceState`.

The scheduler is deterministic for the same graph, constraints, and seed. Its
execution plan records the algorithm identifier, makespan, critical path,
per-node timing, allocated resources, resource-usage data, and provenance.

## Constraints

`SchedulingConstraints` carries:

- coherence windows;
- measurement-to-control feedback-loop deadlines;
- timing constraints and violation actions; and
- wavelength, memory-slot, and concurrency limits.

The current static implementation schedules nodes in graph order after dependency
analysis, derives each node's earliest start from incoming edges, assigns the
available wavelength and memory resources, validates coherence containment and
fidelity thresholds, and rejects feedback-loop deadline violations.

## Conformance evidence

The implementation is exercised directly by unit tests in
`awen-runtime/src/scheduler/mod.rs` and by
`awen-runtime/tests/scheduling_integration.rs`. The integration suite covers:

- deterministic replay;
- critical-path identification;
- coherence-window enforcement;
- accepted and rejected feedback-loop deadlines;
- wavelength allocation and skew compensation;
- execution-plan serialization;
- timing constraints and plan validation; and
- multi-node graph scheduling.

The repository's required quality gate runs these tests as part of the complete
`awen-runtime` test suite. There is no separate scheduler check, legacy scheduler
module, or mock-only conformance suite.

## Change control

Only behavior present in the exported module and exercised by the required gate
is conformant. New scheduling strategies or guarantees require a GitHub issue
with an owner and measurable acceptance criteria, followed by implementation,
direct tests, and this document's update in the same pull request.
