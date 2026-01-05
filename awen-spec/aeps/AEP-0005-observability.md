AEP-0005: Observability (v0.1)

Status: Draft

Summary:
Define tracing, metrics, structured logs, time-resolved traces, phase evolution viewers, and artifact formats for observability.

TODO: Define trace/span schema and recommended exporters.

This AEP defines the Observability & Profiling Model v0.1 for AWEN. It mandates:

- A `Tracer` API with `Span` creation, attributes, events, and duration measurement.
- A `MetricsSink` API supporting counters, gauges, and histograms with units.
- A `TimelineBuilder` that aggregates spans into a `timeline.json` compatible with a Nsight-like viewer.
- Export formats: `traces.jsonl` (newline-delimited spans), `metrics.json` (serialised metrics), `timeline.json` (lane-based timeline).
- Correlation IDs: every IR node, kernel, parameter, and artifact must surface a stable `correlation_id` field for deterministic linking.

Conformance requirements:

- Runtimes must provide a file exporter writing the above artifacts into the run artifact bundle.
- Integration tests must validate the presence and basic schema of these artifacts for example runs.

Next actionable items:

1. Create `awen-spec/specs/observability.md` defining JSON schemas for spans, metrics, and timeline.
2. Implement runtime interfaces under `awen-runtime/src/observability`.
3. Add CI integration to validate observability artifacts after `awenctl run` and `awenctl gradient`.

