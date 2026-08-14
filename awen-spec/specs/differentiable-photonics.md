# Differentiable photonics

## Contract

A gradient request names the input graph, differentiated parameters, strategy,
seed, sampling count, and loss. Providers advertise the parameter and operation
families they support. Selection is deterministic for a fixed request and
provider registry. Unsupported analytic requests either use the explicitly
selected finite-difference strategy or return a typed error.

Every result records parameter gradients, evaluated loss, strategy, seed,
sample count, provider identity, runtime version, source identity, and parent
artifact. Stochastic evaluation must use the supplied seed. A result without
this provenance is not replay evidence.

## Implemented reference

- The Rust runtime provides a gradient registry, MZI phase reference provider,
  deterministic finite differences, CLI surface, artifact output, and tests.
- Python NumPy, PyTorch, and JAX reference integrations expose analytic
  gradients for their supported matrix/linear regions and test them against
  framework references.
- AEP-0021 allows external circuit simulators to return immutable model/result
  evidence without importing their equations, optimizer state, or gradients.

## Exclusions

This version does not define universal adjoint rules, arbitrary quantum
parameter-shift rules, optimizer APIs, or a physical-hardware gradient claim.
Adding any of these requires an AEP, versioned schema, reference implementation,
error and noise semantics, and conformance tests.
