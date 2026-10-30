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
| `tmm` | Planar multilayers: characteristic (transfer) matrices and an independent Redheffer S-matrix composition; s/p polarization, oblique incidence, absorbing layers | analytic |
| `rings` | Microring all-pass and add-drop transmission (closed form), explicit round-trip field propagation, FSR, FWHM, loss conversion | analytic / numerical propagation |
| `fdtd1d` | 1-D Yee FDTD for normal-incidence layer stacks; pulse reflectance spectra | full-wave (1-D) |
| `bpm` | 2-D scalar paraxial Crank–Nicolson BPM; imaginary-distance fundamental mode | numerical propagation |
| `shg` | χ⁽²⁾ second-harmonic coupled-wave equations (numerical), undepleted sinc², depleted tanh² and first-order QPM closed forms | numerical propagation / analytic |
| `fdfd_adjoint` | 1-D frequency-domain Helmholtz FD with radiation boundaries; adjoint gradient of probe intensity w.r.t. every pixel permittivity | full-wave (1-D) |
| `enob` | Sine-test ENOB of the Rust photonic GEMM model (`awen_native`) vs theory | compact model |

Checks for `tmm` and `rings` (`tests/test_tmm_rings.py`): bare interfaces vs
Fresnel at three angles (|ΔR| < 1e-12), Brewster zero, quarter-wave mirrors vs
the closed form for 1, 4 and 10 pairs, half-wave absentee layer, S-matrix vs
transfer matrix on 40 random lossy stacks (|Δr| < 1e-12), Beer–Lambert for a
weak index-matched absorber, S-matrix stability where the transfer matrix
overflows; round-trip propagation vs the analytic ring (|ΔT| < 1e-11),
critical coupling, lossless add-drop power conservation, and FSR (rel. 1e-4)
and FWHM (rel. 1e-2) measured from a sampled dispersive spectrum vs formula.

Checks for `fdtd1d` and `bpm` (`tests/test_fdtd_bpm.py`): FDTD half-space vs
Fresnel; FDTD two-layer stack vs `tmm` over 0.8–1.4 µm at dx = 20, 10, 5 nm
with observed convergence order 1.8–2.2 (measured locally: 2.00, 2.00; max
|ΔR| 3.6e-4 at 5 nm); BPM Gaussian beam width vs `w(z)` (rel. < 1e-3 at
dz = 0.5 µm, monotone in dz); exact norm preservation (1e-12); imaginary-distance
n_eff vs the analytic slab (< 1e-5 at 40 pts/µm, order 1.8–2.2); a launched
mode keeps its shape over 500 µm and its phase matches the Crank–Nicolson
eigenvalue (< 1e-6 rad).

Checks for `shg` and `fdfd_adjoint` (`tests/test_shg_adjoint.py`): numerical
SHG vs the undepleted closed form at ~1e-6 conversion for Δk = 0 and three
mismatches (rel. 1e-5), harmonic peak at one coherence length, depleted
phase-matched growth vs tanh² (rel. 1e-9, > 90 % conversion) with power
conservation (1e-10), first-order QPM vs (2/π)² (rel. 1e-5); FDFD reflectance
vs `tmm` at h = 20, 10, 5 nm with observed order 1.8–2.2 (measured locally:
2.01, 2.00); reflection-free radiation boundaries; adjoint gradient vs
central finite differences at 8 pixels including both boundaries (rel. 1e-5);
a gradient step raises the objective as the first-order prediction says.

## Validation notebooks

Each notebook states its claim, its tolerance, its evidence level (L0 or L1), the
source of every printed number and its references. The asserts in the notebook
are the acceptance test. `tests/test_notebooks.py` runs every code cell in CI, and
the gate also runs `python scripts/run_validation_notebooks.py --require-native`,
which fails rather than skips notebook (c) when `awen_native` is missing and
writes a run record (commit, environment, printed output). The last stored record
is in `results/notebooks/`.

| Notebook | Check | Tolerance |
| --- | --- | --- |
| `notebooks/a_slab_waveguide_te_modes.ipynb` | Finite differences vs analytic TE dispersion | \|Δn_eff\| < 1e-5 at 400 pts/µm; convergence order 1.8–2.2 |
| `notebooks/b_clements_mzi_mesh.ipynb` | Mesh vs Clements et al., *Optica* 3, 1460 (2016) | N(N−1)/2 MZIs, depth N, reconstruction and unitarity error < 1e-12 for N ≤ 32 |
| `notebooks/c_shot_noise_enob.ipynb` | Simulated ENOB vs shot-noise and quantization theory; photons per extra bit | \|ΔENOB\| < 0.1 bit; 3.70x–4.37x photons per bit |

```bash
pip install "./awen-physics[test]" ./awen-native
python -m pytest awen-physics/tests
```
