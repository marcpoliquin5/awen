# Opt-in hardware assumptions: before/after on a fixed config

**Measured by us** on the build box (Linux 6.12, Intel Xeon 8 vCPU, no GPU, rustc 1.99.1); L1 noise
simulation and L0 cost model, assumption set `awen.hardware-assumptions.v0`. The config is fixed in
`awen-compiler/examples/assumptions_before_after.rs`.

| File | Commit | Command | SHA-256 |
| --- | --- | --- | --- |
| `before-main-c02eee9.json` | main `c02eee9` (no loader) | in a `c02eee9` checkout: `sed '/BEGIN opt-in/,/END opt-in/d; /use awen_compiler::assumptions/d; s/let mut out/let out/' <this example> > awen-compiler/examples/assumptions_before_after.rs; cd awen-compiler; cargo run -q --example assumptions_before_after` | `275928579bb73859571aebd36439dcea60df91a990ffbe288e2249c922a7e46c` |
| `after.json` | this PR's head | `cd awen-compiler; cargo run -q --example assumptions_before_after` | `cc0e9395fa56347aa5daa20ef40e3ab63bf361d56727d1c5f28fa92c7397098e` |

Result: `after.json["default"]` equals the whole of `before-main-c02eee9.json`. With v0 opted in, the GEMM outputs and
the estimate's energy and latency are identical at all three corners. v0 has a value for only one mapped field
(`dac.weight_dac_bits` typical 8, equal to the default), and every other mapped field is null, so the built-in
value is kept (`from_assumptions: false`). The opted-in cost model differs only by one provenance entry naming the set
and corner. The defaults behind these numbers are illustrative and unsourced (`awen-spec/hardcoded_device_defaults.yaml`).
They are not device predictions.
