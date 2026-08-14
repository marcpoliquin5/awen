# AEP-0021: Physical-design tooling boundary

Status: Accepted and implemented

## Decision

AWEN is a logical compiler and heterogeneous runtime. It does not own PDK
authoring, component geometry, mask layout, GDS editing, routing, DRC, LVS,
foundry rule decks, electromagnetic solvers, or circuit-solver physics.

AWEN exchanges the closed `awen.physical-design.v1` contract with external
physical-design adapters. A compiler exports logical photonic operations,
required ports, unit-bearing mapping constraints, and candidate topologies. A
gdsfactory adapter returns a verified binding to an external PDK, process
corner, component library, topology, circuit models, adapter toolchain, and
verification reports. Every external artifact is identified by an immutable
URN or SHA-256 identity and a lowercase SHA-256 content digest.

The imported binding is part of `awen.device-capability.v1`. A compilation
artifact stores an identity-only `PhysicalDesignProvenance` record. The exact
binding participates in the backend snapshot fingerprint and in the calibrated
topology fingerprint. A PDK version, PDK manifest, process corner, component
library, topology, model, adapter, simulation, or verification change therefore
prevents cache reuse and triggers diagnosed recompilation.

## gdsfactory ownership

gdsfactory remains the source of truth for:

- PDK activation, cell factories, cross sections, layers, layer stacks,
  transitions, material indices, connectivity, database units, and models;
- component settings and metadata;
- named optical, electrical, and placement ports, including center, width,
  orientation, layer, type, wavelength range, and mode;
- geometry, routing, GDS/OASIS writing, and layout visualization; and
- layout verification and foundry workflows.

The AWEN adapter maps gdsfactory component and port metadata into the closed
contract. It may retain an HTTPS discovery location for an open artifact, but
the digest, not that mutable location, is the identity. It must never place
polygons, paths, cells, GDS bytes, layer geometry, rule decks, or foundry source
data in an AWEN request, capability, or compilation artifact.

AWEN's topology contract is a logical graph of component instances, named
ports, and connections. It is not a layout database. Layout constraints are
scalar limits and layer allowlists, not geometry.

## Circulax evaluation and ownership

Circulax is an optional circuit-simulator adapter, not a compiler dependency.
Its `compile_circuit` surface accepts kfnetlist objects, SAX-format dictionaries,
or recursive netlists and maps component classes, SAX functions, or nested
compiled circuits into a callable circuit. Its JAX implementation supports
parameter updates and differentiable solving, including DC, transient,
small-signal/S-parameter, and harmonic-balance analyses.

Those properties make Circulax suitable for:

- differentiable model fitting against calibration measurements;
- batched wavelength and component-parameter sweeps;
- hardware-aware optimization of a candidate topology;
- sensitivity analysis and gradient-based calibration; and
- externally generated circuit-simulation evidence.

AWEN imports only the circuit model's artifact identity, named ports,
wavelength band, public numeric parameters, tool/version, simulation-settings
fingerprint, and immutable result/report identity. Circulax retains ownership of
the JAX program, solver selection, equations, tolerances, state, gradients, and
raw results. AWEN does not vendor, wrap, fork, or reimplement those internals.

The reference contract records a Circulax model and adapter to prove the
boundary, but the Rust compiler/runtime has no gdsfactory, Circulax, SAX,
kfnetlist, JAX, electromagnetic-solver, or foundry package dependency.

## Mapping exchange

`MappingRequest` contains:

- the exact `awen.physical-design.v1` version and a request identity;
- a closed list of logical matrix-multiply, phase-shift, split, combine, and
  detect operations;
- required named ports with kind, center, orientation, width, unit, layer,
  wavelength band, and optional mode;
- unit-bearing maximum dimensions, minimum bend radius, maximum path-length
  imbalance, maximum crossings, and allowed layers; and
- one or more fully connected candidate logical topologies.

`MappingResponse` contains the matching request identity, the exact gdsfactory
adapter contract, and a verified physical-design binding. Import fails unless:

- versions and request identities match;
- the response adapter is gdsfactory and is recorded in the binding;
- the selected topology name was exported as a candidate;
- every required port is present with the same kind and width after canonical
  conversion to micrometers;
- the result respects every size, bend-radius, imbalance, crossing, and layer
  constraint;
- every topology endpoint resolves to a declared port;
- the SHA-256 topology digest matches the imported topology exactly;
- adapter kinds, model names, node IDs, port names, layers, and connections are
  well formed and unique where required;
- a Circulax model has a circuit-simulator adapter;
- every verification kind is supported by a recorded adapter; and
- passed connectivity evidence and immutable verification settings/report
  identities are present.

Failed or unverified mappings are not valid `PhysicalDesignBinding` values.
They may remain external diagnostic artifacts, but they cannot enter backend
capabilities or compilation provenance as verified designs.

## Units

Port and layout lengths declare `nanometer`, `micrometer`, or `meter`. AWEN
preserves the declared unit during serialization and converts to micrometers
only for compatibility checks. Wavelengths are always nanometers. Angles are
degrees in the half-open interval `[0, 360)`. No adapter may infer a unit from a
bare field name or from the active PDK.

## Immutable provenance and invalidation

Each artifact reference contains:

- an immutable `urn:` or complete `sha256:` artifact identity;
- a lowercase `sha256:<64 hex digits>` content digest;
- a media type; and
- for open artifacts only, an optional HTTPS or URN discovery location.

The binding verifies the topology digest against its inline logical contract.
Other digests bind externally owned content that is verified by the adapter
before import. The complete binding receives its own SHA-256 fingerprint.

`PhysicalDesignProvenance` records only identities: binding fingerprint,
classification, PDK name/version/manifest, process-corner identity, component
library, topology, circuit-model artifacts, and verification reports. It does
not contain component settings, model parameters, process parameters, topology
internals, solver state, geometry, or report payloads.

Compilation stores this provenance and hashes the complete backend snapshot.
Refresh emits a specific PDK, process-corner, or general physical-binding
invalidation reason before recompiling. Calibration is bound to the updated
topology fingerprint as well, so a calibration snapshot from the old physical
design cannot be reused on the new one.

## Proprietary PDK boundary

`proprietary_reference` is an identity-only contract. Validation requires:

- every artifact URI to be absent;
- all process-corner, component-setting, and circuit-model parameter maps to be
  empty;
- topology nodes and connections to be absent, leaving only the abstract
  external port interface; and
- PDK, component library, topology, models, settings, and reports to remain
  opaque immutable references.

Public compilation provenance applies a second URI-redaction pass. The public
record has no field capable of carrying process parameters, component settings,
model parameters, topology internals, geometry, foundry rules, solver state, or
raw verification data. Unknown fields fail both Rust deserialization and JSON
Schema validation.

An open reference may include public scalar parameters and a logical topology.
It still may not include geometry or layout/foundry payloads.

## Plugin boundary

The signed `awen.plugin-manifest.v1` schema has an optional
`physical_design_adapters` array. Adapter kinds are closed to `gdsfactory`,
`circuit_simulator`, and `electromagnetic_simulator`. Every adapter declares an
exact tool/version, request/response contract version, and supported evidence
kinds. Runtime validation rejects version skew, duplicate adapter kinds, empty
tool identities, and duplicate evidence declarations.

The plugin mechanism supplies discovery, signature policy, executable location,
and typed contract negotiation. It does not standardize a solver's proprietary
command line, file format, license server, or remote service API. Such details
remain inside the signed adapter and must yield the same versioned response and
immutable evidence contract.

## Backwards compatibility

`awen.device-capability.v1` now requires `physical_design`. This is an
intentional fail-closed tightening of a pre-release contract. Existing backend
manifests must add a verified binding and update their calibration topology
fingerprint. Existing serialized compilation artifacts without
`physical_design_provenance` are not valid under the updated schema and must be
recompiled.

No legacy GDS, YAML PDK scaffold, simulator-specific netlist, or unversioned
plugin payload is implicitly migrated.

## Acceptance evidence

- The compiler round-trips the open gdsfactory/Circulax fixture without losing
  ports, declared units, topology connections, or model parameters.
- A 500-nanometer required port matches a 0.5-micrometer imported port.
- Topology mutation without a new digest is rejected.
- Missing or failed verification and unsupported evidence kinds are rejected.
- PDK identity/version and process-corner changes produce named invalidation,
  change the backend snapshot identity, and recompile.
- Proprietary URIs, parameters, component settings, and topology internals are
  rejected; identity-only public provenance has none of those fields.
- Unknown geometry fields are rejected by both Rust and JSON Schema.
- Typed circuit and electromagnetic adapter manifests validate; version skew
  fails.
- The reference backend manifests and capability documents conform to the
  updated cross-schema contract.
- The dependency-free Python capability parser performs the same closed-field,
  digest, topology, adapter/evidence, and proprietary-boundary validation and
  reproduces the Rust binding/topology fingerprints.
- Compiler and runtime conformance suites cover the boundary without importing
  external physical-design packages or proprietary data.
