# Plugin contracts

AWEN plugin discovery uses the closed `awen.plugin-manifest.v1` schema and the
matching runtime `PluginManifest` type. Unknown fields, unsupported manifest
versions, empty identities, invalid signatures, and invalid typed subcontracts
fail closed.

## Manifest envelope

Every manifest declares:

- `manifest_version`, exactly `awen.plugin-manifest.v1`;
- non-empty plugin `id` and `version`;
- a unique list of non-empty capability names;
- optional Base64 Ed25519 `signature` and `public_key`;
- an optional plugin executable/adapter `path`;
- an optional typed backend contract; and
- optional typed physical-design adapter contracts.

The signature covers canonical Rust/JSON serialization of the entire manifest
after setting `signature` and `public_key` to null. Production discovery admits
only a successfully verified signature. The explicit
`discover_from_dir_allow_unverified(..., true)` path is restricted to tests and
developer workflows.

## Backend plugins

`backend` contains an `awen.device-capability.v1` document and a typed health
query. The v1 health query is a file path relative to the manifest directory.
Absolute paths, parent traversal, and canonical paths escaping the manifest
directory are rejected. Health is read again for each query and must validate
against the static capability's backend, calibration, topology, precision, and
physical-design identities.

The backend capability includes a verified `awen.physical-design.v1` binding.
PDK/process-corner changes consequently affect backend discovery, topology
compatibility, compilation, and cache reuse without compiler source changes.

## Physical-design adapters

`physical_design_adapters` is an array of closed adapter records from
`awen_physical_design.v1.json#/$defs/adapter`. The kinds are:

- `gdsfactory`: maps exported logical constraints/candidates to verified PDK,
  component, port, topology, layout-constraint, and verification identities;
- `circuit_simulator`: evaluates referenced netlists/models, including optional
  Circulax/JAX differentiable fitting and optimization; and
- `electromagnetic_simulator`: evaluates externally owned EM models and reports
  immutable evidence.

Each record declares a non-empty tool name/version, exact v1 request and
response versions, and unique supported evidence kinds. A manifest may declare
at most one adapter of each kind. Version skew or duplicate kinds fail runtime
validation.

The manifest does not embed a solver-specific command protocol, PDK contents,
layout geometry, rule decks, model source, raw results, credentials, license
configuration, or proprietary data. The signed plugin owns those concerns and
must translate them to the same closed AWEN request/response/evidence contract.

## Capability lookup and execution

String capability lookup remains an envelope-level discovery aid. It does not
replace typed validation. A caller must validate the typed backend or physical-
design adapter contract before invoking a plugin.

The runtime owns signature policy, path containment, health re-query, adapter
process/service isolation, timeouts, and transport diagnostics. The compiler
owns logical contract validation and never imports plugin implementation code.
External PDK, layout, DRC/LVS, EM, and circuit tools remain authoritative for
their domain outputs.

See AEP-0021 and `physical-design-boundary.md` for the required mapping,
identity, verification, invalidation, and proprietary-data rules.
