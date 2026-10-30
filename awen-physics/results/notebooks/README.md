# Validation notebooks: stored run record (P1-6)

Source label: **measured by us** (solver and simulator output on the build box, not hardware). Evidence levels as
declared in each notebook: (a) slab modes L0, (b) Clements mesh L0, (c) shot-noise ENOB L1.

Commit `f00120b` (branch `feat/p1-6-validation-notebooks`), 2026-10-30 18:26 ET. Build box: Linux 6.12 x86_64,
8 vCPU, no GPU, Python 3.12.15 venv from `requirements/cpu-py312.txt` (numpy 2.5.4) with `./awen-physics` and
`awen_native` 0.1.0 installed. From the repository root:

```
python scripts/run_validation_notebooks.py --require-native --out awen-physics/results/notebooks/run-f00120b.json
```

Output: `a_slab_waveguide_te_modes.ipynb: passed`, `b_clements_mzi_mesh.ipynb: passed`,
`c_shot_noise_enob.ipynb: passed`, exit 0. The record holds every cell's printed output and each notebook's SHA-256.

| File | SHA-256 |
| --- | --- |
| `run-f00120b.json` | `63270d6fa23d7299698acea83d2f2cf5353a9454cd7fceeffe3d689d7fb72fa7` |

Summary (deterministic seeds; no confidence interval):

| Notebook | Check | Tolerance | Result |
| --- | --- | --- | --- |
| (a) | finite-difference vs analytic TE n_eff at 400 pts/µm, 2 modes | < 1e-5 | 1.12e-06, 4.70e-06; convergence orders 2.0 (4 values) |
| (b) | Clements mesh reconstruction / unitarity, N = 2 to 32 | < 1e-12 | max 5.22e-16 / 8.88e-16 (N = 32) |
| (c) | simulated ENOB vs IEEE 1241 theory, 10 to 1e5 photons/MAC, no ADC and 8-bit ADC | < 0.1 bit | max difference 0.032 bit (10 photons/MAC, 8-bit ADC; from the printed 3-decimal values) |
| (c) | photons per extra bit while shot noise dominates | 3.70x to 4.37x (from the 0.1-bit step tolerance) | 3.999x, theory 10^(6.02/10) = 3.999x |

These validate the solvers' and the simulator's internal consistency against theory. They say nothing about a
fabricated device, and no energy, latency or speedup figure is produced. The phase-error fidelities in (b) and the
photon counts in (c) are sweeps, not device data.
