# A3S - Justfile

default:
    @just --list

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
# A3S OS
# ============================================================================

# Start a3s-box infrastructure (postgres, redis, rustfs)
os-up:
    cd apps/os && docker compose up -d

# Stop a3s-box infrastructure
os-down:
    cd apps/os && docker compose down

# Start OS API server (requires running postgres/redis via os-up)
os-api:
    cd apps/os/api && cargo run --bin a3s-os -- --config ../config/dev.hcl

# Start OS web dev server
os-web:
    cd apps/os/web && npm run dev

# Start both OS API and Web in parallel (requires os-up for infra)
os-dev:
    just os-api &
    just os-web

# Build OS web for production
os-web-build:
    cd apps/os/web && npm run build

# Clean OS build artifacts
os-clean:
    cd apps/os/api && cargo clean
    cd apps/os/web && rm -rf dist

# ============================================================================
# SafeClaw
# ============================================================================

# Start SafeClaw gateway with local config
safeclaw:
    cd apps/safeclaw/crates/safeclaw && just run

# Start SafeClaw + ngrok tunnel (dev mode, auto-prints Feishu callback URL)
safeclaw-dev:
    cd apps/safeclaw/crates/safeclaw && just dev

# Start ngrok tunnel only (SafeClaw already running)
safeclaw-tunnel:
    cd apps/safeclaw/crates/safeclaw && just tunnel

# Install SafeClaw frontend (pnpm) and backend (cargo) dependencies
safeclaw-install:
    cd apps/safeclaw && pnpm install
    cd apps/safeclaw/src-tauri && cargo fetch
    cd apps/safeclaw/crates/safeclaw && cargo fetch

# Start SafeClaw frontend dev server (web only, port 8888)
safeclaw-web:
    cd apps/safeclaw && pnpm dev

# CMake compat flag for libsamplerate-sys (requires cmake >= 3.5 policy)
export CMAKE_POLICY_VERSION_MINIMUM := "3.5"

# Start SafeClaw desktop app (Tauri + embedded gateway)
ui:
    cd apps/safeclaw && pnpm tauri:dev

# Clean and start SafeClaw desktop app
ui-clean:
    cd apps/safeclaw/src-tauri && cargo clean
    cd apps/safeclaw && rm -rf dist node_modules/.cache
    cd apps/safeclaw && pnpm tauri:dev

# Build SafeClaw desktop app for production
ui-build:
    cd apps/safeclaw && pnpm build

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
    cd apps/safeclaw/crates/safeclaw && just clean
    cd apps/safeclaw && just clean
    just os-clean
