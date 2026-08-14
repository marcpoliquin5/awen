# AEP-0003: Kernel semantics

Status: Superseded by AEP-0015

## Historical intent

This proposal established the requirement for a photonic-kernel interface,
composition rules, parameter binding, scheduling semantics, and calibration
hooks. It did not define concrete API signatures, numerical conventions,
compatibility rules, or executable references.

## Resolution

AEP-0015 supplies the versioned `awen.blas.v1` request, backend, result, and
plan contracts; the `awen.blas-benchmark.v1` evidence contract; exact semantics
for the initial 22-kernel registry; CPU references; deterministic simulator;
capability and cost selection; fallback behavior; compatibility policy; CLI;
and conformance requirements. Implementations must use AEP-0015 rather than
this superseded proposal.
