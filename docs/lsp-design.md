# LSP (Language Server Protocol) Integration Design

## Overview

This document describes the design for integrating Language Server Protocol (LSP) support into A3S Code, enabling code intelligence features like hover information, go-to-definition, find references, and diagnostics.

## What is LSP?

LSP is an open protocol developed by Microsoft for communication between code editors/IDEs and language servers. It decouples language intelligence features from editors.

```
┌─────────────────┐                    ┌─────────────────┐
│   Editor/IDE    │  ◄── JSON-RPC ──►  │ Language Server │
│  (VS Code, etc) │                    │  (rust-analyzer,│
│                 │                    │   gopls, etc)   │
└─────────────────┘                    └─────────────────┘
```

## LSP Core Features

| Feature | Method | Description |
|---------|--------|-------------|
| Hover | `textDocument/hover` | Type info and documentation |
| Definition | `textDocument/definition` | Jump to definition |
| References | `textDocument/references` | Find all usages |
| Document Symbols | `textDocument/documentSymbol` | File outline |
| Workspace Symbols | `workspace/symbol` | Global symbol search |
| Diagnostics | `textDocument/diagnostic` | Errors and warnings |
| Completion | `textDocument/completion` | Code completion |
| Rename | `textDocument/rename` | Safe refactoring |

## Common Language Servers

| Language | Server | Installation |
|----------|--------|--------------|
| Rust | rust-analyzer | `rustup component add rust-analyzer` |
| Go | gopls | `go install golang.org/x/tools/gopls@latest` |
| TypeScript/JS | typescript-language-server | `npm i -g typescript-language-server` |
| Python | pyright / pylsp | `pip install pyright` |
| C/C++ | clangd | System package manager |

---

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      A3S Code                                │
├─────────────────────────────────────────────────────────────┤
│  ┌─────────────┐    ┌─────────────┐    ┌───────────────────┐│
│  │ Agent Loop  │───►│ LSP Manager │───►│ Language Server   ││
│  └─────────────┘    └─────────────┘    │   Processes       ││
│         │                  │           └───────────────────┘│
│         ▼                  ▼                                │
│  ┌─────────────┐    ┌─────────────┐                        │
│  │ LSP Tools   │    │ LSP Client  │                        │
│  │ - lsp_hover │    │ (JSON-RPC)  │                        │
│  │ - lsp_def   │    └─────────────┘                        │
│  │ - lsp_refs  │                                           │
│  │ - lsp_diag  │                                           │
│  └─────────────┘                                           │
└─────────────────────────────────────────────────────────────┘
```

## File Structure

```
crates/code/src/
├── lsp/
│   ├── mod.rs           # Module exports
│   ├── manager.rs       # LSP server lifecycle management
│   ├── client.rs        # JSON-RPC client
│   ├── protocol.rs      # LSP protocol type definitions
│   ├── servers.rs       # Language server configurations
│   └── tools.rs         # LSP tool implementations
└── tools/
    └── lsp.rs           # LSP tool registration
```

---

## Core Components

### 1. LSP Manager

Manages language server lifecycle and routing.

```rust
pub struct LspManager {
    /// Running language servers (language -> server instance)
    servers: RwLock<HashMap<String, LspServer>>,
    /// Workspace root directory
    workspace_root: PathBuf,
    /// Server configurations
    config: LspConfig,
}

impl LspManager {
    /// Start server for a language
    pub async fn start_server(&self, language: &str) -> Result<()>;

    /// Stop server for a language
    pub async fn stop_server(&self, language: &str) -> Result<()>;

    /// Get client for a language
    pub async fn get_client(&self, language: &str) -> Option<&LspClient>;

    /// Auto-start server based on file extension
    pub async fn ensure_server_for_file(&self, path: &Path) -> Result<&LspClient>;

    /// Document lifecycle notifications
    pub async fn did_open(&self, path: &Path, content: &str) -> Result<()>;
    pub async fn did_close(&self, path: &Path) -> Result<()>;
    pub async fn did_change(&self, path: &Path, content: &str) -> Result<()>;
}
```

### 2. LSP Client

JSON-RPC client for communicating with language servers.

```rust
pub struct LspClient {
    next_id: AtomicU64,
    writer: Mutex<ChildStdin>,
    pending_requests: DashMap<u64, oneshot::Sender<JsonValue>>,
    notification_tx: mpsc::Sender<LspNotification>,
}

impl LspClient {
    /// Send request and wait for response
    pub async fn request<P, R>(&self, method: &str, params: P) -> Result<R>;

    /// Send notification (no response)
    pub async fn notify<P>(&self, method: &str, params: P) -> Result<()>;

    // LSP method wrappers
    pub async fn initialize(&self, root_uri: &str) -> Result<InitializeResult>;
    pub async fn hover(&self, uri: &str, line: u32, character: u32) -> Result<Option<Hover>>;
    pub async fn goto_definition(&self, uri: &str, line: u32, character: u32) -> Result<Option<GotoDefinitionResponse>>;
    pub async fn find_references(&self, uri: &str, line: u32, character: u32) -> Result<Vec<Location>>;
    pub async fn document_symbols(&self, uri: &str) -> Result<Vec<DocumentSymbol>>;
    pub async fn workspace_symbols(&self, query: &str) -> Result<Vec<SymbolInformation>>;
    pub async fn get_diagnostics(&self, uri: &str) -> Result<Vec<Diagnostic>>;
}
```

### 3. Language Server Configuration

```rust
pub struct LanguageServerConfig {
    pub command: String,
    pub args: Vec<String>,
    pub env: HashMap<String, String>,
    pub extensions: Vec<String>,
    pub language_id: String,
    pub initialization_options: Option<JsonValue>,
}

/// Built-in server configurations
pub fn builtin_servers() -> HashMap<String, LanguageServerConfig> {
    // rust-analyzer, gopls, typescript-language-server, pyright, etc.
}

/// Map file extension to language
pub fn language_for_extension(ext: &str) -> Option<&'static str> {
    match ext {
        "rs" => Some("rust"),
        "ts" | "tsx" | "js" | "jsx" => Some("typescript"),
        "py" => Some("python"),
        "go" => Some("go"),
        _ => None,
    }
}
```

---

## LSP Tools

### Tool Definitions

| Tool | Description | Parameters |
|------|-------------|------------|
| `lsp_hover` | Get type info and documentation | file_path, line, column |
| `lsp_definition` | Jump to symbol definition | file_path, line, column |
| `lsp_references` | Find all usages of a symbol | file_path, line, column, include_declaration |
| `lsp_symbols` | Search workspace symbols | query, limit |
| `lsp_diagnostics` | Get errors and warnings | file_path (optional), severity |
| `lsp_document_symbols` | Get file outline | file_path |

### Example Tool Usage

```json
// lsp_hover
{
  "file_path": "src/main.rs",
  "line": 42,
  "column": 15
}

// lsp_references
{
  "file_path": "src/lib.rs",
  "line": 100,
  "column": 8,
  "include_declaration": false
}

// lsp_symbols
{
  "query": "parse_config",
  "limit": 20
}
```

---

## Implementation Phases

| Phase | Tasks | Priority |
|-------|-------|----------|
| **Phase 1: Infrastructure** | |
| | Implement JSON-RPC client | P0 |
| | Implement LSP Manager lifecycle | P0 |
| | Add built-in server configs | P0 |
| **Phase 2: Core Tools** | |
| | Implement lsp_hover | P0 |
| | Implement lsp_definition | P0 |
| | Implement lsp_references | P0 |
| | Implement lsp_symbols | P0 |
| **Phase 3: Advanced Features** | |
| | Implement lsp_diagnostics | P1 |
| | Implement lsp_document_symbols | P1 |
| | Add document sync (didOpen/didChange/didClose) | P1 |
| **Phase 4: Integration** | |
| | Integrate with Agent Loop | P1 |
| | Add caching and optimization | P2 |
| | Write tests | P1 |

## Dependencies

```toml
lsp-types = "0.95"          # LSP protocol type definitions
dashmap = "5.5"             # Concurrent HashMap
```

---

## References

- [LSP Specification](https://microsoft.github.io/language-server-protocol/specifications/lsp/3.17/specification/)
- [rust-analyzer](https://rust-analyzer.github.io/)
- [gopls](https://pkg.go.dev/golang.org/x/tools/gopls)
