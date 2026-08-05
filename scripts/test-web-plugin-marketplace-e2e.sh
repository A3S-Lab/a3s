#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
integration_target="${A3S_USE_E2E_TARGET:-${repository_root}/target/use-hotplug-e2e}"
use_target="${integration_target}/use"
web_target="${integration_target}/web"
package_temp="$(mktemp -d "${TMPDIR:-/tmp}/a3s-web-plugin-package.XXXXXX")"
science_package="${package_temp}/a3s-use-science"

cleanup() {
  rm -rf -- "${package_temp}"
}
trap cleanup EXIT

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    echo "The real Web Marketplace plugin proof currently requires a Unix host." >&2
    exit 2
    ;;
esac

CARGO_TARGET_DIR="${use_target}" cargo build \
  --manifest-path "${repository_root}/crates/use/Cargo.toml" \
  --locked \
  -p a3s-use

CARGO_TARGET_DIR="${use_target}" \
  "${repository_root}/crates/use/crates/science/scripts/package.sh" \
  "${science_package}"

CARGO_TARGET_DIR="${web_target}" \
A3S_USE_E2E_BIN="${use_target}/debug/a3s-use" \
A3S_USE_SCIENCE_E2E_PACKAGE="${science_package}" \
  cargo test \
    --manifest-path "${repository_root}/crates/cli/Cargo.toml" \
    --locked \
    --test web_plugin_marketplace \
    real_marketplace_ \
    -- \
    --ignored \
    --nocapture
