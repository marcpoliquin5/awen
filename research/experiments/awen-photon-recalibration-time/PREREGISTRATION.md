# AWEN-Photon recalibration time axis from a published phase random-walk rate (E-039): pre-registration

Registered 2026-11-09 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run**.
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-023 (#217, run in #219) reports tolerable drift budgets `s` per recalibration interval (rms random-walk offset of
each normalized programmed weight) with no time scale, because the assumption set had no sourced drift rate. What
recalibration intervals in seconds do those budgets correspond to, if a published Brownian phase rate is used as the
drift rate under a stated phase-to-weight rule?

## Source (checked 2026-11-09)

G. Elmas, I. A. Litvin, P. Kohl, J. Nötzel, "Modeling and analysis of phase instability in a photonic processor",
Applied Optics 64(14), 3995 (2025), doi:10.1364/AO.560370 (Crossref record checked: title, authors, volume, issue,
page, date). Accepted manuscript arXiv:2606.25196v1, read in full (HTML). Source key `elmas2025` in
`awen-spec/hardware_assumptions.yaml`, values in `parameters.drift.published_designs`.

| Point | Value | Where in the paper | What it is |
| --- | --- | --- | --- |
| `brownian_fit_to_phase_distribution` | sigma = 0.25 [sec]^-1/2 | Sec. 4 text and Fig. 3 caption | Brownian-model parameter whose simulated phase distribution matches 35 measured runs of 300 s (discrepancy about 9%) |
| `brownian_fit_to_feedback_trace` | sigma = 0.15 [sec]^-1/2 | Sec. 6 text and Fig. 6 caption | Brownian-model parameter matched to one feedback-control run (38 Hz correction, off for the first 60 s) |

Model in the paper (Sec. 2, Eq. 2): var(phi(dt)) = sigma^2 dt, phase in rad. Source label: peer-reviewed; each sigma is
the authors' model value matched to their measurements, not a directly reported statistic. The two points stay
separate (never averaged or interpolated).

## Stated limits of the source (why this is a proxy)

- The disturbance is the relative phase between two input fibers, driven by air-cooling-fan airflow, outside the
  chip. It is not drift of programmed mesh phases or weights, and not temperature drift of phase shifters.
- E-023 drifts programmed weights only; input-path drift is listed there as not modeled. Applying this rate to every
  programmed weight independently is the **registered proxy assumption** of E-039, not a property of any device.
- One lab setup; packaging and environment set the rate. No transfer to other devices is claimed.

## Rule (estimate with formula)

- Weight drift rate `D = k * sigma` in normalized weight units per sqrt(s); interval `T = (s / D)^2` seconds, the
  same formula as `awen_models.recalibration.interval_for_budget`.
- `k` (normalized weight units per rad), two registered first-order bounds, both reported, none preferred:
  - `phase_to_amplitude_max` k = 1.0: |d cos(phi) / d phi| <= 1 (largest change of a unit-amplitude interference
    term per rad; gives the shortest interval).
  - `mzi_internal_half_angle_max` k = 0.5: MZI internal phase theta to field amplitude cos(theta/2), |d/d theta| <= 1/2.
- For every E-023 regime (standard, noise-aware-100, noise-aware-1000), every non-zero grid budget and the stored
  tolerable budget, report `T` for both points and both `k`. Nothing else from E-023 is recomputed.

## Inputs (pinned)

| Input | Path | Pin |
| --- | --- | --- |
| E-023 result | `awen-models/results/photon-recalibration/results.json` | SHA-256 `12e7db5fa60f594bf4acb0748ea73066b4fc9e9b4b3ffd0f7065040704cc3401` |
| Published points | `awen-spec/hardware_assumptions.yaml` `drift.published_designs` | sources, bases and sigma values must equal the table above |

## Command (from `awen-models/`, CPU, no training)

    python -m awen_models.recalibration_time_axis --registration e039 \
        --recalibration results/photon-recalibration/results.json \
        --out results/photon-recalibration-time/results.json

Output stored with its SHA-256, commit and environment in the run PR.

## Verdict rule

**pass** when the E-023 input matches its pin, the published points match the table, every tolerable budget is copied
unchanged, and every interval equals the closed form within relative tolerance 1e-12 (`failures` empty); otherwise
**fail**. This is a consistency check on an estimate, not a hypothesis test.

## What does not follow

No hardware, recalibration-time, speed or energy claim. The intervals are an estimate (formula) on stored simulation
results (L1) under the proxy above. The compiler's `drift_rad_per_sqrt_s` default (0.0, inventoried in
`awen-spec/hardcoded_device_defaults.yaml`) is not changed by this registration.
