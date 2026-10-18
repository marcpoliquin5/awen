# awen-physics

Reference physics models that check AWEN's compact models against analytic
and independent numerical results. Every result carries a fidelity label:
analytic, compact model, numerical propagation, full-wave, digital twin, or
measured. Nothing in this package is a hardware measurement.

This package does not replace a physical-design tool or a full-wave solver
(gdsfactory, Meep and others; see AEP-0021 and AEP-0023). It exists so that
every number the AWEN simulator produces can be traced to a validated model.

| Module | Contents | Fidelity |
| --- | --- | --- |
| `slab` | Symmetric slab TE modes: analytic dispersion relation, and an independent 1-D finite-difference eigen-solver | analytic / full-wave (1-D) |
| `clements` | Clements (2016) rectangular MZI-mesh decomposition, reconstruction, phase-error sensitivity | analytic |
| `enob` | Sine-test ENOB of the Rust photonic GEMM model (`awen_native`) vs theory | compact model |

## Validation notebooks

Each notebook states its claim, its tolerance and its references. The asserts
in the notebook are the acceptance test. `tests/test_notebooks.py` runs every
code cell in CI.

| Notebook | Check | Tolerance |
| --- | --- | --- |
| `notebooks/a_slab_waveguide_te_modes.ipynb` | Finite differences vs analytic TE dispersion | \|Δn_eff\| < 1e-5 at 400 pts/µm; convergence order 1.8–2.2 |
| `notebooks/b_clements_mzi_mesh.ipynb` | Mesh vs Clements et al., *Optica* 3, 1460 (2016) | N(N−1)/2 MZIs, depth N, reconstruction and unitarity error < 1e-12 for N ≤ 32 |
| `notebooks/c_shot_noise_enob.ipynb` | Simulated ENOB vs shot-noise and quantization theory | \|ΔENOB\| < 0.1 bit |

```bash
pip install "./awen-physics[test]" ./awen-native
python -m pytest awen-physics/tests
```
