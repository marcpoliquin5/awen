# E-026: energy-per-token accounting with published DAC designs (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-energy-accounting-dac/PREREGISTRATION.md` (#229, `c6c44b5`,
D-027), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `73c10d0`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-04 00:28 ET, exit 0, about 1.3 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e026 --counts results/conversions-per-token/results.json --out results/energy-per-token-dac/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`219d0697f5a3e928a383957258415db04874238b827ea3cff969a63ba3332c83`. A second run gave a byte-identical
`results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): every required term is listed, every
reported contribution recomputes to count x value x unit, no not-sourced term carries a contribution, no total is
emitted (`total_J` is null, "7 terms not sourced"), and both DAC terms carry one contribution per published design
(3 each). `failures` is empty. This matches the registered expectation.

## What could be filled (reported, no test)

Four of the eleven terms now have a sourced per-event value: input DAC, weight DAC (each per published design, never
interpolated), ADC and the optical detection floor (as in E-025). Seven stay "not sourced": laser, modulator, weight
hold, control electronics, thermal stabilization, memory movement and host transfer. So there is still **no energy
per token**, only labelled partial terms.

Input DAC conversions per token are 1,088 at every point. Weight DAC conversions per token depend only on the
position count (E-024), so the DAC terms are the same for both architectures and every tile size.

| Term | Positions | Conversions per token | sedighi2012 (5.2 pJ) | palmers2010 (16.875 pJ) | kim2016 (5.71 pJ) |
| --- | --- | --- | --- | --- | --- |
| input DAC | any | 1,088 | 5.66 nJ | 18.4 nJ | 6.21 nJ |
| weight DAC | 1 | 90,112 | 469 nJ | 1.52 uJ | 515 nJ |
| weight DAC | 8 | 11,264 | 58.6 nJ | 190 nJ | 64.3 nJ |
| weight DAC | 32 | 2,816 | 14.6 nJ | 47.5 nJ | 16.1 nJ |
| weight DAC | 128 | 704 | 3.66 nJ | 11.9 nJ | 4.02 nJ |

The ADC and optical-floor rows are unchanged from E-025 (`results/energy-per-token/README.md`).

Source labels: DAC conversions are measured by us (simulator counts, E-024). The per-conversion DAC energies are
estimates derived from three peer-reviewed measured standalone DAC designs (`sedighi2012`: 26 mW / 5 GS/s, 8 bit;
`palmers2010`: 27 mW / 1.6 GS/s, 10 bit; `kim2016`: 17.7 mW / 3.1 GS/s, 6 bit), as listed in
`hardware_assumptions.yaml`. They are standalone DACs, not modulator or phase-shifter drivers; their resolution,
rate and load are not those of the simulated contract, and the three designs are never interpolated. Values in the
table are rounded from `results.json`.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. The DAC and ADC terms together
are not a lower bound on system energy per token in any claim-ready sense, because the modulator (driven at every
input DAC conversion), laser, weight-hold, control, thermal, memory-movement and host-transfer terms have no sourced
value. Filling them needs checked sources for modulator energy per symbol, laser and phase-shifter hold power,
control and thermal power, a memory-movement parameter, a byte count for host transfer, and a registered time model
for the power terms.
