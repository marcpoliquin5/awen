# AEP-0008: Differentiable photonics

Status: Implemented experimental reference

## Decision

AWEN exposes analytic framework gradients for supported NumPy/PyTorch/JAX
reference operations and a runtime gradient-provider interface with deterministic
finite-difference fallback. Gradient requests record strategy, parameters,
seed, samples, loss and provenance; unsupported parameters fail explicitly.

The contract and limitations are in `../specs/differentiable-photonics.md`.
Implementation and tests are in `../../awen-runtime/src/gradients.rs`,
`../../awen-runtime/tests`, and the Python framework integration tests.

Circulax/JAX physical circuit differentiation remains external to AWEN through
the AEP-0021 adapter boundary. AWEN does not claim general adjoint support,
parameter-shift coverage for arbitrary quantum operations, or differentiable
physical hardware.
