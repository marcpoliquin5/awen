# Physical-design boundary

This document is normative for `awen.physical-design.v1`. AEP-0021 records the
architectural decision and ecosystem evaluation.

## Ownership

AWEN owns logical photonic operations, required interfaces, mapping constraints,
candidate topologies, capability negotiation, compilation, cache identity, and
identity-only provenance.

External tools own PDK contents, component construction, cross sections, layer
stacks, material models, geometry, routing, mask layout, GDS/OASIS, DRC, LVS,
electromagnetic models/solves, circuit equations/solves, calibration fitting,
and foundry workflows.

The boundary is intentionally asymmetric: AWEN exports constraints and imports
verified identities plus the minimum public logical metadata required for
compilation. It never imports a layout database.

## Contract documents

The JSON Schema `awen_physical_design.v1.json` defines three closed documents:

1. `mapping_request` exports logical operations, required ports, constraints,
   and candidate topologies.
2. `mapping_response` binds the request to a gdsfactory adapter and verified
   design.
3. `binding` is the verified physical-design capability imported by a backend.

The schema also publishes `$defs/adapter` for plugin manifests and
`$defs/provenance` for compilation artifacts. All objects set
`additionalProperties: false`. Rust equivalents use `deny_unknown_fields`.

## Required validation sequence

An implementation must perform the following steps in order:

1. Validate the request against the exact v1 schema and Rust/host semantics.
2. Execute the external adapter outside the compiler's semantic core.
3. Require the response schema version and request identity to match.
4. Require a recorded gdsfactory adapter and an exported candidate topology.
5. Resolve every topology endpoint and verify unique identifiers.
6. Canonicalize length comparisons to micrometers without rewriting the
   serialized units.
7. Check required ports and every exported mapping constraint.
8. Recompute the logical topology SHA-256 digest.
9. Validate immutable PDK, process-corner, component, model, adapter,
   verification-settings, and report identities.
10. Require passed connectivity evidence and adapter support for every imported
    evidence kind.
11. Apply the proprietary-data policy.
12. Compute the complete binding fingerprint and install the binding in the
    backend capability snapshot.

No warning or best-effort import may bypass a failed step.

## gdsfactory mapping

The adapter maps external values as follows:

| gdsfactory concept | AWEN contract | Rule |
| --- | --- | --- |
| active PDK and versioned manifest | `pdk` | immutable artifact identity and digest |
| process corner | `process_corner` | named identity, digest, temperature, public scalar parameters |
| registered cell library | `component_library` | immutable identity; no cell geometry |
| `Component.settings` | `TopologyNode.settings` | public finite numeric values only |
| `Component.ports` | node/external `PortContract` | preserve name, kind, center, orientation, width, unit, layer, wavelength, mode |
| component/netlist connectivity | `TopologyContract` | logical instances and named endpoints only |
| placement/routing objectives | `LayoutConstraints` | scalar limits and layer allowlist only |
| DRC/LVS/connectivity result | `VerificationEvidence` | passed status plus tool/settings/report identities |
| GDS/OASIS and polygons | no representation | remain external |
| layer map, rule deck, foundry source | no representation | remain external |

gdsfactory's micrometer convention must be declared as `micrometer`; an
adapter must not omit the unit simply because gdsfactory uses that convention.

## Circuit and electromagnetic models

`CircuitModelReference.framework` is closed to `circulax`, `sax`, `touchstone`,
or `analytic`. The record contains a model name, immutable external artifact,
named ports, wavelength band, and public finite numeric parameters. A Circulax
model requires a `circuit_simulator` adapter.

Circuit and electromagnetic results enter only as `VerificationEvidence` with
a closed kind, passed status, exact tool/version, SHA-256 settings fingerprint,
and immutable report reference. Raw traces, matrices, fields, meshes, solver
state, gradients, convergence history, and licensed model files stay external.

An adapter may use Circulax/JAX for differentiable calibration fitting or
hardware-aware topology optimization. It must export the optimized public
parameters in the binding and identify the exact model, settings, and result by
digest. Re-running with different solver settings produces a new settings
fingerprint and binding fingerprint.

## Invalidation

The complete `PhysicalDesignBinding` is serialized deterministically and
SHA-256 fingerprinted. `DeviceCapabilities::topology_fingerprint` incorporates
that fingerprint, and `BackendSnapshot` serialization incorporates the complete
binding. Both calibration compatibility and compilation cache identity are
therefore physical-design-sensitive.

`refresh_for_backend` compares the compilation provenance with the current
binding. It reports:

- `PDK identity or version changed` for PDK manifest/version changes;
- `physical-design process corner changed` for corner identity/digest changes;
  or
- `physical-design binding changed` for other component, topology, model,
  adapter, simulation, constraint, or verification changes.

Refresh never patches a stale artifact in place. It recompiles with automatic
placement and may fall back to a digital backend if the new complete contract
is not eligible.

## Public and proprietary records

Open fixtures may carry public logical topology and finite numeric parameters.
They still carry no geometry or foundry workflow data.

Proprietary bindings are reference-only. URIs, process parameters, component
settings, model parameters, topology nodes, and topology connections are
forbidden. Only an abstract external port interface and opaque immutable
artifact references may remain. Public compilation provenance strips URIs and
does not possess fields for any inline parameters or topology internals.

An adapter that cannot produce this redacted representation must keep the
binding and compilation artifact private; it may not weaken validation or add
unknown payload fields.

## Reference fixture

`awen-ecosystem/pdks/example_silicon_pdk.json` is public synthetic metadata. It
describes a two-port MZI logical cell, explicit micrometer units, a nominal
process corner, an external gdsfactory component library, a Circulax circuit
model, and passed connectivity evidence. It contains no polygons, GDS, foundry
rule deck, confidential model, or proprietary process value.

`awen-spec/fixtures/physical_design_mapping_request.v1.json` exports the same
candidate topology and demonstrates cross-unit port comparison with 500
nanometers versus 0.5 micrometers.
