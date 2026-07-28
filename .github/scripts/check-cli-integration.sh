#!/usr/bin/env bash

set -euo pipefail

fail() {
  echo "CLI integration check failed: $*" >&2
  exit 1
}

test -f Cargo.toml || fail "root Cargo.toml is missing"
test -f src/main.rs || fail "root src/main.rs is missing"
test -f Cargo.lock || fail "root Cargo.lock is missing"
test -f .github/workflows/a3s-cli-release.yml \
  || fail "main-repository release workflow is missing"

if git ls-files --stage -- crates/cli | grep -q '^160000 '; then
  fail "crates/cli is still registered as a gitlink"
fi
if grep -Fq 'path = crates/cli' .gitmodules; then
  fail "crates/cli is still registered as a submodule"
fi

grep -Fqx 'repository = "https://github.com/A3S-Lab/a3s"' Cargo.toml \
  || fail "Cargo metadata does not point at A3S-Lab/a3s"
grep -Fqx 'REPOSITORY="A3S-Lab/a3s"' install.sh \
  || fail "Unix installer does not resolve A3S-Lab/a3s releases"
grep -Fq "\$repository = 'A3S-Lab/a3s'" install.ps1 \
  || fail "Windows installer does not resolve A3S-Lab/a3s releases"

echo "root-owned CLI integration verified"
