# SafeClaw API Specification

> Version: 0.2.0 | Status: Draft | Last Updated: 2025-07-15

## Table of Contents

- [Overview](#overview)
- [Conventions](#conventions)
- [Authentication](#authentication)
- [1. Agent Chat](#1-agent-chat)
- [2. Events](#2-events)
- [3. Settings](#3-settings)
- [4. Common Endpoints](#4-common-endpoints)
- [5. WebSocket Protocol](#5-websocket-protocol)
- [Implementation Priority](#implementation-priority)

---

## Overview

SafeClaw exposes a REST + WebSocket API for its UI pages. The AI agent backend is powered by **a3s-code** `SessionManager`, which provides the full agent loop: LLM conversation, tool calling, HITL (human-in-the-loop) confirmation, and session persistence.

### Architecture

```
Browser (React UI)
  │
  ├── REST  ──→  Axum Router (/api/v1/...)  ──→  Handlers  ──→  a3s-code SessionManager
  │                                                           ──→  Domain stores (Events, ...)
  └── WS    ──→  /ws/agent/browser/:id      ──→  AgentEngine ──→  SessionManager.generate_streaming()
                                                               ──→  AgentEvent → BrowserIncomingMessage
```

### Base URLs

| Protocol | URL |
|----------|-----|
| REST API | `http://127.0.0.1:18790/api/v1` |
| WebSocket | `ws://127.0.0.1:18790/ws/...` |
| Health (no prefix) | `http://127.0.0.1:18790/health` |

---

## Conventions

### URL & JSON Style

- URL paths: `kebab-case` (e.g., `/api/v1/my-agents`)
- JSON fields: `camelCase` (e.g., `sessionId`, `totalCost`)
- Timestamps: Unix milliseconds (`1707753600000`) unless noted otherwise

### Pagination

Query: `?page=1&perPage=20`

```json
{
  "data": [],
  "pagination": {
    "page": 1,
    "perPage": 20,
    "total": 142,
    "totalPages": 8
  }
}
```

### Filtering & Search

- Full-text search: `?q=keyword`
- Field filters: `?category=finance&status=running`
- Date range: `?since=1707753600000` (Unix ms)
- Sorting: `?sortBy=updatedAt&sortOrder=desc`

### Error Response

```json
{
  "error": {
    "code": "NOT_FOUND",
    "message": "Session sess-abc123 not found"
  }
}
```

Standard error codes: `BAD_REQUEST`, `UNAUTHORIZED`, `FORBIDDEN`, `NOT_FOUND`, `CONFLICT`, `INTERNAL_ERROR`.

### Authentication (Reserved)

```
Authorization: Bearer <token>
```

Not enforced in v0.1. All endpoints are open. Auth header is accepted and ignored.

---

## 1. Agent Chat

The agent chat system is backed by **a3s-code** `SessionManager`. Each session wraps an LLM agent loop with tool execution, permission management, and streaming output. Session CRUD is REST; real-time chat flows through WebSocket.

### Endpoints (8)

| Method | Path | Description |
|--------|------|-------------|
| POST | `/api/v1/agent/sessions` | Create session |
| GET | `/api/v1/agent/sessions` | List sessions |
| GET | `/api/v1/agent/sessions/:id` | Get session detail |
| PATCH | `/api/v1/agent/sessions/:id` | Update session |
| DELETE | `/api/v1/agent/sessions/:id` | Delete session |
| POST | `/api/v1/agent/sessions/:id/relaunch` | Relaunch session |
| GET | `/api/v1/agent/backends` | List available models |
| WS | `/ws/agent/browser/:id` | Real-time chat |

### POST `/api/v1/agent/sessions`

Create a new agent session. This calls `SessionManager::create_session()` under the hood, initializing an LLM client, tool set, and permission policy.

**Request:**

```json
{
  "model": "claude-sonnet-4-20250514",
  "permissionMode": "default",
  "cwd": "/home/user/project",
  "baseUrl": "https://api.anthropic.com",
  "apiKey": "sk-...",
  "systemPrompt": "You are a financial analyst...",
  "skills": ["code-review", "test-writer"]
}
```

All fields are optional. `model` defaults to the configured default model. `permissionMode` defaults to `"default"`. `cwd` defaults to the server working directory.

**Response:** `201 Created`

```json
{
  "sessionId": "sess-a1b2c3",
  "pid": 12345,
  "state": "connected",
  "model": "claude-sonnet-4-20250514",
  "permissionMode": "default",
  "cwd": "/home/user/project",
  "createdAt": "2025-01-15T10:30:00Z",
  "archived": false,
  "name": null
}
```

### GET `/api/v1/agent/sessions`

List all sessions. Backed by `SessionManager::list_sessions()` plus UI-layer metadata (name, archived state).

**Query Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `archived` | bool | Filter by archived status. Default: show all |

**Response:** `200 OK`

```json
[
  {
    "sessionId": "sess-a1b2c3",
    "pid": 12345,
    "state": "connected",
    "model": "claude-sonnet-4-20250514",
    "permissionMode": "default",
    "cwd": "/home/user/project",
    "createdAt": "2025-01-15T10:30:00Z",
    "cliSessionId": "cli-xyz",
    "archived": false,
    "name": "Refactor auth module"
  }
]
```

`state` is one of: `"starting"`, `"connected"`, `"running"`, `"exited"`.

### GET `/api/v1/agent/sessions/:id`

Get session detail. Returns the same shape as the list item.

**Response:** `200 OK` — `AgentProcessInfo` object (same as list item).

**Error:** `404` if session not found.

### PATCH `/api/v1/agent/sessions/:id`

Update session metadata (UI-layer only, does not affect the a3s-code session).

**Request:**

```json
{
  "name": "Auth module refactor",
  "archived": true
}
```

Both fields optional. At least one must be provided.

**Response:** `200 OK` — Updated `AgentProcessInfo`.

### DELETE `/api/v1/agent/sessions/:id`

Delete a session. Calls `SessionManager::destroy_session()` to clean up the agent loop, then removes UI-layer state.

**Response:** `204 No Content`

### POST `/api/v1/agent/sessions/:id/relaunch`

Destroy and recreate a session with the same configuration. Useful when a session enters an error state.

**Response:** `200 OK` — New `AgentProcessInfo`.

### GET `/api/v1/agent/backends`

List available model backends. Derived from `CodeConfig.providers` and their configured models.

**Response:** `200 OK`

```json
[
  {
    "id": "claude-sonnet-4-20250514",
    "name": "Claude Sonnet 4",
    "provider": "anthropic",
    "isDefault": true
  },
  {
    "id": "gpt-4o",
    "name": "GPT-4o",
    "provider": "openai",
    "isDefault": false
  }
]
```

### WS `/ws/agent/browser/:id`

WebSocket connection for real-time agent chat. See [Section 5: WebSocket Protocol](#5-websocket-protocol) for the full message schema.

Connection flow:
1. Client opens WebSocket to `/ws/agent/browser/:id`
2. Server sends `session_init` with current `AgentSessionState`
3. Server replays `message_history` if reconnecting
4. Client sends `user_message` to start generation
5. Server streams `AgentEvent`s translated to `BrowserIncomingMessage`s

---

## 2. Events

Events represent real-time signals from external sources (market data, news, social media) and internal triggers (system alerts, compliance flags, task completions). Personas can subscribe to event categories and the system routes events to relevant agents.

### Endpoints (5)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/events` | List events |
| GET | `/api/v1/events/:id` | Event detail |
| POST | `/api/v1/events` | Create event |
| GET | `/api/v1/events/counts` | Category counts |
| PUT | `/api/v1/events/subscriptions/:personaId` | Update subscriptions |

### GET `/api/v1/events`

**Query Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `category` | string | `market\|news\|social\|task\|system\|compliance` |
| `q` | string | Full-text search on summary/detail |
| `since` | number | Unix ms timestamp, events after this time |
| `page` | number | Page number (default: 1) |
| `perPage` | number | Items per page (default: 20) |

**Response:** `200 OK` — Paginated `EventItem[]`

### GET `/api/v1/events/:id`

**Response:** `200 OK`

```json
{
  "id": "evt-1",
  "category": "market",
  "topic": "forex.usd_cny",
  "summary": "USD/CNY broke through 7.35",
  "detail": "Exchange rate: 7.3521 (+0.42%), triggered by Fed policy signal",
  "timestamp": 1707753600000,
  "source": "Reuters Forex",
  "subscribers": ["financial-analyst", "risk-analyst"],
  "reacted": true,
  "reactedAgent": "financial-analyst"
}
```

### POST `/api/v1/events`

Create an event (triggered by system or agent).

**Request:**

```json
{
  "category": "system",
  "topic": "deploy.gateway",
  "summary": "Gateway v3.12.1 deployed successfully",
  "detail": "Zero-downtime rolling update completed in 45s",
  "source": "CI/CD Pipeline",
  "subscribers": ["devops-engineer"]
}
```

`id` and `timestamp` are server-generated.

**Response:** `201 Created` — Full `EventItem`.

### GET `/api/v1/events/counts`

**Query Parameters:**

| Param | Type | Description |
|-------|------|-------------|
| `since` | number | Unix ms timestamp |

**Response:** `200 OK`

```json
{
  "market": 24,
  "news": 18,
  "social": 12,
  "task": 31,
  "system": 8,
  "compliance": 5,
  "total": 98
}
```

### PUT `/api/v1/events/subscriptions/:personaId`

Update which event categories a persona subscribes to.

**Request:**

```json
{
  "categories": ["market", "compliance", "system"]
}
```

**Response:** `200 OK`

```json
{
  "personaId": "financial-analyst",
  "categories": ["market", "compliance", "system"]
}
```

---

## 3. Settings

Application settings management. API keys are stored server-side and returned in masked form. The settings model covers LLM provider configuration, gateway behavior, and UI preferences.

### Endpoints (4)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/settings` | Get settings |
| PATCH | `/api/v1/settings` | Update settings |
| POST | `/api/v1/settings/reset` | Reset to defaults |
| GET | `/api/v1/settings/info` | Server info |

### Settings Schema

```json
{
  "provider": "anthropic",
  "model": "claude-sonnet-4-20250514",
  "baseUrl": "",
  "apiKey": "sk-ant-...****7f3a",
  "gateway": {
    "listenAddr": "127.0.0.1:18790",
    "teeEnabled": false,
    "corsOrigins": ["http://localhost:1420"]
  },
  "privacy": {
    "classificationEnabled": true,
    "sensitivePatterns": ["SSN", "credit_card"],
    "redactionEnabled": false
  },
  "storage": {
    "backend": "file",
    "sessionsDir": "~/.safeclaw/sessions"
  }
}
```

### GET `/api/v1/settings`

Returns current settings. API keys are masked (first 8 + last 4 characters visible).

**Response:** `200 OK` — `Settings` object.

### PATCH `/api/v1/settings`

Partial update. Only provided fields are changed.

**Request:**

```json
{
  "provider": "openai",
  "model": "gpt-4o",
  "apiKey": "sk-proj-..."
}
```

When `apiKey` is provided, it is stored in full. The response returns the masked version.

**Response:** `200 OK` — Updated `Settings` (with masked API key).

### POST `/api/v1/settings/reset`

Reset all settings to defaults. This does not delete sessions or knowledge data.

**Response:** `200 OK` — Default `Settings`.

### GET `/api/v1/settings/info`

Server runtime information.

**Response:** `200 OK`

```json
{
  "version": "0.3.1",
  "buildDate": "2025-01-15",
  "rustVersion": "1.83.0",
  "os": "macos-aarch64",
  "uptime": 86400,
  "sessionsDir": "/Users/user/.safeclaw/sessions",
  "configPath": "/Users/user/.safeclaw/config.toml",
  "a3sCodeVersion": "0.3.1",
  "features": {
    "tee": false,
    "privacy": true,
    "gateway": true
  }
}
```

---

## 4. Common Endpoints

### Health & Status (existing, no v1 prefix)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/health` | Health check |
| GET | `/status` | Gateway status |

#### GET `/health`

**Response:** `200 OK`

```json
{
  "status": "ok",
  "version": "0.3.1"
}
```

#### GET `/status`

**Response:** `200 OK`

```json
{
  "state": "Running",
  "teeEnabled": false,
  "sessionCount": 3,
  "channels": ["webchat"],
  "a3sGatewayMode": false
}
```

### Personas (4)

Personas are agent identities with avatars, system prompts, and default configurations. They are the universal foreign key across events, systems, projects, and marketplace. SafeClaw ships with 13 builtin personas; users can create custom ones.

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/personas` | List all personas |
| GET | `/api/v1/personas/:id` | Persona detail |
| POST | `/api/v1/personas` | Create custom persona |
| PATCH | `/api/v1/personas/:id` | Update persona |

#### AgentPersona Schema

```json
{
  "id": "financial-analyst",
  "name": "Financial Analyst",
  "description": "Senior financial analysis and reporting specialist",
  "avatar": {
    "sex": "woman",
    "faceColor": "#F9C9B6",
    "earSize": "small",
    "eyeStyle": "circle",
    "noseStyle": "round",
    "mouthStyle": "smile",
    "shirtStyle": "polo",
    "glassesStyle": "none",
    "hairColor": "#000",
    "hairStyle": "womanLong",
    "hatStyle": "none",
    "hatColor": "#000",
    "eyeBrowStyle": "up",
    "shirtColor": "#6BD9E9",
    "bgColor": "#E0DDFF"
  },
  "systemPrompt": "You are a senior financial analyst...",
  "defaultModel": "claude-sonnet-4-20250514",
  "defaultPermissionMode": "default",
  "builtin": true,
  "undeletable": true
}
```

`avatar`: `react-nice-avatar` full configuration object. The frontend renders this directly.

`builtin`: `true` for the 13 shipped personas, `false` for user-created ones.

`undeletable`: `true` for personas that cannot be removed (core system personas).

#### GET `/api/v1/personas`

**Response:** `200 OK` — `AgentPersona[]`

Returns all personas (builtin + custom). No pagination (typically < 50).

#### GET `/api/v1/personas/:id`

**Response:** `200 OK` — `AgentPersona`.

#### POST `/api/v1/personas`

**Request:**

```json
{
  "name": "Tax Specialist",
  "description": "Corporate tax planning and compliance",
  "avatar": { "sex": "man", "faceColor": "#F9C9B6" },
  "systemPrompt": "You are a tax specialist...",
  "defaultModel": "claude-sonnet-4-20250514",
  "defaultPermissionMode": "default"
}
```

`id` is auto-generated from `name` (kebab-case).

**Response:** `201 Created` — `AgentPersona`.

#### PATCH `/api/v1/personas/:id`

Update a custom persona. Builtin personas cannot be modified (returns `403`).

**Request:**

```json
{
  "name": "Senior Tax Specialist",
  "systemPrompt": "Updated prompt..."
}
```

**Response:** `200 OK` — Updated `AgentPersona`.

### User Profile (1)

| Method | Path | Description |
|--------|------|-------------|
| GET | `/api/v1/user/profile` | Current user info |

#### GET `/api/v1/user/profile`

**Response:** `200 OK`

```json
{
  "id": 1,
  "nickname": "Roy Lin",
  "email": "admin@elljs.com",
  "avatar": "https://github.com/user.png"
}
```

Currently returns a hardcoded user. Will be backed by auth system in the future.

---

## 5. WebSocket Protocol

The primary real-time channel is `/ws/agent/browser/:id`, connecting the browser to an agent session. The protocol uses JSON messages with a `type` discriminator.

### Connection Lifecycle

```
Browser                          Server
  │                                │
  ├── WS CONNECT ────────────────→ │
  │                                ├── session_init (AgentSessionState)
  │                                ├── message_history (if reconnecting)
  │  ←──────────────────────────── │
  │                                │
  ├── user_message ──────────────→ │
  │                                ├── SessionManager.generate_streaming()
  │                                ├── stream_event (TextDelta, ToolStart, ...)
  │                                ├── assistant (complete message)
  │                                ├── tool_progress / tool_use_summary
  │                                ├── permission_request (if HITL needed)
  │  ←──────────────────────────── │
  │                                │
  ├── permission_response ───────→ │  (allow/deny tool execution)
  ├── interrupt ─────────────────→ │  (cancel current generation)
  ├── set_model ─────────────────→ │  (switch model mid-session)
  ├── set_permission_mode ───────→ │  (change permission policy)
  │                                │
```

### Server → Browser Messages

#### `session_init`

Sent immediately after WebSocket connection. Contains the full session state.

```json
{
  "type": "session_init",
  "sessionState": {
    "session_id": "sess-a1b2c3",
    "model": "claude-sonnet-4-20250514",
    "cwd": "/home/user/project",
    "tools": ["bash", "read", "write", "edit", "grep", "glob", "ls"],
    "permission_mode": "default",
    "mcp_servers": [{ "name": "github", "status": "connected" }],
    "agents": [],
    "slash_commands": [],
    "skills": ["code-review"],
    "total_cost_usd": 0.0,
    "num_turns": 0,
    "context_used_percent": 0.0,
    "is_compacting": false,
    "total_lines_added": 0,
    "total_lines_removed": 0
  }
}
```

#### `session_update`

Partial update to session state (e.g., after model switch, cost change).

```json
{
  "type": "session_update",
  "sessionState": {
    "total_cost_usd": 0.042,
    "num_turns": 3,
    "context_used_percent": 12.5
  }
}
```

#### `assistant`

A complete assistant message (sent after streaming finishes or for non-streamed responses).

```json
{
  "type": "assistant",
  "message": {
    "id": "msg-1",
    "role": "assistant",
    "content": "I'll help you refactor that module.",
    "contentBlocks": [
      { "type": "text", "text": "I'll help you refactor that module." }
    ],
    "timestamp": "2025-01-15T10:30:00Z",
    "model": "claude-sonnet-4-20250514",
    "stopReason": "end_turn"
  }
}
```

#### `stream_event`

Real-time streaming events from the agent loop. Maps from a3s-code `AgentEvent` variants.

```json
{ "type": "stream_event", "event": "turn_start" }
{ "type": "stream_event", "event": "text_delta", "delta": "Here's the " }
{ "type": "stream_event", "event": "text_delta", "delta": "refactored code:" }
{ "type": "stream_event", "event": "tool_start", "toolName": "write", "toolUseId": "tu-1" }
{ "type": "stream_event", "event": "tool_end", "toolUseId": "tu-1" }
{ "type": "stream_event", "event": "turn_end" }
```

#### `result`

Generation completed.

```json
{
  "type": "result",
  "result": "Generation completed successfully",
  "subtype": "success"
}
```

`subtype`: `success` | `error` | `interrupted` | `max_turns`

#### `permission_request`

HITL confirmation required for a tool call. The UI must display this and send back a `permission_response`.

```json
{
  "type": "permission_request",
  "permission": {
    "request_id": "perm-1",
    "tool_name": "bash",
    "input": { "command": "rm -rf /tmp/old-build" },
    "description": "Execute shell command",
    "tool_use_id": "tu-2",
    "timestamp": "2025-01-15T10:30:05Z"
  }
}
```

#### `permission_cancelled`

A previously requested permission is no longer needed (e.g., generation was interrupted).

```json
{
  "type": "permission_cancelled",
  "requestId": "perm-1"
}
```

#### `tool_progress`

Progress update during long-running tool execution.

```json
{
  "type": "tool_progress",
  "toolUseId": "tu-1",
  "toolName": "bash",
  "progress": "Running tests... 42/100 passed"
}
```

#### `tool_use_summary`

Summary after a tool call completes.

```json
{
  "type": "tool_use_summary",
  "toolUseId": "tu-1",
  "toolName": "write",
  "summary": "Wrote 45 lines to src/handler.rs",
  "isError": false
}
```

#### `status_change`

Session status changed (e.g., compacting context).

```json
{
  "type": "status_change",
  "status": "compacting"
}
```

#### `error`

An error occurred during generation.

```json
{
  "type": "error",
  "error": "Rate limit exceeded, retrying in 30s",
  "code": "RATE_LIMIT"
}
```

#### `user_message`

Echo of the user's message (for multi-device sync / history replay).

```json
{
  "type": "user_message",
  "message": {
    "id": "msg-0",
    "role": "user",
    "content": "Refactor the auth module",
    "timestamp": "2025-01-15T10:29:55Z"
  }
}
```

#### `message_history`

Full message history replay (sent on reconnection).

```json
{
  "type": "message_history",
  "messages": [
    { "id": "msg-0", "role": "user", "content": "...", "timestamp": "..." },
    { "id": "msg-1", "role": "assistant", "content": "...", "timestamp": "..." }
  ]
}
```

#### `session_name_update`

Auto-generated session name (from `SessionManager::generate_title()`).

```json
{
  "type": "session_name_update",
  "sessionId": "sess-a1b2c3",
  "name": "Auth module refactor"
}
```

### Browser → Server Messages

#### `user_message`

Send a message to the agent. Triggers `SessionManager::generate_streaming()`.

```json
{
  "type": "user_message",
  "content": "Refactor the auth module to use JWT",
  "images": [
    { "media_type": "image/png", "data": "base64..." }
  ]
}
```

`images` is optional. Supports vision-capable models.

#### `permission_response`

Respond to a `permission_request`.

```json
{
  "type": "permission_response",
  "request_id": "perm-1",
  "allowed": true
}
```

#### `interrupt`

Cancel the current generation. Calls `SessionManager::cancel_operation()`.

```json
{
  "type": "interrupt"
}
```

#### `set_model`

Switch the LLM model for this session. Calls `SessionManager::configure()`.

```json
{
  "type": "set_model",
  "model": "claude-sonnet-4-20250514"
}
```

#### `set_permission_mode`

Change the permission policy for this session.

```json
{
  "type": "set_permission_mode",
  "mode": "auto-accept"
}
```

`mode`: `default` | `auto-accept` | `deny-all`

---

## Implementation Priority

| Phase | Scope | Endpoints | Description |
|-------|-------|-----------|-------------|
| P0 | Agent Chat + Settings + Personas | 17 | Already implemented. Migrate to `/api/v1` prefix. |
| P1 | Events | 5 | Event routing and subscription management. |

### Total: 24 endpoints (8 Agent + 5 Events + 4 Settings + 7 Common)

### P0 → P1 Migration Notes

The existing agent endpoints (`/api/agent/sessions/...`) should be aliased to `/api/v1/agent/sessions/...`. The old paths can remain as deprecated aliases during the transition period.

### Backend Dependencies

| Domain | Storage | a3s-code Integration |
|--------|---------|---------------------|
| Agent Chat | `SessionManager` (in-memory + file persistence) | Direct — all CRUD and generation |
| Events | `EventStore` (file-based JSON) | Indirect — agents can create events via tool calls |
| Settings | `SafeClawConfig` (TOML file) | Direct — model/provider config feeds into `CodeConfig` |
| Personas | `PersonaStore` (builtin JSON + custom file) | Direct — persona's `systemPrompt` and `defaultModel` used in session creation |
