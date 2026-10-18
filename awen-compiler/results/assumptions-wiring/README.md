# Opt-in hardware assumptions: before/after on a fixed config

Source label: **measured by us** on the build box (Linux 6.12, Intel Xeon 8 vCPU, no GPU,
rustc 1.99.1), simulated (L1 noise model, L0 cost model). Assumption set `awen.hardware-assumptions.v0`.

Fixed config (in `awen-compiler/examples/assumptions_before_after.rs`): photonic GEMM
`X[4,64] = sin(0.37 i)`, `W[8,64] = cos(0.11 i)`, `tile_k` 16, seed 7, `PhotonicNoiseContract::default()`;
cost estimate for a 256x256x256 F16 GEMM on `DeviceCapabilities::pace_like_128()` with
`CostModelInputs::from_capabilities(.., Assumed)`, tile 128, 16 wavelengths.

| File | Commit | Command | SHA-256 |
| --- | --- | --- | --- |
| `before-main-c02eee9.json` | main `c02eee9` (no loader) | in a `c02eee9` worktree: `sed '/BEGIN opt-in/,/END opt-in/d; /use awen_compiler::assumptions/d; s/let mut out/let out/' <this branch's example> > awen-compiler/examples/assumptions_before_after.rs && cd awen-compiler && cargo run -q --example assumptions_before_after` | `275928579bb73859571aebd36439dcea60df91a990ffbe288e2249c922a7e46c` |
| `after.json` | this branch (see PR) | `cd awen-compiler && cargo run -q --example assumptions_before_after` | `cc0e9395fa56347aa5daa20ef40e3ab63bf361d56727d1c5f28fa92c7397098e` |

Result: the `default` section of `after.json` is identical to `before-main-c02eee9.json` (every
number, compared with `json.load(a)["default"] == json.load(b)["default"]`). With the v0 set opted in, the
GEMM outputs and the cost estimate's energy and latency are also identical at the low, typical and high
corners, because v0 has a value for only one mapped field: `dac.weight_dac_bits` typical 8, equal to the
default `dac_bits` 8. Every other mapped field (`laser.rin`, `photodetector.bandwidth`, `mzi.crosstalk`,
`modulator.energy_per_symbol`) is null in v0, so the built-in value is kept and the report says so
(`from_assumptions: false`). The only visible difference in the opted-in cost model is one added provenance
entry naming the assumption set and corner.

Default numbers (from `before-main-c02eee9.json`): GEMM RMS error vs exact 1.2259965581371104, max abs error
2.578891058277216; cost estimate energy 70.87756160543196 uJ, latency 32534.322 ns, throughput 1031.35 GOPS.
These are outputs of illustrative, unsourced defaults (see `awen-spec/hardcoded_device_defaults.yaml`), not
device predictions.
