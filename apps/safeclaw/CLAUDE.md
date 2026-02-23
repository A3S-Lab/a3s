# CLAUDE.md

This file provides guidance to Claude Code when working with SafeClaw.

## Project Overview

SafeClaw is a secure personal AI assistant with TEE support. This directory contains:
- **Desktop app** (Tauri v2 + React + TypeScript + Tailwind CSS)
- **Gateway library + CLI** (`crates/safeclaw/` — Rust library and standalone binary)

The Tauri app embeds the SafeClaw gateway as an in-process server, so the UI and backend share the same process.

## Development Commands

```bash
# Install frontend dependencies (pnpm only)
pnpm install

# Start frontend dev server only (port 8888)
pnpm dev

# Start Tauri dev mode (frontend + native window + embedded gateway)
pnpm tauri:dev

# Build production Tauri app
pnpm tauri:build

# Format frontend code with Biome
pnpm format

# Check/build the SafeClaw library
cd crates/safeclaw && cargo check
cd crates/safeclaw && cargo fmt
```

## Architecture

- **Build tool:** Rsbuild (Rspack-based)
- **Desktop runtime:** Tauri v2
- **State management:** Valtio
- **UI components:** shadcn/ui + Radix UI
- **Routing:** React Router v7 (hash router for Tauri compatibility)
- **Gateway:** Embedded SafeClaw gateway (in-process, port 18790)
- **Gateway bootstrap:** `crates/safeclaw/src/bootstrap.rs` — shared startup logic

## Key Directories

- `src/` — React frontend source
- `src-tauri/` — Tauri Rust backend (thin wrapper)
- `crates/safeclaw/` — SafeClaw gateway library + CLI binary
- `env/` — Environment variables
