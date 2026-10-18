# awen-qphotonic

Quantum-photonic reference simulators for AWEN. This is the quantum domain
from AEP-0020 and is kept separate from the classical photonic noise model:
it has its own package, its own tests, and no shared numerical semantics.

Everything here is **simulation**. Nothing is a hardware measurement, and
nothing here shows or claims a quantum advantage.

| Module | What it computes | Checked against |
| --- | --- | --- |
| `permanent` | Matrix permanent by Ryser's formula (Gray code, O(2^n n)) | Sum over all n! permutations, n ≤ 6 |
| `fock` | Fock-space transition amplitudes `<t|U|s>` for passive interferometers, exact output distributions, distinguishable-photon distributions, small-n boson sampling, HOM dip with partial distinguishability | Explicit creation-operator expansion; closed-form HOM coincidence `R² + T² − 2RTξ`; single-photon trajectory enumeration |
| `gaussian` | Covariance formalism (ħ = 2, xxpp ordering): squeezers, two-mode squeezer, passive interferometers, displacement, pure loss, thermal and coherent states, purity, symplectic eigenvalues, vacuum probability, homodyne sampling and conditioning | Closed forms (e^{∓2r}, sinh²r, 1/cosh r, EPR variance 2e^{−2r}, conditional variance 1/cosh 2r); squeeze operator exponentiated in a 60-level Fock basis |
| `detection` | Photon-number-resolving detection with efficiency and uniform loss (binomial thinning), threshold detectors with dark counts, post-selection success probability | 2m-mode unitary with environment modes traced out |
| `resources` | Resource estimates: per-photon efficiency, n-fold success probability, dark-count-free probability, expected triggers and wall time, outcome count, shots for a target relative error, samples for a total-variation target (Bretagnolle–Huber–Carol), Ryser operation counts | Lossy-interferometer oracle; Monte Carlo success rates, estimator variance and an empirical check of the TVD bound |
| `kernel` | Photonic quantum-kernel experiment (Fock feature map, exact and shot-noise kernels) with RBF and linear baselines on the same splits | Kernel PSD, unit diagonal, inversion-test probability; coherent-state kernel equals RBF exactly |

Limits: exact enumeration grows as C(n+m−1, n) outcomes and 2^n n per
amplitude, so these simulators are for small n (tests use n ≤ 6). Loss is
modelled as uniform per mode; mode-dependent loss and partial
distinguishability beyond two photons are not modelled yet.

```bash
pip install -e "./awen-qphotonic[test]"
python -m pytest awen-qphotonic/tests -q
```

```python
import numpy as np
from awen_qphotonic import fock
u = fock.haar_unitary(5, np.random.default_rng(0))
dist = fock.output_distribution(u, (1, 1, 1, 0, 0))   # exact, 35 outcomes
print(fock.hom_coincidence_probability(0.5, 1.0))      # 0.0 (ideal HOM dip)
```

## Quantum kernel vs classical RBF (simulation)

`python -m awen_qphotonic.kernel --out results/kernel_vs_rbf.json` reproduces
[`results/kernel_vs_rbf.json`](results/kernel_vs_rbf.json). Setup: two
features, 4 modes, input |1,1,0,0⟩, two phase layers between seeded
Haar-random mixers; kernel ridge classifier; 120 training and 200 test
points; kernel hyperparameters and regularization chosen by 4-fold CV on the
training set only; shot-noise variant uses 1,000 post-selected inversion-test
shots per kernel entry. Test accuracy, mean ± sample std over seeds 0, 1, 2:

| Dataset (noise 0.15) | RBF | Photonic, exact kernel | Photonic, 1,000 shots | Linear |
| --- | --- | --- | --- | --- |
| moons | 0.993 ± 0.003 | 0.832 ± 0.035 | 0.828 ± 0.013 | 0.865 ± 0.009 |
| circles | 0.927 ± 0.010 | 0.718 ± 0.042 | 0.717 ± 0.040 | 0.500 ± 0.026 |

On these toy datasets the photonic kernel is clearly worse than the RBF
baseline; it beats a linear kernel only on circles. This is a simulation
used to compare kernels on equal terms, not a hardware result, and it shows
no quantum advantage. A related exact fact is tested: a coherent-state
feature map |α = c·x⟩ gives exactly the RBF kernel with γ = c², so Gaussian
coherent-state encodings add nothing beyond a classical RBF kernel.

## Resource estimate example

`resources.estimate(ExperimentSpec(photons=4, modes=8, source_efficiency=0.8,
circuit_transmission=0.9, detector_efficiency=0.9, trigger_rate_hz=1e6,
target_samples=1000))` gives per-photon efficiency 0.648, four-fold success
probability 0.176, about 5,672 triggers (5.7 ms at 1 MHz) for 1,000 samples,
330 output patterns, and about 46,347 samples (262,857 triggers) for total
variation ≤ 0.05 at 95% confidence. These are formula outputs for the
parameters given, not measurements of any device.
