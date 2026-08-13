# Hardware benchmark and evidence contract

AWEN full-system benchmarks are declared by `awen.hil-suite.v1`, executed by
`awenctl benchmark-suite`, and serialized as `awen.hil-artifact.v1`. Physical
drivers implement `awen.hil-driver.v1`. Public prose is generated only through
`awenctl benchmark-claims` and `awen.benchmark-claims.v1`.

## Run the portable reference suite

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  benchmark-suite benchmarks/reference_hil_suite.json \
  --output-dir awen_hil_artifacts \
  --commit-sha "$(git rev-parse HEAD)" \
  --runner-id local-reference
```

The output directory must be absent or empty. It receives `suite.json`, a
content-addressed `benchmark-<sha256>.json`, and `SHA256SUMS`. The portable
reference suite is measured software timing plus estimated power/energy and
simulated photonic execution. It is not hardware performance evidence.
The aggregate artifact embeds and fingerprints the complete suite, so it is
self-contained even when copied independently of `suite.json`.

## External driver contract

An external backend is configured as:

```json
{
  "id": "lab-rig-01",
  "class": "lab_rig",
  "runner": {
    "runner": "external_command",
    "executable": "/opt/awen/bin/lab-driver",
    "args": ["--device", "rig-01"],
    "timeout_seconds": 3600
  },
  "regression": {
    "enforcement": "advisory_hardware",
    "reference_artifact": "sha256:...",
    "max_p95_latency_ns": 1000000.0,
    "max_p95_energy_j": 0.1,
    "min_throughput_gops": 1.0,
    "max_p99_absolute_error": 0.05,
    "max_p99_relative_error": 0.05
  }
}
```

The driver reads one request JSON object from standard input, writes one
response JSON object to standard output, and writes diagnostics to standard
error. It must execute the supplied warmup and repetitions against the supplied
literal fixture and seed. It must return one complete raw sample and output
sample for each repetition. The orchestrator calculates accuracy; a driver may
not replace output data with a self-reported verdict.

For each response, report actual hardware/software versions, topology, clocks,
temperature, commit, runner ID, calibration, raw instrument counters, and
separate evidence sources. If a sensor or field truly is unavailable, name it
in `unavailable_fields`.

## Measurement boundary

Application latency includes host transfer, memory, scheduling,
reconfiguration, calibration amortization, DAC, modulation, optical-device
execution, detection, ADC, digital post-processing, and support overhead when
material. Optical propagation or optical-device time alone is never application
latency.

Energy includes transfers, memory, scheduling, reconfiguration, calibration,
lasers, modulation, DACs, optical devices, detectors, ADCs, digital
post-processing, and cooling/support power. Every component breakdown must sum
to its raw total.

## Physical workflow

Use the `Manual physical hardware benchmark` GitHub Actions workflow with a
reviewed repository manifest and a controlled self-hosted runner labeled
`awen-hardware`. The workflow records the exact GitHub SHA and stable runner ID
and uploads the immutable artifact set. It is intentionally not a required pull
request check.

## Claims

After publishing a verified physical artifact at an immutable HTTPS URL whose
final path segment contains its SHA-256 digest and which has no query string or
fragment:

```bash
awenctl benchmark-claims benchmark-<sha256>.json \
  --artifact-url https://benchmarks.example/benchmark-<sha256>.json \
  --baseline cpu-baseline \
  --candidate hardware-accelerator \
  --output claims.json \
  --markdown-output claims.md
```

The command refuses simulated, estimated, vendor-specified, mutable,
uncalibrated, inaccurate, or non-accelerating evidence. Generated Markdown
links directly to the immutable artifact.

## Schemas

- `awen_hil_suite.v1.json`
- `awen_hil_driver.v1.json`
- `awen_hil_artifact.v1.json`
- `awen_benchmark_claims.v1.json`
- `awen_blas.v1.json`

See AEP-0019 for the full rationale, validation rules, automation separation,
security boundary, and limitations.
