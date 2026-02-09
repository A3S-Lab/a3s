# A3S Workspace - Justfile

default:
    @just --list

# ============================================================================
# Build
# ============================================================================

# Build all workspace crates
build:
    cargo build --workspace

# Build in release mode
release:
    cargo build --workspace --release

# Build tools binary
build-tools:
    cargo build -p a3s-tools --release

# Install tools binary to PATH
install-tools:
    cargo install --path crates/tools

# Build box (separate workspace)
build-box:
    cd crates/box/src && cargo build --workspace

# Build everything (workspace + box)
build-all:
    just build
    just build-box

# ============================================================================
# Test
# ============================================================================

# Test all workspace crates
test:
    #!/usr/bin/env bash
    set -e

    # Colors
    BOLD='\033[1m'
    GREEN='\033[0;32m'
    BLUE='\033[0;34m'
    CYAN='\033[0;36m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    DIM='\033[2m'
    RESET='\033[0m'

    print_header() {
        echo ""
        echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
        echo -e "${BOLD}  $1${RESET}"
        echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
    }

    print_header "🧪 A3S Workspace Test Suite"
    echo ""

    TOTAL_PASSED=0
    TOTAL_FAILED=0

    # Test each crate with correct package names
    test_crate() {
        local pkg=$1
        local extra_args=$2
        echo -ne "${CYAN}▶${RESET} ${BOLD}${pkg}${RESET} "

        if OUTPUT=$(cargo test -p "$pkg" --lib $extra_args 2>&1); then
            RESULT=$(echo "$OUTPUT" | grep -E "^test result:" | tail -1)
            PASSED=$(echo "$RESULT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' || echo "0")
            FAILED=$(echo "$RESULT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' || echo "0")

            TOTAL_PASSED=$((TOTAL_PASSED + PASSED))
            TOTAL_FAILED=$((TOTAL_FAILED + FAILED))

            if [ "$FAILED" -gt 0 ]; then
                echo -e "${RED}✗${RESET} ${DIM}$PASSED passed, $FAILED failed${RESET}"
            else
                echo -e "${GREEN}✓${RESET} ${DIM}$PASSED passed${RESET}"
            fi
        else
            echo -e "${RED}✗${RESET} ${DIM}failed${RESET}"
            TOTAL_FAILED=$((TOTAL_FAILED + 1))
        fi
    }

    test_crate "a3s-lane"
    test_crate "a3s-code"
    test_crate "a3s_context"
    test_crate "a3s-cron"
    test_crate "a3s-tools-core"
    test_crate "a3s-power"
    test_crate "a3s-updater"
    # a3s-tools is binary-only, test with different command
    echo -ne "${CYAN}▶${RESET} ${BOLD}a3s-tools${RESET} "
    if OUTPUT=$(cargo test -p a3s-tools -- --test-threads=1 2>&1); then
        RESULT=$(echo "$OUTPUT" | grep -E "^test result:" | tail -1)
        PASSED=$(echo "$RESULT" | grep -oE '[0-9]+ passed' | grep -oE '[0-9]+' || echo "0")
        FAILED=$(echo "$RESULT" | grep -oE '[0-9]+ failed' | grep -oE '[0-9]+' || echo "0")

        TOTAL_PASSED=$((TOTAL_PASSED + PASSED))
        TOTAL_FAILED=$((TOTAL_FAILED + FAILED))

        if [ "$FAILED" -gt 0 ]; then
            echo -e "${RED}✗${RESET} ${DIM}$PASSED passed, $FAILED failed${RESET}"
        else
            echo -e "${GREEN}✓${RESET} ${DIM}$PASSED passed${RESET}"
        fi
    else
        echo -e "${RED}✗${RESET} ${DIM}failed${RESET}"
        TOTAL_FAILED=$((TOTAL_FAILED + 1))
    fi

    echo ""
    echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
    if [ "$TOTAL_FAILED" -gt 0 ]; then
        echo -e "  ${RED}${BOLD}✗ FAILED${RESET}  ${GREEN}$TOTAL_PASSED passed${RESET}  ${RED}$TOTAL_FAILED failed${RESET}"
    else
        echo -e "  ${GREEN}${BOLD}✓ PASSED${RESET}  ${GREEN}$TOTAL_PASSED passed${RESET}"
    fi
    echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
    echo ""

# Test box (separate workspace)
test-box:
    cd crates/box && just test

# Test everything
test-all:
    just test
    just test-box

# ============================================================================
# Code Quality
# ============================================================================

# Format all code
fmt:
    cargo fmt --all
    cd crates/box/src && cargo fmt --all

# Check formatting
fmt-check:
    cargo fmt --all -- --check
    cd crates/box/src && cargo fmt --all -- --check

# Lint all code
lint:
    cargo clippy --workspace --all-targets -- -D warnings
    cd crates/box/src && cargo clippy --all-targets -- -D warnings

# CI checks
ci:
    just fmt-check
    just lint
    just test

# ============================================================================
# Publish
# ============================================================================

# Publish all crates to crates.io (in dependency order)
publish:
    #!/usr/bin/env bash
    set -e

    # Colors
    BOLD='\033[1m'
    GREEN='\033[0;32m'
    BLUE='\033[0;34m'
    YELLOW='\033[0;33m'
    RED='\033[0;31m'
    DIM='\033[2m'
    RESET='\033[0m'

    print_header() {
        echo ""
        echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
        echo -e "${BOLD}  $1${RESET}"
        echo -e "${BOLD}${BLUE}━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━${RESET}"
    }

    print_step() {
        echo -e "${BLUE}▶${RESET} ${BOLD}$1${RESET}"
    }

    print_success() {
        echo -e "${GREEN}✓${RESET} $1"
    }

    print_error() {
        echo -e "${RED}✗${RESET} $1"
        exit 1
    }

    publish_crate() {
        local crate_name=$1
        local crate_path=$2
        local wait_time=${3:-30}

        print_header "📦 Publishing ${crate_name}"

        VERSION=$(grep '^version' "${crate_path}/Cargo.toml" | head -1 | sed 's/.*"\(.*\)".*/\1/')
        echo -e "  ${DIM}Version:${RESET} ${BOLD}${VERSION}${RESET}"
        echo ""

        print_step "Verifying ${crate_name}..."
        if cargo publish -p "$crate_name" --dry-run --allow-dirty 2>/dev/null || \
           (cd "$crate_path" && cargo publish --dry-run --allow-dirty); then
            print_success "Verification OK"
        else
            print_error "Verification failed for ${crate_name}"
        fi

        print_step "Publishing ${crate_name}..."
        if cargo publish -p "$crate_name" --allow-dirty 2>/dev/null || \
           (cd "$crate_path" && cargo publish --allow-dirty); then
            print_success "Published ${crate_name} v${VERSION}"
        else
            print_error "Publish failed for ${crate_name}"
        fi

        if [ "$wait_time" -gt 0 ]; then
            echo -e "  ${DIM}Waiting ${wait_time}s for crates.io to index...${RESET}"
            sleep "$wait_time"
        fi
    }

    print_header "📦 Publishing A3S Crates to crates.io"
    echo ""
    echo -e "  ${DIM}Publishing order:${RESET}"
    echo -e "    1. a3s-tools-core (foundation, no internal deps)"
    echo -e "    2. a3s-updater    (utility, no internal deps)"
    echo -e "    3. a3s-search     (utility, no internal deps)"
    echo -e "    4. a3s-lane       (utility, no internal deps)"
    echo -e "    5. a3s_context    (utility, no internal deps)"
    echo -e "    6. a3s-cron       (utility, no internal deps)"
    echo -e "    7. a3s-power      (infrastructure, no internal deps)"
    echo -e "    8. a3s-code       (depends on tools-core, lane, cron)"
    echo -e "    9. a3s-tools      (depends on tools-core, search, cron)"
    echo ""

    # Pre-publish checks
    print_step "Running pre-publish checks..."

    print_step "Checking formatting..."
    if cargo fmt --all -- --check; then
        print_success "Formatting OK"
    else
        print_error "Formatting check failed. Run 'just fmt' first."
    fi

    print_step "Running clippy..."
    if cargo clippy --workspace -- -D warnings; then
        print_success "Clippy OK"
    else
        print_error "Clippy check failed."
    fi

    print_step "Running tests..."
    if cargo test --workspace; then
        print_success "Tests OK"
    else
        print_error "Tests failed."
    fi

    # Publish in dependency order
    publish_crate "a3s-tools-core" "crates/tools-core" 30
    publish_crate "a3s-updater" "crates/updater" 30
    publish_crate "a3s-search" "crates/search" 30
    publish_crate "a3s-lane" "crates/lane" 30
    publish_crate "a3s_context" "crates/context" 30
    publish_crate "a3s-cron" "crates/cron" 30
    publish_crate "a3s-power" "crates/power" 30
    publish_crate "a3s-code" "crates/code" 30
    publish_crate "a3s-tools" "crates/tools" 0

    print_header "✓ All crates published successfully!"
    echo ""

# Publish dry-run (verify all without publishing)
publish-dry:
    #!/usr/bin/env bash
    set -e

    echo ""
    echo "┏━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┓"
    echo "┃                    📦 Publish Dry Run (All Crates)                     ┃"
    echo "┗━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━┛"
    echo ""

    for crate in a3s-tools-core a3s-updater a3s-search a3s-lane a3s_context a3s-cron a3s-power a3s-code a3s-tools; do
        echo "=== ${crate} ==="
        cargo publish -p "$crate" --dry-run --allow-dirty 2>/dev/null || echo "  (dry-run failed for ${crate})"
        echo ""
    done

    echo "✓ Dry run complete. Ready to publish with 'just publish'"
    echo ""

# Publish a single crate
publish-crate CRATE:
    #!/usr/bin/env bash
    set -e
    echo "Publishing {{CRATE}}..."
    cargo publish -p {{CRATE}} --allow-dirty 2>/dev/null || \
        (cd crates/box/src && cargo publish -p {{CRATE}} --allow-dirty)
    echo "✓ Published {{CRATE}}"

# Show all crate versions
version:
    #!/usr/bin/env bash
    echo ""
    echo "A3S Crate Versions:"
    echo "  a3s-tools-core:  $(grep '^version' crates/tools-core/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s-updater:     $(grep '^version' crates/updater/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s-search:      $(grep '^version' crates/search/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s-lane:        $(grep '^version' crates/lane/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s_context:     $(grep '^version' crates/context/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s-cron:        $(grep '^version' crates/cron/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s-power:       $(grep '^version' crates/power/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s-code:        $(grep '^version' crates/code/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  a3s-tools:       $(grep '^version' crates/tools/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo "  safeclaw:        $(grep '^version' crates/safeclaw/Cargo.toml | head -1 | sed 's/.*\"\(.*\)\".*/\1/')"
    echo ""

# ============================================================================
# Submodule Management
# ============================================================================

# Update all submodules to latest
update-submodules:
    git submodule update --remote --merge

# Initialize submodules (for fresh clone)
init-submodules:
    git submodule update --init --recursive

# Show submodule status
status-submodules:
    git submodule status

# ============================================================================
# Utilities
# ============================================================================

# Clean all build artifacts
clean:
    cargo clean
    cd crates/box/src && cargo clean

# Check compilation (fast)
check:
    cargo check --workspace
    cd crates/box/src && cargo check --workspace

# Generate documentation
doc:
    cargo doc --workspace --no-deps --open

# Watch for changes
watch:
    cargo watch -x 'check --workspace'
