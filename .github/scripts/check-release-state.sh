#!/usr/bin/env bash

set -euo pipefail

expected_version="${1:?usage: check-release-state.sh <cli-version> <core-version> <tui-version> <search-version>}"
expected_core="${2:?usage: check-release-state.sh <cli-version> <core-version> <tui-version> <search-version>}"
expected_tui="${3:?usage: check-release-state.sh <cli-version> <core-version> <tui-version> <search-version>}"
expected_search="${4:?usage: check-release-state.sh <cli-version> <core-version> <tui-version> <search-version>}"

if ! [[ "$expected_version" =~ ^[0-9]+\.[0-9]+\.[0-9]+$ ]]; then
  echo "invalid CLI release version: $expected_version" >&2
  exit 1
fi

manifest_version="$(
  awk '
    /^\[package\]$/ { in_package = 1; next }
    /^\[/ { in_package = 0 }
    in_package && /^version = "/ {
      value = $0
      sub(/^version = "/, "", value)
      sub(/".*$/, "", value)
      print value
      exit
    }
  ' Cargo.toml
)"

lock_version="$(
  awk '
    /^\[\[package\]\]$/ { package_name = ""; next }
    /^name = "a3s"$/ { package_name = "a3s"; next }
    package_name == "a3s" && /^version = "/ {
      value = $0
      sub(/^version = "/, "", value)
      sub(/".*$/, "", value)
      print value
      exit
    }
  ' Cargo.lock
)"

if [ "$manifest_version" != "$expected_version" ]; then
  echo "Cargo.toml version $manifest_version does not match release $expected_version" >&2
  exit 1
fi
if [ "$lock_version" != "$expected_version" ]; then
  echo "Cargo.lock a3s version $lock_version does not match release $expected_version" >&2
  exit 1
fi

grep -Fqx "a3s-code-core = \"=$expected_core\"" Cargo.toml || {
  echo "Cargo.toml must pin a3s-code-core exactly to $expected_core" >&2
  exit 1
}
grep -Fqx "a3s-tui = \"=$expected_tui\"" Cargo.toml || {
  echo "Cargo.toml must pin a3s-tui exactly to $expected_tui" >&2
  exit 1
}
grep -Fqx "a3s-search = { version = \"=$expected_search\", features = [\"lightpanda\"] }" Cargo.toml || {
  echo "Cargo.toml must pin a3s-search exactly to $expected_search" >&2
  exit 1
}

if ! awk -v header="## [$expected_version]" '
  index($0, header) == 1 { found = 1; next }
  found && /^## \[/ { exit }
  found && /^- / { entry = 1 }
  END { exit !(found && entry) }
' CHANGELOG.md; then
  echo "CHANGELOG.md has no release entry for $expected_version" >&2
  exit 1
fi

echo "release state is consistent at CLI $expected_version, Core $expected_core, TUI $expected_tui, Search $expected_search"
