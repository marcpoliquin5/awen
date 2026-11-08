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

## D-022 · CUDA lockfile targets cu129, not cu128
Decided by: agent, 2026-10-29 (within the brief's "cu128 or newer" requirement).
The first CUDA lockfile (cu128 index, torch 2.11.0+cu128) failed the required gate's dependency review:
torch 2.11.0 requires `setuptools<82`, and advisory GHSA-h35f-9h28-mq5c is fixed only in setuptools 83.
The cu129 index offers torch 2.13.0+cu129, which allows setuptools 84.2.1, so `requirements/cu129-py312.txt`
replaces the cu128 plan (#183). Rejected: overriding torch's setuptools metadata on cu128 (a pin that
contradicts torch's declared requirement), and cu130 (torch 2.14.1, but needs a CUDA 13 driver). The laptop
driver must support CUDA 12.9. Reversible: regenerate with another backend in `scripts/lock_cu129.sh`.

## D-023 · AWEN-Photon sim-to-real control: model B definition (E-022)
Decided by: agent, 2026-11-01 (registered before any run).
The second, independent noise model for E-005 uses only sourced values from `hardware_assumptions.yaml` v0
(system ENOB 7.61 and weight DAC 8 bits from hua2025, TFLN extinction ratio 30 dB from wang2018), a lumped
output-referred Gaussian term instead of photon counting, per-tile weight scaling and NumPy's PCG64 RNG; see
`research/experiments/awen-photon-sim2real/PREREGISTRATION.md`. Rejected: re-parameterizing the Rust model
(shares code and noise structure with the training model), and using unsourced values such as RIN or MZI
crosstalk. Reversible: a new registration.

## D-024 · AWEN-Photon recalibration-interval curve: drift model (E-023)
Decided by: agent, 2026-11-02 (registered before any run).
Weight drift between calibrations is an independent Gaussian random walk on the programmed weights in normalized
units, reset by each recalibration, with the rms offset per interval (drift budget) swept as a free parameter;
results are reported against the budget, not hours, because the assumption set has no sourced drift rate. Evaluated
under model B (D-023) at its sourced point; see `research/experiments/awen-photon-recalibration/PREREGISTRATION.md`.
Rejected: picking a drift rate in rad/h without a source (would make the hours axis an invented number), and drifting
model A (it is the training model, so the result would not be independent). Reversible: a new registration.


## D-025 · AWEN-Photon conversions-per-token curve: counting rule and per-token definition (E-024)
Decided by: agent, 2026-11-02 (registered before any run).
Conversions per token are the capability report's totals for one no-cache call over P positions divided by P, under
the simulator's counting rule (input DAC per row and inner element, weights programmed once per call, one ADC read per
row, output and tile), swept over tile size and P for the tiny Llama and Qwen3 models; the registered check is exact
agreement with a closed form from the model config. See `research/experiments/awen-photon-conversions/PREREGISTRATION.md`.
Rejected: a weight-stationary device rule that amortizes weight programming across calls (no sourced reprogramming
model), and adding energy now (needs full system accounting). Reversible: a new registration.

## D-026 · AWEN-Photon energy-per-token accounting: terms and rules (E-025)
Decided by: agent, 2026-11-03 (registered before any run).
The E-024 per-token counts are paired only with sourced per-event values from `awen-spec/hardware_assumptions.yaml`;
null or unsourced values, power terms (no time model) and terms with no parameter stay "not sourced", and no total is
emitted while any term is not sourced. ADC uses each published design separately; the shot-noise floor is optics only.
See `research/experiments/awen-photon-energy-accounting/PREREGISTRATION.md`.
Rejected: deriving per-MAC energy from the PACE whole-system TOPS/W (another device, not composable with these
counts), and interpolating ADC energy between designs (the assumption set forbids it). Reversible: a new registration.

## D-027 · AWEN-Photon energy-per-token accounting with published DAC designs (E-026)
Decided by: agent, 2026-11-03 (registered before any run).
`dac.energy_per_conversion` stays null; three measured standalone DACs (sedighi2012, palmers2010, kim2016) are added as
separate points in `dac.published_designs` (power / sample rate from the title or abstract), and E-026 reads them for
the input and weight DAC terms, one contribution per design. E-025 stays reproducible as the default registration.
See `research/experiments/awen-photon-energy-accounting-dac/PREREGISTRATION.md`.
Rejected: a single low/typical/high DAC value (the designs differ in bits, rate, process and load, so a range would
imply an interpolation the data does not support), and DAC figures from vendor datasheets (vendor claims, lower
evidence than measured peer-reviewed chips). Reversible: a new registration.

## D-028 · AWEN-Photon energy-per-token accounting with a published measured modulator (E-027)
Decided by: agent, 2026-11-04 (registered before any run).
`modulator.energy_per_symbol` stays null; one measured peer-reviewed silicon microdisk modulator (timurdogan2014) is
added as two separate points in `modulator.published_designs` (0.91 fJ per NRZ bit and 3.65 fJ per full-swing
transition, both from the paper's TDR measurement), and E-027 reads them for the modulator term, driven once per input
DAC conversion, one contribution per point. E-025 and E-026 stay reproducible as their registrations.
See `research/experiments/awen-photon-energy-accounting-modulator/PREREGISTRATION.md`.
Rejected: a thin-film lithium niobate MZM figure from the existing `wang2018` source (it reports drive voltage and
bandwidth, not a measured energy per symbol, so an energy would be an AWEN derivation with an assumed load), a single
low/typical/high modulator value (one device, two bases, binary only), and including driver energy from other papers
(a different device and setup). Reversible: a new registration.

## D-029 · Host-transfer bytes per token counted from the conversion counts (E-028)
Decided by: agent, 2026-11-04 (registered before any run).
The host-transfer term now counts bytes per token: one code per input DAC conversion, weight DAC conversion and ADC
read crosses the host boundary once, packed at 1 or 2 bytes per code, reported separately. `system.host_transfer_energy`
stays null, so the term stays "not sourced" for energy and there is still no total. E-025 to E-027 stay reproducible
as their registrations. See `research/experiments/awen-photon-host-transfer-bytes/PREREGISTRATION.md`.
Rejected: filling a host-link energy per byte without a checked source; counting the simulator's float64 NumPy
buffers (an artifact of the simulator, not a device interface); counting memory movement in the same step (needs a
memory model of where weights and activations live, a separate concern). Reversible: a new registration.

## D-030 · Memory-movement bytes per token counted under two placements (E-029)
Decided by: agent, 2026-11-05 (registered before any run).
The memory-movement term now counts host-memory bytes per token for the lowered GEMMs under two placements reported
side by side: weights streamed from host memory every compiled call, and weights resident on the device after
programming. Inputs are read once and ADC reads written once in both, at 1 or 2 bytes per element. The assumption
set gets no memory parameter, so the term stays "not sourced" for energy and there is still no total. E-025 to E-028
stay reproducible as their registrations. See `research/experiments/awen-photon-memory-movement-bytes/PREREGISTRATION.md`.
Rejected: picking one placement as the default (a design choice with hardware consequences, not settled by the
simulator); counting host-side ops, KV cache or accumulator re-reads (needs a model of the host graph, a separate
concern); filling a DRAM energy per byte without a checked source. Reversible: a new registration.


## D-031 · Memory-movement energy from a published HBM2 access point (E-030)
Decided by: agent, 2026-11-05 (registered before any run).
`memory.published_designs` adds one peer-reviewed HBM2 access-energy point (O'Connor et al., MICRO-50 2017,
doi:10.1145/3123939.3124545: approximately 3.97 pJ/bit, Section 1 / Fig. 1b), and E-030 reads it for the
memory-movement term on top of the E-029 byte counts: energy = bytes x 8 bits/byte x energy_per_bit, reported per
published point, per placement and per byte width. `system.host_transfer_energy` stays null (no fitted host-link-only
source). E-025 to E-029 stay reproducible as their registrations; e029 still does not apply memory energy. See
`research/experiments/awen-photon-memory-energy/PREREGISTRATION.md`.
Rejected: filling host-transfer energy from a PCIe PHY-only or mixed host+device figure; treating the HBM2 number as
a host-link or photonic-memory claim; inventing low/typical/high without a source; running E-030 in the same slice as
registration. Reversible: a new registration.

## D-032 · Weight-hold energy from harris2014 Pπ·τ (E-031)
Decided by: agent, 2026-11-06 (registered before any run).
`phase_shifter.thermo_optic_power_per_pi` and `thermo_optic_reconfiguration_time` are filled from Harris et al.,
Optics Express 22, 10487 (2014) / arXiv:1410.3616 (Pπ 24.77 ± 0.43 mW measured; τ 2.69 µs from measured
130.0 kHz bandwidth via the paper's τ = 0.35/f_3dB). `phase_shifter.published_designs` adds one estimate point
(Table I Pπ·τ = 66.9 pJ), and E-031 reads it for the weight-hold term: energy = weight DAC conversions ×
energy_per_pi, one contribution per published point. Continuous hold during compute is not included. E-025 to
E-030 stay reproducible as their registrations; e030 still reports weight_hold as not sourced (power term). See
`research/experiments/awen-photon-weight-hold-energy/PREREGISTRATION.md`.
Rejected: inventing a hold-duration per token without a source; treating Pπ alone as composable energy; filling
low/typical/high for a single energy without the published-designs separation used for DAC/modulator/memory;
running E-031 in the same slice as registration. Reversible: a new registration.

## D-033 · Laser energy from diamantopoulos2021 DML DC energy-per-bit (E-032)
Decided by: agent, 2026-11-06 (registered before any run).
`laser.wall_plug_efficiency` is filled from Diamantopoulos et al., Photonics 8, 31 (2021) /
doi:10.3390/photonics8020031 (measured 6.44% at 25 °C / 9.1 mA). `laser.published_designs` adds two estimate
points (DC energy-per-bit 0.17 pJ/bit at 25 °C and 0.34 pJ/bit at 50 °C, from measured DC power / net rate;
RF driver excluded), and E-032 reads them for the laser term: energy = input DAC conversions × energy_per_bit,
one contribution per published point, under a binary optical-input proxy (one bit per input DAC conversion).
This is an interconnect-style DML point, not a CW external-laser power model; it must not be summed with
`modulator.published_designs` in one architecture total. E-025 to E-031 stay reproducible as their registrations;
e031 still reports laser as not sourced (power term). See
`research/experiments/awen-photon-laser-energy/PREREGISTRATION.md`.
Rejected: deriving laser watts from PACE including-vs-excluding-lasers system SoA; inventing a CW hold-duration
per token; mixing vaskasi2022 WPE with hua2025 optical power as one operating point; running E-032 in the same
slice as registration. Reversible: a new registration.

## D-034 · Thermal-stabilization energy from zortman2012 heater energy-per-bit (E-033)
Decided by: agent, 2026-11-06 (registered before any run).
`thermal.heater_efficiency` is filled from Zortman et al., OFC/NFOEC 2012 paper OW4I.5 /
doi:10.1364/ofc.2012.ow4i.5 (measured 7 µW/GHz). `thermal.published_designs` adds one estimate point
(180 fJ/bit at 10 Gbps for a stated manufacturing-plus-thermal budget; heater only), and E-033 reads it for
the thermal_stabilization term: energy = input DAC conversions × energy_per_bit, one contribution per
published point, under a binary optical-input proxy. `system.thermal_stabilization_power` stays null (no
continuous Watt / time model). This is on-chip resonant-heater trimming, not a package TEC and not
control-ASIC logic; it must not be summed with `modulator.published_designs` athermal EO compensation in one
architecture total. E-025 to E-032 stay reproducible as their registrations; e032 still reports
thermal_stabilization as not sourced (power term). See
`research/experiments/awen-photon-thermal-energy/PREREGISTRATION.md`.
Rejected: inventing a continuous TEC hold-duration per token; treating Sun et al. JSSC 2016 0.74 mW tuner
logic as this slice's thermal term (control electronics, deferred); filling thermal_stabilization_power in
Watts without a time model; running E-033 in the same slice as registration. Reversible: a new registration.

## D-035 · Control-electronics energy from sun2016 microring tuner-logic energy-per-bit (E-034)
Decided by: agent, 2026-11-07 (registered before any run).
`control.tuner_logic_power` is filled from Sun et al., IEEE JSSC 51(4), 893-907 (2016) /
doi:10.1109/JSSC.2016.2519390 (measured 0.74 mW tuner logic, from the abstract). `control.published_designs`
adds one estimate point (0.74 mW / 5 Gb/s = 148 fJ/bit; logic only), and E-034 reads it for the
control_electronics term: energy = input DAC conversions × energy_per_bit, one contribution per published
point, under a binary optical-input proxy. `system.control_electronics_power` stays null (no whole-system
Watt / time model). The first author's open MIT thesis (hdl:1721.1/99784, Fig. 6-16: backend 152 fJ/bit,
frontend 39 fJ/bit at 5 Gb/s) is recorded as a cross-check only. This is per-ring wavelength-lock logic, not a
system controller, sequencer, clocking or host interface; the heater stays the thermal term (zortman2012).
E-025 to E-033 stay reproducible as their registrations; e033 still reports control_electronics as not sourced
(power term). See `research/experiments/awen-photon-control-energy/PREREGISTRATION.md`.
Rejected: using the thesis per-bit values as the point (doctoral thesis, not peer-reviewed; no DOI); using the
VLSI 2015 abstract's 275 fJ/bit (no breakdown, basis unclear); adding the Sun heater 290 fJ/bit (would double
the thermal term); filling control_electronics_power in Watts without a time model; running E-034 in the same
slice as registration. Reversible: a new registration.

## D-036 · Host-transfer energy from li2018 PCIe Gen4 PHY transceiver energy-per-bit (E-035)
Decided by: agent, 2026-11-07 (registered before any run).
`host_link.published_designs` adds one measured point from Li et al., 2018 IEEE A-SSCC pp. 5-8 /
doi:10.1109/ASSCC.2018.8579314 (abstract: Gen4 PCIe transceiver with 3-tap Tx EQ and 8-tap Rx DFE in 10 nm
FinFET; power efficiency 7.5 pJ/b). Packing conversion 7.5 × 8 = 60.0 pJ/byte is stored on the point and
labelled. E-035 reads it for the host_transfer term: energy = counted host bytes × 8 × energy_per_bit, one
contribution per published point and packing width (1 or 2 bytes per code). `system.host_transfer_energy` stays
null so e028–e034 stay reproducible. This is one-end host-side PHY TX+RX, not TX-only, not device DRAM, not a
whole-system host+device mix. E-025 to E-034 stay reproducible as their registrations; e034 still reports
host_transfer as not sourced. See `research/experiments/awen-photon-host-transfer-energy/PREREGISTRATION.md`.
Rejected: Bae 2021 TX-only 1.93 pJ/bit; CICC 2020 11.4 pJ/bit SerDes (prior non-fit); Zhao Perlmutter 167.9
pJ/bit (host+device interfaces); Jalali ISSCC 2018 6 pJ/b multi-standard (not PCIe-labelled); Zhao NIC+PCIe
4.1 pJ/bit; Analog Bits SemiWiki vendor 6 pJ/bit; running E-035 in the same slice as registration; filling the
scalar (would change e028–e034). Reversible: a new registration.

## D-037 · Scenario-envelope energy-per-token total (E-036)
Decided by: agent, 2026-11-07 (registered before any run).
E-035 sourced all 11 terms but left `total_J = "not defined by this registration"`. E-036 uses the same term set
(`TERMS_E036 = TERMS_E035`) and accounting version `awen.energy-per-token-accounting.v12`. When any term is not
sourced the total stays unset (`None`), as today. When every term is sourced under e036 only, `total_J` is a
structured `scenario_envelope_v1` object: min/max labelled sums within one transmitter model, with a disclosure
that this is not one physical device and not a speedup or savings claim.
Two transmitter models keep the laser caveat: `dml_laser` includes laser and excludes modulator;
`modulator_eo` includes modulator and excludes laser. Thermal stabilization is included in both, because the
registered modulator points are EO switching energy, not athermal EO compensation (the thermal caveat forbids
only the latter pairing). Each scenario uses one published DAC source id for both input and weight DACs, then
the cartesian product of published ADC designs, detection-floor effective-bits keys, weight-hold, control,
thermal, memory placement×width, host packing width, and the transmitter-specific laser or modulator point.
e025–e035 stay reproducible as their registrations (string total or `None`). See
`research/experiments/awen-photon-energy-total/PREREGISTRATION.md`.
Rejected: a single bare-float architecture total (would mix laser with modulator against D-033); omitting
thermal from `modulator_eo` (the registered modulator points are not athermal compensation); mixing DAC
source ids across input and weight in one scenario; changing e025–e035 totals; running E-036 in the same
slice as registration. Reversible: a new registration.

## D-038 · System energy per lowered MAC from the E-036 envelopes (E-037)
Decided by: agent, 2026-11-08 (registered before any run).
The energy-per-MAC curve (Master Prompt v2 section 7) is defined as the E-036 `scenario_envelope_v1` bounds per
transmitter model divided by E-024 lowered MACs per token (`counts.lowered_macs / positions`, exact integer).
The numerator is unchanged (same 11 terms and published points); the optical detection floor is reported per
MAC beside it as a labelled optics-only component. Inputs are pinned by SHA-256. No accuracy axis: the E-024
graphs are random-init. Module `awen_models.energy_per_mac`, curve `awen.energy-per-mac-curve.v1`. See
`research/experiments/awen-photon-energy-per-mac/PREREGISTRATION.md`.
Rejected: dividing by model MACs including host fallbacks (the numerator has no host-compute term, so the
ratio would understate energy per computed MAC); reporting the detection floor as "optical energy per MAC"
alone (optics-only figures may not stand in for system numbers, section 9); a single bare-float per-MAC figure
(would undo the D-037 transmitter split); running E-037 in the same slice as registration. Reversible: a new
registration.

## D-039 · Accuracy axis for the energy-per-MAC curve on AWEN-Mini (E-038)
Decided by: agent, 2026-11-08 (registered before any run).
The accuracy axis uses the bundled, trained AWEN-Mini character model (E-001) because it is a trained language model whose
runtime capability report gives both an accuracy metric and simulator lowering counts. Accuracy is held-out next-character NLL
from the capability report on 16 validation windows (seed 2024), under `ideal` and the simulator default
contract with `dac_bits = adc_bits` of 4, 6 and 8, 3 noise seeds. Energy reuses the E-036 accounting
(`scenario_envelope_v1`) on AWEN-Mini counts and the E-037 per-lowered-MAC division, unchanged. Checkpoint,
assumption set and corpus are pinned by SHA-256. Module `awen_models.accuracy_energy_mac`, curve
`awen.accuracy-energy-per-mac-curve.v1`. See `research/experiments/awen-photon-accuracy-energy-per-mac/PREREGISTRATION.md`.
Rejected: the MNIST photon-training MLP (its stored curves pair accuracy with modeled photon energy only, and it
is not a language model with per-token counts); the random-init E-024 graphs (no meaningful accuracy); downloading an open model with real
weights (waits on Marc); scaling DAC/ADC energy with the bits key (the published points are fixed designs and
there is no sourced scaling law in the assumption set); charging host-fallback MACs (no host-compute term);
running E-038 in the same slice as registration. Reversible: a new registration.
