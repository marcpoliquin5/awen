# Research-to-code pipeline

How an idea moves from the research ledger into AWEN code without losing the
evidence trail. Each stage has an exit test; an idea that cannot pass one
stays at that stage.

| stage | artefact | exit test |
|---|---|---|
| 1. Ledger | entry in `research/ledger/ledger.json` | schema-valid; `verified` only with a checked citation |
| 2. Hypothesis | for frontier ideas, `research/frontier-physics/H*.md` | states math, assumptions, observables and **falsification conditions** |
| 3. Baseline check | function in `frontier_checks.py` or a notebook in `awen-physics` | reproduces the established result it builds on, in CI |
| 4. Prototype | module behind an explicit fidelity label (`math_reference`, `photonic_sim`, …) | unit tests against an analytic or reference oracle |
| 5. Measured study | script plus committed results JSON (multi-seed; `awen-bench` evidence for timing) | results reproduce from the committed command and seed |
| 6. Integration | compiler/runtime/library change via issue-linked PR | required gate green; tracker row updated |
| 7. Hardware validation | comparison with measured device data | only with real lab data; never asserted from simulation |

## Rules

1. **Provenance travels with the code.** A module that implements a ledger
   direction names the ledger `id` in its docstring, and the ledger entry's
   `awen_mapping.modules` names the module.
2. **Fidelity labels never move up without evidence.** Simulation results
   are labeled simulated; energy and speed derived from models are labeled
   estimated.
3. **Negative results are kept.** A falsified hypothesis keeps its file, and
   the result goes into it with the data that falsified it.
4. **Frontier stays separate.** `research/` is never imported by `awen-*`
   packages. Promoting an idea means re-implementing it in core with tests
   (stage 4), not importing the research check.
5. **No unverified citations.** Imports that lack a checked citation land as
   `citation pending` (see `research/ledger/import_ledger.py`).

## Example: an idea that went through the pipeline

`photoelectric-multiplication` (ledger) → shot-noise term in the AEP-0022
noise contract (stage 4, `awen-runtime` noise model) → AWEN-Photon
noise-injection training curves (stage 5,
`awen-models/results/photon-training/`) → `Runtime(device="photonic-sim")`
(stage 6). Stage 7 is open: there is no hardware data.
