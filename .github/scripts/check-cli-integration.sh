#!/usr/bin/env bash

set -euo pipefail

fail() {
  echo "CLI integration check failed: $*" >&2
  exit 1
}

repository_root="$(git rev-parse --show-toplevel)"
cd "${repository_root}"

for forbidden in \
  Cargo.toml \
  Cargo.lock \
  build.rs \
  src \
  tests \
  skills \
  bin/a3s \
  CHANGELOG.md; do
  test ! -e "${forbidden}" || fail "repository root must not own ${forbidden}"
done

test ! -e .github/workflows/a3s-cli-release.yml \
  || fail "the monorepo must not publish the CLI"
test -f .github/workflows/relay-cli-release.yml \
  || fail "the legacy-client release relay is missing"

test "$(git config -f .gitmodules --get submodule.crates/cli.path)" = "crates/cli" \
  || fail "crates/cli submodule path is missing"
test "$(git config -f .gitmodules --get submodule.crates/cli.url)" = "git@github.com:A3S-Lab/CLI.git" \
  || fail "crates/cli does not point at A3S-Lab/CLI"

set -- $(git ls-files --stage -- crates/cli)
test "$#" -eq 4 || fail "crates/cli must have exactly one index entry"
test "$1" = "160000" || fail "crates/cli is not a gitlink"
gitlink="$2"

test -f crates/cli/Cargo.toml || fail "crates/cli is not initialized"
checkout="$(git -C crates/cli rev-parse HEAD)"
test "${checkout}" = "${gitlink}" \
  || fail "crates/cli checkout ${checkout} does not match gitlink ${gitlink}"

grep -Fqx 'repository = "https://github.com/A3S-Lab/CLI"' crates/cli/Cargo.toml \
  || fail "CLI Cargo metadata does not point at A3S-Lab/CLI"
grep -Fqx 'PRIMARY_REPOSITORY="A3S-Lab/CLI"' install.sh \
  || fail "Unix installer does not resolve the canonical A3S-Lab/CLI releases"
grep -Fq "\$repositories = @('A3S-Lab/CLI', 'A3S-Lab/a3s')" install.ps1 \
  || fail "Windows installer does not resolve the canonical A3S-Lab/CLI releases"

echo "standalone CLI integration verified at ${gitlink}"
