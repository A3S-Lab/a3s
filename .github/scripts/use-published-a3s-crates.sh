#!/usr/bin/env bash
# Normalize a CLI manifest for an exact crates.io build. The root manifest
# normally has no path patches; removing an optional trailing patch section
# keeps archived or developer-generated manifests safe for release jobs.
#
# An optional second argument reuses a previously published Cargo.lock. When
# the manifest already has a valid sibling lockfile, preserve it so a tagged
# build cannot resolve a different graph.

set -euo pipefail

manifest="${1:-Cargo.toml}"
published_lock="${2:-}"
lockfile="$(dirname "$manifest")/Cargo.lock"

perl -0pi -e 's/\n\[patch\.crates-io\][\s\S]*\z/\n/' "$manifest"
if [ -n "$published_lock" ]; then
  cp "$published_lock" "$lockfile"
elif [ -f "$lockfile" ] \
  && cargo metadata --locked --no-deps --manifest-path "$manifest" >/dev/null 2>&1; then
  :
else
  cargo generate-lockfile --manifest-path "$manifest"
fi
