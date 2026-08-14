# Simulator plugin implementation guide

This directory is documentation, not a loadable plugin. Start from the tested
reference implementation at
`awen-runtime/plugins/reference_sim/backend-manifest.json` and preserve the
closed `awen.plugin.v1` contract described in
`awen-spec/specs/plugin-contracts.md`.

A plugin contribution is complete only when it includes:

1. a schema-valid `backend-manifest.json` with a unique backend ID, exact ABI,
   declared capabilities, health path, and typed physical-design adapters;
2. a schema-valid health document bound to the same backend, calibration, and
   topology identities;
3. a runtime registration implementation that is isolated from the compiler;
4. discovery, signature, path-sandboxing, capability, and execution tests; and
5. an immutable marketplace checksum of the exact manifest bytes.

Unsigned manifests are development-only and require the explicit
`--allow-unverified` flag. Normal discovery requires an Ed25519 signature;
deployments must separately pin authorized signer keys because a self-supplied
key proves integrity but not identity. Never place credentials, proprietary PDK
material, executable payloads, or mutable artifact URLs in a manifest.
