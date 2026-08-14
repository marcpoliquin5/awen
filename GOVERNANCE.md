# AWEN governance

## Roles

- **Maintainer:** reviews and merges changes, administers releases and security
  reports, owns compatibility policy, and appoints or removes maintainers.
- **Contributor:** proposes and implements changes and participates in review.
- **AEP author:** carries a normative proposal from motivation through
  compatibility analysis, implementation, and conformance evidence.

Current maintainers and ownership areas are recorded in `MAINTAINERS.md` and
`.github/CODEOWNERS`.

## Decisions

Routine changes use pull-request review and the protected
`AWEN required quality gate`. Normative changes require an AEP. The maintainer
records the decision in the pull request or AEP and must identify compatibility,
security, and evidence consequences. When consensus is unavailable, the
maintainer decides and documents the rationale; there is no unrecorded voting
or private standards process.

## Releases

Only the procedure in `docs/RELEASING.md` may create a release. A milestone,
phase label, tag, or merged feature is not a release. Release support and
security status are stated in `SECURITY.md`.

## Changes to governance

Governance changes use a pull request, require the same protected check as code,
and must name the affected roles and transition plan. Maintainer changes update
`MAINTAINERS.md` and `CODEOWNERS` in the same commit.
