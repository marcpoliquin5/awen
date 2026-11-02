# E-022: AWEN-Photon sim-to-real noise control (measured by us, L1)

Registered run of `research/experiments/awen-photon-sim2real/PREREGISTRATION.md` (#213, `2f1cf90`), run unchanged.

Command (from `awen-models/`, at commit `0fe7495`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-02 00:27 ET, exit 0, 2 min 9 s wall:

    PYTHONPATH=. python -m awen_models.sim2real --seeds 1 2 3 --out results/photon-sim2real

Output: `results.json` (SHA-256 in `SHA256SUMS`). Printed summary (mean paired difference, verdict, per model-B point
in the order ENOB 3, 4, 5, 6, 7.61 primary, 10):

    noise-aware-100 - standard [(0.3355, 'transfers'), (0.5705, 'transfers'), (0.4548, 'transfers'), (0.0904, 'transfers'), (-0.0904, 'reverses'), (-0.0998, 'reverses')]
    noise-aware-1000 - standard [(0.0262, 'transfers'), (0.1114, 'transfers'), (0.2746, 'transfers'), (0.1334, 'transfers'), (-0.0081, 'reverses'), (-0.0143, 'reverses')]

## Verdict at the registered primary point (model B, all three sourced values)

| Paired difference (3 seeds) | Mean | 95% t interval | Verdict |
| --- | --- | --- | --- |
| noise-aware-100 minus standard | -0.0904 | [-0.1003, -0.0804] | **reverses** |
| noise-aware-1000 minus standard | -0.0081 | [-0.0100, -0.0062] | **reverses** |

Under model B at its sourced operating point (system ENOB 7.61, 8-bit weights, 30 dB extinction ratio), both
noise-aware regimes score below the standard network. The E-005 noise-aware advantage, measured under model A, does
not transfer to this independent model at this point. This is a negative result and is recorded as one.

## Mean accuracy over seeds 1-3 (10,000 MNIST test images; descriptive)

| Regime | Model A | B, ENOB 3 | B, 4 | B, 5 | B, 6 | B, 7.61 (primary) | B, 10 |
| --- | --- | --- | --- | --- | --- | --- | --- |
| standard | 0.8201 | 0.1155 | 0.1717 | 0.3814 | 0.7706 | 0.9601 | 0.9702 |
| noise-aware-100 | 0.8595 | 0.4510 | 0.7422 | 0.8362 | 0.8611 | 0.8697 | 0.8704 |
| noise-aware-1000 | 0.9112 | 0.1416 | 0.2831 | 0.6560 | 0.9041 | 0.9520 | 0.9559 |

Secondary points (exploratory, ENOB is a free parameter there): at ENOB 3, 4, 5 and 6 both noise-aware regimes
**transfer** (95% intervals above 0, see `verdicts_model_b` in `results.json`); at ENOB 10 both **reverse**. So in this
study the noise-aware advantage appears only when model B's noise is harsh, and costs accuracy when it is mild.
Model B's primary point is milder than model A (its output rms is 0.00296 of the half-range; model A's analytic tile
effective bits at 1000 photons per MAC is 5.54, E-005), as the preregistration anticipated.

Limits: one small MLP on MNIST, 3 epochs, CPU, simulated noise only (L1). Model B omits phase error, crosstalk and
drift (no checked source). No energy, speed or hardware claim follows from these numbers.
