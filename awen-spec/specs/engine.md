# Runtime Engine Conformance

## Exported engines

The runtime exports two engine modules with different data models:

- `awen_runtime::engine::Engine` executes the primary IR `Graph`, validates its
  conditional branches, emits a typed local artifact bundle, and returns the
  artifact path.
- `awen_runtime::engine_v2::Engine` executes its `ComputationGraph` against an
  explicit `ExecutionPlan`, seed, calibration identifier, and safety constraints,
  returning a typed `ExecutionResult`.

The module names are part of the public API. No compatibility claim is made
between the two graph or result models.

## Primary engine behavior

`engine::Engine::run_graph` performs the following observable work:

1. validates the input graph and measurement-conditioned branches;
2. executes the graph with a caller-supplied or generated seed;
3. records scheduler, kernel, measurement, and state information;
4. builds the typed artifact through the storage module; and
5. persists the artifact below the runtime artifact directory.

Calibration updates pass through `apply_calibration`, including safety-bound
validation before values are applied.

## Version-two engine behavior

`engine_v2::Engine::run_graph` validates graph references and plan membership,
executes plan phases, checks hard and soft safety constraints, tracks coherence
violations, records per-node execution logs and measurement outcomes, and returns
status, timing, and artifact data. Seeded execution is deterministic within the
implemented simulator model.

The version-two implementation is a Rust runtime model. It does not claim remote
or physical-device execution merely because a node type names photonic or quantum
operations.

## Conformance evidence

Direct evidence consists of unit tests in both engine modules plus
`awen-runtime/tests/engine_integration.rs`. Those tests cover classical and quantum
node variants, measurements, calibration, coherence budgets, safety limits,
seeded replay, invalid graph references, branching and linear graphs, failure
tracking, and timing records. Primary-engine artifact behavior is additionally
covered in its module tests and the artifact integration suites.

The repository's required quality gate runs the entire runtime suite. There is no
separate engine check whose result could diverge from that required context.

## Change control

Only exported behavior exercised by direct automated evidence is conformant. A
new engine guarantee requires a GitHub issue with an owner and measurable
acceptance criteria, implementation, and direct tests before this document may
claim it.
