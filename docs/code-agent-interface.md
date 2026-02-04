# A3S Code Agent Interface Specification

## Overview

This document defines the standard interface for A3S Code Agent. Any coding agent implementing this interface can be seamlessly integrated into A3S Box.

## Design Principles

1. **Protocol Agnostic** - Support gRPC, REST, WebSocket, and other protocols
2. **Capability Declaration** - Agents can declare their supported features
3. **Extensible Tools** - Support custom tools and extensions
4. **Session Management** - Support multi-session concurrency
5. **Streaming Responses** - Support streaming generation and event push

## Core Interface

### 1. Agent Service

All coding agents must implement the following core interface:

```protobuf
syntax = "proto3";
package a3s.code.agent.v1;

// Coding Agent Service
service CodeAgentService {
  // === Lifecycle Management ===

  // Health check
  rpc HealthCheck(HealthCheckRequest) returns (HealthCheckResponse);

  // Get agent capabilities
  rpc GetCapabilities(GetCapabilitiesRequest) returns (GetCapabilitiesResponse);

  // Initialize agent
  rpc Initialize(InitializeRequest) returns (InitializeResponse);

  // Shutdown agent
  rpc Shutdown(ShutdownRequest) returns (ShutdownResponse);

  // === Session Management ===

  // Create session
  rpc CreateSession(CreateSessionRequest) returns (CreateSessionResponse);

  // Destroy session
  rpc DestroySession(DestroySessionRequest) returns (DestroySessionResponse);

  // List sessions
  rpc ListSessions(ListSessionsRequest) returns (ListSessionsResponse);

  // Get session info
  rpc GetSession(GetSessionRequest) returns (GetSessionResponse);

  // Configure session
  rpc ConfigureSession(ConfigureSessionRequest) returns (ConfigureSessionResponse);

  // === Code Generation ===

  // Generate code (synchronous)
  rpc Generate(GenerateRequest) returns (GenerateResponse);

  // Generate code (streaming)
  rpc StreamGenerate(GenerateRequest) returns (stream GenerateChunk);

  // Generate structured output (synchronous)
  rpc GenerateStructured(GenerateStructuredRequest) returns (GenerateStructuredResponse);

  // Generate structured output (streaming)
  rpc StreamGenerateStructured(GenerateStructuredRequest) returns (stream GenerateStructuredChunk);

  // === Tool Execution ===

  // Execute tool
  rpc ExecuteTool(ExecuteToolRequest) returns (ExecuteToolResponse);

  // Batch execute tools
  rpc ExecuteToolBatch(ExecuteToolBatchRequest) returns (ExecuteToolBatchResponse);

  // List available tools
  rpc ListTools(ListToolsRequest) returns (ListToolsResponse);

  // Register custom tool
  rpc RegisterTool(RegisterToolRequest) returns (RegisterToolResponse);

  // === Skill Management ===

  // Load skill
  rpc LoadSkill(LoadSkillRequest) returns (LoadSkillResponse);

  // Unload skill
  rpc UnloadSkill(UnloadSkillRequest) returns (UnloadSkillResponse);

  // List skills
  rpc ListSkills(ListSkillsRequest) returns (ListSkillsResponse);

  // === Context Management ===

  // Get context usage
  rpc GetContextUsage(GetContextUsageRequest) returns (GetContextUsageResponse);

  // Compact context
  rpc CompactContext(CompactContextRequest) returns (CompactContextResponse);

  // Clear context
  rpc ClearContext(ClearContextRequest) returns (ClearContextResponse);

  // === Event Stream ===

  // Subscribe to events
  rpc SubscribeEvents(SubscribeEventsRequest) returns (stream AgentEvent);

  // === Control Operations ===

  // Cancel operation
  rpc Cancel(CancelRequest) returns (CancelResponse);

  // Pause operation
  rpc Pause(PauseRequest) returns (PauseResponse);

  // Resume operation
  rpc Resume(ResumeRequest) returns (ResumeResponse);
}
```

### 2. Message Definitions

#### 2.1 Health Check

```protobuf
message HealthCheckRequest {}

message HealthCheckResponse {
  enum Status {
    UNKNOWN = 0;
    HEALTHY = 1;
    DEGRADED = 2;
    UNHEALTHY = 3;
  }

  Status status = 1;
  string message = 2;
  map<string, string> details = 3;
}
```

#### 2.2 Capability Declaration

```protobuf
message GetCapabilitiesRequest {}

message GetCapabilitiesResponse {
  // Agent basic info
  AgentInfo info = 1;

  // Supported features
  repeated string features = 2;

  // Supported tools
  repeated ToolCapability tools = 3;

  // Supported models
  repeated ModelCapability models = 4;

  // Resource limits
  ResourceLimits limits = 5;

  // Extension metadata
  map<string, string> metadata = 6;
}

message AgentInfo {
  string name = 1;           // Agent name, e.g., "a3s-code", "opencode"
  string version = 2;        // Version number, e.g., "0.1.0"
  string description = 3;    // Description
  string author = 4;         // Author
  string license = 5;        // License
  string homepage = 6;       // Homepage
}

message ToolCapability {
  string name = 1;           // Tool name
  string description = 2;    // Tool description
  repeated string parameters = 3;  // Parameter list
  bool async = 4;            // Supports async execution
}

message ModelCapability {
  string provider = 1;       // Provider, e.g., "anthropic", "openai"
  string model = 2;          // Model name
  repeated string features = 3;  // Supported features
}

message ResourceLimits {
  uint64 max_context_tokens = 1;    // Maximum context tokens
  uint32 max_concurrent_sessions = 2;  // Maximum concurrent sessions
  uint32 max_tools_per_request = 3;    // Maximum tools per request
}
```

#### 2.3 Session Management

```protobuf
message CreateSessionRequest {
  // Session ID (optional, auto-generated if not provided)
  string session_id = 1;

  // Session configuration
  SessionConfig config = 2;

  // Initial context
  repeated Message initial_context = 3;
}

message CreateSessionResponse {
  string session_id = 1;
  Session session = 2;
}

message SessionConfig {
  // Session name
  string name = 1;

  // Working directory
  string workspace = 2;

  // LLM configuration (overrides global config)
  LLMConfig llm = 3;

  // System prompt
  string system_prompt = 4;

  // Maximum context length
  uint32 max_context_length = 5;

  // Auto compact context
  bool auto_compact = 6;
}

message Session {
  string session_id = 1;
  SessionConfig config = 2;
  SessionState state = 3;
  ContextUsage context_usage = 4;
  int64 created_at = 5;
  int64 updated_at = 6;
}

enum SessionState {
  SESSION_STATE_UNKNOWN = 0;
  SESSION_STATE_ACTIVE = 1;
  SESSION_STATE_PAUSED = 2;
  SESSION_STATE_COMPLETED = 3;
  SESSION_STATE_ERROR = 4;
}

message ContextUsage {
  uint32 total_tokens = 1;
  uint32 prompt_tokens = 2;
  uint32 completion_tokens = 3;
  uint32 message_count = 4;
}
```

#### 2.4 Code Generation

```protobuf
message GenerateRequest {
  // Session ID
  string session_id = 1;

  // User messages
  repeated Message messages = 2;

  // Generation options
  GenerateOptions options = 3;
}

message Message {
  enum Role {
    ROLE_UNKNOWN = 0;
    ROLE_USER = 1;
    ROLE_ASSISTANT = 2;
    ROLE_SYSTEM = 3;
    ROLE_TOOL = 4;
  }

  Role role = 1;
  string content = 2;
  repeated Attachment attachments = 3;
  map<string, string> metadata = 4;
}

message GenerateOptions {
  // Enable tools
  bool enable_tools = 1;

  // Allowed tools list
  repeated string allowed_tools = 2;

  // Maximum tool calls
  uint32 max_tool_calls = 3;

  // Generation parameters
  float temperature = 4;
  uint32 max_tokens = 5;
  repeated string stop_sequences = 6;

  // Return intermediate steps
  bool return_intermediate_steps = 7;
}

message GenerateResponse {
  // Session ID
  string session_id = 1;

  // Generated message
  Message message = 2;

  // Tool calls
  repeated ToolCall tool_calls = 3;

  // Usage
  Usage usage = 4;

  // Finish reason
  FinishReason finish_reason = 5;

  // Metadata
  map<string, string> metadata = 6;
}

message ToolCall {
  string id = 1;
  string name = 2;
  string arguments = 3;  // JSON format
  ToolResult result = 4;
}

message ToolResult {
  bool success = 1;
  string output = 2;
  string error = 3;
  map<string, string> metadata = 4;
}

enum FinishReason {
  FINISH_REASON_UNKNOWN = 0;
  FINISH_REASON_STOP = 1;
  FINISH_REASON_LENGTH = 2;
  FINISH_REASON_TOOL_CALLS = 3;
  FINISH_REASON_CONTENT_FILTER = 4;
  FINISH_REASON_ERROR = 5;
}

// Streaming response
message GenerateChunk {
  enum ChunkType {
    CHUNK_TYPE_UNKNOWN = 0;
    CHUNK_TYPE_CONTENT = 1;
    CHUNK_TYPE_TOOL_CALL = 2;
    CHUNK_TYPE_TOOL_RESULT = 3;
    CHUNK_TYPE_METADATA = 4;
    CHUNK_TYPE_DONE = 5;
  }

  ChunkType type = 1;
  string session_id = 2;
  string content = 3;
  ToolCall tool_call = 4;
  ToolResult tool_result = 5;
  map<string, string> metadata = 6;
}
```

## Built-in Tool Specification

All coding agents should support the following built-in tools:

### File Operations (6 tools)
1. **read_file** - Read file contents
2. **write_file** - Write file contents
3. **edit_file** - Edit file (exact replacement)
4. **delete_file** - Delete file
5. **list_files** - List files
6. **search_files** - Search files (glob pattern)

### Code Operations (5 tools)
7. **grep** - Search code contents
8. **find_definition** - Find definition
9. **find_references** - Find references
10. **format_code** - Format code
11. **lint_code** - Code lint

### Command Execution (2 tools)
12. **bash** - Execute bash command
13. **run_script** - Run script

### Git Tools (4 tools)
14. **git_status** - Git status
15. **git_diff** - Git diff
16. **git_commit** - Git commit
17. **git_log** - Git log

### Other Tools (3 tools)
18. **web_search** - Web search
19. **web_fetch** - Fetch web content
20. **ask_user** - Ask user

## Tool Parameter Specification

Each tool must provide a JSON Schema defining its parameters:

```json
{
  "name": "read_file",
  "description": "Read the contents of a file",
  "parameters": {
    "type": "object",
    "properties": {
      "path": {
        "type": "string",
        "description": "Path to the file to read"
      },
      "encoding": {
        "type": "string",
        "enum": ["utf-8", "ascii", "base64"],
        "default": "utf-8",
        "description": "File encoding"
      }
    },
    "required": ["path"]
  }
}
```

## Protocol Adapters

### gRPC Implementation (Recommended)

Directly implement the protobuf-defined interface.

### REST API Implementation

If the agent uses REST API (like OpenCode), an adapter is needed:

```
POST /sessions                    → CreateSession
DELETE /sessions/{id}             → DestroySession
POST /sessions/{id}/generate      → Generate
GET /sessions/{id}/generate/stream → StreamGenerate
POST /sessions/{id}/tools/{name}  → ExecuteTool
GET /health                       → HealthCheck
GET /capabilities                 → GetCapabilities
```

### WebSocket Implementation

Transmit JSON format messages via WebSocket:

```json
{
  "method": "generate",
  "params": {
    "session_id": "session-123",
    "messages": [...]
  },
  "id": "request-456"
}
```

## Implementation Examples

### Minimal Implementation (Rust)

```rust
use tonic::{Request, Response, Status};
use a3s_code_agent::*;

pub struct MyCodeAgent {
    // Agent state
}

#[tonic::async_trait]
impl CodeAgentService for MyCodeAgent {
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        Ok(Response::new(HealthCheckResponse {
            status: HealthCheckResponse::Status::Healthy as i32,
            message: "OK".to_string(),
            details: HashMap::new(),
        }))
    }

    async fn get_capabilities(
        &self,
        _request: Request<GetCapabilitiesRequest>,
    ) -> Result<Response<GetCapabilitiesResponse>, Status> {
        Ok(Response::new(GetCapabilitiesResponse {
            info: Some(AgentInfo {
                name: "my-code-agent".to_string(),
                version: "1.0.0".to_string(),
                description: "My custom coding agent".to_string(),
                author: "Me".to_string(),
                license: "MIT".to_string(),
                homepage: "https://example.com".to_string(),
            }),
            features: vec![
                "code_generation".to_string(),
                "tool_execution".to_string(),
            ],
            tools: vec![
                ToolCapability {
                    name: "read_file".to_string(),
                    description: "Read file contents".to_string(),
                    parameters: vec!["path".to_string()],
                    async_: false,
                },
            ],
            models: vec![],
            limits: Some(ResourceLimits {
                max_context_tokens: 200000,
                max_concurrent_sessions: 10,
                max_tools_per_request: 20,
            }),
            metadata: HashMap::new(),
        }))
    }

    // Implement other methods...
}
```

### Minimal Implementation (Python)

```python
import grpc
from concurrent import futures
from a3s_code_agent_pb2_grpc import CodeAgentServiceServicer
from a3s_code_agent_pb2 import *

class MyCodeAgent(CodeAgentServiceServicer):
    def HealthCheck(self, request, context):
        return HealthCheckResponse(
            status=HealthCheckResponse.HEALTHY,
            message="OK",
            details={}
        )

    def GetCapabilities(self, request, context):
        return GetCapabilitiesResponse(
            info=AgentInfo(
                name="my-code-agent",
                version="1.0.0",
                description="My custom coding agent",
                author="Me",
                license="MIT",
                homepage="https://example.com"
            ),
            features=["code_generation", "tool_execution"],
            tools=[
                ToolCapability(
                    name="read_file",
                    description="Read file contents",
                    parameters=["path"],
                    async_=False
                )
            ],
            models=[],
            limits=ResourceLimits(
                max_context_tokens=200000,
                max_concurrent_sessions=10,
                max_tools_per_request=20
            ),
            metadata={}
        )

    # Implement other methods...

def serve():
    server = grpc.server(futures.ThreadPoolExecutor(max_workers=10))
    add_CodeAgentServiceServicer_to_server(MyCodeAgent(), server)
    server.add_insecure_port('[::]:4088')
    server.start()
    server.wait_for_termination()
```

## Integration with A3S Box

### 1. Configure Coding Agent

```yaml
# box-config.yaml
coding_agent:
  kind: "custom"
  name: "my-code-agent"
  image: "ghcr.io/myorg/my-code-agent:v1"
  protocol: "grpc"  # or "rest", "websocket"
  port: 4088
```

### 2. A3S Box Auto Discovery

A3S Box will:
1. Start the agent container
2. Call `HealthCheck` to confirm readiness
3. Call `GetCapabilities` to get capabilities
4. Call `Initialize` to initialize the agent
5. Start using the agent

### 3. Protocol Adaptation

If the agent uses non-gRPC protocol, A3S Box will automatically load an adapter:

```rust
// A3S Box internal
let agent_client = match config.protocol {
    Protocol::Grpc => GrpcAgentClient::new(config),
    Protocol::Rest => RestAgentAdapter::new(config),
    Protocol::WebSocket => WebSocketAgentAdapter::new(config),
};
```

## Compatibility Matrix

| Agent | Protocol | Adapter | Status |
|-------|----------|---------|--------|
| A3S Code | gRPC | Native | Fully Supported |
| OpenCode | REST | REST Adapter | Fully Supported |
| Claude Code | Proprietary | Proprietary Adapter | Planned |
| Custom Agent | gRPC/REST/WS | Auto Detection | Fully Supported |

## Testing and Validation

### Interface Testing

```bash
# Health check
grpcurl -plaintext localhost:4088 a3s.code.agent.v1.CodeAgentService/HealthCheck

# Get capabilities
grpcurl -plaintext localhost:4088 a3s.code.agent.v1.CodeAgentService/GetCapabilities

# Create session
grpcurl -plaintext -d '{"config": {"name": "test"}}' \
  localhost:4088 a3s.code.agent.v1.CodeAgentService/CreateSession
```

### Compatibility Testing

A3S Box provides a test suite to verify agent compatibility:

```bash
a3s-box test-agent --image ghcr.io/myorg/my-agent:latest
```

## Future Plans

### Near-term
- Multimodal support (images, audio)
- Collaborative editing
- Real-time collaboration

### Long-term
- Distributed agents
- Inter-agent communication
- Federated learning

---

**Last Updated**: 2026-02-04
