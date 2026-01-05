# AWEN Spec Sections

## Existing Sections


## AWEN Photonics — PHOTONICS-V5

  - Canonical IR schema: [awen-spec/schemas/photonic_ir.v5.json](awen-spec/schemas/photonic_ir.v5.json)
  - Runtime chokepoint interface (to be added under `awen-runtime`)
  - Conformance tests (to be added under `awen-runtime/tests/photonic_conformance`)

Notes:

## Milestones

- **Enforce Rust Quality Gate & Add Copilot Super-Prompt**

  - **PR:** [#1](https://github.com/marcpoliquin5/awen/pull/1) — added `awen/docs/COPILOT_SUPER_PROMPT.md` and CI fixes.
  - **Commit SHA:** f0edeeefb473d75cf69d8fa39c958657566aa372 (merged into `main` on 2026-01-05).
  - **Verification:** CI green for the merged PR; `awen-runtime` artifacts uploaded (artifact id reported in CI run: 5030575133).
  - **Notes:** Work included adding `working-directory: ./awen-runtime` to relevant GitHub Actions steps so crate-level checks (rustfmt --check, clippy -D, cargo test including trybuild) run correctly.
