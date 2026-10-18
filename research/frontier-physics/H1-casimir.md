# H1: Casimir forces as a design constraint and actuator in nanophotonic MEMS

Status: hypothesis (not tested against measured devices). Separate from the
AWEN core; nothing here feeds product claims. **This is not a free-energy
proposal**: the ideal Casimir force is conservative and any closed cycle does
zero net work (checked in `frontier_checks.cycle_work_per_area`).

## Hypothesis
In MEMS/NEMS photonic phase shifters and couplers with sub-micron gaps, the
Casimir pressure is large enough to set pull-in and stiction limits, and can
be included in the device model as a gap-dependent force that the AWEN
physics package predicts within the accuracy of the Lifshitz theory for the
materials used.

## Math
Ideal perfect conductors at separation a (Casimir 1948):
P(a) = π²ħc / (240 a⁴), E(a)/A = −π²ħc / (720 a³); the force per area −d(E/A)/da = −P is attractive.
At a = 1 µm, P = 1.30 mPa (Casimir quotes 0.013 dyn/cm² / a_µm⁴, the same
value). Real metals and dielectrics need the Lifshitz formula with measured
permittivities; the ideal value is an upper bound at these gaps.

## Assumptions
Ideal, flat, parallel conductors; zero temperature; gap much larger than the
plasma wavelength for the ideal limit. Each assumption must be relaxed
(finite conductivity, roughness, temperature, patch potentials) before a
comparison with data.

## Observables
Pull-in gap of a cantilever or membrane versus spring constant; static
deflection versus gap; resonance frequency shift versus gap.

## Method
1. Add the ideal pressure (done here) and a Lifshitz-corrected pressure with
   tabulated permittivities to an electromechanical phase-shifter model.
2. Predict the frequency shift df(a) of a resonator held at gap a.
3. Compare with published measurements, then with lab data if available.

## Comparison to established theory
Lamoreaux 1997 (doi:10.1103/PhysRevLett.78.5) measured the force in the
0.6–6 µm range in agreement with theory at the level reported there. Any
AWEN model must reproduce such measurements before being used for design.

## Falsification
The hypothesis (that the model is good enough to set design rules) fails if
the predicted frequency shift or pull-in gap differs from measurement by more
than the combined experimental and permittivity-data uncertainty, after
electrostatic patch effects are removed.

## Runnable check
`frontier_checks.casimir_pressure`, `casimir_energy_per_area`,
`cycle_work_per_area`; tests in `research/tests/test_frontier.py`.
