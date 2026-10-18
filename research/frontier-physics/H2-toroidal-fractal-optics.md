# H2: Toroidal (anapole) and fractal geometries for compact photonic elements

Status: hypothesis. Separate from the AWEN core.

## Hypothesis
(a) Elements designed to sit near an anapole condition, where electric- and
toroidal-dipole far fields cancel, give higher near-field energy density per
unit radiated loss than a plain electric-dipole resonator of the same size,
which could shorten modulators or detectors. (b) Self-similar (fractal)
layouts give multiband responses with band ratios set by the scale factor,
which could serve several WDM channels with one element.

## Math
(a) Electric and toroidal dipoles share one far-field pattern, so their
fields add through an effective dipole p_eff = p + ikT (fixed convention in
`frontier_checks.effective_dipole`). The far field vanishes when
T = ip/k, while the near-field energy does not. Background:
Papasimakis et al. 2016 (doi:10.1038/nmat4563).
(b) A Sierpinski gasket has box-counting dimension log 3 / log 2 ≈ 1.585.
Multiband behaviour of the Sierpinski antenna, with bands spaced by the
self-similarity factor, was reported by Puente-Baliarda et al. 1998
(doi:10.1109/8.664115) at RF.

## Assumptions
Dipole-order multipole truncation; lossless dielectrics for (a). For (b), the
RF scaling result carries over only if fabrication resolves several levels
of self-similarity at optical scale, which is a strong assumption.

## Observables
(a) Scattering cross-section minimum at the anapole wavelength together with
near-field enhancement (simulated with a full-wave solver, then measured).
(b) Transmission or reflection peaks and their wavelength ratios against the
geometric scale factor.

## Method
Full-wave simulations (FDTD/FDFD in `awen-physics`) of a dielectric disk at
the anapole condition and of 2–4-level Sierpinski apertures; compare the
figures of merit with a matched-footprint conventional resonator.

## Comparison to established theory
Results must agree with the multipole decomposition of the computed currents
and with the cited RF fractal antenna results under scaling.

## Falsification
(a) fails if, at matched footprint and loss, the anapole element's figure of
merit is not higher than the conventional resonator's. (b) fails if the band
ratios do not follow the scale factor, or if fewer than two usable bands
appear at fabricable feature sizes.

## Runnable check
`frontier_checks.effective_dipole`, `anapole_toroidal_moment`,
`sierpinski_points`, `box_counting_dimension`.
