# AWEN research

Research material kept **separate from the AWEN core packages**. Nothing in
`awen-*` imports from here, and nothing here supports product claims.

- `ledger/`: the research ledger, with the JSON schema `ledger.schema.json`
  (`awen.research-ledger.v1`), the v2 field list `schema.yaml`, the ledger index
  `ledger.json` (empty until Marc's export is committed) and `import_ledger.py`
  (CSV or Markdown table import).
- `frontier-physics/`: long-range hypotheses H1–H3, each with its hypothesis,
  math, assumptions, observables, method, comparison to established theory and
  falsification conditions, plus `frontier_checks.py`, which reproduces the
  textbook results the hypotheses start from.
- `tests/`: `python -m pytest research/tests -q` (runs in CI).
- [`docs/RESEARCH_PIPELINE.md`](../docs/RESEARCH_PIPELINE.md): how a ledger
  entry becomes code.

## The 72-paper historical ledger is not available

The original 72-paper research ledger file has not been provided to this
repository, so none of its entries are reproduced here and none are
paraphrased from memory. When the file is supplied, import it with:

```bash
python research/ledger/import_ledger.py ledger-72.csv --dry-run   # report only
python research/ledger/import_ledger.py ledger-72.csv
```

The importer marks a row `verified` only when it has a DOI or https URL, a
title, authors and a year, **and** a recorded verification method and date.
Every other row is imported as `citation pending`, whatever its status column
says. Existing entries are never overwritten unless `--replace` is given.

## Agent-sourced reference list (not the ledger)

`reference/agent-sourced.json` is a list chosen by the coding agent. It is
**not** Marc's ledger. It covers 15 directions in photonic and physical AI. Each
cites one paper whose title, authors and year were checked against the
Crossref record for its DOI on 2026-10-18 (status `verified`). The summaries
and AWEN mappings are this project's own reading of each direction, not
quotations, and they carry no numerical results from the papers. Each entry's
`awen_mapping` records whether AWEN already implements the idea (`implemented`
/ `partial` / `not-started`) and the next concrete step.

## Rules

- No result, number or citation goes in without a checked source.
- Frontier hypotheses must state how they could be falsified.
- No free-energy, perpetual-motion or unsupported quantum-advantage claims.
  H1 includes a check that the Casimir force does zero net work around a
  closed cycle.
