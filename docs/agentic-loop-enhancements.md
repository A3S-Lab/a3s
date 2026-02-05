# A3S Code AgenticLoop Enhancements

## Overview

This document outlines enhancements needed to make A3S Code's `AgentLoop` more similar to OpenCode's `AgenticLoop` mechanism, enabling more autonomous and intelligent agent behavior.

## Current State Analysis

### What A3S Code Already Has ✅

1. **Basic Agent Loop** (`crates/code/src/agent.rs`)
   - User prompt → LLM → Tool calls → Tool results → LLM → Final response
   - Max tool rounds protection (default: 50)
   - Streaming support with events

2. **Tool Execution**
   - Comprehensive tool executor
   - Tool result handling
   - Error handling

3. **Permission System**
   - Allow/Deny/Ask rules
   - Permission policy evaluation
   - Permission denied events

4. **Human-in-the-Loop (HITL)**
   - Confirmation requests
   - Timeout handling (Reject/AutoApprove)
   - YOLO mode for specific lanes
   - Confirmation events

5. **Context Management**
   - Context providers
   - Context resolution
   - Augmented system prompts
   - Turn completion callbacks

6. **Event System**
   - Rich event types (43 different events)
   - Streaming events
   - Event broadcasting

### What's Missing (Compared to OpenCode) ❌

1. **Autonomous Planning**
   - No explicit planning phase before execution
   - No task decomposition
   - No step-by-step execution tracking

2. **Self-Reflection**
   - No reflection on tool results
   - No error recovery strategies
   - No learning from failures

3. **Goal-Oriented Behavior**
   - No explicit goal tracking
   - No progress monitoring
   - No completion criteria

4. **Adaptive Behavior**
   - No dynamic strategy adjustment
   - No context-aware tool selection
   - No performance optimization

5. **Memory & Learning**
   - Limited memory integration
   - No learning from past interactions
   - No pattern recognition

## Proposed Enhancements

### 1. Planning Phase

Add an explicit planning phase before execution:

```rust
/// Planning phase result
pub struct ExecutionPlan {
    /// High-level goal
    pub goal: String,
    /// Decomposed steps
    pub steps: Vec<PlanStep>,
    /// Estimated complexity
    pub complexity: Complexity,
    /// Required tools
    pub required_tools: Vec<String>,
}

pub struct PlanStep {
    pub id: String,
    pub description: String,
    pub tool: Option<String>,
    pub dependencies: Vec<String>,
    pub status: StepStatus,
}

pub enum StepStatus {
    Pending,
    InProgress,
    Completed,
    Failed,
    Skipped,
}

impl AgentLoop {
    /// Create an execution plan before starting
    pub async fn plan(
        &self,
        prompt: &str,
        context: Option<&str>,
    ) -> Result<ExecutionPlan> {
        // Use LLM to create a plan
        // Return structured plan
    }

    /// Execute with planning
    pub async fn execute_with_planning(
        &self,
        history: &[Message],
        prompt: &str,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<AgentResult> {
        // 1. Create plan
        let plan = self.plan(prompt, None).await?;

        // 2. Execute plan step by step
        // 3. Track progress
        // 4. Adapt if needed
    }
}
```

### 2. Self-Reflection

Add reflection after each tool execution:

```rust
/// Reflection on tool execution
pub struct ToolReflection {
    /// Was the tool successful?
    pub success: bool,
    /// What was learned?
    pub insights: Vec<String>,
    /// Should we retry?
    pub should_retry: bool,
    /// Alternative approach?
    pub alternative: Option<String>,
}

impl AgentLoop {
    /// Reflect on tool execution result
    async fn reflect_on_tool_result(
        &self,
        tool_name: &str,
        tool_args: &serde_json::Value,
        tool_output: &str,
        exit_code: i32,
    ) -> Result<ToolReflection> {
        // Use LLM to analyze the result
        // Determine if successful
        // Suggest improvements
    }

    /// Execute tool with reflection
    async fn execute_tool_with_reflection(
        &self,
        tool_call: &ToolCall,
    ) -> Result<(String, i32, ToolReflection)> {
        // Execute tool
        let result = self.tool_executor.execute(&tool_call.name, &tool_call.args).await?;

        // Reflect on result
        let reflection = self.reflect_on_tool_result(
            &tool_call.name,
            &tool_call.args,
            &result.output,
            result.exit_code,
        ).await?;

        Ok((result.output, result.exit_code, reflection))
    }
}
```

### 3. Goal Tracking

Add explicit goal tracking:

```rust
/// Agent goal
pub struct AgentGoal {
    /// Goal description
    pub description: String,
    /// Success criteria
    pub success_criteria: Vec<String>,
    /// Current progress (0.0 - 1.0)
    pub progress: f32,
    /// Is goal achieved?
    pub achieved: bool,
}

pub struct AgentConfig {
    // ... existing fields ...

    /// Goal tracking enabled
    pub goal_tracking: bool,
}

impl AgentLoop {
    /// Extract goal from prompt
    async fn extract_goal(&self, prompt: &str) -> Result<AgentGoal> {
        // Use LLM to extract goal and success criteria
    }

    /// Check if goal is achieved
    async fn check_goal_achievement(
        &self,
        goal: &AgentGoal,
        current_state: &str,
    ) -> Result<bool> {
        // Use LLM to check if goal is achieved
    }

    /// Update goal progress
    async fn update_goal_progress(
        &self,
        goal: &mut AgentGoal,
        completed_steps: usize,
        total_steps: usize,
    ) {
        goal.progress = completed_steps as f32 / total_steps as f32;
    }
}
```

### 4. Adaptive Strategy

Add dynamic strategy adjustment:

```rust
/// Execution strategy
pub enum ExecutionStrategy {
    /// Direct execution (no planning)
    Direct,
    /// Plan then execute
    Planned,
    /// Iterative refinement
    Iterative,
    /// Parallel execution
    Parallel,
}

pub struct StrategySelector {
    /// Select strategy based on prompt complexity
    pub fn select_strategy(&self, prompt: &str, complexity: Complexity) -> ExecutionStrategy {
        match complexity {
            Complexity::Simple => ExecutionStrategy::Direct,
            Complexity::Medium => ExecutionStrategy::Planned,
            Complexity::Complex => ExecutionStrategy::Iterative,
            Complexity::VeryComplex => ExecutionStrategy::Parallel,
        }
    }
}

impl AgentLoop {
    /// Execute with adaptive strategy
    pub async fn execute_adaptive(
        &self,
        history: &[Message],
        prompt: &str,
        event_tx: Option<mpsc::Sender<AgentEvent>>,
    ) -> Result<AgentResult> {
        // 1. Analyze prompt complexity
        let complexity = self.analyze_complexity(prompt).await?;

        // 2. Select strategy
        let strategy = StrategySelector::new().select_strategy(prompt, complexity);

        // 3. Execute with selected strategy
        match strategy {
            ExecutionStrategy::Direct => self.execute(history, prompt, event_tx).await,
            ExecutionStrategy::Planned => self.execute_with_planning(history, prompt, event_tx).await,
            ExecutionStrategy::Iterative => self.execute_iterative(history, prompt, event_tx).await,
            ExecutionStrategy::Parallel => self.execute_parallel(history, prompt, event_tx).await,
        }
    }
}
```

### 5. Memory Integration

Enhance memory and learning:

```rust
/// Agent memory
pub struct AgentMemory {
    /// Short-term memory (current session)
    pub short_term: Vec<MemoryItem>,
    /// Long-term memory (persistent)
    pub long_term: Arc<dyn MemoryStore>,
    /// Working memory (active context)
    pub working: Vec<MemoryItem>,
}

pub struct MemoryItem {
    pub id: String,
    pub content: String,
    pub timestamp: i64,
    pub importance: f32,
    pub tags: Vec<String>,
}

impl AgentLoop {
    /// Store successful patterns
    async fn remember_success(
        &self,
        prompt: &str,
        tools_used: &[String],
        result: &str,
    ) -> Result<()> {
        // Store in memory for future reference
    }

    /// Recall similar past experiences
    async fn recall_similar(
        &self,
        prompt: &str,
    ) -> Result<Vec<MemoryItem>> {
        // Search memory for similar tasks
    }

    /// Learn from failures
    async fn learn_from_failure(
        &self,
        prompt: &str,
        error: &str,
        attempted_tools: &[String],
    ) -> Result<()> {
        // Store failure pattern to avoid repeating
    }
}
```

### 6. Enhanced Events

Add new event types for agentic behavior:

```rust
pub enum AgentEvent {
    // ... existing events ...

    /// Planning phase started
    #[serde(rename = "planning_start")]
    PlanningStart { prompt: String },

    /// Planning phase completed
    #[serde(rename = "planning_end")]
    PlanningEnd { plan: ExecutionPlan },

    /// Step execution started
    #[serde(rename = "step_start")]
    StepStart { step_id: String, description: String },

    /// Step execution completed
    #[serde(rename = "step_end")]
    StepEnd { step_id: String, status: StepStatus },

    /// Reflection on tool result
    #[serde(rename = "tool_reflection")]
    ToolReflection { tool_name: String, reflection: ToolReflection },

    /// Goal progress update
    #[serde(rename = "goal_progress")]
    GoalProgress { goal: String, progress: f32 },

    /// Goal achieved
    #[serde(rename = "goal_achieved")]
    GoalAchieved { goal: String },

    /// Strategy selected
    #[serde(rename = "strategy_selected")]
    StrategySelected { strategy: ExecutionStrategy },

    /// Memory recalled
    #[serde(rename = "memory_recalled")]
    MemoryRecalled { items: Vec<MemoryItem> },
}
```

## Implementation Plan

### Phase 1: Planning & Goal Tracking (Week 1-2)

1. **Add Planning Phase**
   - Create `ExecutionPlan` struct
   - Implement `plan()` method
   - Add planning events
   - Test with simple tasks

2. **Add Goal Tracking**
   - Create `AgentGoal` struct
   - Implement goal extraction
   - Implement progress tracking
   - Add goal events

### Phase 2: Reflection & Adaptation (Week 3-4)

3. **Add Self-Reflection**
   - Create `ToolReflection` struct
   - Implement reflection after tool execution
   - Add retry logic based on reflection
   - Test with failing tools

4. **Add Adaptive Strategy**
   - Create `ExecutionStrategy` enum
   - Implement strategy selector
   - Implement different execution modes
   - Test with various complexity levels

### Phase 3: Memory & Learning (Week 5-6)

5. **Enhance Memory Integration**
   - Create `AgentMemory` struct
   - Implement memory storage
   - Implement memory recall
   - Add learning from failures

6. **Integration & Testing**
   - Integrate all components
   - End-to-end testing
   - Performance optimization
   - Documentation

## Usage Examples

### Example 1: Execute with Planning

```rust
use a3s_code::agent::{AgentLoop, AgentConfig};

let agent = AgentLoop::new(llm_client, tool_executor, config);

// Execute with automatic planning
let result = agent.execute_with_planning(
    &history,
    "Create a REST API with authentication",
    Some(event_tx),
).await?;

// The agent will:
// 1. Create a plan (design API, implement auth, add tests)
// 2. Execute each step
// 3. Track progress
// 4. Reflect on results
// 5. Adapt if needed
```

### Example 2: Execute with Goal Tracking

```rust
let config = AgentConfig {
    goal_tracking: true,
    ..Default::default()
};

let agent = AgentLoop::new(llm_client, tool_executor, config);

let result = agent.execute_adaptive(
    &history,
    "Fix all bugs in the authentication module",
    Some(event_tx),
).await?;

// The agent will:
// 1. Extract goal: "Fix all bugs"
// 2. Define success criteria
// 3. Track progress
// 4. Report when goal is achieved
```

### Example 3: Execute with Memory

```rust
let memory = AgentMemory::new(memory_store);
let config = AgentConfig {
    memory: Some(memory),
    ..Default::default()
};

let agent = AgentLoop::new(llm_client, tool_executor, config);

let result = agent.execute_adaptive(
    &history,
    "Implement user registration",
    Some(event_tx),
).await?;

// The agent will:
// 1. Recall similar past tasks
// 2. Use successful patterns
// 3. Avoid known failures
// 4. Store new learnings
```

## Benefits

### 1. More Autonomous
- Agent can plan and execute complex tasks independently
- Less need for user intervention
- Better handling of multi-step tasks

### 2. More Intelligent
- Learns from past experiences
- Adapts strategy based on task complexity
- Reflects on results and improves

### 3. More Reliable
- Goal tracking ensures task completion
- Reflection catches errors early
- Memory prevents repeating mistakes

### 4. Better User Experience
- Clear progress tracking
- Transparent decision-making
- Predictable behavior

## Comparison: Before vs After

| Feature | Current A3S Code | With Enhancements | OpenCode |
|---------|------------------|-------------------|----------|
| Basic Loop | ✅ | ✅ | ✅ |
| Tool Execution | ✅ | ✅ | ✅ |
| Streaming | ✅ | ✅ | ✅ |
| Permissions | ✅ | ✅ | ⚠️ Limited |
| HITL | ✅ | ✅ | ⚠️ Limited |
| Planning | ❌ | ✅ | ✅ |
| Goal Tracking | ❌ | ✅ | ✅ |
| Reflection | ❌ | ✅ | ✅ |
| Adaptive Strategy | ❌ | ✅ | ✅ |
| Memory/Learning | ⚠️ Limited | ✅ | ✅ |
| Context Providers | ✅ | ✅ | ⚠️ Limited |

## Testing Strategy

### Unit Tests

```rust
#[tokio::test]
async fn test_planning_phase() {
    let agent = create_test_agent();
    let plan = agent.plan("Create a web server", None).await.unwrap();

    assert!(!plan.steps.is_empty());
    assert!(plan.required_tools.contains(&"write".to_string()));
}

#[tokio::test]
async fn test_goal_tracking() {
    let agent = create_test_agent_with_goal_tracking();
    let result = agent.execute_with_planning(&[], "Fix bug", None).await.unwrap();

    // Check goal was tracked and achieved
}

#[tokio::test]
async fn test_reflection() {
    let agent = create_test_agent();
    let reflection = agent.reflect_on_tool_result(
        "bash",
        &json!({"command": "invalid"}),
        "command not found",
        127,
    ).await.unwrap();

    assert!(!reflection.success);
    assert!(reflection.should_retry);
}
```

### Integration Tests

```rust
#[tokio::test]
async fn test_end_to_end_with_planning() {
    // Test complete workflow with planning
}

#[tokio::test]
async fn test_adaptive_strategy_selection() {
    // Test strategy selection based on complexity
}

#[tokio::test]
async fn test_memory_recall() {
    // Test memory recall and learning
}
```

## Migration Path

### For Existing Users

1. **Backward Compatible**
   - All existing code continues to work
   - New features are opt-in via `AgentConfig`

2. **Gradual Adoption**
   - Start with planning: `execute_with_planning()`
   - Add goal tracking: `goal_tracking: true`
   - Enable memory: `memory: Some(memory)`

3. **Full Migration**
   - Use `execute_adaptive()` for automatic strategy selection
   - Enable all features for maximum autonomy

## References

- **OpenCode**: https://opencode.ai/
- **Current A3S Agent**: `crates/code/src/agent.rs`
- **System Prompt**: `crates/code/prompts/default_system_prompt.md`
- **Context Providers**: `crates/code/src/context.rs`

---

**Status**: 📋 Design Document
**Next Steps**: Implement Phase 1 (Planning & Goal Tracking)
**Target**: v0.2.0
