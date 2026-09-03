# CLI Repository Ownership

The `a3s` CLI is owned by the standalone
[`A3S-Lab/CLI`](https://github.com/A3S-Lab/CLI) repository. The A3S monorepo
mounts one reviewed CLI revision at `crates/cli`; its root is orchestration and
does not contain another Rust package.

## History

The CLI was originally integrated as a submodule. Version 0.11.0 temporarily
moved the package into the `A3S-Lab/a3s` root in commit `d9261341`, and versions
0.11.0 and 0.11.1 used the monorepo release endpoint. This made source, CI,
documentation, tags, and releases span two ownership models.

The standalone repository is canonical again. It owns:

- `Cargo.toml`, `Cargo.lock`, `build.rs`, `src/`, `tests/`, and `skills/`;
- CLI pull requests and CI;
- stable `vMAJOR.MINOR.PATCH` tags and GitHub releases;
- the `a3s` crates.io package and Homebrew formula updates; and
- CLI product, architecture, and command documentation.

The monorepo owns only the exact `crates/cli` gitlink, cross-project integration
scripts, the Web application, the documentation site, and installer entrypoint
copies. A CLI change must merge in `A3S-Lab/CLI` before the monorepo advances
its gitlink.

## Release Compatibility

Current installers and CLI self-update resolve releases from `A3S-Lab/CLI`.
Versions 0.11.0 and 0.11.1 still query `A3S-Lab/a3s`, so the monorepo retains a
manual `relay-cli-release.yml` workflow. It accepts only a published stable CLI
tag, verifies the exact platform asset set, GitHub digests, and checksum
manifests, then publishes byte-identical GitHub assets.

The relay does not build the CLI, publish crates.io, or update Homebrew. Its
release notes record the canonical source tag, commit, and release identity so
an existing relay can be verified and resumed safely.

The current coordinated release is `v0.14.0`. Its five target archives carry
the matching Moli 1.1.1 headless-browser sidecar, while source and Cargo
installs use the same digest-verified per-user cache with cross-process locking.
This keeps Code Core 8.1.0 and Search 3.1.0 on one release graph and avoids a
separate browser download for each CLI installation.

## Documentation

Use the [pinned CLI reference](../crates/cli/docs/cli-reference.md) when
validating this integration snapshot. Use the
[`A3S-Lab/CLI` main branch](https://github.com/A3S-Lab/CLI/tree/main/docs) for
the latest CLI design documents.
