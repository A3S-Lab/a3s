# Response Transformation Middleware Design

## Current State (Phase 1)

### Implementation
Currently, we have simple transformer functions in `service.rs`:

```rust
fn remove_think_tags(text: &str) -> String
fn extract_json(text: &str) -> String
fn transform_for_structured_output(text: &str) -> String
```

### Usage
Applied automatically in `generate_object()`:
```rust
let json_str = transform_for_structured_output(&result.text);
```

### Limitations
- Not configurable by users
- Cannot be composed or extended
- Only works for non-streaming responses
- Hardcoded transformation pipeline

## Future Design (Phase 2)

### Goals
1. **Extensibility**: Users can add custom transformers
2. **Composability**: Chain multiple transformers
3. **Streaming Support**: Transform streaming responses in real-time
4. **Type Safety**: Rust type system ensures correctness
5. **Performance**: Minimal overhead

### Architecture

#### Transformer Trait
```rust
/// Response transformer trait
pub trait ResponseTransformer: Send + Sync {
    /// Transform text synchronously
    fn transform(&self, text: &str) -> String;

    /// Transform streaming text (optional)
    fn transform_stream(&self, text: &str) -> Option<String> {
        Some(self.transform(text))
    }
}
```

#### Built-in Transformers
```rust
pub struct RemoveThinkTags;
pub struct ExtractJSON;
pub struct ExtractMarkdown;
pub struct TrimWhitespace;
pub struct RemoveCodeBlocks;
```

#### Transformer Chain
```rust
pub struct TransformerChain {
    transformers: Vec<Box<dyn ResponseTransformer>>,
}

impl TransformerChain {
    pub fn new() -> Self { ... }

    pub fn add(mut self, transformer: Box<dyn ResponseTransformer>) -> Self {
        self.transformers.push(transformer);
        self
    }

    pub fn transform(&self, text: &str) -> String {
        self.transformers.iter().fold(text.to_string(), |acc, t| {
            t.transform(&acc)
        })
    }
}
```

#### Session Configuration
```rust
pub struct SessionConfig {
    pub system: Option<String>,
    pub model: Option<ModelConfig>,
    pub transformers: Option<TransformerChain>,
}
```

### Usage Examples

#### Basic Usage
```rust
let chain = TransformerChain::new()
    .add(Box::new(RemoveThinkTags))
    .add(Box::new(ExtractJSON));

let config = SessionConfig {
    transformers: Some(chain),
    ..Default::default()
};

let session = session_manager.create_session(config).await?;
```

#### Custom Transformer
```rust
struct CustomTransformer;

impl ResponseTransformer for CustomTransformer {
    fn transform(&self, text: &str) -> String {
        // Custom logic
        text.replace("foo", "bar")
    }
}

let chain = TransformerChain::new()
    .add(Box::new(CustomTransformer))
    .add(Box::new(RemoveThinkTags));
```

#### Streaming Support
```rust
async fn stream_with_transform(
    session_id: &str,
    prompt: &str,
    transformers: &TransformerChain,
) -> impl Stream<Item = String> {
    let stream = session_manager.stream(session_id, prompt).await?;

    stream.map(move |chunk| {
        transformers.transform_stream(&chunk)
            .unwrap_or(chunk)
    })
}
```

## Implementation Phases

### Phase 1: Current (✅ Completed)
- [x] Simple transformer functions
- [x] Applied to `generate_object()`
- [x] Modular code structure

### Phase 2: Basic Middleware (Future)
- [ ] Define `ResponseTransformer` trait
- [ ] Implement built-in transformers
- [ ] Add `TransformerChain`
- [ ] Integrate with `SessionConfig`
- [ ] Update gRPC API to accept transformer config

### Phase 3: Advanced Features (Future)
- [ ] Streaming transformer support
- [ ] Async transformers (for I/O operations)
- [ ] Conditional transformers (apply based on context)
- [ ] Transformer metrics (track performance)
- [ ] User-defined transformers via SDK

### Phase 4: SDK Integration (Future)
- [ ] TypeScript SDK transformer API
- [ ] Python SDK transformer API
- [ ] Pre-built transformer library
- [ ] Documentation and examples

## Design Decisions

### Why Not Implement Now?
1. **YAGNI Principle**: We only have one use case (JSON extraction)
2. **Complexity**: Middleware adds significant complexity
3. **Performance**: Need to measure overhead before committing
4. **API Stability**: Want to stabilize core API first

### When to Implement?
Trigger conditions for Phase 2:
1. **Multiple Use Cases**: 3+ different transformation needs
2. **User Requests**: Users ask for custom transformations
3. **Performance Issues**: Current approach causes problems
4. **Streaming Needs**: Need to transform streaming responses

### Inspiration
- [Vercel AI SDK Middleware](https://sdk.vercel.ai/docs/ai-sdk-core/middleware)
- [LangChain Output Parsers](https://python.langchain.com/docs/modules/model_io/output_parsers/)
- [Axum Middleware](https://docs.rs/axum/latest/axum/middleware/)

## References
- Current implementation: `src/code/src/service.rs` (Response Transformers section)
- Related issue: Handling `<think>` tags in structured output
- Design principles: CLAUDE.md (Rule #4: Only What's Used)
