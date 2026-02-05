# MCP (Model Context Protocol) Integration Design

## Overview

This document describes the design for integrating Model Context Protocol (MCP) support into A3S Code, enabling connection to external tools and data sources through a standardized interface.

## What is MCP?

MCP is an open protocol developed by Anthropic for connecting AI assistants to external tools and data sources. It provides a standardized way for AI models to interact with external systems.

```
┌─────────────────┐                    ┌─────────────────┐
│   AI Agent      │  ◄── JSON-RPC ──►  │   MCP Server    │
│  (A3S Code)     │     (stdio/HTTP)   │  (Tools/Data)   │
└─────────────────┘                    └─────────────────┘
```

## MCP Core Concepts

| Concept | Description |
|---------|-------------|
| **Tools** | Callable functions provided by MCP servers |
| **Resources** | Data resources (files, databases, etc.) |
| **Prompts** | Pre-defined prompt templates |
| **Sampling** | MCP server requesting AI to generate content |

## Transport Types

| Transport | Description | Use Case |
|-----------|-------------|----------|
| **stdio** | Standard input/output | Local process communication |
| **HTTP + SSE** | HTTP requests + Server-Sent Events | Remote servers |

## Common MCP Servers

| Server | Functionality |
|--------|---------------|
| `@modelcontextprotocol/server-filesystem` | File system access |
| `@modelcontextprotocol/server-github` | GitHub API |
| `@modelcontextprotocol/server-postgres` | PostgreSQL database |
| `@modelcontextprotocol/server-slack` | Slack integration |
| `@modelcontextprotocol/server-memory` | Persistent memory |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                        A3S Code                                  │
├─────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌───────────────────┐   │
│  │ Agent Loop  │───►│ MCP Manager │───►│ MCP Server Pool   │   │
│  └─────────────┘    └─────────────┘    │  ┌─────────────┐  │   │
│         │                  │           │  │ stdio server│  │   │
│         ▼                  ▼           │  └─────────────┘  │   │
│  ┌─────────────┐    ┌─────────────┐    │  ┌─────────────┐  │   │
│  │ToolRegistry │◄───│ MCP Tools   │    │  │ HTTP server │  │   │
│  │ (unified)   │    │ (dynamic)   │    │  └─────────────┘  │   │
│  └─────────────┘    └─────────────┘    └───────────────────┘   │
│         │                                                       │
│         ▼                                                       │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │ Tools: mcp__github__create_issue, mcp__postgres__query  │   │
│  └─────────────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────────────┘
```

## File Structure

```
crates/code/src/
├── mcp/
│   ├── mod.rs              # Module exports
│   ├── manager.rs          # MCP server lifecycle management
│   ├── client.rs           # MCP client (JSON-RPC)
│   ├── transport/
│   │   ├── mod.rs          # Transport layer abstraction
│   │   ├── stdio.rs        # stdio transport
│   │   └── http.rs         # HTTP + SSE transport
│   ├── protocol.rs         # MCP protocol type definitions
│   ├── auth.rs             # OAuth authentication
│   └── tools.rs            # MCP tool registration to ToolRegistry
└── proto/
    └── mcp.proto           # gRPC API extension (optional)
```

---

## Core Components

### 1. MCP Configuration

```rust
/// MCP server configuration
pub struct McpServerConfig {
    /// Server name (used for tool prefix, e.g., mcp__github__)
    pub name: String,
    /// Transport type
    pub transport: McpTransport,
    /// Whether enabled
    pub enabled: bool,
    /// Environment variables
    pub env: HashMap<String, String>,
    /// OAuth configuration (optional)
    pub oauth: Option<OAuthConfig>,
}

/// Transport configuration
pub enum McpTransport {
    /// Local process (stdio)
    Stdio {
        command: String,
        args: Vec<String>,
    },
    /// Remote HTTP + SSE
    Http {
        url: String,
        headers: HashMap<String, String>,
    },
}

/// OAuth configuration
pub struct OAuthConfig {
    pub auth_url: String,
    pub token_url: String,
    pub client_id: String,
    pub client_secret: Option<String>,
    pub scopes: Vec<String>,
    pub redirect_uri: String,
}
```

### 2. MCP Protocol Types

```rust
/// MCP tool definition
pub struct McpTool {
    pub name: String,
    pub description: Option<String>,
    pub input_schema: JsonValue,
}

/// Tool call result
pub struct CallToolResult {
    pub content: Vec<ToolContent>,
    pub is_error: bool,
}

/// Tool output content
pub enum ToolContent {
    Text { text: String },
    Image { data: String, mime_type: String },
    Resource { resource: ResourceContent },
}

/// MCP resource
pub struct McpResource {
    pub uri: String,
    pub name: String,
    pub description: Option<String>,
    pub mime_type: Option<String>,
}
```

### 3. Transport Layer

```rust
/// MCP transport trait
#[async_trait]
pub trait McpTransport: Send + Sync {
    /// Send request and wait for response
    async fn request(&self, method: &str, params: Option<JsonValue>) -> Result<JsonValue>;

    /// Send notification (no response)
    async fn notify(&self, method: &str, params: Option<JsonValue>) -> Result<()>;

    /// Get notification receiver
    fn notifications(&self) -> mpsc::Receiver<McpNotification>;

    /// Close connection
    async fn close(&self) -> Result<()>;
}
```

### 4. MCP Client

```rust
pub struct McpClient {
    pub name: String,
    transport: Arc<dyn McpTransport>,
    capabilities: ServerCapabilities,
    tools: RwLock<Vec<McpTool>>,
    resources: RwLock<Vec<McpResource>>,
}

impl McpClient {
    /// Create and initialize MCP client
    pub async fn connect(name: String, transport: Arc<dyn McpTransport>) -> Result<Self>;

    /// Refresh tool list
    pub async fn refresh_tools(&self) -> Result<()>;

    /// Get tool list
    pub async fn list_tools(&self) -> Vec<McpTool>;

    /// Call tool
    pub async fn call_tool(&self, name: &str, arguments: Option<JsonValue>) -> Result<CallToolResult>;

    /// List resources
    pub async fn list_resources(&self) -> Result<Vec<McpResource>>;

    /// Read resource
    pub async fn read_resource(&self, uri: &str) -> Result<ReadResourceResult>;
}
```

### 5. MCP Manager

```rust
pub struct McpManager {
    /// Connected servers
    servers: RwLock<HashMap<String, Arc<McpClient>>>,
    /// Server configurations
    configs: RwLock<HashMap<String, McpServerConfig>>,
    /// Tool change callback
    on_tools_changed: Option<Box<dyn Fn() + Send + Sync>>,
}

impl McpManager {
    /// Register server configuration
    pub async fn register_server(&self, config: McpServerConfig);

    /// Connect to server
    pub async fn connect(&self, name: &str) -> Result<()>;

    /// Disconnect from server
    pub async fn disconnect(&self, name: &str) -> Result<()>;

    /// Get all MCP tools (with prefix)
    pub async fn get_all_tools(&self) -> Vec<(String, McpTool)>;

    /// Call MCP tool
    pub async fn call_tool(&self, full_name: &str, arguments: Option<JsonValue>) -> Result<CallToolResult>;

    /// Get server status
    pub async fn get_status(&self) -> HashMap<String, McpServerStatus>;
}
```

---

## Tool Naming Convention

MCP tools are registered with the prefix `mcp__{server}__{tool}`:

| Full Name | Server | Tool |
|-----------|--------|------|
| `mcp__github__create_issue` | github | create_issue |
| `mcp__github__search_repos` | github | search_repos |
| `mcp__postgres__query` | postgres | query |
| `mcp__filesystem__read_file` | filesystem | read_file |

---

## gRPC API Extension

```protobuf
// MCP management
rpc RegisterMcpServer(RegisterMcpServerRequest) returns (RegisterMcpServerResponse);
rpc ConnectMcpServer(ConnectMcpServerRequest) returns (ConnectMcpServerResponse);
rpc DisconnectMcpServer(DisconnectMcpServerRequest) returns (DisconnectMcpServerResponse);
rpc ListMcpServers(ListMcpServersRequest) returns (ListMcpServersResponse);
rpc GetMcpTools(GetMcpToolsRequest) returns (GetMcpToolsResponse);

// OAuth
rpc StartMcpOAuth(StartMcpOAuthRequest) returns (StartMcpOAuthResponse);
rpc CompleteMcpOAuth(CompleteMcpOAuthRequest) returns (CompleteMcpOAuthResponse);
```

---

## Configuration File Format

```yaml
# mcp.yaml
servers:
  # Local stdio server
  github:
    transport:
      type: stdio
      command: npx
      args:
        - -y
        - "@modelcontextprotocol/server-github"
    env:
      GITHUB_TOKEN: "${GITHUB_TOKEN}"
    enabled: true

  filesystem:
    transport:
      type: stdio
      command: npx
      args:
        - -y
        - "@modelcontextprotocol/server-filesystem"
        - "/workspace"
    enabled: true

  # Remote HTTP server
  custom-api:
    transport:
      type: http
      url: "https://mcp.example.com/api"
      headers:
        Authorization: "Bearer ${API_TOKEN}"
    enabled: true

  # Server requiring OAuth
  slack:
    transport:
      type: stdio
      command: npx
      args:
        - -y
        - "@modelcontextprotocol/server-slack"
    oauth:
      auth_url: "https://slack.com/oauth/v2/authorize"
      token_url: "https://slack.com/api/oauth.v2.access"
      client_id: "${SLACK_CLIENT_ID}"
      client_secret: "${SLACK_CLIENT_SECRET}"
      scopes:
        - "channels:read"
        - "chat:write"
      redirect_uri: "http://localhost:8080/oauth/callback"
    enabled: true
```

---

## Usage Flow

```
┌─────────────────────────────────────────────────────────────────┐
│                        Usage Flow                                │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Register MCP Server                                          │
│     RegisterMcpServer(config)                                   │
│              │                                                   │
│              ▼                                                   │
│  2. Connect Server (optional OAuth)                              │
│     ConnectMcpServer(name)                                      │
│     ├─ If OAuth needed: StartMcpOAuth() → User auth → Complete  │
│     └─ Direct connect: Start process/HTTP connection            │
│              │                                                   │
│              ▼                                                   │
│  3. Tools Auto-registered to ToolRegistry                        │
│     mcp__github__create_issue                                   │
│     mcp__github__search_repos                                   │
│     mcp__postgres__query                                        │
│              │                                                   │
│              ▼                                                   │
│  4. Agent Uses Tools                                             │
│     Agent: "I need to create a GitHub issue"                    │
│     → Call mcp__github__create_issue                            │
│     → MCP Manager routes to github server                       │
│     → Return result                                              │
│              │                                                   │
│              ▼                                                   │
│  5. Disconnect                                                   │
│     DisconnectMcpServer(name)                                   │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Implementation Phases

| Phase | Tasks | Priority |
|-------|-------|----------|
| **Phase 1: Infrastructure** | |
| | Define MCP protocol types | P0 |
| | Implement JSON-RPC basics | P0 |
| | Implement Stdio transport | P0 |
| | Implement MCP Client | P0 |
| **Phase 2: Core Features** | |
| | Implement MCP Manager | P0 |
| | Implement tool list retrieval | P0 |
| | Implement tool calling | P0 |
| | Integrate with ToolRegistry | P0 |
| **Phase 3: gRPC API** | |
| | Extend proto definitions | P1 |
| | Implement gRPC handlers | P1 |
| | Add config file loading | P1 |
| **Phase 4: Advanced Features** | |
| | Implement HTTP + SSE transport | P2 |
| | Implement OAuth authentication | P2 |
| | Implement resource access | P2 |
| | Implement Prompts support | P3 |
| **Phase 5: Testing & Docs** | |
| | Unit tests | P1 |
| | Integration tests (mock server) | P1 |
| | Documentation and examples | P2 |

## Dependencies

```toml
serde_json = "1.0"
eventsource-stream = "0.2"  # SSE parsing
oauth2 = "4.4"              # OAuth
dashmap = "5.5"             # Concurrent HashMap
```

---

## Error Handling

```rust
pub enum McpError {
    SpawnFailed(String, std::io::Error),
    ServerNotFound(String),
    ServerDisabled(String),
    ServerNotConnected(String),
    InvalidToolName(String),
    RpcError(i32, String),
    Timeout,
    ChannelClosed,
    OAuth(String),
    Transport(String),
    Protocol(String),
}
```

---

## References

- [MCP Specification](https://spec.modelcontextprotocol.io/)
- [MCP TypeScript SDK](https://github.com/modelcontextprotocol/typescript-sdk)
- [MCP Servers](https://github.com/modelcontextprotocol/servers)
