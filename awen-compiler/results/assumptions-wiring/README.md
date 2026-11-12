# Opt-in hardware assumptions: before/after on a fixed config

**Measured by us** on the build box (Linux 6.12, Intel Xeon 8 vCPU, no GPU, rustc 1.99.1); L1 noise
simulation and L0 cost model, assumption set `awen.hardware-assumptions.v0`. The config is fixed in
`awen-compiler/examples/assumptions_before_after.rs`.

| File | Commit | Command | SHA-256 |
| --- | --- | --- | --- |
| `before-main-c02eee9.json` | main `c02eee9` (no loader) | in a `c02eee9` checkout: `sed '/BEGIN opt-in/,/END opt-in/d; /use awen_compiler::assumptions/d; s/let mut out/let out/' <this example> > awen-compiler/examples/assumptions_before_after.rs; cd awen-compiler; cargo run -q --example assumptions_before_after` | `275928579bb73859571aebd36439dcea60df91a990ffbe288e2249c922a7e46c` |
| `after.json` | this PR's head | `cd awen-compiler; cargo run -q --example assumptions_before_after` | `bbe1819f5df1a814bfa033d1cbd385e341d8362d43592d23f25e43dec561f4a4` |

Result: `config` and `default` in `after.json` equal the whole of `before-main-c02eee9.json`. With v0 opted in, the GEMM outputs and
the estimate's energy and latency are identical at all three corners. v0 has a value for only one mapped field
(`dac.weight_dac_bits` typical 8, equal to the default), and every other mapped field is null, so the built-in
value is kept (`from_assumptions: false`, no source label). The applied `dac_bits` is labeled `source: hua2025`, `source_type: measured`. The opted-in cost model differs only by one provenance entry naming the set
and corner; its source is derived from the applied cost fields (none in v0, so `assumed`). The defaults behind these numbers are illustrative and unsourced (`awen-spec/hardcoded_device_defaults.yaml`).
They are not device predictions.

Later change (D-042, 2026-11-10): the assumption set now has a value for a second mapped field, `photodetector.bandwidth`
(chen2016, typical 67 GHz, low/high null). Opting in at the typical corner therefore sets `detection_bandwidth_hz` to 67 GHz,
which changes the noisy GEMM outputs at that corner. `after.json` above is the record of the #135 head and was not regenerated.
The default contract is unchanged (checked by `awen-compiler/tests/assumption_report.rs`).

Later change (D-043, 2026-11-10): the set now stores `mzi.extinction_ratio` (demarinis2021, 22.9/26.8/42.5 dB). It is not a
mapped field, and `mzi.crosstalk` stays null, so opting in still leaves `crosstalk_db` at the built-in -35 dB at every corner
(checked by `awen-compiler/tests/assumption_report.rs`).

Later change (D-044, 2026-11-10): `laser.rin` now has a measured upper bound (vaskasi2022: below -150 dB/Hz up to 6 GHz),
stored at the high corner only. Opting in at the high corner therefore applies `rin_db_per_hz` = -150 dB/Hz with that label;
it equals the built-in value, so no noisy output changes at any corner (checked by `awen-compiler/tests/assumption_report.rs`).
`laser.linewidth` (28 kHz / 440 kHz, same source) is not a mapped field.

Later change (D-045, 2026-11-12): the set now stores `photodetector.quantum_efficiency` (0.552/0.592/0.632), an AWEN estimate
from the chen2016 measured responsivity at 1550 nm. It is not a mapped field, so opting in still leaves `quantum_efficiency`
at the built-in 0.8 at every corner (checked by `awen-compiler/tests/assumption_report.rs`).
