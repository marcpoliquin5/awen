# E-028: energy-per-token accounting with host-transfer bytes counted (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-host-transfer-bytes/PREREGISTRATION.md` (#237, `67caa02`,
D-029), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `9e7cc69`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-05 00:35 ET, exit 0, about 1.3 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e028 --counts results/conversions-per-token/results.json --out results/energy-per-token-host-transfer/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`0b7ed6abec9637b0eef875020a572be30ba13b7be9576cac4116f07952ecb930`; accounting version
`awen.energy-per-token-accounting.v4`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-027 rule holds (every required
term listed, every reported contribution recomputes to count x value x unit, no not-sourced term carries a
contribution, no total), and the host-transfer term lists its codes per token and its bytes per token at 1 and 2
bytes per code, each equal to codes x width, with no energy contribution because `system.host_transfer_energy` is
null. `failures` is empty. This matches the registered expectation: six of eleven terms are still not sourced
(laser, weight hold, control electronics, thermal stabilization, memory movement, host transfer), so there is still
**no energy per token**.

## Bytes per token (reported, counting rule only)

Codes per token = input DAC + weight DAC + ADC conversions (E-024 `per_token.total`). The two architectures give the
same counts at every point, so one table covers both. Bytes at 2 bytes per code are twice the 1-byte column.

| Tile K | 1 position | 8 positions | 32 positions | 128 positions |
| --- | --- | --- | --- | --- |
| 8 | 102,464 | 23,616 | 15,168 | 13,056 |
| 16 | 96,832 | 17,984 | 9,536 | 7,424 |
| 32 | 94,016 | 15,168 | 6,720 | 4,608 |
| 64 | 92,608 | 13,760 | 5,312 | 3,200 |
| 128 | 92,480 | 13,632 | 5,184 | 3,072 |

Values are codes per token, which equal bytes per token at 1 byte per code (range 3,072 to 102,464 bytes; at 2 bytes
per code, 6,144 to 204,928 bytes). At one position the weight DAC codes (90,112 per token) dominate because weights
are reprogrammed once per call and shared by fewer tokens; with more positions per call the ADC reads dominate.

Source labels: codes per token are measured by us (simulator counts, E-024). Bytes per token are an estimate by the
registered formula codes x bytes per code, under a stated packing rule that counts no headers, framing,
compression, protocol overhead or host-side copies. This is a counting rule about the simulator's partitioning, not
a measurement of any link, bus or device.

## Scope

No energy for host transfer, no total, energy per MAC, speedup, savings or hardware claim follows from this run.
Turning the byte count into energy needs a checked, sourced host-link energy per byte in
`hardware_assumptions.yaml` (`system.host_transfer_energy`), registered as a new experiment. Memory movement still
needs a registered memory model.
