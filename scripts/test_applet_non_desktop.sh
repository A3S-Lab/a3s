#!/usr/bin/env bash
# First-principles gate for non-Desktop Applet exits (P1–P5).
#
# Mission: prove Use supply, Code freeze, CLI reference projection, Box/WebView
# boundary freezes, and use-registry applet-demo supply without running
# apps/desktop product UiHost work.
#
# Tracks:
#   U1 Use package surfaces + Executable Tool projection + six-surface lifecycle
#   U2 Use operation progress (phase · checkpoint)
#   C1 Code UiBinding + ui_projection HOST-UI1 regressions
#   L1 CLI applet_demo FP (package bytes + Executable Tool + bind_tool)
#   L2 CLI live Use E2E (admissions + committed registry) when bins exist
#   B1 Box applet_boundary (not page shell)
#   W1 WebView applet_boundary (not product UiHost)
#   R1 use-registry Track A/J6/S0/S1 via test_just_registry.sh
#
# Requires: cargo, bash; monorepo checkouts of crates/{use,code,cli,box,webview}
# and use-registry. Optional: built a3s-use + a3s-use-registry-tools for L2.
set -euo pipefail

ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
pass=0
fail=0

pass() {
  echo "PASS  $1"
  pass=$((pass + 1))
}

fail() {
  echo "FAIL  $1${2:+ — $2}"
  fail=$((fail + 1))
}

run_cargo_filter() {
  local label="$1"
  local dir="$2"
  shift 2
  local out
  if out="$(cd "${dir}" && "$@" 2>&1)"; then
    if echo "${out}" | rg -q "FAILED|error: test failed"; then
      fail "${label}" "cargo reported failure"
      echo "${out}" | tail -40
      return 1
    fi
    if ! echo "${out}" | rg -q "test result: ok\."; then
      fail "${label}" "no ok test result line"
      echo "${out}" | tail -40
      return 1
    fi
    pass "${label}"
    return 0
  fi
  fail "${label}" "cargo exited non-zero"
  echo "${out}" | tail -40
  return 1
}

echo "applet non-Desktop first-principles gate"
echo "monorepo=${ROOT}"

# --- U1 Use supply ---
run_cargo_filter \
  "U1 Use monorepo_applet_demo package surfaces" \
  "${ROOT}/crates/use" \
  cargo test -p a3s-use-extension monorepo_applet_demo_package_surfaces -- --nocapture \
  || true

run_cargo_filter \
  "U1 Use executable_echo without Runtime BindingStore" \
  "${ROOT}/crates/use" \
  cargo test --lib monorepo_applet_demo_executable_echo -- --nocapture \
  || true

run_cargo_filter \
  "U1 Use plugin_manager_six_surface" \
  "${ROOT}/crates/use" \
  cargo test plugin_manager_six_surface -- --nocapture \
  || true

# --- U2 observation progress ---
run_cargo_filter \
  "U2 Use operation_progress" \
  "${ROOT}/crates/use" \
  cargo test operation_progress -- --nocapture \
  || true

# --- C1 Code freeze ---
run_cargo_filter \
  "C1 Code ui_binding" \
  "${ROOT}/crates/code" \
  cargo test -p a3s-code-core --lib ui_binding -- --nocapture \
  || true

run_cargo_filter \
  "C1 Code ui_projection" \
  "${ROOT}/crates/code" \
  cargo test -p a3s-code-core --lib ui_projection -- --nocapture \
  || true

# --- L1 CLI reference host ---
run_cargo_filter \
  "L1 CLI applet_demo FP" \
  "${ROOT}/crates/cli" \
  cargo test --lib applet_demo -- --nocapture \
  || true

# --- L2 live Use E2E when bins are present ---
USE_BIN="${A3S_USE_E2E_BIN:-${ROOT}/crates/use/target/debug/a3s-use}"
TOOLS_BIN="${A3S_USE_REGISTRY_TOOLS_BIN:-${ROOT}/crates/use/target/debug/a3s-use-registry-tools}"
if [[ -x "${USE_BIN}" && -x "${TOOLS_BIN}" ]]; then
  if out="$(
    cd "${ROOT}/crates/cli" && \
      A3S_USE_E2E_BIN="${USE_BIN}" \
      A3S_USE_REGISTRY_TOOLS_BIN="${TOOLS_BIN}" \
      cargo test --lib real_use_installs_applet_demo -- --ignored --nocapture 2>&1
  )"; then
    if echo "${out}" | rg -q "test result: ok\." && ! echo "${out}" | rg -q "FAILED"; then
      pass "L2 CLI live Use applet_demo E2E (admissions + committed)"
    else
      fail "L2 CLI live Use applet_demo E2E" "unexpected output"
      echo "${out}" | tail -40
    fi
  else
    fail "L2 CLI live Use applet_demo E2E" "cargo exited non-zero"
    echo "${out}" | tail -40
  fi
else
  echo "SKIP  L2 CLI live Use applet_demo E2E (build a3s-use + a3s-use-registry-tools or set A3S_USE_E2E_BIN / A3S_USE_REGISTRY_TOOLS_BIN)"
fi

# --- B1 Box boundary ---
run_cargo_filter \
  "B1 Box core applet_boundary" \
  "${ROOT}/crates/box/src/core" \
  cargo test applet_boundary -- --nocapture \
  || true

run_cargo_filter \
  "B1 Box runtime applet_boundary" \
  "${ROOT}/crates/box/src/runtime" \
  cargo test applet_boundary -- --nocapture \
  || true

# --- W1 WebView boundary ---
run_cargo_filter \
  "W1 WebView applet_boundary" \
  "${ROOT}/crates/webview" \
  cargo test applet_boundary -- --nocapture \
  || true

# --- R1 registry supply ---
if [[ -x "${ROOT}/use-registry/scripts/test_just_registry.sh" ]]; then
  export A3S_USE_BIN="${A3S_USE_BIN:-${USE_BIN}}"
  export A3S_USE_REGISTRY_TOOLS_BIN="${A3S_USE_REGISTRY_TOOLS_BIN:-${TOOLS_BIN}}"
  if [[ ! -x "${A3S_USE_BIN}" ]]; then
    A3S_USE_BIN="${ROOT}/crates/use/target/release/a3s-use"
  fi
  if [[ ! -x "${A3S_USE_REGISTRY_TOOLS_BIN}" ]]; then
    A3S_USE_REGISTRY_TOOLS_BIN="${ROOT}/crates/use/target/release/a3s-use-registry-tools"
  fi
  if out="$(cd "${ROOT}/use-registry" && bash scripts/test_just_registry.sh 2>&1)"; then
    if echo "${out}" | rg -q "summary pass=.*fail=0"; then
      pass "R1 use-registry test_just_registry (Track A/J6/S0/S1)"
    else
      fail "R1 use-registry test_just_registry" "summary not clean"
      echo "${out}" | tail -20
    fi
  else
    fail "R1 use-registry test_just_registry" "script exited non-zero"
    echo "${out}" | tail -40
  fi
else
  fail "R1 use-registry test_just_registry" "script missing"
fi

echo
echo "summary pass=${pass} fail=${fail}"
if [[ "${fail}" -ne 0 ]]; then
  exit 1
fi
