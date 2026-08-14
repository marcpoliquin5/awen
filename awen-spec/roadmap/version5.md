# AWEN version 5 roadmap

Owner: [@marcpoliquin5](https://github.com/marcpoliquin5)

Status: experimental development; no supported v5 release is scheduled.

## Completed development gates

The compiler/runtime vertical slice, MLIR GEMM path, typed dialects, capabilities,
partitioning, cost/precision/calibration contracts, framework boundaries, HIL
protocol, and physical-design evidence boundary are implemented and tested on
`main`. Completion means their declared conformance tests pass; it does not mean
hardware performance, product availability, or general operator coverage.

## Next release gate

A v5 release may be proposed only after all release criteria in
`../../docs/RELEASING.md` pass on the exact tagged commit. The maintainer owns
that decision. There is no date commitment.

Required criteria are one protected green quality context, clean-clone build
and tests, schema/ABI compatibility notes, dependency/license/secret scans,
complete security and governance files, immutable artifact checksums, and no
public quantitative claim without verified end-to-end evidence.

Cloud hosting, a desktop Studio, marketplace commerce, and physical hardware
acceleration are outside the current release surface. Future work in those
areas requires an owned issue with measurable acceptance criteria before code
or product claims are added.
