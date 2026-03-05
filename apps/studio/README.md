# Studio

> One-person Company Workstation

A modern desktop application built with Tauri, React, and NestJS, designed for individual entrepreneurs and small teams.

## 🏗️ Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        Tauri Desktop                         │
│  ┌───────────────────────────────────────────────────────┐  │
│  │                   Frontend (React)                     │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌──────────────┐  │  │
│  │  │   Rsbuild   │  │   Router    │  │  Components  │  │  │
│  │  │   (Build)   │  │  (React)    │  │  (shadcn/ui) │  │  │
│  │  └─────────────┘  └─────────────┘  └──────────────┘  │  │
│  │         │                 │                 │          │  │
│  │         └─────────────────┴─────────────────┘          │  │
│  │                           │                             │  │
│  │                           ▼                             │  │
│  │                  HTTP Client (fetch)                    │  │
│  └───────────────────────────┬─────────────────────────────┘  │
│                              │                                │
│                              │ HTTP (localhost:3000)          │
│                              ▼                                │
│  ┌───────────────────────────────────────────────────────┐  │
│  │              Backend Sidecar (NestJS)                  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │           Application Layer (CQRS)              │  │  │
│  │  │  ┌──────────────┐      ┌──────────────────┐    │  │  │
│  │  │  │   Commands   │      │     Queries      │    │  │  │
│  │  │  └──────────────┘      └──────────────────┘    │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │              Domain Layer (DDD)                 │  │  │
│  │  │  ┌──────────┐  ┌──────────┐  ┌─────────────┐  │  │  │
│  │  │  │ Entities │  │   VOs    │  │  Services   │  │  │  │
│  │  │  └──────────┘  └──────────┘  └─────────────┘  │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  │  ┌─────────────────────────────────────────────────┐  │  │
│  │  │         Infrastructure Layer                    │  │  │
│  │  │  ┌──────────────┐      ┌──────────────────┐    │  │  │
│  │  │  │ In-Memory DB │      │  In-Memory Cache │    │  │  │
│  │  │  └──────────────┘      └──────────────────┘    │  │  │
│  │  └─────────────────────────────────────────────────┘  │  │
│  └───────────────────────────────────────────────────────┘  │
│                                                               │
│  Packaged as: Node.js SEA (Single Executable Application)    │
└─────────────────────────────────────────────────────────────┘
```

## 🚀 Tech Stack

### Frontend
- **Framework**: React 18 + TypeScript
- **Build Tool**: Rsbuild (Rspack-based)
- **Desktop**: Tauri 2
- **Router**: React Router v7
- **UI Components**: shadcn/ui (Radix UI + Tailwind CSS)
- **State Management**: Valtio
- **Animation**: Motion (Framer Motion)
- **Styling**: Tailwind CSS + CSS Modules

### Backend (Sidecar)
- **Framework**: NestJS 10
- **Architecture**: Domain-Driven Design (DDD) + CQRS
- **Compiler**: SWC (Fast TypeScript compilation)
- **Packaging**: Node.js SEA (Single Executable Application)
- **Storage**: In-memory (Map-based, no external dependencies)
- **API**: RESTful + Swagger/OpenAPI

### Build & Deployment
- **Package Manager**: pnpm (workspace)
- **Bundler**: esbuild (for SEA packaging)
- **Binary Injection**: postject (Node.js SEA tooling)
- **Desktop Packaging**: Tauri CLI

## 📦 Project Structure

```
studio/
├── src/                      # Frontend source code
│   ├── components/           # React components
│   ├── pages/                # Page components
│   ├── layouts/              # Layout components
│   ├── hooks/                # Custom React hooks
│   ├── lib/                  # Utility libraries
│   └── router.tsx            # Route configuration
├── src-tauri/                # Tauri Rust backend
│   ├── src/                  # Rust source code
│   ├── capabilities/         # Tauri capabilities
│   ├── icons/                # App icons
│   ├── sidecar/              # SEA binaries (output)
│   └── tauri.conf.json       # Tauri configuration
├── sidecar/                  # NestJS backend (embedded)
│   ├── apps/api/             # Main API application
│   │   ├── src/
│   │   │   ├── modules/      # Feature modules (DDD)
│   │   │   ├── shared/       # Shared utilities
│   │   │   ├── app.module.ts
│   │   │   └── main.ts
│   │   ├── scripts/
│   │   │   └── build-sea.mjs # SEA build script
│   │   └── package.json
│   └── packages/             # Shared packages
│       ├── kysely/           # Database utilities (unused)
│       └── redisson/         # Redis utilities (unused)
├── public/                   # Static assets
├── env/                      # Environment configs
├── package.json              # Frontend dependencies
├── rsbuild.config.ts         # Rsbuild configuration
├── tailwind.config.js        # Tailwind CSS config
└── README.md                 # This file
```

## 🛠️ Development

### Prerequisites
- Node.js 24+ (for SEA support)
- pnpm 9+
- Rust 1.70+ (for Tauri)

### Install Dependencies

```bash
pnpm install
```

This will automatically install both frontend and sidecar dependencies via `postinstall` hook.

### Development Modes

#### 1. Frontend Only (Fast)
```bash
pnpm dev
```
- Starts Rsbuild dev server on `http://localhost:8889`
- Hot reload enabled
- No backend (API calls will fail)

#### 2. Full Stack (Frontend + Backend)
```bash
pnpm dev:all
```
- Starts both UI and NestJS sidecar concurrently
- UI: `http://localhost:8889`
- API: `http://localhost:3000`
- Swagger: `http://localhost:3000/api/docs`

#### 3. Backend Only
```bash
pnpm dev:sidecar
```
- Installs dependencies
- Builds workspace packages
- Starts NestJS in watch mode

#### 4. Tauri Desktop App
```bash
pnpm tauri:dev
```
- Builds frontend
- Starts Tauri window with embedded webview
- Hot reload enabled

### Build for Production

#### Build Frontend
```bash
pnpm build
```

#### Build Sidecar (Node.js SEA)
```bash
pnpm build:sidecar
```

This will:
1. Compile TypeScript → JavaScript (via NestJS + SWC)
2. Bundle all dependencies → single `bundle.cjs` (via esbuild)
3. Generate SEA blob → `sea-prep.blob` (via Node.js)
4. Inject blob into Node binary → `studio-sidecar-<triple>[.exe]` (via postject)
5. Output to `src-tauri/sidecar/` for Tauri packaging

#### Build Desktop App
```bash
pnpm tauri:build
```

This will:
1. Build frontend (`pnpm build`)
2. Build sidecar SEA (`pnpm build:sidecar`)
3. Package Tauri app with embedded sidecar
4. Output installers to `src-tauri/target/release/bundle/`

## 🎯 Features

### Current
- ✅ Desktop application (Tauri 2)
- ✅ Modern React UI with shadcn/ui
- ✅ Embedded NestJS backend (Node.js SEA)
- ✅ In-memory storage (no external dependencies)
- ✅ Domain-Driven Design architecture
- ✅ CQRS pattern for business logic
- ✅ RESTful API with Swagger docs
- ✅ Hot reload in development
- ✅ Single executable packaging

### Planned
- 🚧 Order management demo (frontend integration)
- 🚧 Persistent storage (SQLite/PostgreSQL)
- 🚧 Redis caching layer
- 🚧 Authentication & authorization
- 🚧 Multi-window support
- 🚧 System tray integration
- 🚧 Auto-update mechanism

## 📝 API Documentation

When running `pnpm dev:all`, visit:
- **Swagger UI**: http://localhost:3000/api/docs
- **API Base**: http://localhost:3000/api

### Example Endpoints

#### Orders
- `POST /api/orders` - Create order
- `GET /api/orders` - List orders
- `GET /api/orders/:id` - Get order by ID
- `POST /api/orders/:id/confirm` - Confirm order
- `POST /api/orders/:id/cancel` - Cancel order

## 🔧 Configuration

### Environment Variables

Create `.env` files in `env/` directory:

```bash
env/
├── .env.development
├── .env.production
└── .env.local  # Git-ignored, for local overrides
```

Example `.env.development`:
```env
APP_PORT=3000
NODE_ENV=development
```

### Tauri Configuration

Edit `src-tauri/tauri.conf.json`:
- `identifier`: App bundle identifier
- `productName`: Display name
- `version`: App version
- `externalBin`: Sidecar binary path

## 🐛 Troubleshooting

### Sidecar fails to start
- Ensure Node.js 24+ is installed
- Check if port 3000 is available
- Run `pnpm dev:sidecar` separately to see detailed logs

### Frontend can't connect to backend
- Verify sidecar is running (`http://localhost:3000/api/docs`)
- Check CORS is enabled in `sidecar/apps/api/src/main.ts`
- In dev mode, rsbuild proxy should forward `/api` → `localhost:3000`

### Build fails
- Clear caches: `pnpm clean` (frontend) + `rm -rf sidecar/node_modules`
- Reinstall: `pnpm install`
- Check Node.js version: `node --version` (should be 24+)

### SEA binary doesn't work
- Verify `postject` is installed: `npx postject --version`
- Check platform-specific requirements:
  - **macOS**: `codesign` must be available
  - **Windows**: Binary must be unsigned or properly signed
  - **Linux**: No special requirements

## 📄 License

MIT

## 🤝 Contributing

This is a personal project template. Feel free to fork and adapt for your needs.

---

Built with ❤️ using Tauri, React, and NestJS
