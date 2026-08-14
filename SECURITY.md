# Security policy

## Supported versions

AWEN has no supported product release. Security fixes are applied to the
default branch. Historical tags and development snapshots receive no fixes.

## Reporting a vulnerability

Do not open a public issue. Use GitHub's private vulnerability-reporting form:

https://github.com/marcpoliquin5/awen/security/advisories/new

Include affected commits, impact, reproduction steps, and any suggested fix.
The maintainer will acknowledge a complete report within three business days,
provide an initial assessment within seven days, and coordinate disclosure
after a fix is available. These are response targets, not a warranty.

## Scope

Reports may cover compiler/runtime input validation, plugin loading and
signatures, path handling, artifact integrity, CI/release provenance, exposed
credentials, or unsafe FFI behavior. Public browser configuration is not a
secret, but authorization must never rely on obscuring it; backend policy and
row-level security remain required.

Never include live credentials, proprietary PDK data, personal data, or exploit
payloads in public issues, pull requests, fixtures, logs, or artifacts.
