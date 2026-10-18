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
