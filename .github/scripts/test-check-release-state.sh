#!/usr/bin/env bash

set -euo pipefail

script_dir="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
checker="$script_dir/check-release-state.sh"
fixture="$(mktemp -d)"
trap 'rm -rf "$fixture"' EXIT

write_valid_fixture() {
  cat >"$fixture/Cargo.toml" <<'EOF'
[package]
name = "a3s"
version = "0.9.8"

[dependencies]
a3s-code-core = "=6.1.0"
a3s-search = { version = "=2.0.0", features = ["lightpanda"] }
a3s-tui = "=0.1.13"
EOF
  cat >"$fixture/Cargo.lock" <<'EOF'
version = 4

[[package]]
name = "a3s"
version = "0.9.8"
EOF
  cat >"$fixture/CHANGELOG.md" <<'EOF'
# Changelog

## [0.9.8] - 2026-07-21

### Added

- Added a release fixture.

## [0.9.7] - 2026-07-19
EOF
}

run_checker() {
  (cd "$fixture" && bash "$checker" "$@")
}

expect_failure() {
  local description="$1"
  shift
  if "$@" >/dev/null 2>&1; then
    echo "expected failure: $description" >&2
    exit 1
  fi
}

write_valid_fixture
run_checker 0.9.8 6.1.0 0.1.13 2.0.0 >/dev/null

expect_failure "prerelease tags are unsupported" \
  run_checker 0.9.8-rc.1 6.1.0 0.1.13 2.0.0

write_valid_fixture
sed -i.bak 's/version = "0.9.8"/version = "0.9.7"/' "$fixture/Cargo.lock"
expect_failure "the committed lock version must match" \
  run_checker 0.9.8 6.1.0 0.1.13 2.0.0

write_valid_fixture
sed -i.bak '/^- Added a release fixture\.$/d' "$fixture/CHANGELOG.md"
expect_failure "a heading alone is not a changelog entry" \
  run_checker 0.9.8 6.1.0 0.1.13 2.0.0

write_valid_fixture
sed -i.bak 's/a3s-code-core = "=6.1.0"/a3s-code-core = "6.1.0"/' "$fixture/Cargo.toml"
expect_failure "Core must be pinned exactly" \
  run_checker 0.9.8 6.1.0 0.1.13 2.0.0

write_valid_fixture
sed -i.bak 's/a3s-search = { version = "=2.0.0"/a3s-search = { version = "2.0.0"/' "$fixture/Cargo.toml"
expect_failure "Search must be pinned exactly" \
  run_checker 0.9.8 6.1.0 0.1.13 2.0.0

echo "release-state checks passed"
