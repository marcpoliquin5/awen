# Reference simulator backend plugin

This directory is the development fixture for typed backend discovery:

- `backend-manifest.json` embeds a validated 2x2 simulator capability.
- `health.json` is the live `awen.backend-health.v1` query source.

The runtime re-reads `health.json` on every discovery query, so a simulator or
lab controller can atomically publish resource loss, temperature, drift,
calibration identity, or recovery without changing compiler code.

The manifest is intentionally unsigned and is accepted only with the explicit
development flag:

```bash
cargo run --manifest-path awen-runtime/Cargo.toml --bin awenctl -- \
  backends awen-runtime/plugins/reference_sim --allow-unverified
```

The advertised values are deterministic simulator inputs, not measurements of
a physical device.
