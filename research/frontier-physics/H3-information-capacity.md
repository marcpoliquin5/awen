# H3: Information capacity limits for optical interconnect and optical MACs

Status: hypothesis. Separate from the AWEN core.

## Hypothesis
For a given aperture pair and photon budget, the usable information rate of
an optical matrix-vector unit is bounded by (number of well-coupled spatial
modes) × (Shannon capacity per mode at the shot-noise-limited SNR), and the
AWEN noise contract's effective-bits figure should never exceed this bound.

## Math
Shannon 1948 (doi:10.1002/j.1538-7305.1948.tb01338.x): C = B log₂(1 + SNR).
For shot noise with mean photon count N per sample and a Gaussian
approximation, SNR ≈ N and the capacity is ½ log₂(1 + N) bits per real sample.
Miller 2000 (doi:10.1364/AO.39.001681): the number of well-coupled modes
between apertures is set by the singular values of the Green's function; in
the paraxial 1-D case it is about L_T L_R / (λ z).

## Assumptions
Scalar paraxial propagation; Gaussian approximation to Poisson statistics
(poor for N ≲ 10); no crosstalk beyond what the Green's function contains.

## Observables
Simulated: the singular-value spectrum of the coupling operator, mutual
information of the simulated photonic GEMM against its input distribution.
Measured (needs hardware): bit error rate versus photon budget.

## Method
`frontier_checks.coupling_singular_values` computes the spectrum; the
check compares the count above half-maximum with the paraxial estimate.
Next: compare the photonic simulator's effective bits at N photons/MAC with
½ log₂(1 + N).

## Comparison to established theory
The singular-value count must match Miller's estimate within about one mode
for the paraxial geometries tested (it does, see the tests).

## Falsification
The bound is wrong (or the simulator is) if any configuration of the AWEN
simulator reports more effective bits per MAC than ½ log₂(1 + N) at the
shot-noise limit, or if the singular-value count departs from the estimate
by more than discretisation error in the paraxial regime.

## Runnable check
`shannon_capacity_bits`, `shot_noise_bits_per_sample`,
`coupling_singular_values`, `well_coupled_modes`, `paraxial_mode_estimate`.
