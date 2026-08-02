#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
integration_target="${A3S_USE_E2E_TARGET:-${repository_root}/target/use-hotplug-e2e}"
use_target="${integration_target}/use"
web_target="${integration_target}/web"
use_binary="${use_target}/debug/a3s-use"
web_binary="${web_target}/debug/a3s"
test_port=29654
test_root="$(mktemp -d "${TMPDIR:-/tmp}/a3s-web-plugin-browser.XXXXXX")"
release_root="${test_root}/use-release"
science_package="${release_root}/extensions/a3s/science"
workspace="${test_root}/workspace"
config="${test_root}/config/config.acl"
web_state="${test_root}/web-state"

component_env=(
  "A3S_DATA_HOME=${test_root}/data"
  "A3S_STATE_HOME=${test_root}/state"
  "A3S_CACHE_HOME=${test_root}/cache"
  "A3S_RUNTIME_HOME=${test_root}/runtime"
  "A3S_USE_INSTALL_DIR=${release_root}"
  "A3S_CODE_WEB_STATE_DIR=${web_state}"
  "OPENAI_API_KEY=test"
  "OPENAI_BASE_URL=http://127.0.0.1:1"
)

cleanup() {
  exit_status=$?
  if [[ "${exit_status}" -ne 0 ]]; then
    echo "Web Marketplace browser proof failed; final APIs and Web log follow." >&2
    curl --max-time 5 --silent --show-error \
      "http://127.0.0.1:${test_port}/api/v1/plugins/activities" >&2 || true
    echo >&2
    curl --max-time 5 --silent --show-error \
      "http://127.0.0.1:${test_port}/api/v1/plugins/marketplace" >&2 || true
    echo >&2
    web_log="${test_root}/state/logs/web"
    if [[ -d "${web_log}" ]]; then
      tail -100 "${web_log}"/*.log >&2 || true
    fi
  fi
  if [[ -x "${web_binary}" && -d "${workspace}" ]]; then
    env -u A3S_USE_HOME "${component_env[@]}" \
      "${web_binary}" \
      --config "${config}" \
      --output json \
      web stop \
      --workspace "${workspace}" \
      >/dev/null 2>&1 || true
  fi
  rm -rf -- "${test_root}"
  return "${exit_status}"
}
trap cleanup EXIT

case "$(uname -s)" in
  MINGW*|MSYS*|CYGWIN*)
    echo "The real Web Marketplace browser proof currently requires a Unix host." >&2
    exit 2
    ;;
esac

if ! command -v a3s-test >/dev/null 2>&1; then
  echo "a3s-test is required for the Web Marketplace browser proof." >&2
  exit 127
fi

browser_executable="${A3S_TEST_BROWSER_EXECUTABLE:-}"
browser_action_timeout_ms="${A3S_TEST_BROWSER_ACTION_TIMEOUT_MS:-45000}"
if [[ -z "${browser_executable}" ]]; then
  if ! command -v a3s >/dev/null 2>&1; then
    echo "a3s or A3S_TEST_BROWSER_EXECUTABLE is required by a3s-test." >&2
    exit 127
  fi
  browser_executable="$(command -v a3s)"
fi

mkdir -p \
  "${release_root}/extensions/a3s" \
  "${workspace}" \
  "$(dirname "${config}")" \
  "${web_state}"
bash "${repository_root}/scripts/ensure-dev-submodules.sh" \
  crates/code:sdk/node/examples/configs/test_config.acl
cp "${repository_root}/crates/code/sdk/node/examples/configs/test_config.acl" "${config}"

CARGO_TARGET_DIR="${use_target}" cargo build \
  --manifest-path "${repository_root}/crates/use/Cargo.toml" \
  --locked \
  -p a3s-use
cp "${use_binary}" "${release_root}/a3s-use"
CARGO_TARGET_DIR="${use_target}" \
  "${repository_root}/crates/use/crates/science/scripts/package.sh" \
  "${science_package}"

CARGO_TARGET_DIR="${web_target}" cargo build \
  --manifest-path "${repository_root}/Cargo.toml" \
  --locked \
  --bin a3s

(
  cd "${repository_root}/apps/web"
  bun install --frozen-lockfile
  bun run build
)

env -u A3S_USE_HOME "${component_env[@]}" \
  "${web_binary}" \
  --config "${config}" \
  --output json \
  web \
  --detach \
  --host 127.0.0.1 \
  --port "${test_port}" \
  --workspace "${workspace}" \
  --web-dir "${repository_root}/apps/web/dist/workspace"

curl --fail --silent --show-error "http://127.0.0.1:${test_port}/api/health" >/dev/null
a3s-test check "${repository_root}/tests/e2e/web-plugin-hotplug.acl" --json
AGENT_BROWSER_DEFAULT_TIMEOUT="${browser_action_timeout_ms}" \
  a3s-test run "${repository_root}/tests/e2e/web-plugin-hotplug.acl" \
  --browser-driver a3s \
  --browser-executable "${browser_executable}" \
  --command-timeout-ms 60000 \
  --idle-timeout-ms 60000 \
  --cleanup-timeout-ms 10000 \
  --infrastructure-retries 1 \
  --json
