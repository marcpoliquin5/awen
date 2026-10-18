# AWEN decisions

Numbered, dated log. "Marc" means Marc Poliquin decided; "agent" means the coding
agent decided while executing a broader instruction from Marc. Agent decisions that
fall under Master Prompt v2 section 16 are marked **pending Marc's ratification**.
Times are ET.

## D-001 · 2026-10-18 · Marc
AWEN is built in public in `marcpoliquin5/awen`, through pull requests with the
required quality gate green and no admin bypass.

## D-002 · 2026-10-18 · Marc
Truthfulness rule: remove unverified claims (benchmarks, downloads, metrics); keep the
long-range vision of a photonic compute platform (virtual ISA plus a kernel programming
model), described as a goal, not a result.

## D-003 · 2026-10-18 · Marc
Master Prompt v2 governs from 2026-10-18 11:04 ET and overrides the earlier directive
where they conflict. Merges to `main` now wait for Marc (section 16).

## D-004 · 2026-10-18 · agent · pending Marc's ratification
Merged pull requests #51-#122 without a human review, under Marc's instruction to carry
out the whole earlier directive. Branch protection allowed it (required check only).
Alternative rejected at the time: waiting for review on each PR. From D-003 on, the agent
leaves PRs open and green.

## D-005 · 2026-10-18 · agent · pending Marc's ratification
Tagged and published **v0.1.0** (2026-10-18 09:27 ET; 18 assets, checksums and build
provenance) after a release candidate `v0.1.0-rc.1`. Under v2 section 16 a release tag
needs Marc's approval. Options: (a) ratify as an experimental pre-1.0 release (recommended:
release notes already say simulator-only); (b) mark it pre-release; (c) withdraw it.
No further tags until Marc decides.

## D-006 · 2026-10-18 · agent · pending Marc's ratification
AWEN-Mini was designed and trained **from scratch** (1,016,320 parameters, character level,
Tiny Shakespeare, CPU, PR #83). v2 section 7 says AWEN-Mini should start from a fine-tuned
permissively licensed open model (Qwen3-0.6B/1.7B or SmolLM2-360M/1.7B). Recommendation:
rename the existing model to a test fixture (for example `awen-tiny-char`) and pick an open
base model for AWEN-Mini v0. Base-model choice is Marc's (section 16).

## D-007 · 2026-10-18 · agent · pending Marc's ratification
SmolLM2-135M (Apache-2.0) was used as the base for the CPU LoRA and distillation demos
(#91). This is a base-model and teacher choice under section 16.

## D-008 · 2026-10-18 · agent · pending Marc's ratification
The repository license stayed **MIT** (it was MIT before this work; code and the bundled
checkpoint both ship under it). v2 suggests Apache-2.0 for code and weights. Options:
keep MIT, or relicense to Apache-2.0 (adds an explicit patent grant). Marc decides.

## D-009 · 2026-10-18 · agent · pending Marc's ratification
AWEN wrote its own small reference solvers in `awen-physics` (TMM, S-matrix, 1-D FDTD,
BPM, SHG, adjoint) instead of integrating Meep, SAX or gdsfactory. v2 section 8 says not to
write FDTD/BPM/RCWA without a documented gap. Recommendation: keep them as analytic
validation references and add adapters to Meep/SAX before any new solver work.

## D-010 · 2026-10-18 · agent · pending Marc's ratification
AWEN Studio was built as a local web app in `awen-studio/` in the main repository (#118).
v2 section 11 places the Studio UI in `awen-labs` after a CLI exists. Options: keep it as a
developer tool here, or move the UI later. Not changed without Marc.

## D-011 · 2026-10-18 · agent · pending Marc's ratification
Frontier hypotheses live in `research/frontier-physics/` (#112). v2 section 12 names
`research/frontier/`. Moving it is a reversal, so it stays until Marc agrees.

## D-012 · 2026-10-18 · agent
The 72-paper ledger is not available to the agent. `research/ledger/` keeps the 15 checked
seed entries; no entry is invented. `research/ledger/schema.yaml` records the v2 entry fields.

## D-013 · 2026-10-18 · agent
New multiplexing/fractal work goes under `research/experiments/multiplexing-fractal/`
(v2 section 10) and is opened as draft pull requests only, with pilots labeled as pilots
until the hypotheses are pre-registered.
