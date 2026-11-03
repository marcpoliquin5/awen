# PACE-like reference descriptor defaults: before/after (proposal, not merged)

**Measured by us** on the build box (Linux 6.12, Intel Xeon 8 vCPU, no GPU, rustc 1.99.1); L0 cost model and
L1 noise simulation, same fixed config as `../assumptions-wiring/` (`examples/assumptions_before_after.rs`).

| File | Commit | Command | SHA-256 |
| --- | --- | --- | --- |
| `before-69f5163.json` | `69f5163` (#135 head, old descriptor) | `cd awen-compiler; cargo run -q --example assumptions_before_after` | `bbe1819f5df1a814bfa033d1cbd385e341d8362d43592d23f25e43dec561f4a4` |
| `after.json` | this PR's head | same command | `5d8b98c9ec046520108f17110e2f6b1fc8fc101f4a37b8d1340b35094560a064` |

Changed in `DeviceCapabilities::pace_like_128()`: `dac_bits` 10 -> 8, `effective_bits` 8 -> 7,
`adc_energy_pj_per_sample` 4.0 -> 6.35. Default cost estimate (`default.cost`): energy 70.87756160543196 ->
70.88478080543197 uJ (ADC part 0.012288 -> 0.0195072 uJ), estimated error fraction 0.0040557035259046045 ->
0.007961953525904604, effective bits 8 -> 7; latency and throughput unchanged. The noise-model output does not
use this descriptor and is unchanged. These are illustrative defaults, not device predictions.
