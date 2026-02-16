---
name: rust-sdk-ci
description: 'Configure GitHub Actions CI/CD for Rust crates with Python (PyO3) and Node.js (napi-rs) SDK publishing. Use when user asks to set up CI, configure GitHub Actions, add SDK publishing, or automate releases for a Rust crate with multi-platform SDK builds. Supports: (1) CI checks (fmt, clippy, test), (2) crates.io publish, (3) GitHub Release, (4) Python SDK multi-platform wheels via maturin/PyO3 to PyPI, (5) Node SDK multi-platform native modules via napi-rs to npm, (6) Homebrew formula auto-update, (7) Workspace restructuring for submodule-based repos'
license: MIT
allowed-tools: Bash, Read, Write, Edit, Glob, Grep
---

# Rust Crate + SDK CI/CD Pipeline

## Overview

Set up a complete GitHub Actions CI/CD pipeline for a Rust crate that publishes to crates.io, with optional Python SDK (PyO3/maturin) to PyPI and Node.js SDK (napi-rs) to npm. Designed for crates that live as git submodules in a monorepo.

## Architecture

```
.github/
├── setup-workspace.sh          # Restructures standalone repo into workspace for CI
└── workflows/
    ├── ci.yml                  # Push/PR: fmt, clippy, test
    ├── release.yml             # Tag-triggered: orchestrates all publishing
    ├── publish-node.yml        # Reusable: build + publish Node SDK to npm
    └── publish-python.yml      # Reusable: build + publish Python SDK to PyPI

sdk/
├── node/                       # napi-rs Node.js bindings
│   ├── Cargo.toml              # cdylib crate
│   ├── package.json            # Main npm package with optionalDependencies
│   ├── build.rs                # napi_build::setup()
│   ├── src/                    # Rust source
│   └── npm/                    # Per-platform npm packages
│       ├── darwin-arm64/package.json
│       ├── darwin-x64/package.json
│       ├── linux-arm64-gnu/package.json
│       ├── linux-arm64-musl/package.json
│       ├── linux-x64-gnu/package.json
│       ├── linux-x64-musl/package.json
│       └── win32-x64-msvc/package.json
└── python/                     # PyO3 Python bindings
    ├── Cargo.toml              # cdylib crate
    ├── pyproject.toml           # maturin build config
    └── src/                    # Rust source
```

## Required GitHub Secrets

| Secret | Purpose |
|--------|---------|
| `CARGO_TOKEN` | crates.io publish token |
| `NPM_TOKEN` | npm publish token |
| `PYPI_TOKEN` | PyPI publish token |
| `HOMEBREW_TAP_TOKEN` | (Optional) GitHub PAT for pushing to homebrew-tap repo |

Set secrets via:
```bash
# From local credentials
cat ~/.cargo/credentials.toml  # Get token
echo "<token>" | gh secret set CARGO_TOKEN --repo <org>/<repo>

grep authToken ~/.npmrc  # Get token
echo "<token>" | gh secret set NPM_TOKEN --repo <org>/<repo>

grep password ~/.pypirc  # Get token
echo "<token>" | gh secret set PYPI_TOKEN --repo <org>/<repo>

# For Homebrew (use gh auth token or a PAT)
gh auth token | gh secret set HOMEBREW_TAP_TOKEN --repo <org>/<repo>
```

## Workflow Templates

### 1. setup-workspace.sh (for submodule-based repos)

When a crate lives as a submodule in a monorepo, CI needs to reconstruct the workspace structure. This script restructures the repo in-place:

```bash
#!/bin/bash
# Setup a minimal workspace context for building the crate standalone.
# Restructures: ./ = repo root → ./ = workspace root with crates/<name>/
set -euo pipefail

CRATE_NAME="<crate-name>"  # e.g., "lane", "search"

TMPDIR="$(mktemp -d)"
cp -a . "$TMPDIR/$CRATE_NAME"
find . -maxdepth 1 ! -name '.' ! -name '.git' -exec rm -rf {} +
mkdir -p crates
cp -a "$TMPDIR/$CRATE_NAME/." "crates/$CRATE_NAME/"

cat > Cargo.toml << 'EOF'
[workspace]
resolver = "2"
members = ["crates/<crate-name>"]

[workspace.package]
version = "0.1.0"
edition = "2021"
license = "MIT"

[workspace.dependencies]
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "1"
async-trait = "0.1"
EOF

rm -rf "$TMPDIR"
echo "Workspace restructured. Crate at: crates/$CRATE_NAME/"
```

### 2. ci.yml

```yaml
name: CI

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  check:
    name: Check
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - uses: Swatinem/rust-cache@v2
      - name: Format check
        run: cargo fmt -- --check
      - name: Clippy
        run: cargo clippy -- -D warnings
      - name: Tests
        run: cargo test --lib
```

If the crate is a submodule (needs workspace restructuring), add before Rust install:
```yaml
      - name: Setup workspace context
        run: bash .github/setup-workspace.sh
```
And use `-p <crate-name>` for cargo commands:
```yaml
      - run: cargo fmt -p <crate-name> -- --check
      - run: cargo clippy -p <crate-name> -- -D warnings
      - run: cargo test -p <crate-name> --lib
```

### 3. release.yml

```yaml
name: Release

on:
  push:
    tags:
      - "v*"

permissions:
  contents: write

jobs:
  ci:
    name: CI Checks
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Setup workspace context
        run: bash .github/setup-workspace.sh
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          components: rustfmt, clippy
      - name: Format check
        run: cargo fmt -p <crate-name> -- --check
      - name: Clippy
        run: cargo clippy -p <crate-name> -- -D warnings
      - name: Tests
        run: cargo test -p <crate-name> --lib

  publish-crate:
    name: Publish to crates.io
    needs: ci
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - name: Setup workspace context
        run: bash .github/setup-workspace.sh
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
      - name: Publish
        working-directory: crates/<crate-name>
        env:
          CARGO_REGISTRY_TOKEN: ${{ secrets.CARGO_TOKEN }}
        run: cargo publish --allow-dirty

  github-release:
    name: GitHub Release
    needs: ci
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
        with:
          fetch-depth: 0
      - name: Generate release notes
        run: |
          PREV_TAG=$(git tag --sort=-v:refname | grep '^v' | head -2 | tail -1)
          if [ -z "$PREV_TAG" ]; then
            NOTES="Initial release"
          else
            NOTES=$(git log "${PREV_TAG}..HEAD" --oneline --no-merges --pretty=format:"- %s" | head -50)
          fi
          echo "$NOTES" > /tmp/release-notes.md
      - name: Create or update release
        env:
          GH_TOKEN: ${{ secrets.GITHUB_TOKEN }}
        run: |
          if gh release view "$GITHUB_REF_NAME" &>/dev/null; then
            echo "Release already exists"
          else
            gh release create "$GITHUB_REF_NAME" \
              --title "$GITHUB_REF_NAME" \
              --notes-file /tmp/release-notes.md
          fi

  publish-node:
    name: Node SDK
    needs: ci
    uses: ./.github/workflows/publish-node.yml
    secrets: inherit

  publish-python:
    name: Python SDK
    needs: ci
    uses: ./.github/workflows/publish-python.yml
    secrets: inherit
```

### 4. publish-node.yml

Key points:
- 7-platform build matrix (macOS arm64/x64, Linux x64 gnu/musl, Linux arm64 gnu/musl, Windows x64)
- Uses zig for cross-compilation on Linux targets
- Publishes per-platform packages first, then main package
- Working directory uses workspace path: `crates/<crate-name>/sdk/node`

```yaml
name: Publish Node SDK

on:
  workflow_call:
  workflow_dispatch:

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        settings:
          - host: macos-latest
            target: aarch64-apple-darwin
          - host: macos-latest
            target: x86_64-apple-darwin
          - host: ubuntu-latest
            target: x86_64-unknown-linux-gnu
          - host: ubuntu-latest
            target: x86_64-unknown-linux-musl
            zig: true
          - host: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            zig: true
          - host: ubuntu-latest
            target: aarch64-unknown-linux-musl
            zig: true
          - host: windows-latest
            target: x86_64-pc-windows-msvc

    name: Build ${{ matrix.settings.target }}
    runs-on: ${{ matrix.settings.host }}

    steps:
      - uses: actions/checkout@v4
      - name: Setup workspace context
        shell: bash
        run: bash .github/setup-workspace.sh
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.settings.target }}
      - name: Install Zig
        if: matrix.settings.zig
        uses: goto-bus-stop/setup-zig@v2
        with:
          version: 0.13.0
      - name: Install dependencies
        working-directory: crates/<crate-name>/sdk/node
        run: npm install
      - name: Build native module
        working-directory: crates/<crate-name>/sdk/node
        shell: bash
        run: |
          if [ "${{ matrix.settings.zig }}" = "true" ]; then
            npx napi build --platform --release --target ${{ matrix.settings.target }} --zig
          else
            npx napi build --platform --release --target ${{ matrix.settings.target }}
          fi
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: bindings-${{ matrix.settings.target }}
          path: crates/<crate-name>/sdk/node/*.node
          if-no-files-found: error

  publish:
    name: Publish to npm
    runs-on: ubuntu-latest
    needs: build
    steps:
      - uses: actions/checkout@v4
      - name: Setup Node.js
        uses: actions/setup-node@v4
        with:
          node-version: 20
          registry-url: https://registry.npmjs.org
      - name: Install dependencies
        working-directory: sdk/node
        run: npm install
      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          path: sdk/node/artifacts
      - name: Move artifacts
        working-directory: sdk/node
        run: |
          for artifact_dir in artifacts/bindings-*/; do
            for node_file in "$artifact_dir"*.node; do
              filename="$(basename "$node_file")"
              platform="${filename#<napi-name>.}"
              platform="${platform%.node}"
              target_dir="npm/$platform"
              if [ -d "$target_dir" ]; then
                cp "$node_file" "$target_dir/"
                echo "Copied $filename -> $target_dir/"
              else
                echo "Warning: no npm dir for $platform, skipping $filename"
              fi
            done
          done
      - name: Publish platform packages
        working-directory: sdk/node
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: |
          for dir in npm/*/; do
            if [ -f "$dir/package.json" ] && ls "$dir"/*.node 1>/dev/null 2>&1; then
              echo "Publishing $(basename $dir)..."
              (cd "$dir" && npm publish --access public) || true
            fi
          done
      - name: Publish main package
        working-directory: sdk/node
        env:
          NODE_AUTH_TOKEN: ${{ secrets.NPM_TOKEN }}
        run: npm publish --access public --ignore-scripts
```

### 5. publish-python.yml

Key points:
- 7-platform build matrix with manylinux variants
- Builds wheels for Python 3.9–3.13
- Uses PyO3/maturin-action for building
- Manifest path uses workspace path: `crates/<crate-name>/sdk/python/Cargo.toml`

```yaml
name: Publish Python SDK

on:
  workflow_call:
  workflow_dispatch:

jobs:
  build:
    strategy:
      fail-fast: false
      matrix:
        settings:
          - host: macos-latest
            target: aarch64-apple-darwin
          - host: macos-latest
            target: x86_64-apple-darwin
          - host: ubuntu-latest
            target: x86_64-unknown-linux-gnu
            manylinux: auto
          - host: ubuntu-latest
            target: x86_64-unknown-linux-musl
            manylinux: musllinux_1_2
          - host: ubuntu-latest
            target: aarch64-unknown-linux-gnu
            manylinux: 2_28
          - host: ubuntu-latest
            target: aarch64-unknown-linux-musl
            manylinux: musllinux_1_2
          - host: windows-latest
            target: x86_64-pc-windows-msvc

    name: Build ${{ matrix.settings.target }}
    runs-on: ${{ matrix.settings.host }}

    steps:
      - uses: actions/checkout@v4
      - name: Setup workspace context
        shell: bash
        run: bash .github/setup-workspace.sh
      - name: Setup Python
        uses: actions/setup-python@v5
        with:
          python-version: |
            3.9
            3.10
            3.11
            3.12
            3.13
      - name: Install Rust
        uses: dtolnay/rust-toolchain@stable
        with:
          targets: ${{ matrix.settings.target }}
      - name: Build wheels
        uses: PyO3/maturin-action@v1
        with:
          target: ${{ matrix.settings.target }}
          args: --release --out dist --manifest-path crates/<crate-name>/sdk/python/Cargo.toml -i python3.9 -i python3.10 -i python3.11 -i python3.12 -i python3.13
          manylinux: ${{ matrix.settings.manylinux || 'auto' }}
      - name: Upload artifact
        uses: actions/upload-artifact@v4
        with:
          name: wheels-${{ matrix.settings.target }}
          path: dist/*.whl
          if-no-files-found: error

  publish:
    name: Publish to PyPI
    runs-on: ubuntu-latest
    needs: build
    steps:
      - name: Download all artifacts
        uses: actions/download-artifact@v4
        with:
          pattern: wheels-*
          path: dist
          merge-multiple: true
      - name: List wheels
        run: ls -la dist/
      - name: Publish to PyPI
        uses: PyO3/maturin-action@v1
        env:
          MATURIN_PYPI_TOKEN: ${{ secrets.PYPI_TOKEN }}
        with:
          command: upload
          args: --skip-existing dist/*
```

## npm Platform Packages

Each platform needs a separate `npm/<platform>/package.json`:

```json
{
  "name": "@<scope>/<crate-name>-<platform>",
  "version": "<version>",
  "os": ["<os>"],
  "cpu": ["<cpu>"],
  "main": "<napi-name>.<platform>.node",
  "files": ["<napi-name>.<platform>.node"],
  "description": "Native binding for @<scope>/<crate-name> (<platform>)",
  "license": "MIT",
  "repository": {
    "type": "git",
    "url": "https://github.com/<org>/<repo>",
    "directory": "sdk/node"
  }
}
```

For Linux packages, add `"libc": ["glibc"]` or `"libc": ["musl"]`.

Platform mapping:
| Directory | os | cpu | libc |
|-----------|-----|-----|------|
| darwin-arm64 | darwin | arm64 | — |
| darwin-x64 | darwin | x64 | — |
| linux-x64-gnu | linux | x64 | glibc |
| linux-x64-musl | linux | x64 | musl |
| linux-arm64-gnu | linux | arm64 | glibc |
| linux-arm64-musl | linux | arm64 | musl |
| win32-x64-msvc | win32 | x64 | — |

Main package.json must include `optionalDependencies` referencing all platform packages:
```json
{
  "optionalDependencies": {
    "@<scope>/<name>-win32-x64-msvc": "<version>",
    "@<scope>/<name>-darwin-x64": "<version>",
    "@<scope>/<name>-darwin-arm64": "<version>",
    "@<scope>/<name>-linux-x64-gnu": "<version>",
    "@<scope>/<name>-linux-x64-musl": "<version>",
    "@<scope>/<name>-linux-arm64-gnu": "<version>",
    "@<scope>/<name>-linux-arm64-musl": "<version>"
  }
}
```

## Placeholders Reference

Replace these when generating workflows:

| Placeholder | Example | Description |
|-------------|---------|-------------|
| `<crate-name>` | `lane` | Rust crate directory name (kebab-case) |
| `<napi-name>` | `a3s-lane` | napi name from package.json `napi.name` |
| `<scope>` | `a3s-lab` | npm scope (without @) |
| `<org>` | `A3S-Lab` | GitHub organization |
| `<repo>` | `Lane` | GitHub repository name |
| `<version>` | `0.2.2` | Current version |

## Version Bumping

When releasing, bump version in ALL manifests:
```bash
# Core crate
sed -i '' 's/^version = "OLD"/version = "NEW"/' Cargo.toml

# Python SDK
sed -i '' 's/^version = "OLD"/version = "NEW"/' sdk/python/pyproject.toml

# Node SDK (main + all platform packages)
find sdk/node -name "package.json" -not -path "*/node_modules/*" \
  -exec sed -i '' 's/"OLD"/"NEW"/g' {} \;
```

## Release Workflow

```bash
# 1. Bump versions
# 2. Commit and tag
git add -A && git commit -m "chore: bump version to vX.Y.Z"
git tag vX.Y.Z
git push origin main --tags

# 3. CI auto-triggers: release.yml → publish-crate + github-release + publish-node + publish-python
# 4. Monitor
gh run list --repo <org>/<repo> --limit 3
gh run view <run-id> --repo <org>/<repo>
```

## Troubleshooting

### crates.io: "please provide a non-empty token"
→ `CARGO_TOKEN` secret not set. Run: `echo "<token>" | gh secret set CARGO_TOKEN --repo <org>/<repo>`

### npm: "404 Not Found"
→ npm org/scope doesn't exist. Create at https://www.npmjs.com/org/create

### npm: platform packages not found
→ Missing `sdk/node/npm/<platform>/package.json` directories. Create all 7 platform packages.

### PyPI: version already exists
→ `--skip-existing` flag handles this. If main publish fails, the version is already on PyPI.

### Workspace restructuring fails
→ Check `setup-workspace.sh` creates correct `crates/<name>/` path matching workflow `working-directory` values.
