# OpenCode Adapter Design

## Overview

This document describes how to integrate OpenCode into A3S Box by creating an adapter that converts OpenCode's REST API to the A3S Code Agent standard interface.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│ A3S Box Runtime                                             │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ AgentClient (gRPC)                                   │  │
│  └────────────────────┬─────────────────────────────────┘  │
│                       │                                     │
│  ┌────────────────────▼─────────────────────────────────┐  │
│  │ OpenCodeAdapter                                      │  │
│  │  - gRPC Server (implements CodeAgentService)         │  │
│  │  - REST Client (calls OpenCode API)                  │  │
│  │  - Protocol conversion                               │  │
│  └────────────────────┬─────────────────────────────────┘  │
└───────────────────────┼─────────────────────────────────────┘
                        │ HTTP REST
                        ▼
┌─────────────────────────────────────────────────────────────┐
│ OpenCode Container                                          │
│                                                             │
│  ┌──────────────────────────────────────────────────────┐  │
│  │ OpenCode Server                                      │  │
│  │  - REST API (OpenAPI 3.1.1)                          │  │
│  │  - Project management                                │  │
│  │  - PTY sessions                                      │  │
│  │  - Event streaming (SSE)                             │  │
│  └──────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────┘
```

## API Mapping

### 1. Lifecycle Management

| A3S Code Agent | OpenCode API | Description |
|----------------|--------------|-------------|
| HealthCheck | GET /global/health | Health check |
| GetCapabilities | GET /global/config | Get config and capabilities |
| Initialize | POST /project + PATCH /global/config | Initialize project and config |
| Shutdown | POST /global/dispose | Clean up resources |

### 2. Session Management

OpenCode uses the Project concept, which we map to Session:

| A3S Code Agent | OpenCode API | Description |
|----------------|--------------|-------------|
| CreateSession | POST /project | Create project |
| DestroySession | DELETE /project/{id} | Delete project |
| ListSessions | GET /project | List projects |
| GetSession | GET /project/{id} | Get project info |
| ConfigureSession | PATCH /project/{id} | Update project config |

### 3. Code Generation

OpenCode doesn't have a direct code generation API, so we simulate it via PTY sessions:

| A3S Code Agent | OpenCode Implementation | Description |
|----------------|------------------------|-------------|
| Generate | POST /pty + send message | Create PTY session and send prompt |
| StreamGenerate | GET /pty/{id}/stream | Stream receive response |

### 4. Tool Execution

| A3S Code Agent | OpenCode API | Description |
|----------------|--------------|-------------|
| ExecuteTool | POST /pty/{id}/input | Execute command via PTY |
| ListTools | Static list | OpenCode tools are built-in |

### 5. Event Stream

| A3S Code Agent | OpenCode API | Description |
|----------------|--------------|-------------|
| SubscribeEvents | GET /global/event | Subscribe to global events (SSE) |

## Implementation

### 1. OpenCodeAdapter Structure

```rust
use tonic::{Request, Response, Status};
use reqwest::Client;
use a3s_code_agent::*;

pub struct OpenCodeAdapter {
    /// OpenCode server address
    base_url: String,

    /// HTTP client
    client: Client,

    /// Session mapping (Session ID -> Project ID)
    sessions: Arc<RwLock<HashMap<String, String>>>,

    /// PTY mapping (Session ID -> PTY ID)
    ptys: Arc<RwLock<HashMap<String, String>>>,
}

impl OpenCodeAdapter {
    pub fn new(base_url: String) -> Self {
        Self {
            base_url,
            client: Client::new(),
            sessions: Arc::new(RwLock::new(HashMap::new())),
            ptys: Arc::new(RwLock::new(HashMap::new())),
        }
    }

    /// Call OpenCode API
    async fn call_opencode<T: serde::de::DeserializeOwned>(
        &self,
        method: &str,
        path: &str,
        body: Option<serde_json::Value>,
    ) -> Result<T, Status> {
        let url = format!("{}{}", self.base_url, path);

        let request = match method {
            "GET" => self.client.get(&url),
            "POST" => self.client.post(&url),
            "PATCH" => self.client.patch(&url),
            "DELETE" => self.client.delete(&url),
            _ => return Err(Status::unimplemented("Unsupported HTTP method")),
        };

        let request = if let Some(body) = body {
            request.json(&body)
        } else {
            request
        };

        let response = request
            .send()
            .await
            .map_err(|e| Status::unavailable(format!("OpenCode API error: {}", e)))?;

        if !response.status().is_success() {
            return Err(Status::internal(format!(
                "OpenCode API returned error: {}",
                response.status()
            )));
        }

        response
            .json()
            .await
            .map_err(|e| Status::internal(format!("Failed to parse response: {}", e)))
    }
}

#[tonic::async_trait]
impl CodeAgentService for OpenCodeAdapter {
    async fn health_check(
        &self,
        _request: Request<HealthCheckRequest>,
    ) -> Result<Response<HealthCheckResponse>, Status> {
        #[derive(serde::Deserialize)]
        struct HealthResponse {
            healthy: bool,
            version: String,
        }

        let health: HealthResponse = self.call_opencode("GET", "/global/health", None).await?;

        Ok(Response::new(HealthCheckResponse {
            status: if health.healthy {
                HealthCheckResponse::Status::Healthy as i32
            } else {
                HealthCheckResponse::Status::Unhealthy as i32
            },
            message: format!("OpenCode v{}", health.version),
            details: HashMap::new(),
        }))
    }

    async fn get_capabilities(
        &self,
        _request: Request<GetCapabilitiesRequest>,
    ) -> Result<Response<GetCapabilitiesResponse>, Status> {
        Ok(Response::new(GetCapabilitiesResponse {
            info: Some(AgentInfo {
                name: "opencode".to_string(),
                version: "0.1.0".to_string(),
                description: "The open source AI coding agent".to_string(),
                author: "Anomaly Co".to_string(),
                license: "MIT".to_string(),
                homepage: "https://opencode.ai".to_string(),
            }),
            features: vec![
                "code_generation".to_string(),
                "tool_execution".to_string(),
                "pty_sessions".to_string(),
                "lsp_support".to_string(),
            ],
            tools: vec![
                ToolCapability {
                    name: "read_file".to_string(),
                    description: "Read file contents".to_string(),
                    parameters: vec!["path".to_string()],
                    async_: false,
                },
                ToolCapability {
                    name: "write_file".to_string(),
                    description: "Write file contents".to_string(),
                    parameters: vec!["path".to_string(), "content".to_string()],
                    async_: false,
                },
                ToolCapability {
                    name: "bash".to_string(),
                    description: "Execute bash command".to_string(),
                    parameters: vec!["command".to_string()],
                    async_: true,
                },
                // ... other tools
            ],
            models: vec![
                ModelCapability {
                    provider: "anthropic".to_string(),
                    model: "claude-3-5-sonnet".to_string(),
                    features: vec!["code_generation".to_string()],
                },
                ModelCapability {
                    provider: "openai".to_string(),
                    model: "gpt-4".to_string(),
                    features: vec!["code_generation".to_string()],
                },
            ],
            limits: Some(ResourceLimits {
                max_context_tokens: 200000,
                max_concurrent_sessions: 10,
                max_tools_per_request: 20,
            }),
            metadata: HashMap::new(),
        }))
    }

    async fn create_session(
        &self,
        request: Request<CreateSessionRequest>,
    ) -> Result<Response<CreateSessionResponse>, Status> {
        let req = request.into_inner();

        #[derive(serde::Serialize)]
        struct CreateProjectRequest {
            directory: String,
            name: Option<String>,
        }

        #[derive(serde::Deserialize)]
        struct Project {
            id: String,
            directory: String,
            name: String,
        }

        let workspace = req.config.as_ref()
            .and_then(|c| Some(c.workspace.clone()))
            .unwrap_or_else(|| "/workspace".to_string());

        let project: Project = self.call_opencode(
            "POST",
            "/project",
            Some(serde_json::json!({
                "directory": workspace,
                "name": req.config.as_ref().and_then(|c| Some(c.name.clone())),
            })),
        ).await?;

        // Generate session ID
        let session_id = req.session_id.clone()
            .unwrap_or_else(|| format!("session-{}", uuid::Uuid::new_v4()));

        // Save mapping
        self.sessions.write().await.insert(session_id.clone(), project.id.clone());

        Ok(Response::new(CreateSessionResponse {
            session_id: session_id.clone(),
            session: Some(Session {
                session_id,
                config: req.config,
                state: SessionState::Active as i32,
                context_usage: Some(ContextUsage::default()),
                created_at: chrono::Utc::now().timestamp(),
                updated_at: chrono::Utc::now().timestamp(),
            }),
        }))
    }

    async fn generate(
        &self,
        request: Request<GenerateRequest>,
    ) -> Result<Response<GenerateResponse>, Status> {
        let req = request.into_inner();

        // Get project ID
        let project_id = self.sessions.read().await
            .get(&req.session_id)
            .cloned()
            .ok_or_else(|| Status::not_found("Session not found"))?;

        // Create PTY session
        #[derive(serde::Deserialize)]
        struct Pty {
            id: String,
        }

        let pty: Pty = self.call_opencode(
            "POST",
            "/pty",
            Some(serde_json::json!({
                "directory": format!("/project/{}", project_id),
            })),
        ).await?;

        // Save PTY mapping
        self.ptys.write().await.insert(req.session_id.clone(), pty.id.clone());

        // Send message to PTY
        let prompt = req.messages.iter()
            .filter(|m| m.role == Message::Role::User as i32)
            .map(|m| m.content.clone())
            .collect::<Vec<_>>()
            .join("\n");

        self.call_opencode::<serde_json::Value>(
            "POST",
            &format!("/pty/{}/input", pty.id),
            Some(serde_json::json!({
                "data": prompt,
            })),
        ).await?;

        // Wait for response (simplified, actual implementation should use streaming)
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;

        // Read output
        #[derive(serde::Deserialize)]
        struct PtyOutput {
            data: String,
        }

        let output: PtyOutput = self.call_opencode(
            "GET",
            &format!("/pty/{}/output", pty.id),
            None,
        ).await?;

        Ok(Response::new(GenerateResponse {
            session_id: req.session_id,
            message: Some(Message {
                role: Message::Role::Assistant as i32,
                content: output.data,
                attachments: vec![],
                metadata: HashMap::new(),
            }),
            tool_calls: vec![],
            usage: Some(Usage::default()),
            finish_reason: FinishReason::Stop as i32,
            metadata: HashMap::new(),
        }))
    }

    // Implement other methods...
}
```

### 2. Streaming Generation Implementation

```rust
impl CodeAgentService for OpenCodeAdapter {
    async fn stream_generate(
        &self,
        request: Request<GenerateRequest>,
    ) -> Result<Response<Self::StreamGenerateStream>, Status> {
        let req = request.into_inner();

        // Get PTY ID
        let pty_id = self.ptys.read().await
            .get(&req.session_id)
            .cloned()
            .ok_or_else(|| Status::not_found("PTY not found"))?;

        // Create SSE stream
        let url = format!("{}/pty/{}/stream", self.base_url, pty_id);
        let response = self.client.get(&url)
            .send()
            .await
            .map_err(|e| Status::unavailable(format!("Failed to connect to stream: {}", e)))?;

        // Convert to gRPC stream
        let stream = response.bytes_stream()
            .map(|chunk| {
                let chunk = chunk.map_err(|e| Status::internal(format!("Stream error: {}", e)))?;

                // Parse SSE data
                let data = String::from_utf8_lossy(&chunk);

                Ok(GenerateChunk {
                    type_: GenerateChunk::ChunkType::Content as i32,
                    session_id: req.session_id.clone(),
                    content: data.to_string(),
                    tool_call: None,
                    tool_result: None,
                    metadata: HashMap::new(),
                })
            });

        Ok(Response::new(Box::pin(stream)))
    }
}
```

### 3. Tool Execution Implementation

```rust
impl CodeAgentService for OpenCodeAdapter {
    async fn execute_tool(
        &self,
        request: Request<ExecuteToolRequest>,
    ) -> Result<Response<ExecuteToolResponse>, Status> {
        let req = request.into_inner();

        // Get PTY ID
        let pty_id = self.ptys.read().await
            .get(&req.session_id)
            .cloned()
            .ok_or_else(|| Status::not_found("PTY not found"))?;

        // Parse tool arguments
        let args: serde_json::Value = serde_json::from_str(&req.arguments)
            .map_err(|e| Status::invalid_argument(format!("Invalid arguments: {}", e)))?;

        // Build command based on tool name
        let command = match req.tool_name.as_str() {
            "read_file" => {
                let path = args["path"].as_str()
                    .ok_or_else(|| Status::invalid_argument("Missing 'path' parameter"))?;
                format!("cat {}", path)
            }
            "write_file" => {
                let path = args["path"].as_str()
                    .ok_or_else(|| Status::invalid_argument("Missing 'path' parameter"))?;
                let content = args["content"].as_str()
                    .ok_or_else(|| Status::invalid_argument("Missing 'content' parameter"))?;
                format!("echo '{}' > {}", content, path)
            }
            "bash" => {
                args["command"].as_str()
                    .ok_or_else(|| Status::invalid_argument("Missing 'command' parameter"))?
                    .to_string()
            }
            _ => return Err(Status::unimplemented(format!("Tool '{}' not supported", req.tool_name))),
        };

        // Execute command
        self.call_opencode::<serde_json::Value>(
            "POST",
            &format!("/pty/{}/input", pty_id),
            Some(serde_json::json!({
                "data": command,
            })),
        ).await?;

        // Wait for output
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;

        // Read output
        #[derive(serde::Deserialize)]
        struct PtyOutput {
            data: String,
        }

        let output: PtyOutput = self.call_opencode(
            "GET",
            &format!("/pty/{}/output", pty_id),
            None,
        ).await?;

        Ok(Response::new(ExecuteToolResponse {
            result: Some(ToolResult {
                success: true,
                output: output.data,
                error: String::new(),
                metadata: HashMap::new(),
            }),
        }))
    }
}
```

## Configuration

### 1. A3S Box Configuration

```yaml
# box-config.yaml
coding_agent:
  kind: "opencode"
  version: "latest"
  adapter:
    type: "rest"
    base_url: "http://localhost:3000"
  config:
    provider: "anthropic"
    model: "claude-3-5-sonnet"
    api_key: "${ANTHROPIC_API_KEY}"
```

### 2. OpenCode Configuration

```yaml
# opencode-config.yaml
provider: anthropic
model: claude-3-5-sonnet-20241022
apiKey: ${ANTHROPIC_API_KEY}
```

## Limitations and Considerations

### 1. Feature Differences

| Feature | A3S Code Agent | OpenCode | Adapter Support |
|---------|----------------|----------|-----------------|
| Session Management | Yes | Yes (Project) | Yes |
| Code Generation | Yes | Yes (PTY) | Yes |
| Streaming Response | Yes | Yes (SSE) | Yes |
| Tool Execution | Yes | Yes (PTY) | Yes |
| Structured Output | Yes | No | Partial |
| Skill Management | Yes | No | No |
| Context Compaction | Yes | No | No |

### 2. Performance Considerations

- **Latency**: REST API has slightly higher latency than gRPC
- **Streaming**: SSE to gRPC stream conversion has additional overhead
- **Concurrency**: OpenCode's concurrency limits may affect performance

### 3. Compatibility

- OpenCode version >= 0.1.0
- OpenCode server must be running at an accessible address
- Some advanced features may not be available

## Deployment

### Docker Compose

```yaml
version: '3.8'

services:
  opencode:
    image: opencode/opencode:latest
    ports:
      - "3000:3000"
    volumes:
      - ./workspace:/workspace
    environment:
      - ANTHROPIC_API_KEY=${ANTHROPIC_API_KEY}

  opencode-adapter:
    image: a3s/opencode-adapter:latest
    ports:
      - "4088:4088"
    environment:
      - OPENCODE_URL=http://opencode:3000
    depends_on:
      - opencode
```

### Kubernetes

```yaml
apiVersion: v1
kind: Service
metadata:
  name: opencode-adapter
spec:
  selector:
    app: opencode-adapter
  ports:
    - port: 4088
      targetPort: 4088
---
apiVersion: apps/v1
kind: Deployment
metadata:
  name: opencode-adapter
spec:
  replicas: 1
  selector:
    matchLabels:
      app: opencode-adapter
  template:
    metadata:
      labels:
        app: opencode-adapter
    spec:
      containers:
      - name: adapter
        image: a3s/opencode-adapter:latest
        ports:
        - containerPort: 4088
        env:
        - name: OPENCODE_URL
          value: "http://opencode:3000"
```

## Future Improvements

1. **Performance Optimization**
   - Connection pooling
   - Request caching
   - Batch operations

2. **Feature Enhancement**
   - Support more OpenCode features
   - Better error handling
   - Retry mechanism

3. **Monitoring and Logging**
   - Prometheus metrics
   - Structured logging
   - Distributed tracing

---

**Version**: 1.0.0
**Last Updated**: 2026-02-04
**Status**: Draft
