# E-029: energy-per-token accounting with memory-movement bytes counted (partial table, no total, L1)

Registered run of `research/experiments/awen-photon-memory-movement-bytes/PREREGISTRATION.md` (#241, `b27ea78`,
D-030), run unchanged on the stored E-024 counts (`results/conversions-per-token/results.json`, #223).

Command (from `awen-models/`, at commit `819759a`, build box: see `env.txt`; Linux, 8 vCPU, no GPU), started
2026-11-05 08:35 ET, exit 0, about 1 s wall:

    PYTHONPATH=. python -m awen_models.energy_accounting --registration e029 --counts results/conversions-per-token/results.json --out results/energy-per-token-memory-movement/results.json

Inputs: assumption set `awen.hardware-assumptions.v0`, `awen-spec/hardware_assumptions.yaml` SHA-256
`0b7ed6abec9637b0eef875020a572be30ba13b7be9576cac4116f07952ecb930`; accounting version
`awen.energy-per-token-accounting.v5`. A second run gave a byte-identical `results.json` (SHA-256 in `SHA256SUMS`).

## Verdict (registered rule)

**Pass.** At all 40 points (2 architectures x 5 tile sizes x 4 position counts): the E-028 rule holds (every required
term listed, every reported contribution recomputes to count x value x unit, no not-sourced term carries a
contribution, no total), and the memory-movement term lists exactly the two registered placements
(`weights_streamed`, `weights_resident`), each with read and write elements recomputed from the point's per-token
counts and bytes per token equal to elements x 1 and x 2, with no energy contribution because there is no memory
energy-per-byte parameter. `failures` is empty. This matches the registered expectation: six of eleven terms are
still not sourced (laser, weight hold, control electronics, thermal stabilization, memory movement, host transfer),
so there is still **no energy per token**.

## Bytes per token (reported, counting rule only)

Under `weights_streamed`, element counts equal E-028's host-transfer code counts by construction (same lowered-GEMM
rule, different term). Under `weights_resident`, weight DAC elements are not re-read. The two architectures give the
same counts at every point, so one table covers both. Values below are bytes per token at 1 byte per element
(streamed / resident); at 2 bytes per element, both columns double.

| Tile K | 1 position | 8 positions | 32 positions | 128 positions |
| --- | --- | --- | --- | --- |
| 8 | 102,464 / 12,352 | 23,616 / 12,352 | 15,168 / 12,352 | 13,056 / 12,352 |
| 16 | 96,832 / 6,720 | 17,984 / 6,720 | 9,536 / 6,720 | 7,424 / 6,720 |
| 32 | 94,016 / 3,904 | 15,168 / 3,904 | 6,720 / 3,904 | 4,608 / 3,904 |
| 64 | 92,608 / 2,496 | 13,760 / 2,496 | 5,312 / 2,496 | 3,200 / 2,496 |
| 128 | 92,480 / 2,368 | 13,632 / 2,368 | 5,184 / 2,368 | 3,072 / 2,368 |

Streamed range: 3,072 to 102,464 bytes at 1 byte per element (6,144 to 204,928 at 2). Resident range: 2,368 to
12,352 bytes at 1 byte per element (4,736 to 24,704 at 2). Holding weights on the device removes the weight-DAC
share of host memory traffic; the weight hold cost itself stays not sourced (power, no time model). No memory
energy, total, speedup, savings or hardware claim follows from these counts.
