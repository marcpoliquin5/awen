# E-030: energy-per-token accounting with published HBM2 memory energy (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-memory-energy/PREREGISTRATION.md` (#244, `77d6b43`,
D-031), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `80a4d89`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-05 18:22 ET, exit 0, about 1.4 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e030 --counts results/conversions-per-token/results.json --out results/energy-per-token-memory-energy/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`65934d7cb8a2d2d316cdab3909fddf31cad4f4bad2e03253d423ece4e5cee594`; accounting version
`awen.energy-per-token-accounting.v6`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-029 rule holds (every required
term listed, every reported contribution recomputes, no not-sourced term carries a contribution, no total), and the
memory-movement term is sourced from each published design with per-token joules equal to bytes x 8 x energy_per_bit
for each placement and each of 1 and 2 bytes per element. `failures` is empty. This matches the registered
expectation: five of eleven terms are still not sourced (laser, weight hold, control electronics, thermal
stabilization, host transfer), so there is still **no energy per token**.

## Memory-movement energy per token (reported, L1 counts x peer-reviewed HBM2 access)

Under `weights_streamed`, element counts equal E-028's host-transfer code counts by construction. Under
`weights_resident`, weight DAC elements are not re-read. The two architectures give the same counts at every point,
so one table covers both. Energy uses the single published point `oconnor2017` (HBM2 access, 3.97 pJ/bit,
`memory.published_designs`): joules = bytes x 8 bits/byte x 3.97e-12 J/bit. Values below are nJ per token at 1 byte
per element (streamed / resident); at 2 bytes per element, both columns double. Bytes match E-029.

| Tile K | 1 position | 8 positions | 32 positions | 128 positions |
| --- | --- | --- | --- | --- |
| 8 | 3,254 / 392.3 | 750.0 / 392.3 | 481.7 / 392.3 | 414.7 / 392.3 |
| 16 | 3,075 / 213.4 | 571.2 / 213.4 | 302.9 / 213.4 | 235.8 / 213.4 |
| 32 | 2,986 / 124.0 | 481.7 / 124.0 | 213.4 / 124.0 | 146.4 / 124.0 |
| 64 | 2,941 / 79.27 | 437.0 / 79.27 | 168.7 / 79.27 | 101.6 / 79.27 |
| 128 | 2,937 / 75.21 | 433.0 / 75.21 | 164.6 / 75.21 | 97.57 / 75.21 |

Range at 1 byte per element: streamed 97.57 nJ to 3.254 µJ; resident 75.21 nJ to 392.3 nJ. Source labels: bytes are
measured by us under the E-029 counting rule on stored E-024 simulator counts; the 3.97 pJ/bit figure is peer-reviewed
measured HBM2 access energy (O'Connor et al., MICRO-50 2017, doi:10.1145/3123939.3124545, Section 1 / Fig. 1b;
approximate; on-die movement + activation + I/O; write energy not separately reported; not a host-link figure). Values
in the table are rounded from `results.json`.

## What stays not sourced

Five of eleven terms stay "not sourced": laser, weight hold, control electronics, thermal stabilization, and host
transfer (`system.host_transfer_energy` is still null). ADC, DAC (per published design), modulator (per published
basis), optical detection floor, and now memory movement (per published HBM2 point and placement) are filled. There is
still **no total**.

## Scope

No total, energy per MAC, speedup, savings or hardware claim follows from this run. Applying the HBM2 access energy to
counted host-memory bytes is a labelled partial term only: the figure is for HBM2 access (not PCIe/CXL, not GDDR, not
photonic memory), approximate, and uses the same read energy for write bytes because a separate write figure was not
reported. Filling the remaining terms needs checked sources for laser and phase-shifter hold power, control and thermal
power, a fitted host-link-only energy per byte, and a registered time model for the power terms.
