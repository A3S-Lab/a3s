# Test recipes: `just test::registry`
#
# First-principles gate for `just up::registry` plus ephemeral multi-package
# mock publications (alpha / beta / echo).

[private]
use-registry-submodule:
    sh "{{ source_directory() }}/../scripts/ensure-dev-submodules.sh" \
        use-registry:registry/metadata/root.json \
        use-registry:registry/targets/extensions/a3s/applet-demo/0.1.0/stable/any/a3s-applet-demo-0.1.0-any.tar.gz

[private]
ensure-registry-tools:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ source_directory() }}/.."
    if [[ -n "${A3S_USE_REGISTRY_TOOLS_BIN:-}" && -x "${A3S_USE_REGISTRY_TOOLS_BIN}" ]]; then
      exit 0
    fi
    if [[ -x "${root}/crates/use/target/release/a3s-use-registry-tools" ]]; then
      exit 0
    fi
    if [[ -x "${root}/crates/use/target/debug/a3s-use-registry-tools" ]]; then
      exit 0
    fi
    cargo build --manifest-path "${root}/crates/use/crates/registry-tools/Cargo.toml" -p a3s-use-registry-tools

[private]
ensure-use-bin:
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ source_directory() }}/.."
    if [[ -n "${A3S_USE_BIN:-}" && -x "${A3S_USE_BIN}" ]]; then
      exit 0
    fi
    if [[ -x "${root}/crates/use/target/debug/a3s-use" ]]; then
      exit 0
    fi
    if [[ -x "${root}/crates/use/target/release/a3s-use" ]]; then
      exit 0
    fi
    cargo build --manifest-path "${root}/crates/use/Cargo.toml" -p a3s-use

# Gate: just up::registry + mock multi-package consume
registry: use-registry-submodule ensure-registry-tools ensure-use-bin
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ source_directory() }}/.."
    export A3S_USE_BIN="${A3S_USE_BIN:-${root}/crates/use/target/debug/a3s-use}"
    if [[ ! -x "${A3S_USE_BIN}" ]]; then
      export A3S_USE_BIN="${root}/crates/use/target/release/a3s-use"
    fi
    export A3S_USE_REGISTRY_TOOLS_BIN="${A3S_USE_REGISTRY_TOOLS_BIN:-${root}/crates/use/target/release/a3s-use-registry-tools}"
    if [[ ! -x "${A3S_USE_REGISTRY_TOOLS_BIN}" ]]; then
      export A3S_USE_REGISTRY_TOOLS_BIN="${root}/crates/use/target/debug/a3s-use-registry-tools"
    fi
    chmod +x "${root}/use-registry/scripts/"*.sh
    "${root}/use-registry/scripts/test_just_registry.sh"

# Gate: non-Desktop Applet exits (Use/Code/CLI/Box/WebView + registry)
# Does not run apps/desktop UiHost product proofs.
[private]
applet-non-desktop-submodules:
    sh "{{ source_directory() }}/../scripts/ensure-dev-submodules.sh" \
        crates/use:crates/extension/Cargo.toml \
        crates/code:core/Cargo.toml \
        crates/cli:Cargo.toml \
        crates/box:src/core/Cargo.toml \
        crates/webview:Cargo.toml \
        use-registry:registry/metadata/root.json \
        use-registry:registry/targets/extensions/a3s/applet-demo/0.1.0/stable/any/a3s-applet-demo-0.1.0-any.tar.gz

applet-non-desktop: applet-non-desktop-submodules ensure-registry-tools ensure-use-bin
    #!/usr/bin/env bash
    set -euo pipefail
    root="{{ source_directory() }}/.."
    export A3S_USE_E2E_BIN="${A3S_USE_E2E_BIN:-${root}/crates/use/target/debug/a3s-use}"
    if [[ ! -x "${A3S_USE_E2E_BIN}" ]]; then
      export A3S_USE_E2E_BIN="${root}/crates/use/target/release/a3s-use"
    fi
    export A3S_USE_BIN="${A3S_USE_BIN:-${A3S_USE_E2E_BIN}}"
    export A3S_USE_REGISTRY_TOOLS_BIN="${A3S_USE_REGISTRY_TOOLS_BIN:-${root}/crates/use/target/debug/a3s-use-registry-tools}"
    if [[ ! -x "${A3S_USE_REGISTRY_TOOLS_BIN}" ]]; then
      export A3S_USE_REGISTRY_TOOLS_BIN="${root}/crates/use/target/release/a3s-use-registry-tools"
    fi
    chmod +x "${root}/scripts/test_applet_non_desktop.sh"
    chmod +x "${root}/use-registry/scripts/"*.sh
    "${root}/scripts/test_applet_non_desktop.sh"
