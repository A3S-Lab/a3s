# Extensible Tool System

A3S Box now supports an extensible tool system that allows you to add custom tools beyond the 7 built-in tools.

## Architecture

```
ToolRegistry
  ├── Built-in Tools (7 tools)
  │   ├── bash - Execute shell commands
  │   ├── read - Read file contents
  │   ├── write - Write files
  │   ├── edit - Edit files with string replacement
  │   ├── grep - Search file contents
  │   ├── glob - Find files by pattern
  │   └── ls - List directory contents
  │
  └── Dynamic Tools (loaded from skills)
      ├── BinaryTool - Execute external binaries
      ├── HttpTool - Make HTTP API calls
      └── ScriptTool - Execute scripts (bash, python, node, etc.)
```

## Tool Backend Types

### 1. Binary Tools

Execute external binaries with argument templates.

```yaml
tools:
  - name: my-binary
    description: Execute a custom binary
    backend:
      type: binary
      path: /usr/local/bin/my-tool
      args_template: "--input ${input_file} --output ${output_file}"
    parameters:
      type: object
      properties:
        input_file:
          type: string
        output_file:
          type: string
      required:
        - input_file
        - output_file
```

**Features:**
- Argument template with `${arg_name}` substitution
- Automatic binary download from URL (cached)
- Timeout: 60 seconds
- Output limit: 100KB

### 2. HTTP Tools

Make HTTP API calls with environment variable substitution.

```yaml
tools:
  - name: weather-api
    description: Get weather information
    backend:
      type: http
      url: https://api.example.com/weather
      method: GET
      headers:
        Authorization: "Bearer ${env:API_KEY}"
      timeout_ms: 30000
    parameters:
      type: object
      properties:
        city:
          type: string
      required:
        - city
```

**Features:**
- Environment variable substitution: `${env:VAR_NAME}`
- Argument substitution in URL: `${arg_name}`
- Query parameters for GET requests
- JSON body for POST/PUT/PATCH
- Configurable timeout (default: 30s)

### 3. Script Tools

Execute scripts with interpreters (bash, python, node, etc.).

```yaml
tools:
  - name: process-data
    description: Process data with Python
    backend:
      type: script
      interpreter: python3
      script: |
        import os
        import json

        args = json.loads(os.environ.get('TOOL_ARGS', '{}'))
        data = args.get('data', '')

        # Process the data
        result = data.upper()
        print(f"Result: {result}")
    parameters:
      type: object
      properties:
        data:
          type: string
      required:
        - data
```

**Features:**
- Arguments passed as environment variables:
  - `TOOL_ARG_<NAME>` - Individual arguments (uppercase)
  - `TOOL_ARGS` - Full JSON object
- Supported interpreters: bash, sh, zsh, python, python3, node
- Timeout: 60 seconds
- Output limit: 100KB

## SKILL.md Format

```yaml
---
name: my-skill
description: A custom skill with multiple tools
version: 1.0.0
tools:
  - name: tool1
    description: First tool
    backend:
      type: binary|http|script
      # backend-specific fields...
    parameters:
      type: object
      properties:
        # JSON Schema for parameters
      required: []

  - name: tool2
    # ... more tools
---

# Skill Documentation

Markdown content describing the skill and how to use it.
```

## Usage Examples

### Example 1: PDF Processing Skill

```yaml
---
name: pdf-parser
description: Parse and extract text from PDF documents
tools:
  - name: pdf-extract
    description: Extract text from PDF
    backend:
      type: binary
      url: https://tools.example.com/pdf-extract
      args_template: "${file_path}"
    parameters:
      type: object
      properties:
        file_path:
          type: string
          description: Path to PDF file
      required:
        - file_path
---
```

### Example 2: API Integration Skill

```yaml
---
name: openai-whisper
description: Transcribe audio using OpenAI Whisper API
tools:
  - name: transcribe
    description: Transcribe audio file
    backend:
      type: http
      url: https://api.openai.com/v1/audio/transcriptions
      method: POST
      headers:
        Authorization: "Bearer ${env:OPENAI_API_KEY}"
      body_template: |
        {
          "file": "${audio_file}",
          "model": "whisper-1"
        }
    parameters:
      type: object
      properties:
        audio_file:
          type: string
      required:
        - audio_file
---
```

### Example 3: Data Processing Skill

```yaml
---
name: data-processor
description: Process and analyze data
tools:
  - name: chunk-text
    description: Split text into chunks
    backend:
      type: script
      interpreter: python3
      script: |
        import os
        import json

        args = json.loads(os.environ.get('TOOL_ARGS', '{}'))
        text = args.get('text', '')
        chunk_size = int(args.get('chunk_size', 1000))

        chunks = [text[i:i+chunk_size] for i in range(0, len(text), chunk_size)]

        for i, chunk in enumerate(chunks):
            print(f"Chunk {i+1}: {len(chunk)} chars")
    parameters:
      type: object
      properties:
        text:
          type: string
        chunk_size:
          type: integer
      required:
        - text
---
```

## API Reference

### ToolExecutor

```rust
// Register skill tools
let tool_names = executor.register_skill_tools(skill_content);

// Unregister tools
let removed = executor.unregister_tools(&tool_names);

// Get all tool definitions
let definitions = executor.definitions();

// Execute a tool
let result = executor.execute("tool_name", &args).await?;
```

### SessionManager

```rust
// Activate a skill for a session
let tool_names = session_manager
    .use_skill(session_id, skill_name, skill_content)
    .await?;

// Deactivate a skill
let removed = session_manager
    .remove_skill(session_id, skill_name)
    .await?;

// List active skills
let skills = session_manager
    .list_active_skills(session_id)
    .await?;
```

### gRPC Service

```protobuf
// Activate a skill
rpc UseSkill(UseSkillRequest) returns (UseSkillResponse);

message UseSkillRequest {
  string session_id = 1;
  string skill_name = 2;
  string skill_content = 3; // SKILL.md content
}

message UseSkillResponse {
  repeated string tool_names = 1; // Registered tools
}

// Deactivate a skill
rpc RemoveSkill(RemoveSkillRequest) returns (RemoveSkillResponse);

message RemoveSkillRequest {
  string session_id = 1;
  string skill_name = 2;
}

message RemoveSkillResponse {
  repeated string removed_tools = 1; // Unregistered tools
}
```

## Security Considerations

1. **Workspace Sandboxing**: All file operations are confined to the workspace directory
2. **Path Validation**: Paths are canonicalized and checked to prevent directory traversal
3. **Output Limits**: Tool output is limited to 100KB to prevent memory exhaustion
4. **Timeouts**: All tools have configurable timeouts (default: 60s for binary/script, 30s for HTTP)
5. **Environment Variables**: Secrets should be passed via environment variables, not hardcoded

## Best Practices

1. **Use Environment Variables for Secrets**
   ```yaml
   headers:
     Authorization: "Bearer ${env:API_KEY}"  # Good
     Authorization: "Bearer sk-1234..."      # Bad
   ```

2. **Validate Parameters**
   ```yaml
   parameters:
     type: object
     properties:
       file_path:
         type: string
         pattern: "^[a-zA-Z0-9_/-]+\\.pdf$"  # Validate format
     required:
       - file_path
   ```

3. **Handle Errors Gracefully**
   - Tools return `ToolResult` with `exit_code` and `output`
   - Check `exit_code == 0` for success
   - Parse error messages from `output`

4. **Test Tools Independently**
   - Test binary tools with sample inputs
   - Test HTTP tools with mock servers
   - Test script tools with unit tests

## Backward Compatibility

The extensible tool system is fully backward compatible:

- All 7 built-in tools work exactly as before
- Existing code using `ToolExecutor` continues to work
- Tool definitions are automatically updated when skills are activated/deactivated

## Performance

- **Tool Registration**: O(1) with RwLock
- **Tool Lookup**: O(1) hash map lookup
- **Tool Execution**: Depends on tool type
  - Binary: Process spawn overhead + execution time
  - HTTP: Network latency + API response time
  - Script: Interpreter startup + execution time

## Limitations

1. **Binary Tools**
   - Must be executable on the guest OS (Linux)
   - No interactive input support
   - Output limited to 100KB

2. **HTTP Tools**
   - No streaming response support
   - No file upload support (yet)
   - Response limited to 100KB

3. **Script Tools**
   - No interactive input support
   - Interpreter must be installed in guest
   - Output limited to 100KB

## Future Enhancements

- [ ] Streaming HTTP responses
- [ ] File upload support for HTTP tools
- [ ] WebSocket support
- [ ] Database query tools
- [ ] Tool composition (pipe tools together)
- [ ] Tool versioning and updates
- [ ] Tool marketplace
