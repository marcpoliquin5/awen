# AEP-0005: Observability

Status: Implemented file-artifact contract

## Decision

The runtime provides spans with explicit IDs and optional parents, attributes,
events and status; counters, gauges and histograms with units; structured
events; and lane-based timelines. The file exporter writes `traces.jsonl`,
`metrics.json`, `events.jsonl`, `timeline.json`, and metadata into run artifacts.

The normative model and conformance requirements are in
`../specs/observability.md`. Runtime implementation and integration tests are in
`../../awen-runtime/src/observability` and
`../../awen-runtime/tests/observability_integration.rs`.

AWEN does not expose a network OTLP exporter in this version. Export over an
external telemetry protocol requires a separate reviewed contract and tests;
the former panic-only public surface was removed.
