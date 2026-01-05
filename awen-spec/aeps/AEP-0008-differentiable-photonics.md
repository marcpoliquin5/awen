AEP-0008: Differentiable Photonics (v0.1)

Status: Draft

Summary:
This AEP mandates first-class support in AWEN for differentiable photonics: adjoint/forward sensitivity methods, gradient APIs, noise-aware gradients, and interfaces for hybrid classical-quantum optimization.

Motivation:
Differentiable infrastructure is required for parameter estimation, calibration by optimization, inverse design, end-to-end training of photonic networks, and integration with ML toolchains. AWEN must provide both algorithmic primitives and runtime hooks.

Specification (high-level):
- Gradient Engine: Runtimes must expose a `GradientEngine` trait with methods for `compute_gradient` (forward-mode, adjoint-mode) and `compute_loss_and_gradients`.
- Adjoint contract: Kernels should expose adjoint operations or be mappable to a differentiable primitive. Backends may implement analytical adjoint or numerical approximations.
- Noise-aware gradients: Gradient APIs must accept a `NoiseModel` and `seed` for stochastic gradient estimation; runtimes provide common estimators (score-function, parameter-shift, finite-diff with noise propagation).
- Hybrid gradients: Support composite gradients that span classical control parameters and quantum parameters (CV and DV). Define clear semantics when measurement collapse occurs.
- Differentiable calibration: Calibration kernels may emit differentiable cost functions. Optimizers (SGD, Adam, LBFGS) are pluggable.
- API stability: Define the JSON schema for gradient requests and artifact outputs (gradients.json) for reproducibility and provenance.

Backwards compatibility:
- Runtimes that cannot compute gradients must return a standardized error including recommended fallback (e.g., numerical finite-difference wrapper) and advertise capability flags in HAL.

Test plan:
- Reference adjoint engine in `awen-runtime` must provide a mock gradient that is deterministic and testable.
- Conformance tests include gradient checking (finite-diff vs adjoint) under various noise models.

TODO:
- Define `NoiseModel` JSON schema and gradient artifact format.
- Define parameter-shift rules for common photonic kernels.
- Provide Python bindings for optimizer integration (e.g., PyTorch/Autograd hooks).
