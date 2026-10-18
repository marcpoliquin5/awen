# AEP-0023: Reference physics for validation

Status: Accepted and implemented (slab TE modes, Clements mesh, ENOB)

## Decision

`awen-physics` is a Python package of reference models whose only job is to
validate AWEN's compact models: the photonic noise model (AEP-0022), mesh
programming, and later device models. It does not change the AEP-0021
boundary. Physical design, PDKs, layout and production electromagnetic
simulation stay with external tools such as gdsfactory and Meep, reached
through adapters.

## Rules

1. Every result carries one fidelity label: `analytic`, `compact_model`,
   `numerical_propagation`, `full_wave`, `digital_twin` or `measured`. The
   Python enum matches `awen_compiler::noise::Fidelity`.
2. Each model is checked against an independent method (an analytic result,
   or a second solver that shares no code with the first) at a tolerance
   stated in advance.
3. Each check is a notebook whose code cells assert that tolerance. CI runs
   every code cell.
4. A result is labeled `measured` only when it comes from a physical
   instrument.
5. Quantum-photonic models stay in their own modules and do not share types
   with classical models.

## Implemented checks

| Check | Independent reference | Tolerance |
| --- | --- | --- |
| Slab TE effective indices | Analytic dispersion relation vs 1-D finite differences | 1e-5, second-order convergence |
| MZI mesh | Clements et al., Optica 3, 1460 (2016) | Reconstruction < 1e-12, N(N−1)/2 units, depth N |
| Photonic GEMM ENOB | Shot-noise and quantization theory, IEEE 1241 ENOB | 0.1 bit |
