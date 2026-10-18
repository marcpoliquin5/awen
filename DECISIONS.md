# AWEN decisions

Numbered, dated log. Each entry says who decided: **Marc** (Marc Poliquin) or **agent**
(the coding agent, acting under a broader instruction from Marc). Agent decisions that fall
under Master Prompt v2 section 16 are marked **pending Marc's ratification**. Times are ET.

## D-001 · License stays MIT
Decided by: Marc, 2026-10-18 ~11:07 AM ET (group chat).
The code and the bundled model weights stay under the MIT license, deliberately. v2's default
suggestion (Apache-2.0) was considered and not taken. Reversing this needs Marc.

## D-002 · Agent may squash-merge its own PRs after required checks pass
Decided by: Marc, 2026-10-18 ~11:07 AM ET (group chat).
The agent squash-merges its own pull requests once the required quality gate is green. No admin
bypass, no skipped checks, and no `--delete-branch` on a branch that other PRs are stacked on.
Marc does not review every merge; an audit bot reviews each PR after it merges.

## D-003 · Build AWEN in public through PRs with the required gate green
Decided by: Marc, 2026-10-18 (onboarding).
AWEN is built in public in `marcpoliquin5/awen`, through pull requests with the
required quality gate green and no admin bypass.

## D-004 · Truthfulness rule
Decided by: Marc, 2026-10-18 (onboarding).
Truthfulness rule: remove unverified claims (benchmarks, downloads, metrics); keep the
long-range vision of a photonic compute platform (virtual ISA plus a kernel programming
model), described as a goal, not a result.

## D-005 · Master Prompt v2 governs
Decided by: Marc, 2026-10-18 ~11:04 AM ET.
Master Prompt v2 overrides the earlier directive where they conflict. D-002 sets the merge policy.

## D-006 · Merged #51-#122 without human review
Decided by: agent, 2026-10-18 (under Marc's earlier directive); consistent with D-002.
Branch protection required only the quality gate, and Marc had asked for the whole earlier directive to be
carried out. D-002 now makes this the standing policy.

## D-007 · v0.1.0 tag and release
Decided by: agent, 2026-10-18 · pending Marc's ratification.
Tagged and published **v0.1.0** (2026-10-18 09:27 ET; 18 assets, checksums and build
provenance) after a release candidate `v0.1.0-rc.1`. Under v2 section 16 a release tag
needs Marc's approval. Options: (a) ratify as an experimental pre-1.0 release (recommended:
release notes already say simulator-only); (b) mark it pre-release; (c) withdraw it.
No further tags until Marc decides.

## D-008 · AWEN-Mini trained from scratch
Decided by: agent, 2026-10-18 · pending Marc's ratification.
AWEN-Mini was designed and trained **from scratch** (1,016,320 parameters, character level,
Tiny Shakespeare, CPU, PR #83). v2 section 7 says AWEN-Mini should start from a fine-tuned
permissively licensed open model (Qwen3-0.6B/1.7B or SmolLM2-360M/1.7B). Recommendation:
rename the existing model to a test fixture (for example `awen-tiny-char`) and pick an open
base model for AWEN-Mini v0. Base-model choice is Marc's (section 16).

## D-009 · SmolLM2-135M as LoRA/distillation base
Decided by: agent, 2026-10-18 · pending Marc's ratification.
SmolLM2-135M (Apache-2.0) was used as the base for the CPU LoRA and distillation demos
(#91). This is a base-model and teacher choice under section 16.

## D-010 · Own reference physics solvers
Decided by: agent, 2026-10-18 · pending Marc's ratification.
AWEN wrote its own small reference solvers in `awen-physics` (TMM, S-matrix, 1-D FDTD,
BPM, SHG, adjoint) instead of integrating Meep, SAX or gdsfactory. v2 section 8 says not to
write FDTD/BPM/RCWA without a documented gap. Recommendation: keep them as analytic
validation references and add adapters to Meep/SAX before any new solver work.

## D-011 · Studio location
Decided by: agent, 2026-10-18 · pending Marc's ratification.
AWEN Studio was built as a local web app in `awen-studio/` in the main repository (#118).
v2 section 11 places the Studio UI in `awen-labs` after a CLI exists. Options: keep it as a
developer tool here, or move the UI later. Not changed without Marc.

## D-012 · Frontier hypotheses directory
Decided by: agent, 2026-10-18 · pending Marc's ratification.
Frontier hypotheses live in `research/frontier-physics/` (#112). v2 section 12 names
`research/frontier/`. Moving it is a reversal, so it stays until Marc agrees.

## D-013 · 72-paper ledger absent; agent-sourced list kept separate
Decided by: agent, 2026-10-18.
The 72-paper ledger is not available to the agent. Its 15 agent-written, Crossref-checked
entries moved to `research/reference/agent-sourced.json`; `research/ledger/ledger.json` is an empty
index for Marc's export. No entry is invented. `research/ledger/schema.yaml` records the v2 entry fields.

## D-014 · Multiplexing/fractal work location
Decided by: agent, 2026-10-18.
New multiplexing/fractal work goes under `research/experiments/multiplexing-fractal/`
(v2 section 10) and is opened as draft pull requests only, with pilots labeled as pilots
until the hypotheses are pre-registered.

## D-015 · v0.1.0 GitHub release marked pre-release with an experimental notice
Decided by: Marc, 2026-10-18 11:27 AM ET (approved; change made outside this repository's PRs).
The v0.1.0 release page is marked pre-release and carries an experimental notice. The tag
itself (D-007) still needs Marc's ratification, and it is unsigned (see STATUS.md blockers).

## D-016 · awen-labs roadmap copy (awen-labs PR #21)
Decided by: Marc, 2026-10-18 11:27 AM ET (approved; merged in `awen-labs`).

## D-017 · awen-labs site copy and design
Decided by: Marc, 2026-10-18 (his requests, earlier in the session).
The awen-labs site is an interactive 3D storefront, and it uses the "Why we build with light"
mission copy, which Marc approved. Marc declined an investor one-pager.
