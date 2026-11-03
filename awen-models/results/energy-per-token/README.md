# E-025: energy-per-token accounting for the E-024 counts (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-energy-accounting/PREREGISTRATION.md` (#225, `0cb526b`, D-026),
run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `c1ca8a1`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-03 12:28 ET, exit 0, about 1.2 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --counts results/conversions-per-token/results.json --out results/energy-per-token/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`a895ea024c6caa34083a713c5174f77d1901f99b310189b210b2d223fc831f0d`. A second run gave a byte-identical
`results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): every required term is listed, every
reported contribution recomputes to count x value x unit, no not-sourced term carries a contribution, and no total is
emitted (`total_J` is null, "9 terms not sourced"). `failures` is empty.

## What could be filled (reported, no test)

Only two of the eleven terms have a sourced per-event value today. Nine stay "not sourced": laser, modulator, input
DAC, weight DAC, weight hold, control electronics, thermal stabilization, memory movement and host transfer. So there
is **no energy per token**, only two labelled partial terms.

ADC reads per token depend only on the tile size (E-024), so these partial terms are the same for both architectures
and every position count.

| Tile size | ADC reads per token | ADC term, lagos2021 design (6.35 pJ per read) | ADC term, ramkaj2020 design (31.7 pJ per read) | Optical detection floor, 8 effective bits |
| --- | --- | --- | --- | --- |
| 8 | 11,264 | 71.5 nJ | 357 nJ | 94.5 pJ |
| 16 | 5,632 | 35.8 nJ | 179 nJ | 47.3 pJ |
| 32 | 2,816 | 17.9 nJ | 89.3 nJ | 23.6 pJ |
| 64 | 1,408 | 8.94 nJ | 44.6 nJ | 11.8 pJ |
| 128 | 1,280 | 8.13 nJ | 40.6 nJ | 10.7 pJ |

Source labels: ADC reads are measured by us (simulator counts, E-024). The per-read ADC energies are estimates
derived from two peer-reviewed measured standalone CMOS ADC designs (`lagos2021`: Walden figure of merit 6.2 fJ x
2^10.0; `ramkaj2020`: 158.6 mW / 5 GS/s), as listed in `hardware_assumptions.yaml`; their effective bits and sample
rates are not those of the simulated contract, and the two designs are never interpolated. The optical detection floor
is an estimate (photon energy at 1550 nm x 4^b detected photons, b = 8; the 4- and 6-bit rows are in `results.json`),
a lower bound on optical energy only, not a system energy. Values in the table are rounded from `results.json`.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. The ADC term alone is not a
lower bound on system energy per token in any claim-ready sense, because the dominant E-024 count (weight DAC, up to
90,112 per token at one position) has no sourced energy. Filling the missing terms needs checked sources for DAC,
modulator, laser, phase-shifter hold power, control and thermal power, a memory-movement parameter, a byte count for
host transfer, and a registered time model for the power terms.
