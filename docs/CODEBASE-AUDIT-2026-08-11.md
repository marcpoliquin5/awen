# AWEN codebase audit — 2026-08-11, reconciled 2026-08-13

This audit records repository facts, not product or performance claims. The
reconciliation was performed against the protected `main` branch and issue #18.

## Repository boundaries

- `awen-compiler` is an executable experimental Rust compiler and reference
  kernel library.
- `awen-mlir` implements the narrow registered StableHLO GEMM lowering path.
- `awen-runtime` contains the reference runtime, HAL, scheduler, simulator,
  artifact, benchmark, plugin, classical-photonic, and quantum-photonic paths.
- `awen-spec` contains normative schemas, specifications, and AEP history.
- `awen-ecosystem` contains reference Python integrations, physical-design
  metadata, reference marketplace metadata, and plugin guidance.
- Nonfunctional Cloud, Studio, and datacom directories were removed. Their
  prior contents remain available only in Git history and never represented a
  shipping product.

## Evidence boundaries

The checked-in device capabilities, cost models, PDK binding, and simulator
outputs are synthetic, assumed, or estimated reference data. No public physical
accelerator benchmark artifact is present. AWEN therefore makes no measured
hardware speed, energy, availability, customer, or pricing claim.

The HIL claim generator accepts only immutable end-to-end evidence containing
host transfer, memory, scheduling, reconfiguration, laser, DAC/ADC, calibration,
digital post-processing, support power, accuracy, environment, and exact commit
identity. Simulated or estimated evidence is rejected for public claims.

## Hygiene resolution

- The previously tracked Rust library artifact was removed and binary/run
  patterns are ignored and policy-checked.
- Root, runtime, and spec carry the complete MIT text; Rust and Python package
  metadata declare MIT consistently.
- Obsolete phase sign-offs, session logs, and delivery manifests were removed
  because they were not immutable CI or release evidence.
- One push/pull-request workflow exposes the stable required context
  `AWEN required quality gate`; the physical hardware workflow is manual.
- Repository policy, schemas, compilers, runtimes, MLIR, Python tests,
  dependency vulnerabilities, licenses, and secrets are checked in that gate.
- Root governance, maintainer, contribution, conduct, security, issue/PR
  templates, dependency updates, implementation status, and release criteria
  are explicit.

## Release truth

There is no supported AWEN product release. The January 2026 phase tag is a
historical development snapshot and is not performance or release evidence.
Future releases must follow `docs/RELEASING.md` and bind notes and checksums to
an exact green commit and quality-gate run.

## Linked repositories

The public laboratory application and legacy website are separate repositories.
Their security/configuration and public-claim remediations are tracked in
`marcpoliquin5/awen-labs#1` and
`marcpoliquin5/awenphotonics.github.io#1`; epic #5 is not complete until both
default branches and issues are reconciled.
