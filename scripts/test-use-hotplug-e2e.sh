#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cli_source="${repository_root}/crates/cli"
cli_copy="$(mktemp -d "${TMPDIR:-/tmp}/a3s-use-hotplug-cli.XXXXXX")"
use_home="$(mktemp -d "${TMPDIR:-/tmp}/a3s-use-hotplug-home.XXXXXX")"

cleanup() {
  rm -rf -- "${cli_copy}"
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

tar \
  --exclude='./.git' \
  --exclude='./target' \
  -cf - \
  -C "${cli_source}" . |
  tar -xf - -C "${cli_copy}"

bash "${cli_copy}/.github/scripts/use-published-a3s-crates.sh" \
  "${cli_copy}/Cargo.toml"

cargo test \
  --manifest-path "${cli_copy}/Cargo.toml" \
  --locked \
  --lib \
  use_registry::tests::real_use_process_converges_install_upgrade_rebuild_disable_and_enable \
  -- \
  --ignored \
  --nocapture

if test "${windows_host}" = false; then
  cargo test \
    --manifest-path "${cli_copy}/Cargo.toml" \
    --locked \
    --test code_use_first_use \
    code_tui_first_use_installs_a_real_use_release_before_the_first_turn \
    -- \
    --ignored \
    --nocapture
else
  echo "First-use release installation remains covered by the dedicated Windows E2E suite."
fi
