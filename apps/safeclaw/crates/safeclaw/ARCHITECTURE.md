# SafeClaw Architecture Overview

Last updated: 2026-03-09

## Current Architecture

SafeClaw consists of two parallel architectures that serve different purposes:

### 1. Agent Architecture (Primary)
**Purpose**: Direct agent interaction via WebSocket
**Entry point**: `agent/` module
**Key components**:
- `AgentEngine` - Core engine wrapping a3s-code's SessionManager (2505 lines)
- `AgentSessionStore` - UI state persistence
- `AgentBus` - Inter-agent messaging via a3s-event
- WebSocket handlers for real-time communication

**Usage**: Desktop app, direct API access

### 2. Runtime Architecture (Legacy)
**Purpose**: Multi-channel webhook integration
**Entry point**: `runtime/` module
**Key components**:
- `Runtime` - Lifecycle orchestrator
- `SessionManager` - TEE-aware session management (659 lines)
- `MessageProcessor` - Webhook message processing
- Channel adapters (7 platforms)

**Usage**: Webhook endpoints, multi-channel bots

## Module Analysis

### Core Modules (Active)

| Module | Lines | External Usage | Status |
|--------|-------|----------------|--------|
| `agent/` | ~5000 | Primary interface | ✅ Active |
| `api/` | 1830 | HTTP routes | ✅ Active |
| `runtime/` | ~1500 | Webhook processing | ✅ Active |
| `channels/` | ~2000 | 7 adapters, all used | ✅ Active |
| `config/` | 888 | 22 types | ✅ Active |
| `workflows/` | ~1000 | Flow engine | ✅ Active |

### Support Modules

| Module | Lines | External Usage | Notes |
|--------|-------|----------------|-------|
| `session/` | 659 | 3 uses | Used by runtime only |
| `tee/` | 1452 | 9 uses | TEE detection & sealed storage |
| `bootstrap/` | 380 | Startup only | Initialization logic |
| `error/` | 52 | Everywhere | Error types |
| `hardening/` | 74 | Startup only | Process hardening |

### Removed Modules

- ~~`sentinel/`~~ - Removed (9 files, ~3000 lines)
- ~~`skills/`~~ - Removed (re-export wrapper)

## Code Quality Metrics

- **Total files**: 42 Rust files
- **Public items**: 128
- **Doc comments**: 909 (excellent coverage)
- **Clippy warnings**: 6 (all in test code)
- **Unused code**: None detected
- **Performance issues**: None detected (no nested Arc, no double clones)

## Architecture Decisions

### Why Two Architectures?

1. **Agent Architecture** (new):
   - Built on a3s-code framework
   - WebSocket-based real-time communication
   - Rich UI state management
   - Desktop app primary use case

2. **Runtime Architecture** (legacy):
   - Webhook-based integration
   - Multi-channel support (Telegram, Slack, Discord, etc.)
   - TEE-aware session routing
   - Bot/webhook use cases

Both are actively used and serve different purposes.

### TEE Module Design

The TEE module (1452 lines) provides:
- Runtime environment detection (SEV-SNP, VM, process-only)
- Sealed storage for secrets
- Security level reporting

External usage is minimal (9 references) but critical for:
- Security level reporting in API
- Session routing decisions
- Storage encryption

**Decision**: Keep as-is. Low usage is expected for infrastructure code.

### Session Module Design

The session module (659 lines) is used only by the runtime architecture:
- Session lifecycle management
- TEE upgrade decisions
- Routing logic

**Decision**: Keep as-is. It's a cohesive unit for webhook/channel processing.

### Channels Module Design

7 channel adapters, all actively used:
- Telegram, Slack, Discord, Feishu, DingTalk, WeCom, WebChat
- Each adapter: 400-650 lines
- Consistent interface via `ChannelAdapter` trait

**Decision**: Keep all adapters. Each serves real use cases.

## Optimization Opportunities

### 1. Feature Gates (Low Priority)
If webhook/channel functionality is rarely used, consider:
```toml
[features]
default = ["agent"]
agent = []
webhooks = ["channels", "runtime", "session", "tee"]
```

**Benefit**: Faster compilation for agent-only use cases
**Cost**: Increased maintenance complexity

### 2. API Consolidation (Low Priority)
The API has 76 functions across multiple routers. Consider:
- Grouping related endpoints into sub-routers
- Extracting common response patterns

**Benefit**: Easier to navigate
**Cost**: Refactoring effort

### 3. Documentation (Medium Priority)
Add architecture diagrams:
- Agent architecture flow
- Runtime/webhook architecture flow
- Module dependency graph

**Benefit**: Easier onboarding
**Cost**: Maintenance overhead

## Recommendations

### Short Term (Done ✅)
- ✅ Remove unused modules (sentinel, skills)
- ✅ Apply clippy fixes
- ✅ Format all code
- ✅ Document architecture

### Medium Term (Optional)
- Consider feature gates if compilation time becomes an issue
- Add architecture diagrams to docs/
- Extract common API response patterns

### Long Term (Strategic)
- Evaluate if both architectures are still needed
- Consider unifying session management if possible
- Monitor TEE module usage as hardware TEE adoption grows

## Conclusion

SafeClaw's codebase is in good shape:
- Clean architecture with clear separation of concerns
- Excellent documentation coverage
- No dead code or performance issues
- Two architectures serve different, valid use cases

The main complexity comes from supporting both agent-based and webhook-based interactions, which is a feature, not a bug.
