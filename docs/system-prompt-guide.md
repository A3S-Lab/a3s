# A3S Code System Prompt Implementation Guide

## Overview

This document explains how A3S Code implements system prompts to enable autonomous coding, skill discovery, and skill invocation capabilities, inspired by OpenCode and Claude Code.

## System Prompt Architecture

### 1. Default System Prompt

**Location**: `crates/code/prompts/default_system_prompt.md`

The default system prompt provides:
- **Autonomous Coding**: Instructions for independent code writing, modification, and debugging
- **Tool Usage**: Guidelines for using Read, Write, Edit, Glob, Grep, Bash, etc.
- **Skill Discovery**: How to find and list available skills
- **Skill Invocation**: How to load and use skills dynamically
- **Best Practices**: Code quality, communication, and workflow guidelines

### 2. Prompt Module

**Location**: `crates/code/src/prompts.rs`

Provides functions to:
- `get_default_system_prompt()` - Get the base prompt
- `get_system_prompt_with_context(context)` - Add custom context

### 3. Integration with Agent

The system prompt is integrated into the agent through:

```rust
// In agent.rs
pub struct AgentConfig {
    pub system_prompt: Option<String>,
    // ... other fields
}

// Usage
let config = AgentConfig {
    system_prompt: Some(prompts::get_default_system_prompt()),
    // ... other config
};
```

## Key Capabilities

### 1. Autonomous Coding

The prompt instructs the agent to:
- **Understand tasks** independently
- **Explore codebases** using Glob, Read, Grep
- **Plan implementations** before coding
- **Write/modify code** using appropriate tools
- **Verify solutions** through testing
- **Communicate results** clearly

**Example Workflow**:
```
User: "Add error handling to the API endpoint"

Agent:
1. Uses Grep to find the API endpoint
2. Uses Read to examine the current code
3. Analyzes what error handling is needed
4. Uses Edit to add try-catch blocks
5. Runs tests with Bash
6. Explains the changes made
```

### 2. Skill Discovery

The prompt teaches the agent to:
- **List available skills** using `listSkills()`
- **Understand skill capabilities** from descriptions
- **Identify when skills are useful**

**Example**:
```
User: "Help me create a video with Remotion"

Agent:
1. Uses listSkills() to see available skills
2. Finds "remotion-best-practices" skill
3. Loads the skill with loadSkill()
4. Uses the skill's tools to help with Remotion
5. Unloads the skill when done
```

### 3. Skill Invocation

The prompt explains how to:
- **Load skills** with `loadSkill(sessionId, skillName)`
- **Use skill tools** once loaded
- **Unload skills** with `unloadSkill(sessionId, skillName)`

**Skill Lifecycle**:
```
1. Discovery: listSkills() → See available skills
2. Loading: loadSkill() → Make skill tools available
3. Usage: Use the skill's tools
4. Cleanup: unloadSkill() → Remove skill tools
```

## Comparison with OpenCode

### OpenCode Approach
- **System Prompt**: Embedded in the agent configuration
- **Tool Discovery**: Tools are pre-registered
- **Autonomous Behavior**: Enabled through prompt engineering
- **Multi-turn Conversations**: Agent can use tools across multiple turns

### A3S Code Approach
- **System Prompt**: Modular, file-based, customizable
- **Skill Discovery**: Dynamic skill loading/unloading
- **Autonomous Behavior**: Comprehensive prompt + tool ecosystem
- **Multi-turn Conversations**: Session-based with context management
- **Additional Features**:
  - Permission policies
  - Human-in-the-loop confirmations
  - External task handling
  - Context providers

## Usage Examples

### Example 1: Using Default Prompt

```rust
use a3s_code::prompts;

let system_prompt = prompts::get_default_system_prompt();

let config = SessionConfig {
    name: "coding-session".to_string(),
    workspace: "/path/to/project".to_string(),
    system_prompt: Some(system_prompt),
    ..Default::default()
};
```

### Example 2: Adding Custom Context

```rust
use a3s_code::prompts;

let custom_context = r#"
This project uses:
- Rust with Tokio for async
- PostgreSQL for database
- gRPC for API

Follow these conventions:
- Use snake_case for functions
- Add doc comments to public APIs
- Write tests for all features
"#;

let system_prompt = prompts::get_system_prompt_with_context(custom_context);
```

### Example 3: Project-Specific Prompt

```rust
let project_prompt = format!(
    "{}\n\n## Project Context\n\n{}\n\n## Coding Standards\n\n{}",
    prompts::get_default_system_prompt(),
    project_description,
    coding_standards
);
```

## Best Practices

### 1. Prompt Customization

**Do**:
- Add project-specific context
- Include coding standards
- Specify technology stack
- Mention important conventions

**Don't**:
- Remove core instructions
- Contradict tool usage guidelines
- Make the prompt too long (>10K tokens)

### 2. Skill Management

**Do**:
- Create skills for repeated patterns
- Document skill capabilities clearly
- Load skills only when needed
- Unload skills after use

**Don't**:
- Load too many skills at once
- Create overlapping skills
- Forget to unload skills

### 3. Tool Usage

**Do**:
- Use Read before Edit
- Use Glob to find files
- Use Grep to search content
- Batch related operations

**Don't**:
- Use Bash for file operations
- Skip error checking
- Ignore tool output

## Testing the System Prompt

### Unit Tests

```rust
#[test]
fn test_agent_with_default_prompt() {
    let prompt = prompts::get_default_system_prompt();
    let config = AgentConfig {
        system_prompt: Some(prompt),
        ..Default::default()
    };

    // Test that agent can understand and execute tasks
    // ...
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_autonomous_coding() {
    let client = create_test_client();
    let session = client.create_session(SessionConfig {
        system_prompt: Some(prompts::get_default_system_prompt()),
        ..Default::default()
    }).await.unwrap();

    // Test: Agent should be able to write code autonomously
    let response = client.generate(
        session.session_id,
        vec![Message {
            role: MessageRole::USER,
            content: "Create a function that adds two numbers".to_string(),
            ..Default::default()
        }]
    ).await.unwrap();

    // Verify agent used Write tool and created valid code
    assert!(response.tool_calls.iter().any(|t| t.name == "Write"));
}
```

## Maintenance

### Updating the Prompt

1. **Edit the markdown file**: `crates/code/prompts/default_system_prompt.md`
2. **Test changes**: Run unit and integration tests
3. **Document changes**: Update this guide if needed
4. **Version control**: Commit with clear message

### Adding New Capabilities

When adding new tools or features:
1. Update the "Tool Reference" section
2. Add examples to "Example Workflows"
3. Update "Best Practices" if needed
4. Test with real scenarios

### Monitoring Effectiveness

Track:
- **Task completion rate**: How often agent completes tasks successfully
- **Tool usage patterns**: Which tools are used most
- **Skill adoption**: How often skills are discovered and used
- **Error rates**: Common failure modes

## Future Enhancements

### Planned Improvements

1. **Context-Aware Prompts**: Automatically adjust prompt based on:
   - Programming language
   - Project type
   - User preferences

2. **Skill Recommendations**: Agent suggests relevant skills:
   - Based on task type
   - Based on codebase analysis
   - Based on user history

3. **Learning from Feedback**: Improve prompt based on:
   - User corrections
   - Successful patterns
   - Common mistakes

4. **Multi-Agent Coordination**: Prompts for:
   - Specialized sub-agents
   - Parallel task execution
   - Agent collaboration

## References

- **OpenCode**: https://opencode.ai/
- **Claude Code**: https://claude.ai/code
- **A3S Code Documentation**: `crates/code/README.md`
- **Skill System**: `crates/code/src/subagent.rs`
- **Tool System**: `crates/code/src/tools/`

## Support

For questions or issues:
- Check the documentation: `docs/`
- Review examples: `examples/`
- Open an issue: GitHub Issues
- Contact maintainers: See MAINTAINERS.md

---

**Last Updated**: 2026-02-05
**Version**: 0.1.0
