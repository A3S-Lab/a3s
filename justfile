# A3S - Justfile

default:
    @just --list

# ============================================================================
# A3S Box
# ============================================================================

# Build a3s-box
box-build:
    cd crates/box && just build

# Run a3s-box unit tests
box-test:
    cd crates/box && just test

# Run a3s-box VM integration tests (requires built binary + HVF/KVM)
# Usage: just box-test-vm                          # run all tests
#        just box-test-vm test_alpine_full_lifecycle  # run a specific test
box-test-vm *ARGS:
    cd crates/box && just test-vm {{ARGS}}

# Run a3s-box TEE integration tests (requires built binary + HVF/KVM)
# Usage: just box-test-tee                              # run all TEE tests
#        just box-test-tee test_tee_seal_unseal_lifecycle  # run a specific test
box-test-tee *ARGS:
    cd crates/box && just test-tee {{ARGS}}

# ============================================================================
# A3S Code
# ============================================================================

# Start a3s-code server (dev mode)
code:
    cd crates/code && just serve

# ============================================================================
# Documentation Site
# ============================================================================

# Start docs dev server
docs:
    cd apps/docs && bun run dev

# Build docs site
docs-build:
    cd apps/docs && bun run build

# ============================================================================
# Architecture Diagrams
# ============================================================================

# Start LikeC4 architecture diagram dev server
arch:
    npx likec4 serve docs/architecture

# Export architecture diagrams to PNG
arch-export-png:
    npx likec4 export png -o docs/architecture/output docs/architecture

# Export architecture diagrams to SVG
arch-export-svg:
    npx likec4 export svg -o docs/architecture/output docs/architecture

# Build architecture diagrams as static site
arch-build:
    npx likec4 build -o docs/architecture/dist docs/architecture

# ============================================================================
# SafeClaw
# ============================================================================

# Start SafeClaw with local config
safeclaw:
    cd crates/safeclaw && just run

# Start SafeClaw + ngrok tunnel (dev mode, auto-prints Feishu callback URL)
safeclaw-dev:
    cd crates/safeclaw && just dev

# Start ngrok tunnel only (SafeClaw already running)
safeclaw-tunnel:
    cd crates/safeclaw && just tunnel

# Start SafeClaw UI dev server (web, port 8888)
safeclaw-ui:
    cd apps/safeclaw-ui && pnpm dev

# Start SafeClaw UI in Tauri desktop mode
safeclaw-ui-desktop:
    cd apps/safeclaw-ui && pnpm tauri:dev

# Build SafeClaw UI for production
safeclaw-ui-build:
    cd apps/safeclaw-ui && pnpm build

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
    cd crates/safeclaw && just clean
    cd apps/os && just clean
    cd apps/safeclaw-ui && just clean
