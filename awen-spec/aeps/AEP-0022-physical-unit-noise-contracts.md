# AEP-0022: Physical-unit noise contracts

Status: Accepted and implemented (compact model; not validated against hardware)

## Motivation

AEP-0017 expresses analog error as dimensionless fractions of signal. Those
numbers cannot be derived from, or checked against, device datasheets. This
AEP adds a contract in physical units and a seeded noise model that turns the
contract into simulated GEMM error, so precision claims trace back to photons,
decibels, radians and bits.

## Contract

`awen.photonic-noise-contract.v1`
(`awen-spec/schemas/awen_photonic_noise_contract.v1.json`, Rust type
`awen_compiler::noise::PhotonicNoiseContract`):

| Field | Unit | Meaning |
| --- | --- | --- |
| `photons_per_mac` | photons | Mean photons per multiply-accumulate at full scale |
| `quantum_efficiency` | 1 | Detector quantum efficiency |
| `insertion_loss_db` | dB | Laser-to-detector optical loss |
| `phase_sigma_rad` | rad | Static phase-programming error (std) |
| `drift_rad_per_sqrt_s`, `seconds_since_calibration` | rad/√s, s | Phase random walk since calibration |
| `rin_db_per_hz`, `detection_bandwidth_hz` | dB/Hz, Hz | Laser relative intensity noise and its integration bandwidth |
| `thermal_noise_electrons` | e⁻ rms | Input-referred detector thermal noise per sample |
| `crosstalk_db` | dB | Adjacent output-channel power coupling |
| `dac_bits`, `adc_bits` | bits | Signed converter resolution |

Null disables an optional term. The default contract in the fixture is an
illustrative operating point chosen for testing; it is not a measured device.

## Model (fidelity: compact model)

For a contraction tile of length `k` with operands scaled to `[-1, 1]`:

- Full-scale photoelectrons: `N_fs = photons_per_mac · k · 10^(−IL/10) · η`.
- Shot noise (Poisson, balanced detection, signal independent):
  `σ_shot = 1/√N_fs` of full scale.
- Thermal noise: `σ_th = n_th / N_fs`.
- RIN: `σ_RIN = |z| · √(10^(RIN/10) · B)`.
- Phase error on an amplitude-encoded weight `w = cos θ`:
  `δw = √(1 − w²) · δθ`, `δθ ~ N(0, σ_φ² + D² t)`.
- Crosstalk: `z_o ← z_o + 10^(XT/10) · (z_{o−1} + z_{o+1})`.
- Converters: signed uniform quantizers on `[-1, 1]`, step `1/(2^(b−1) − 1)`,
  noise `step/√12`.
- ENOB for a full-scale sine (IEEE Std 1241): `ENOB = (SINAD_dB − 1.76)/6.02`
  with `SINAD = (1/2)/σ²`. The shot-limited case gives
  `ENOB = (10 log10(N_fs/2) − 1.76)/6.02`.

Tiles are digitized independently and accumulated digitally, matching the
existing compiler tiling.

## Validation

`awen-compiler/src/noise.rs` tests each term by Monte Carlo against its closed
form (shot, thermal and RIN within 3%; phase error within 5%; converter error
bounded by half a step and within 3% of `step/√12`; crosstalk exact), checks
the ENOB inversion exactly, determinism per seed, schema/fixture agreement, and
rejection of out-of-range contracts.

## Limits

Compact model only. It ignores mesh-level error propagation in multi-stage
interferometers (covered separately by the Clements mesh validation),
nonlinear detector response, coherent crosstalk phase, temperature dynamics,
and correlated noise between outputs. Parameter values must come from measured
devices before any result is called experimentally validated.

## References

- IEEE Std 1241-2010, *IEEE Standard for Terminology and Test Methods for
  Analog-to-Digital Converters*, doi:10.1109/IEEESTD.2011.5692956.
- B. E. A. Saleh and M. C. Teich, *Fundamentals of Photonics*, 3rd ed., Wiley,
  2019 (photon statistics and photodetector noise).
