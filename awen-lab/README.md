# awen-lab: the AWEN Lab Agent (simulation only)

Experiment orchestration over the existing AWEN simulators: the Rust photonic
GEMM model (`awen_native`) and the quantum-photonic reference simulators
(`awen_qphotonic`). **It has no hardware path.** Every hardware request is
refused, and an approval cannot unlock one.

```bash
pip install ./awen-native ./awen-qphotonic ./awen-lab
awen-lab --store .awen-lab truth experiment.json
```

## Workflows

| workflow | what it does | output state |
|---|---|---|
| Quick | one seed, fast feedback | runs with metrics and output hashes |
| Truth | checks the simulator against exact oracles and analytic predictions: the ideal contract matches the float64 oracle (< 1e-9); the noisy RMS error lies within 0.75–1.33× the analytic noise budget; boson-sampling total variation lies within 3·sqrt(support/shots); HOM coincidences are 0 and 0.5 | pass/fail per check |
| Repro | re-runs a stored state and requires bit-identical outputs per seed | `reproduced` true/false; parent = original |
| Compare | multi-seed runs of two experiments; mean, std, bootstrap 95% CI and a paired delta; setting differences | parents = head + both sides |
| Publish | writes a self-verifying local bundle (state, full lineage, approval, hashes). It needs a recorded approval naming the approver and the reason. Nothing is uploaded or sent | bundle file + publish state |

## Lab states

Each state is JSON holding the experiment, metrics, provenance (agent
version, UTC time, git SHA, Python and package versions, operating bounds)
and the recent policy audit. It is addressed by the SHA-256 of its canonical
JSON and stored write-once under `objects/`. Reading a state re-hashes it, so
any edit on disk is detected (`StateIntegrityError`). Branches live under
`refs/`. `branch`, `restore` (hash, unique prefix or branch), `history` and
`diff` work like a small version-control system.

## Safety

- Allowed targets: `photonic-sim`, `awen-reference`, `qphotonic-sim`.
  Anything else is refused as a hardware action (`HardwareActionRefused`).
- Operating bounds (`OperatingBounds`) cap matrix sizes, inner dimension,
  photon budgets, seeds, modes, photons and shots (`BoundsViolation`).
- Publish needs `Policy.approve(state_hash, approver, reason)` for that exact
  state (`ApprovalRequired`).
- Every decision is appended to the policy audit, which is stored in the
  next state.
