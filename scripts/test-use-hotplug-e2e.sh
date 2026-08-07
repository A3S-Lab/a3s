#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cli_manifest="${repository_root}/crates/cli/Cargo.toml"
use_home="$(mktemp -d "${TMPDIR:-/tmp}/a3s-use-hotplug-home.XXXXXX")"

cleanup() {
  rm -rf -- "${use_home}"
}
trap cleanup EXIT

required_environment=(
  A3S_USE_E2E_BIN
  A3S_USE_E2E_BROWSER_BIN
  A3S_USE_E2E_SOURCE_ROOT
  A3S_USE_E2E_BROWSER_SOURCE_ROOT
  A3S_USE_E2E_OCR_SOURCE_ROOT
  CARGO_TARGET_DIR
)
for name in "${required_environment[@]}"; do
  if test -z "${!name:-}"; then
    echo "${name} must be set" >&2
    exit 1
  fi
done

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    export A3S_USE_HOME="$(cygpath -w "${use_home}")"
    windows_host=true
    ;;
  *)
    export A3S_USE_HOME="${use_home}"
    windows_host=false
    ;;
esac

test -f "${cli_manifest}" || {
  echo "crates/cli is not initialized" >&2
  exit 1
}

cargo test \
  --manifest-path "${cli_manifest}" \
  --locked \
  --lib \
  use_registry::tests::real_use_process_converges_signed_install_upgrade_rebuild_and_uninstall \
  -- \
  --ignored \
  --nocapture

if test "${windows_host}" = false; then
  cargo test \
    --manifest-path "${cli_manifest}" \
    --locked \
    --test code_use_first_use \
    code_tui_first_use_installs_a_real_use_release_before_the_first_turn \
    -- \
    --ignored \
    --nocapture
else
  echo "First-use release installation remains covered by the dedicated Windows E2E suite."
fi
