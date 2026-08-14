# Reproducibility Artifacts

## Scope

The runtime's `storage` module provides typed, local artifact bundles. It does not
ship a cloud registry, object-store client, or an `awenctl artifact` command.

An `ArtifactBundle` contains its manifest, source IR, optional lowered IR,
parameters, calibration state, results, environment snapshot, provenance,
observability data, lineage, tags, notes, and citation metadata. A deterministic
artifact identifier is derived from canonical inputs.

## Supported operations

The Rust API supports:

- building a bundle with `BundleBuilder`;
- validating bundle identity and content with `validate_bundle`;
- saving a bundle to a local artifact directory with `save_artifact`;
- exporting to a directory or gzip-compressed tar archive with `export_bundle`;
- importing either supported representation with `import_bundle`; and
- loading replay inputs with `load_artifact_for_replay`.

Callers that upload an exported archive are responsible for transport,
authentication, retention, and access control. Remote storage is not part of the
runtime's conformance boundary.

## Integrity and replay

The manifest indexes bundle content and records input and output hashes. Import
and validation reject malformed or inconsistent bundle data. Deterministic IDs
are invariant to parameter-map ordering and vary when replay-relevant inputs
change.

The implementation does not claim that a bundle proves who authorized a run or
that local metadata supplies an external compliance audit trail. Those properties
must be supplied by the system that stores and controls the exported artifact.

## Conformance evidence

Unit tests under `awen-runtime/src/storage/` and the integration suite at
`awen-runtime/tests/reproducibility_integration.rs` cover deterministic identity,
bundle construction, directory and archive round trips, validation, citations,
lineage, observability data, and checksum rejection. The repository's required
quality gate executes all of them.

## Change control

Additional transports, registries, compliance guarantees, or scale claims require
a GitHub issue with an owner and measurable acceptance criteria. They become part
of this specification only when implementation and direct test evidence land in
the same pull request.
