Differentiable Photonics (spec v0.1)

Overview
--------
AWEN's differentiable photonics layer provides standard definitions and APIs so researchers and engineers can compute gradients of scalar cost functions w.r.t. kernel parameters, under realistic noise and measurement semantics.

Key Concepts
------------
- GradientEngine: runtime component implementing gradient computation strategies (adjoint, parameter-shift, finite-difference, score-function).
- NoiseModel: structured description of noise sources (shot_noise, thermal_noise, phase_noise, loss_variation) and parameters.
- GradientRequest: a serialized request that includes IR snapshot, target parameters, cost function spec (reference to kernel or telemetry-based cost), noise_model, seed, and execution mode (deterministic/experimental).
- GradientResult: structured output with gradients for each parameter, gradient covariance (when stochastic), provenance, and links to traces.

API primitives
--------------
The runtime must implement an interface equivalent to:

- compute_loss_and_gradients(ir_snapshot, params, cost_spec, noise_model, options) -> GradientResult
- compute_adjoint_gradient(kernel_instance, param_list, options) -> GradientResult

Gradient strategies
-------------------
- Adjoint (preferred where analytic adjoints exist): exact gradients with cost proportional to a few simulator runs.
- Parameter-shift (quantum-friendly): exact for certain gates; requires specific shift rules per kernel.
- Finite-difference with noise-propagation: robust fallback, must model noise in gradient variance.
- Score-function estimators: for nondifferentiable measurement channels.

Provenance & reproducibility
----------------------------
Gradient runs must be fully captured in artifact bundles: IR snapshot, noise_model, seed, optimizer state, and gradient traces. GradientResult must include a `confidence` field for stochastic estimators.

Extensibility
-------------
Backends declare capability flags in HAL (e.g., supports_adjoint, supports_parameter_shift, supports_stochastic_gradients). Plugin backends register gradient providers implementing `GradientProvider` trait.

Integration
-----------
Provide bindings and examples for:
- PyTorch connector (wrap compute_loss_and_gradients into autograd.Function)
- JAX / NumPyro adapters (for Bayesian optimization)

TODO
----
- Formalize `cost_spec` DSL.
- Provide parameter-shift tables for common AWEN kernels (MZI, ring, phase shifter).
- Implement reference PyTorch wrapper in awen-ecosystem.
