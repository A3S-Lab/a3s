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

# Build NestJS sidecar (must run before ui or ui-dev)
safeclaw-api-build:
    cd apps/safeclaw-api && npm run build

# Start SafeClaw desktop app (Tauri + NestJS sidecar)
# Builds sidecar first, then starts Tauri
dev: safeclaw-api-build
    cd apps/safeclaw && pnpm tauri:dev

# Start SafeClaw desktop app without rebuilding sidecar
ui-no-build:
    cd apps/safeclaw && pnpm tauri:dev

# Clean and start SafeClaw desktop app
dev-clean:
    cd apps/safeclaw/src-tauri && cargo clean
    cd apps/safeclaw && rm -rf dist node_modules/.cache
    cd apps/safeclaw-api && rm -rf dist
    just dev

# Build SafeClaw desktop app for production
ui-build: safeclaw-api-build
    cd apps/safeclaw && pnpm build

# ============================================================================
# Development (hot reload)
# ============================================================================

# Sync models config from SQLite to config.hcl (run before safeclaw-api-dev)
safeclaw-api-sync:
    cd apps/safeclaw-api && npx tsx scripts/sync-config.ts

# Start safeclaw-api (builds and runs without watch)
# Runs sync first to ensure config.hcl matches SQLite, then builds and starts the API server
safeclaw-api-dev: safeclaw-api-sync
    cd apps/safeclaw-api && npm run build && npm run start

# Dev mode with hot reload API (run API separately, then start Tauri)
# Terminal 1: just safeclaw-api-dev
# Terminal 2: just ui-no-build (runs Tauri without rebuilding sidecar)
# Note: If API is already running, Tauri will detect port in use and offer to kill it

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
