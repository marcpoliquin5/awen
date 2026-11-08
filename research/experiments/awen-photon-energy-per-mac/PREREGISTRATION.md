# AWEN-Photon system energy per lowered MAC (E-037): pre-registration

Registered 2026-11-08 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

Master Prompt v2 section 7 asks for an energy-per-MAC curve. E-036 (#270) stored `scenario_envelope_v1`
energy-per-token bounds for two transmitter models at the 40 E-024 grid points. Dividing those bounds by the
lowered MACs per token that E-024 counted, what are the min and max system energy per lowered MAC for each
transmitter model at every grid point, and how does that compare with the optics-only detection floor per MAC?

## Disclosure

- The E-024 counts and the E-036 envelopes were seen before this registration. While writing it, the agent
  read the stored E-024 `lowered_macs` and `positions` to confirm the denominator divides exactly. No per-MAC
  figure was computed from the stored files, and none is reported here.
- No new device points and no new terms. The numerator is exactly the E-036 envelope (same 11 terms, same
  published points, `awen.energy-per-token-accounting.v12`, assumption set `awen.hardware-assumptions.v0`).
- Denominator: lowered MACs per token = `counts.lowered_macs / positions` (E-024, prefill, no KV cache).
  These are MACs routed to the photonic GEMM simulator only. Host-fallback MACs are not counted, and host
  CPU compute is not a term of the numerator. So this is energy per *lowered* MAC, not per model MAC.
- The bounds are sums of labelled published points within one transmitter model. They are not one physical
  device, not a measured value, and not a speedup or savings figure. No electronic baseline is compared.
- There is no accuracy axis. The E-024 graphs are tiny random-init models with no meaningful accuracy. The
  existing MNIST accuracy-versus-photons-per-MAC curves (`awen-models/results/photon-training/`) use modeled
  photon energy only and stay a separate, optics-only curve. Pairing accuracy with system energy per MAC
  needs one model with both an accuracy and a lowering count, which is later work.
- The optical detection floor per MAC is reported beside the system bounds as a labelled component
  (optics only, lower bound), never as the system figure.
- During development the code ran only in unit tests on synthetic inputs. No E-037 output was stored.

## Inputs (pinned)

| Input | Path (from `awen-models/`) | SHA-256 |
| --- | --- | --- |
| E-036 totals | `results/energy-per-token-total/results.json` | `0f78b7a14dc76f870c0ea7d4108053034a12abd0d3b81a23fcd57bee7f7b7e4b` |
| E-024 counts | `results/conversions-per-token/results.json` | `6cec0563175b6c0b5fb07bb068a7eb06338b19a3a7a3f67dd98f869da8432267` |

The command hashes both files and records the hashes; a mismatch is a failure (`inputs`).

## Design

Module `awen_models.energy_per_mac`, curve `awen.energy-per-mac-curve.v1`. Per point
(arch, tile_k, positions):

- `lowered_macs_per_token` (exact integer, else failure)
- per transmitter (`dml_laser`, `modulator_eo`): `min_J_per_mac = min_J / macs`, `max_J_per_mac = max_J / macs`,
  the per-token bounds, `n_scenarios`, `excluded_terms` and the min/max picks copied from E-036
- `optical_detection_floor_J_per_mac` by effective bits (4, 6, 8), labelled optics only
- `conversions_per_token` and `conversions_per_mac` (from E-024, for context)

Command (from `awen-models/`, CPU):

    python -m awen_models.energy_per_mac --registration e037 --totals results/energy-per-token-total/results.json --counts results/conversions-per-token/results.json --out results/energy-per-mac/results.json

Output stored with its SHA-256, commit and environment.

## Verdict rule

**Pass** when all of this holds: both inputs are passing results of the named versions with the pinned
SHA-256s; totals and counts cover the same grid points (40) with equal per-token counts; at every point the
denominator is a positive exact integer, both transmitter models are present, `0 < min_J_per_mac <= max_J_per_mac`
(finite), `n_scenarios >= 1`, the per-MAC bounds times the denominator recompute the per-token bounds
(relative tolerance 1e-12), the largest detection floor per MAC does not exceed either `max_J_per_mac`, and
conversions per MAC recompute. Otherwise **fail** with the failing points listed.

Expected: pass at all 40 points. Figures are L1 counts times labelled published points, divided by L1 counts.
No speedup, savings or hardware claim follows. This PR registers only; it does not run E-037.
