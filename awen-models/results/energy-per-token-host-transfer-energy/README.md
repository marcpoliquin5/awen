# E-035: energy-per-token accounting with published PCIe Gen4 host-link PHY energy-per-bit (all terms sourced, total not defined, L1)

Registered run of `research/experiments/awen-photon-host-transfer-energy/PREREGISTRATION.md` (#264, `c52b3db`,
D-036), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `bbc22663942d8b3c2a34bc175483b842b16e34a8`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-07 15:34 ET, exit 0, about 1.7 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e035 --counts results/conversions-per-token/results.json --out results/energy-per-token-host-transfer-energy/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`e0965ba697e23559d0638fdbf31a405f497f898215ec75a3349a0330651d246f`; accounting version
`awen.energy-per-token-accounting.v11`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-034 rule holds (every required
term listed, every reported contribution recomputes, no not-sourced term carries a contribution), and the
host_transfer term is sourced from each published design with per-token joules equal to
`bytes_per_token[width] × 8 × energy_per_bit` for each packing width. `failures` is empty. `not_sourced` is empty at
every point. This matches the registered expectation: all eleven terms are sourced under e035, but
`total_J = "not defined by this registration"` (a numeric total needs a later registration). There is still **no
energy per token** as a single summed figure.

## Host-transfer energy per token (reported, L1 counts x measured Gen4 PCIe PHY efficiency)

Codes per token = input DAC + weight DAC + ADC (E-024 `per_token.total`; range 3,072 to 102,464). Bytes at width 1
equal codes; at width 2 they are twice that. Energy uses the published point `li2018`
(`host_link.published_designs`): 7.5 pJ/bit measured Gen4 PCIe transceiver (TX EQ + RX DFE), packed as 60.0 pJ/byte
via × 8 (labelled). Joules = bytes × 8 × energy_per_bit × 1e-12. Values below are per token. One-end host-side PHY
only; protocol above the PHY, the far-end device PHY, device DRAM (E-030) and whole-node GPU uncore are excluded.
Do **not** treat this as a whole host–accelerator path.

| Basis | Energy / bit | Energy / byte | Codes / token (min–max) | Host energy / token (1 B/code) | Host energy / token (2 B/code) |
| --- | --- | --- | --- | --- | --- |
| phy_transceiver_tx_rx | 7.5 pJ | 60.0 pJ | 3,072 – 102,464 | 184.32 nJ – 6.148 µJ | 368.64 nJ – 12.30 µJ |

Range at 1 byte per code: 184.32 nJ to 6.148 µJ; at 2 bytes per code: 368.64 nJ to 12.30 µJ. The two architectures
give the same counts at every point. Source labels: codes (and therefore bytes under the registered packing rule)
are measured by us under the E-024 / E-028 counting rules on stored simulator counts; the 7.5 pJ/bit figure is
peer-reviewed measured PHY power efficiency (Li et al., 2018 IEEE A-SSCC, pp. 5–8, doi:10.1109/ASSCC.2018.8579314;
abstract states 7.5 pJ/b for a 2.5–16 Gbps Gen4 PCIe transceiver in 10 nm FinFET). Source type in the report is
`measured` for the PHY efficiency; the × 8 packing step is labelled. Values in the table are rounded from
`results.json`.

## What stays not defined

Zero of eleven terms stay "not sourced". ADC, DAC (per published design), modulator (per published basis), optical
detection floor, memory movement (per published HBM2 point and placement), weight hold (per published
π-reconfiguration point), laser (per published DML DC energy-per-bit point), thermal (per published heater
energy-per-bit point), control (per published tuner-logic energy-per-bit point), and now host transfer (per
published Gen4 PCIe PHY energy-per-bit point) are filled. The registration still reports
`total_J = "not defined by this registration"`; a numeric total needs a separate registration.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. Applying one measured Gen4
PCIe PHY efficiency to counted host bytes is a labelled one-end PHY term only. Defining a summed energy per token
needs a new registration that chooses how the sourced terms combine.
