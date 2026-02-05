# SDK Examples Summary

## Overview

Comprehensive examples have been created for both TypeScript and Python SDKs, demonstrating all major features of the A3S Code SDK.

## Files Created

### TypeScript SDK Examples

**Location**: `sdk/typescript/examples/src/`

| File | Lines | Description |
|------|-------|-------------|
| `simple-test.ts` | 182 | Basic SDK usage (already existed) |
| `skill-management.ts` | 90 | Skill system demonstration |
| `permission-policy.ts` | 120 | Permission policy management |
| `event-streaming.ts` | 150 | Real-time event streaming |
| `README.md` | Updated | Comprehensive documentation |

### Python SDK Examples

**Location**: `sdk/python/examples/`

| File | Lines | Description |
|------|-------|-------------|
| `basic_usage.py` | 120 | Basic SDK usage |
| `skill_management.py` | 90 | Skill system demonstration |
| `permission_policy.py` | 130 | Permission policy management |
| `event_streaming.py` | 140 | Real-time event streaming |
| `README.md` | 250 | Comprehensive documentation |

## Features Demonstrated

### 1. Basic Usage ✅

**TypeScript**: `simple-test.ts`
**Python**: `basic_usage.py`

- Client creation and configuration
- Health checks
- Capability discovery
- Session management (create, list, destroy)
- Context usage tracking
- Basic text generation
- Streaming generation
- Message history retrieval

### 2. Skill Management ✅

**TypeScript**: `skill-management.ts`
**Python**: `skill_management.py`

- Listing available skills
- Loading skills dynamically
- Using skill capabilities
- Unloading skills
- Skill-enhanced generation

### 3. Permission Policies ✅

**TypeScript**: `permission-policy.ts`
**Python**: `permission_policy.py`

- Setting permission policies
- Allow/deny specific tools
- Checking permissions
- Adding rules dynamically
- Testing with actual generation

### 4. Event Streaming ✅

**TypeScript**: `event-streaming.ts`
**Python**: `event_streaming.py`

- Subscribing to real-time events
- Handling different event types
- Monitoring agent execution
- Tracking tool usage
- Event counting and summary

## Running the Examples

### TypeScript

```bash
cd sdk/typescript/examples

# Install dependencies
npm install

# Run examples
npx tsx src/simple-test.ts
npx tsx src/skill-management.ts
npx tsx src/permission-policy.ts
npx tsx src/event-streaming.ts
```

### Python

```bash
cd sdk/python

# Install SDK
pip install -e .

# Run examples
python examples/basic_usage.py
python examples/skill_management.py
python examples/permission_policy.py
python examples/event_streaming.py
```

## Prerequisites

### 1. Start A3S Code Agent

```bash
# From a3s repository root
just run-code
```

### 2. Configure API Keys

Create `~/.a3s/config.json`:

```json
{
  "defaultProvider": "anthropic",
  "defaultModel": "claude-sonnet-4-20250514",
  "apiKey": "your-api-key-here",
  "baseUrl": "https://api.anthropic.com"
}
```

Or set environment variables:

```bash
export ANTHROPIC_API_KEY=your-api-key-here
export A3S_ADDRESS=localhost:4088
```

## Event Types Covered

All examples demonstrate handling of these event types:

| Event Type | Description | Example |
|------------|-------------|---------|
| `EVENT_TYPE_AGENT_START` | Agent started | All examples |
| `EVENT_TYPE_TURN_START` | LLM turn started | All examples |
| `EVENT_TYPE_TEXT_DELTA` | Streaming text | Streaming examples |
| `EVENT_TYPE_TOOL_START` | Tool execution started | Event streaming |
| `EVENT_TYPE_TOOL_END` | Tool execution completed | Event streaming |
| `EVENT_TYPE_TURN_END` | LLM turn completed | All examples |
| `EVENT_TYPE_AGENT_END` | Agent completed | All examples |
| `EVENT_TYPE_ERROR` | Error occurred | Error handling |
| `EVENT_TYPE_CONTEXT_RESOLVING` | Context resolution | Event streaming |
| `EVENT_TYPE_CONTEXT_RESOLVED` | Context resolved | Event streaming |
| `EVENT_TYPE_PERMISSION_DENIED` | Permission denied | Permission examples |
| `EVENT_TYPE_CONFIRMATION_REQUIRED` | HITL confirmation | Event streaming |

## Code Patterns

### TypeScript Patterns

```typescript
// Client creation
const client = new A3sClient({ address: 'localhost:4088' });

// Session creation
const session = await client.createSession({
  name: 'demo',
  workspace: '/tmp/test',
  systemPrompt: 'You are helpful.',
});

// Generation
const response = await client.generate(session.sessionId, [
  { role: 'ROLE_USER', content: 'Hello!' }
]);

// Streaming
for await (const chunk of client.streamGenerate(sessionId, messages)) {
  if (chunk.type === 'CHUNK_TYPE_CONTENT') {
    process.stdout.write(chunk.content);
  }
}

// Event streaming
for await (const event of client.subscribeEvents(sessionId)) {
  console.log(event.type);
}

// Cleanup
client.close();
```

### Python Patterns

```python
# Client creation with context manager
async with A3sClient(address="localhost:4088") as client:
    # Session creation
    session = await client.create_session(
        name="demo",
        workspace="/tmp/test",
        system_prompt="You are helpful.",
    )

    # Generation
    response = await client.generate(
        session_id=session["session_id"],
        messages=[{"role": "ROLE_USER", "content": "Hello!"}],
    )

    # Streaming
    async for chunk in client.stream_generate(session_id, messages):
        if chunk.get("type") == "CHUNK_TYPE_CONTENT":
            print(chunk.get("content", ""), end="")

    # Event streaming
    async for event in client.subscribe_events(session_id):
        print(event.get("type"))

    # Cleanup (automatic with context manager)
```

## Testing Checklist

Use these examples to test SDK functionality:

- [ ] **Basic Usage**
  - [ ] Client creation
  - [ ] Health check
  - [ ] Session management
  - [ ] Basic generation
  - [ ] Streaming generation

- [ ] **Skill Management**
  - [ ] List skills
  - [ ] Load skill
  - [ ] Use skill
  - [ ] Unload skill

- [ ] **Permission Policies**
  - [ ] Set policy
  - [ ] Check permissions
  - [ ] Add rules
  - [ ] Test enforcement

- [ ] **Event Streaming**
  - [ ] Subscribe to events
  - [ ] Handle all event types
  - [ ] Track execution
  - [ ] Count events

## Next Steps

### Additional Examples to Create

1. **HITL (Human-in-the-Loop)**
   - Confirmation requests
   - Timeout handling
   - YOLO mode

2. **External Task Handling**
   - Set lane handler
   - Complete external tasks
   - List pending tasks

3. **Provider Configuration**
   - List providers
   - Add/update/remove providers
   - Set default model

4. **Todo/Task Tracking**
   - Get todos
   - Set todos
   - Track progress

5. **Context Management**
   - Compact context
   - Clear context
   - Monitor usage

### Documentation Improvements

1. Add API reference documentation
2. Create troubleshooting guide
3. Add performance benchmarks
4. Create video tutorials

### Testing

1. Add automated tests for examples
2. Create CI/CD pipeline
3. Add integration tests
4. Performance testing

## Benefits

### For Users

- **Easy to understand**: Clear, well-documented examples
- **Copy-paste ready**: Code can be used directly
- **Comprehensive**: Covers all major features
- **Language parity**: Same examples in TypeScript and Python

### For Developers

- **Testing**: Examples serve as integration tests
- **Documentation**: Living documentation of SDK usage
- **Debugging**: Easy to reproduce issues
- **Onboarding**: New developers can learn quickly

## Maintenance

### Keeping Examples Updated

1. **When adding new features**: Add corresponding examples
2. **When changing APIs**: Update all affected examples
3. **When fixing bugs**: Add examples demonstrating the fix
4. **Regular testing**: Run all examples periodically

### Version Compatibility

- Examples are compatible with SDK v0.1.0+
- Update examples when breaking changes occur
- Maintain backward compatibility examples

## Contributing

To add a new example:

1. Create the example file in `examples/src/` (TypeScript) or `examples/` (Python)
2. Follow the existing code style and structure
3. Add documentation to the README
4. Test the example thoroughly
5. Submit a pull request

## License

MIT License - see LICENSE for details.

---

**Created**: 2026-02-05
**SDK Version**: 0.1.0
**Status**: ✅ Complete
