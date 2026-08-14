# Verified release procedure

AWEN currently has no supported product release. A future release must satisfy
every requirement below.

## Required evidence

1. The release tag is annotated, uses `vMAJOR.MINOR.PATCH`, and points to a
   commit reachable from `main`.
2. The exact commit has a completed, successful run of
   `.github/workflows/observability-quality-gate.yml`; reruns on another commit
   do not count.
3. The release workflow records the immutable commit SHA, quality-gate run ID
   and URL, source-archive SHA-256, toolchain versions, and artifact checksums.
4. `CHANGELOG.md`, compatibility notes, schemas, package versions, and security
   support are consistent with the tag.
5. Any performance statement links to a verified immutable HIL artifact and
   names its complete system boundary. Simulated, estimated, assumed, or vendor
   values cannot become measured product claims.

## Publication

Run the manual `Verified draft release` workflow with the annotated tag. The
workflow refuses a tag without exact green evidence and creates a draft release
whose generated notes include that evidence. A maintainer compares the draft
with this checklist before publication. Draft creation never changes the
protected quality-gate result.

Milestones, phase documents, local test logs, and branch checks are not release
evidence.
