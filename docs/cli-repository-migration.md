# CLI Repository Migration

The `a3s` CLI is owned directly by the `A3S-Lab/a3s` repository root as of
version 0.11.0. It is no longer a `crates/cli` submodule.

## Provenance

- Final standalone source tag: `v0.10.14`
- Final standalone source commit:
  `58f1bc6565f393ab693d4babebf5671978b560d6`
- Preserved history in this repository: branch `archive/a3s-cli`
- Preserved final source tag in this repository:
  `cli-legacy/v0.10.14`

The root `Cargo.toml`, `src/`, `tests/`, `skills/`, and CLI documents were
imported from that immutable source commit. The monorepo README and shared
installer files remain the canonical integration documentation.

## Release Ownership

Stable CLI tags retain the `vMAJOR.MINOR.PATCH` form. The
`.github/workflows/a3s-cli-release.yml` workflow builds and verifies the CLI,
matching Web workspace, native binaries, crates.io package, GitHub release,
and Homebrew formula from a tag reachable from `A3S-Lab/a3s` `main`.

Standalone installers, matching Web downloads, and self-update all resolve
published stable CLI releases from `A3S-Lab/a3s`. They deliberately ignore
component release tags such as `a3s-code-vX.Y.Z`.
