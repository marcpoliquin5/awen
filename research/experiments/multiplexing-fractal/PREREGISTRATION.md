# Multiplexing / fractal hypotheses H1-H4: pre-registration

Registered 2026-10-18 (commit of this file) under AWEN Master Prompt v2, section 10. Status of every
hypothesis: **registered, full runs not started**. The full-run configs are in `configs/` and are
committed here, before any full run. Each config's `command` uses a `--config` flag that the
per-hypothesis PRs add; the pilot scripts in #128 do not accept it yet. A change to a config after a full run starts is a new
registration with its own date, and the change and its reason go in this file.

## Pilots already ran (disclosure)

Pilots for H1, H2 and H3 ran on 2026-10-18 (started 10:58-11:01 ET, per `PILOT_RUNS.json`), **before** this registration. Their code and
outputs are in draft PR #128 (branch `research/fractal-track` at `0974f38`; code at `2c3e38a`), with
SHA-256 checksums in `results/PILOT_RUNS.json` there. The pilots shaped this design, so they are
**exploratory, not confirmatory**. No pilot number counts as evidence for or against a hypothesis, and
none is reused in the keep/kill report. What the pilots measured (source: stored result, `0974f38`):

- **H1 pilot** (`meshes_n8.json`, `meshes_n16.json`; lossless unitary fits to Haar-random targets):
  mean fidelity reached 1.0 for butterfly at depth 9 and 0.999929 for Clements at depth 8 (N = 8), and
  0.999905 for butterfly at depth 16 vs 0.991918 for Clements at depth 16 (N = 16). Butterfly paths carry
  crossings (18 at N = 8, depth 9; 64 at N = 16, depth 16), Clements paths none. No loss budget, no task accuracy and no fabrication-noise
  seeds, so the H1 criterion was **not tested**.
- **H2 pilot** (`h2_pilot_runs.jsonl`; character LM FFN variants at matched stored parameters,
  2 seeds): validation loss baseline 1.72089 / 1.71541, Monarch 1.765 / 1.73982, Kronecker-3 1.72315 /
  1.70822, recursive 1.61527 / 1.60752 (seeds 1 / 2). Batch-1 forward latency of the three variants, per seed, was 2.2-8.5x the same-seed baseline
  (recursive seed 2: 3.208 / 1.459 = 2.20; kron3 seed 1: 10.622 / 1.25 = 8.50), measured while two runs shared the CPU. Only 2 seeds and no conversions-per-MAC count, so the H2 criterion was
  **not tested**. `recursive` is weight sharing, not a dimension-factorized layer, and is not an H2 candidate.
- **H3 pilot** (`stacks.json`; exact lossless TMM of Fibonacci and Cantor stacks): trace-map invariant and
  band-measure checks, plus a Cantor self-similarity scaling scan (best factor 2.3-2.5 vs the naive 3). No WDM
  channel plan, no crosstalk and no comparison against a uniform grid, so the H3 criterion was **not tested**.
- **H4**: no pilot.
- Also run: `boxcount.json` (box-counting dimension, exploratory, not an H1-H4 test).

## Rules for every hypothesis

- Source labels: every number in the report is **measured by us** (simulation on the build box, L1 unless
  stated), with command, commit, environment and output file plus SHA-256. A parameter that the assumption set
  (`awen-spec/hardware_assumptions.yaml`) has no checked value for is a **free parameter** of the study. Its
  grid is fixed in the config and labeled that way, and no result is presented as a device prediction.
- Seeds: at least 3 per configuration (H1: 5 initialization seeds, each with 5 fabrication-noise seeds). Seed
  lists are fixed in the configs. No run is dropped; a crashed or diverged run is reported as such.
- **Confirmatory seeds never reuse a pilot seed.** The pilots used seeds 0, 1 and 2 (`PILOT_SEEDS` in
  `research/tests/test_fractal_preregistration.py`); every confirmatory seed list starts at 101 (fabrication
  noise at 201), and a test checks that the sets are disjoint.
- Intervals: 95% two-sided Student-t interval of the **paired per-seed difference** (candidate minus
  baseline, same seed), n = number of independent seeds (stated per hypothesis). "Beats" means the whole
  interval is on the better side of 0. Effect size (mean paired difference and Cohen's d_z) and the interval
  are reported whatever the outcome.
- Multiple tasks: a hypothesis is **kept** only if its criterion holds on every registered task.
  Otherwise it is **killed**, and the report gives per-task results. Negative results are recorded with the same detail
  (v2 section 2).
- Compute: CPU only on the build box (tier T0). No cloud runs, no spending.
- Output: `results/<hypothesis>/` with raw per-seed rows and checksums, then one keep/kill report
  `REPORT.md`.

## H1: butterfly / Monarch photonic meshes

- **Hypothesis**: at an equal total insertion-loss budget, butterfly meshes give higher task accuracy per dB
  than a Clements mesh and a low-rank (SVD, rank r) mesh.
- **Tasks (3)**: (a) unitary approximation of Haar-random targets, scored by fidelity
  `|Tr(V†U)|²/N²`; (b) linear classification of 16-feature PCA-reduced MNIST through one photonic
  layer + detection, scored by test accuracy; (c) DFT-matrix approximation, scored by fidelity.
- **Primary metric (one)**: task score at a fixed loss budget. For each mesh family, the configuration
  scored is the deepest one whose path loss `loss_dB = mzis_per_path * il_mzi_db + crossings_per_path *
  il_crossing_db` is at most the budget. The primary point is N = 16, budget 6 dB, `il_mzi_db` 0.25,
  `il_crossing_db` 0.05, phase sigma 0.02 rad, splitter imbalance 0.02 (all free parameters; see config).
  Score per dB, N = 8 and the rest of the grid are secondary, sensitivity only.
- **Pairing and n**: fabrication-noise seeds are nested in initialization seeds, so the 25 runs are not
  independent. The score is averaged over the 5 fabrication-noise seeds within each initialization seed; the
  paired t-test then runs across the **5 initialization seeds (n = 5, df = 4)**. n = 5 has low power: a
  small real effect can fail the threshold, and the effect size and interval are reported either way. A
  mixed model with initialization seed as a random effect is a secondary analysis only.
- **Decision threshold**: butterfly beats a baseline on a task if the lower bound of the 95% paired-t
  interval of (butterfly - baseline) at the primary point is > 0.
- **Falsified if**, on any of the 3 tasks, butterfly does not beat **both** Clements and low-rank.

- **Definitions fixed 2026-10-24, before any H1 full run** (the config leaves them open; `configs/h1.yaml`
  is unchanged): phase error = i.i.d. N(0, `phase_sigma_rad`) on every phase shifter (theta, phi and output
  phases); splitter imbalance = each of an MZI's two couplers has power split 0.5 + eta with eta ~
  N(0, `splitter_imbalance`), clipped to |eta| <= 0.49 (`physical_mzi` in `h1_meshes.py`, equal to the
  pilot's `T(theta, phi)` at eta = 0). Each initialization seed s draws its own Haar targets from
  `default_rng(s)` (the DFT target is fixed) and starts the fit from seed s; fabrication draw f for seed s
  uses torch seed `f * 1000 + s`. Phases are fit without noise, then scored on the fabricated mesh.
- **Low-rank and MNIST definitions fixed 2026-10-24 (session 2026-10-24-04), before any H1 full run**
  (`configs/h1.yaml` unchanged): the rank-r low-rank SVD mesh is `M = U2[:, :r] diag(a) U1[:r, :]`. U1 is
  a Reck-style triangle of `r N - r (r + 1) / 2` adjacent-pair MZIs that sets the first r rows; U2 is the
  same list reversed and sets the first r columns; `a` comes from r attenuator MZIs, each coupling one mode
  to a dump port (amplitude = element [0, 0] of the two-coupler MZI). Its loss uses the same worst per-mode
  MZI-count proxy as the other meshes, with no crossings; "depth" for budget selection is the rank. Fabrication
  draws for U1, the attenuators and U2 use torch seeds `t`, `t + 7e6` and `t + 14e6` (`t = f * 1000 + s`).
  On a unitary target its fidelity is at most (r / N)^2. MNIST-PCA16: PCA fit on the 60,000 training
  images only (train mean removed, top-16 covariance eigenvectors, sign fixed), each sample scaled to unit
  L2 norm and fed to modes 0-15; logits = `10 * exp(g) * |M x|^2` on output modes 0-9 with one trained
  electronic gain g. Fit: full-batch cross-entropy on the training split, Adam as in the config, the restart
  with the lowest final training loss is kept (test data never selects). Score = test accuracy on the 10,000
  test images, mean over the fabrication draws. The task runs only at N = 16 (16 features); at N = 8 it is
  recorded as not applicable. Verdict: "not decided" if any registered family or task is missing or a family
  has no configuration within budget; otherwise the registered rule above.

## H2: dimension-factorized (Kronecker / tensor-train) layers

- **Hypothesis**: Kronecker-3 and tensor-train FFN layers deliver more useful MACs per optical/electrical
  conversion than a dense layer with the same parameter count, without losing accuracy.
- **Task**: character LM (Tiny Shakespeare, the `awen_models.efficiency` harness), stored parameters
  matched to the dense baseline within ±3%.
- **Metrics**: validation loss (nats/char); conversions per useful MAC, counted analytically from the
  mapping in `configs/h2.yaml` (space ⊗ polarization ⊗ wavelength; one DAC per modulated input and one ADC
  per detected output); batch-1 forward latency, measured with one run on the CPU at a time.
- **Candidates**: Kronecker-3 (pilot code in #128) and the existing 2-core tensor-train FFN
  (`awen_models.efficiency`, variant `tensor-train`). Monarch, which ran in the H2 pilot, is **not** an H2
  candidate: v2 places butterfly/Monarch under H1, and H1 tests it at the mesh level. A Monarch-FFN language-model
  comparison is not registered this round.
- **Seeds**: 101, 102, 103 (n = 3 paired seeds).
- **Falsified if** a candidate's validation loss is worse than the dense baseline by more than the
  paired-seed interval, **or** its conversions per useful MAC do not improve on dense. Also killed under v2
  section 7 if it wins on parameters but loses more than 20% on latency.

## H3: self-similar (Cantor) channel plans

- **Hypothesis**: a Cantor-stack filter bank gives a multi-scale WDM channel plan with lower adjacent-channel
  crosstalk and higher channel density than a uniform-grid filter bank at equal footprint.
- **Method**: exact TMM (lossless, normal incidence), indices n = 2.3 / 1.38 (free parameters), equal
  footprint = equal total physical thickness and layer count, 5 layer-thickness perturbation seeds
  (sigma in config; seeds 101-105).
- **Metrics**: number of usable channels (passband T >= 0.5, isolation >= the config threshold), worst
  adjacent-channel crosstalk (dB), channels per normalized-frequency unit.
- **Falsified if** the Cantor plan's crosstalk and channel density are worse than the uniform grid at equal
  footprint (as in v2: worse on both kills it). Better on one and worse on the other is reported as
  **not kept**.

## H4: multi-scale (Fourier / wavelet) layers

- **Hypothesis**: spectral (FNO) and Haar-wavelet layers beat dense baselines at matched compute (FLOPs per
  sample) on tasks with multi-scale structure.
- **Tasks (3)**: 1-D Burgers PDE surrogate (data generated by a pseudo-spectral solver, config fixed); multi-scale
  1-D signal denoising (sum of chirps + noise, generator fixed); 28x28 MNIST classification.
- **Metric**: relative L2 test error (Burgers, denoising), test accuracy (MNIST), at matched FLOPs (±5%).
- **Seeds**: 101, 102, 103 (n = 3 paired seeds).
- **Falsified if**, on any task, neither the FNO nor the wavelet model beats the dense baseline at matched compute.

## Out of scope here

The multiplexing-aware lowering pass (v2 section 10) is separate work, and H1-H4 do not depend on it.
Self-similar structure can save components, loss and energy, but it cannot raise the stored-knowledge bound
(v2 section 10, explicit limit). No result here is evidence about that.
