# AWEN-Photon energy-per-token accounting with published host-link PHY energy-per-bit (E-035): pre-registration

Registered 2026-11-07 (commit of this file) under AWEN Master Prompt v2. Status: **registered, not run.**
A change after the run starts is a new registration with its own date, and the change and its reason go in this file.

## Question

E-034 left the host-transfer term "not sourced" because `system.host_transfer_energy` is null (bytes are
counted under e028+, but energy per byte was not). With one published host-side PCIe Gen4 PHY transceiver
energy-per-bit point now in the assumption set (`host_link.published_designs`, li2018, 7.5 pJ/bit → 60 pJ/byte
via × 8), what is the per-token host-transfer contribution for the stored E-024 counts under the registered
host byte rule, and does any term stay not sourced?

## Disclosure

- The E-024 counts and the E-025 to E-034 results were seen before this registration. Host transfer was chosen
  because STATUS next-task 1 named it as the only remaining unsourced energy term.
- Source checked on 2026-11-07: Li et al., 2018 IEEE A-SSCC, pp. 5–8, doi:10.1109/ASSCC.2018.8579314, metadata
  on Crossref and the published abstract via Semantic Scholar (full text is closed access; OpenAlex reports no
  open copy). The abstract states a 2.5–16 Gbps Gen4 PCIe transceiver with 3-tap Tx EQ and 8-tap Rx DFE in
  10 nm FinFET CMOS, with power efficiency **7.5 pJ/b**.
- The registered point is **measured** (chip power efficiency as stated in the abstract). Packing conversion to
  per-byte is labelled: 7.5 pJ/bit × 8 bits/byte = **60.0 pJ/byte**.
- Rejected candidates (kept out of the point):
  - 2026-11-05: Bae et al. Electronics 2021 ~1.93 pJ/bit PCIe Gen4 PHY **TX-only** (fails TX+RX / host-link fit).
  - 2026-11-05: CICC 2020 ~11.4 pJ/bit Gen5 SerDes (previously judged not a fitted host-link-only term).
  - 2026-11-05: Zhao et al. Perlmutter ~167.9 pJ/bit PCIe (paper states host **and** device PCIe interfaces;
    whole-path mix that STATUS rejected).
  - 2026-11-07: Jalali et al. ISSCC 2018 6 pJ/b multi-standard transceiver (not PCIe-labelled in the title/abstract).
  - 2026-11-07: Zhao NIC+PCIe 4.1±3.2 pJ/bit (host–NIC path, not host–accelerator; large SEM).
  - 2026-11-07: Analog Bits SemiWiki Gen4 6 pJ/bit (vendor blog, not peer-reviewed).
- The registered energy model counts **host bytes × 8 × energy_per_bit** at each packing width (1 or 2 bytes per
  code) under the E-028 host byte rule (one code per input DAC, weight DAC and ADC read). This is **one-end**
  host-side PHY transceiver energy (TX+RX). Protocol layers above the PHY, the far-end device PHY, device DRAM
  (E-030 / oconnor2017), and whole-node GPU uncore wake-up are not included. `system.host_transfer_energy`
  stays null so e028–e034 remain reproducible.
- During development the accounting code ran only in unit tests on synthetic counts and in a structural check
  (e034 still leaves host_transfer not sourced; e035 sources it). No E-035 output was stored or reported.

## Terms and rules

Identical to E-034 (`research/experiments/awen-photon-control-energy/PREREGISTRATION.md`) except:

| Term | Count per token | Parameter | Rule |
|---|---|---|---|
| host_transfer | codes = input DAC + weight DAC + ADC (bytes = codes × 1 or 2) | `host_link.published_designs` | bytes × 8 × each published point (pJ per bit), never interpolated |

`awen_models.energy_accounting.TERMS_E035`; accounting version `awen.energy-per-token-accounting.v11`. The `e025`
(default) to `e034` registrations are unchanged: e034 still treats host_transfer as not sourced (null scalar).

## Design

- Input: the stored E-024 `results.json`, unchanged; no model is compiled or run.
- Command (from `awen-models/`, CPU):
  `python -m awen_models.energy_accounting --registration e035 --counts results/conversions-per-token/results.json --out results/energy-per-token-host-transfer-energy/results.json`
  Output stored with its SHA-256, commit and environment.

## Verdict rule

Same as E-034, plus: at every point the host_transfer term is sourced from each published design, with
per-token joules equal to `bytes_per_token[width] × 8 × energy_per_bit` for each packing width, and `check()`
rejects a contribution that does not recompute or a not-sourced row that still carries designs. **Pass** when
all of this holds at all 40 points; otherwise **fail** with the failing points listed.

Expected: pass, 0 of 11 terms not sourced under e035 on the stored counts, so the registration would report
`total_J = "not defined by this registration"` (a numeric total needs a later registration). Host figures are
L1 counts times a labelled measured PHY efficiency. No speedup, savings or hardware claim follows. This PR
registers only; it does not run E-035.
