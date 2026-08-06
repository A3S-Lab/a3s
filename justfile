# A3S - Justfile

host := env_var_or_default("A3S_CODE_WEB_HOST", "127.0.0.1")
port := env_var_or_default("A3S_CODE_WEB_PORT", "29653")
use_e2e_target := env_var_or_default("A3S_USE_E2E_TARGET", justfile_directory() / "target/use-hotplug-e2e")
use_e2e_use_target := use_e2e_target / "use"
use_e2e_code_target := use_e2e_target / "code"
use_e2e_executable := if os() == "windows" { "a3s-use.exe" } else { "a3s-use" }
use_e2e_browser_executable := if os() == "windows" { "a3s-use-browser-driver.exe" } else { "a3s-use-browser-driver" }
use_e2e_bin := use_e2e_use_target / "debug" / use_e2e_executable
use_e2e_browser_bin := use_e2e_use_target / "debug" / use_e2e_browser_executable
agent_island_target := justfile_directory() / "target/agent-island-dev"
agent_island_executable := if os() == "windows" { "a3s-webview.exe" } else { "a3s-webview" }
agent_island_bin := agent_island_target / "debug" / agent_island_executable

[private]
use-e2e-submodules:
    sh scripts/ensure-dev-submodules.sh \
        crates/browser:crates/browser-driver/Cargo.toml \
        crates/ocr:Cargo.toml \
        crates/use:crates/extension/Cargo.toml

[private]
marketplace-e2e-submodules:
    sh scripts/ensure-dev-submodules.sh crates/use:crates/science/Cargo.toml

[private]
webview-submodule:
    sh scripts/ensure-dev-submodules.sh crates/webview:Cargo.toml

default:
    @just --list

# ============================================================================
# Development
# ============================================================================

# Start the primary development surface
dev: code

# ============================================================================
# A3S CLI Website
# ============================================================================

# Start the website development server
docs:
    cd apps/docs && npm run dev

# Build the website
docs-build:
    cd apps/docs && npm run build

# ============================================================================
# A3S Cloud
# ============================================================================

# Start the Cloud API and hot-reloading web console
cloud:
    cd apps/cloud && just cloud

# ============================================================================
# A3S GUI
# ============================================================================

# Start the A3S GUI native calculator example for this operating system
calculator:
    cd crates/gui && just calculator

# Start the A3S GUI semantic component playground for this operating system
playground:
    cd crates/gui && just playground

# ============================================================================
# A3S Code CLI
# ============================================================================
# Run the local umbrella CLI and forward all arguments

# Example: `just a3s search status` or `just a3s --help`
a3s *args:
    cargo run --manifest-path Cargo.toml -- {{ args }}

# Start the A3S Code TUI in the current repository
code: webview-submodule
    CARGO_TARGET_DIR='{{ agent_island_target }}' cargo build --manifest-path crates/webview/Cargo.toml --bin a3s-webview
    A3S_AGENT_ISLAND_BIN='{{ agent_island_bin }}' cargo run --manifest-path Cargo.toml -- code

# Test Code hot-plug against a real, independently built A3S Use process
use-hotplug-e2e: use-e2e-submodules
    CARGO_TARGET_DIR='{{ use_e2e_use_target }}' cargo build --manifest-path crates/use/Cargo.toml -p a3s-use
    CARGO_TARGET_DIR='{{ use_e2e_use_target }}' cargo build --manifest-path crates/browser/Cargo.toml -p a3s-use-browser-driver
    CARGO_TARGET_DIR='{{ use_e2e_code_target }}' A3S_USE_E2E_BIN='{{ use_e2e_bin }}' A3S_USE_E2E_BROWSER_BIN='{{ use_e2e_browser_bin }}' A3S_USE_E2E_SOURCE_ROOT='{{ justfile_directory() }}/crates/use' A3S_USE_E2E_BROWSER_SOURCE_ROOT='{{ justfile_directory() }}/crates/browser' A3S_USE_E2E_OCR_SOURCE_ROOT='{{ justfile_directory() }}/crates/ocr' bash scripts/test-use-hotplug-e2e.sh

# Test the Web Marketplace against real signed and release-bundled plugins
marketplace-science-e2e: marketplace-e2e-submodules
    A3S_USE_E2E_TARGET='{{ use_e2e_target }}' bash scripts/test-web-plugin-marketplace-e2e.sh

# Test the release-bundled Science lifecycle through the real Web UI
marketplace-science-browser-e2e: marketplace-e2e-submodules
    A3S_USE_E2E_TARGET='{{ use_e2e_target }}' bash scripts/test-web-plugin-marketplace-browser-e2e.sh

# Build and start the A3S Web application
web:
    cd apps/web && A3S_HOST={{ host }} A3S_PORT={{ port }} just web

# Start the Windhole visual A3S Bench laboratory
windhole:
    cd apps/windhole && just dev

# Validate the Windhole frontend and local Bench bridge
windhole-check:
    cd apps/windhole && just check

# ============================================================================
# A3S Flow
# ============================================================================

# Check the A3S Flow Rust SDK
flow-check:
    cd crates/flow && cargo check --all-targets

# Test the A3S Flow Rust SDK
flow-test:
    cd crates/flow && cargo test --all-targets

# ============================================================================
# A3S Cloud Compatibility
# ============================================================================

# Verify the exact Cloud stack, resolve it, and run its cross-repository gates
cloud-stack-check:
    node --test scripts/verify-cloud-stack.test.mjs
    node scripts/verify-cloud-stack.mjs
    cargo metadata --manifest-path apps/cloud/Cargo.toml --locked --format-version 1 > /dev/null
    cargo check --manifest-path apps/cloud/Cargo.toml --workspace --all-targets --locked
    cargo test --manifest-path apps/cloud/Cargo.toml --locked -p a3s-cloud-contracts
    cargo check --manifest-path crates/gateway/Cargo.toml --locked --all-targets --features wire

# ============================================================================
# Maintenance
# ============================================================================

# Clean all build artifacts across the monorepo
clean:
    cd crates/box && just clean
    cd crates/code && just clean
    cd crates/event && just clean
    cd crates/lane && just clean
    cd crates/power && just clean
    cd crates/search && just clean
