# E-031: energy-per-token accounting with published thermo-optic weight-hold energy (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-weight-hold-energy/PREREGISTRATION.md` (#248, `b542a52`,
D-032), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `4d01488`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-06 06:36 ET, exit 0, about 1.0 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e031 --counts results/conversions-per-token/results.json --out results/energy-per-token-weight-hold/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`7150bc14729ac5f2a5169eee0b3da022d2a5da9cc982baa6c99a4bbab6e86dee`; accounting version
`awen.energy-per-token-accounting.v7`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-030 rule holds (every required
term listed, every reported contribution recomputes, no not-sourced term carries a contribution, no total), and the
weight-hold term is sourced from each published design with per-token joules equal to `dac_weight` × energy_per_pi.
`failures` is empty. This matches the registered expectation: four of eleven terms are still not sourced (laser,
control electronics, thermal stabilization, host transfer), so there is still **no energy per token**.

## Weight-hold energy per token (reported, L1 counts x peer-reviewed Pπ·τ estimate)

Weight DAC conversions are independent of tile size under the E-024 closed form (90,112 / P), so every tile gives
the same weight-hold energy at a given position count. The two architectures also match. Energy uses the single
published point `harris2014` (`phase_shifter.published_designs`, Table I Pπ·τ = 66.9 pJ): joules =
`dac_weight` × 66.9e-12 J. Values below are per token. Continuous hold during compute is **not** included (one
π-reconfiguration energy per weight DAC conversion only).

| Positions | Weight DAC / token | Weight-hold energy / token |
| --- | --- | --- |
| 1 | 90,112 | 6.028 µJ |
| 8 | 11,264 | 753.6 nJ |
| 32 | 2,816 | 188.4 nJ |
| 128 | 704 | 47.10 nJ |

Range: 47.10 nJ to 6.028 µJ per token. Source labels: weight DAC counts are measured by us under the E-024 counting
rule on stored simulator counts; the 66.9 pJ figure is a peer-reviewed estimate (Harris et al., Opt. Express 22,
10487 (2014) / arXiv:1410.3616, Table I Pπ·τ product from measured Pπ 24.77 ± 0.43 mW and τ = 0.35/f_3dB =
2.69 µs from measured 130.0 kHz bandwidth). Source type in the report is `estimate`. Values in the table are
rounded from `results.json`.

## What stays not sourced

Four of eleven terms stay "not sourced": laser, control electronics, thermal stabilization, and host transfer
(`system.host_transfer_energy` is still null). ADC, DAC (per published design), modulator (per published basis),
optical detection floor, memory movement (per published HBM2 point and placement), and now weight hold (per
published π-reconfiguration point) are filled. There is still **no total**.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. Counting one full-π thermal
reconfiguration energy per weight DAC conversion is a labelled upper-bound-style proxy for a programming transient,
not a claim about steady-state mesh power: actual programmed phase deltas are not all π, and continuous hold power
during compute is excluded. Filling the remaining terms needs checked sources for laser, control and thermal power,
a fitted host-link-only energy per byte, and a registered time model for the power terms.
