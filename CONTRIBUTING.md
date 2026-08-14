# Contributing to AWEN

AWEN is an experimental compiler/runtime project. Contributions must keep the
implemented surface, tests, documentation, and public claims in agreement.

## Before opening a change

1. Search the issue tracker and open an issue for behavior or architecture that
   is not already approved.
2. Use an AEP for changes to a normative schema, IR, ABI, plugin contract, or
   compatibility guarantee.
3. Do not commit generated binaries, run outputs, credentials, proprietary PDK
   data, or benchmark claims without immutable end-to-end evidence.

## Local verification

Run the commands in `README.md` from a clean checkout. The pull request check
named `AWEN required quality gate` is the authoritative merge gate. It covers
formatting, linting, Rust and Python tests, MLIR build/tests, schemas,
repository policy, dependency vulnerabilities, dependency licenses, and secret
scanning.

## Pull requests

- Keep one coherent change per pull request and link its issue or AEP.
- Add or update tests for observable behavior.
- Update schemas, fixtures, compatibility notes, and docs together.
- State which values are measured, simulated, estimated, or assumed.
- Never describe an unfinished structure, simulator result, or cost-model input as a shipped
  product or measured accelerator result.
- Add a changelog entry for user-visible behavior.

All contributions are licensed under the repository's MIT License.
