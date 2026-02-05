# Phase 1 Implementation Guide

## Overview

This guide explains how to integrate Phase 1 (Planning and Goal Tracking) into `agent.rs`.

## Files Created

1. `src/planning.rs` - Core planning and goal tracking structures
2. `src/agent_phase1_additions.rs` - Code to add to agent.rs
3. `src/lib.rs` - Updated to include planning module

## Integration Steps

### Step 1: Add Import

In `agent.rs`, after line 16 (after the existing imports), add:

```rust
use crate::planning::{AgentGoal, Complexity, ExecutionPlan, PlanStep, StepStatus};
```

### Step 2: Update AgentConfig

In the `AgentConfig` struct (around line 28), add these fields before the closing brace:

```rust
/// Enable planning phase before execution
pub planning_enabled: bool,
/// Enable goal tracking
pub goal_tracking: bool,
```

### Step 3: Update AgentConfig::default()

Replace the `Default` implementation (lines 53-64) with:

```rust
impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            system_prompt: None,
            tools: Vec::new(),
            max_tool_rounds: MAX_TOOL_ROUNDS,
            permission_policy: None,
            confirmation_manager: None,
            context_providers: Vec::new(),
            planning_enabled: false,
            goal_tracking: false,
        }
    }
}
```

### Step 4: Update AgentConfig::Debug

In the `Debug` implementation (lines 40-50), add these fields:

```rust
.field("planning_enabled", &self.planning_enabled)
.field("goal_tracking", &self.goal_tracking)
```

### Step 5: Add New Event Types

In the `AgentEvent` enum (after the `SubagentEnd` variant, around line 258), add:

```rust
/// Planning phase started
#[serde(rename = "planning_start")]
PlanningStart { prompt: String },

/// Planning phase completed
#[serde(rename = "planning_end")]
PlanningEnd {
    plan: ExecutionPlan,
    estimated_steps: usize,
},

/// Step execution started
#[serde(rename = "step_start")]
StepStart {
    step_id: String,
    description: String,
    step_number: usize,
    total_steps: usize,
},

/// Step execution completed
#[serde(rename = "step_end")]
StepEnd {
    step_id: String,
    status: StepStatus,
    step_number: usize,
    total_steps: usize,
},

/// Goal extracted from prompt
#[serde(rename = "goal_extracted")]
GoalExtracted { goal: AgentGoal },

/// Goal progress update
#[serde(rename = "goal_progress")]
GoalProgress {
    goal: String,
    progress: f32,
    completed_steps: usize,
    total_steps: usize,
},

/// Goal achieved
#[serde(rename = "goal_achieved")]
GoalAchieved {
    goal: String,
    total_steps: usize,
    duration_ms: i64,
},
```

### Step 6: Add New Methods to AgentLoop

After the `execute_streaming` method (around line 771), before the `AgentBuilder` struct, add all the methods from `agent_phase1_additions.rs`:

- `analyze_complexity()`
- `plan()`
- `parse_plan()`
- `execute_with_planning()`
- `execute_plan()`
- `extract_goal()`
- `check_goal_achievement()`

Copy the entire implementation from `agent_phase1_additions.rs` starting from the comment "Add new methods to AgentLoop impl".

## Testing

### Unit Tests

Add these tests to the `#[cfg(test)]` module at the end of `agent.rs`:

```rust
#[tokio::test]
async fn test_analyze_complexity() {
    let mock_client = Arc::new(MockLlmClient::new(vec![
        MockLlmClient::text_response("Medium"),
    ]));

    let tool_executor = Arc::new(ToolExecutor::new("/tmp".to_string()));
    let config = AgentConfig::default();

    let agent = AgentLoop::new(mock_client, tool_executor, config);
    let complexity = agent.analyze_complexity("Create a web server").await.unwrap();

    assert_eq!(complexity, Complexity::Medium);
}

#[tokio::test]
async fn test_planning() {
    let mock_client = Arc::new(MockLlmClient::new(vec![
        // Complexity analysis
        MockLlmClient::text_response("Medium"),
        // Planning response
        MockLlmClient::text_response(
            "GOAL: Create a web server\nSTEPS:\n1. [tool: write] Create server file\n2. [tool: bash] Install dependencies (depends on: 1)\n3. [tool: bash] Start server (depends on: 2)"
        ),
    ]));

    let tool_executor = Arc::new(ToolExecutor::new("/tmp".to_string()));
    let config = AgentConfig {
        planning_enabled: true,
        ..Default::default()
    };

    let agent = AgentLoop::new(mock_client, tool_executor, config);
    let plan = agent.plan("Create a web server", None).await.unwrap();

    assert_eq!(plan.goal, "Create a web server");
    assert_eq!(plan.steps.len(), 3);
    assert_eq!(plan.complexity, Complexity::Medium);
}

#[tokio::test]
async fn test_goal_extraction() {
    let mock_client = Arc::new(MockLlmClient::new(vec![
        MockLlmClient::text_response(
            "GOAL: Fix authentication bug\nCRITERIA:\n- Users can log in successfully\n- No error messages appear\n- Session persists correctly"
        ),
    ]));

    let tool_executor = Arc::new(ToolExecutor::new("/tmp".to_string()));
    let config = AgentConfig {
        goal_tracking: true,
        ..Default::default()
    };

    let agent = AgentLoop::new(mock_client, tool_executor, config);
    let goal = agent.extract_goal("Fix the authentication bug").await.unwrap();

    assert_eq!(goal.description, "Fix authentication bug");
    assert_eq!(goal.success_criteria.len(), 3);
    assert!(!goal.achieved);
}

#[tokio::test]
async fn test_goal_achievement_check() {
    let mock_client = Arc::new(MockLlmClient::new(vec![
        MockLlmClient::text_response("YES"),
    ]));

    let tool_executor = Arc::new(ToolExecutor::new("/tmp".to_string()));
    let config = AgentConfig::default();

    let agent = AgentLoop::new(mock_client, tool_executor, config);
    let goal = AgentGoal::new("Complete task")
        .with_criteria(vec!["Task is done".to_string()]);

    let achieved = agent
        .check_goal_achievement(&goal, "Task completed successfully")
        .await
        .unwrap();

    assert!(achieved);
}
```

## Verification

After integration, verify:

1. **Compilation**: `cargo build` should succeed
2. **Tests**: `cargo test agent::tests` should pass
3. **Planning module**: `cargo test planning::tests` should pass

## Usage Example

```rust
use a3s_code::agent::{AgentLoop, AgentConfig};
use a3s_code::prompts;

let config = AgentConfig {
    system_prompt: Some(prompts::get_default_system_prompt()),
    planning_enabled: true,
    goal_tracking: true,
    ..Default::default()
};

let agent = AgentLoop::new(llm_client, tool_executor, config);

// Execute with planning
let result = agent.execute_with_planning(
    &[],
    "Create a REST API with authentication",
    Some(event_tx),
).await?;
```

## Event Handling

```rust
let (tx, mut rx) = mpsc::channel(100);

tokio::spawn(async move {
    while let Some(event) = rx.recv().await {
        match event {
            AgentEvent::PlanningStart { prompt } => {
                println!("Planning: {}", prompt);
            }
            AgentEvent::PlanningEnd { plan, estimated_steps } => {
                println!("Plan created: {} steps", estimated_steps);
            }
            AgentEvent::StepStart { step_id, description, step_number, total_steps } => {
                println!("[{}/{}] Starting: {}", step_number, total_steps, description);
            }
            AgentEvent::StepEnd { step_id, status, .. } => {
                println!("Step {} completed: {:?}", step_id, status);
            }
            AgentEvent::GoalProgress { goal, progress, .. } => {
                println!("Goal '{}': {:.1}% complete", goal, progress * 100.0);
            }
            AgentEvent::GoalAchieved { goal, .. } => {
                println!("Goal achieved: {}", goal);
            }
            _ => {}
        }
    }
});
```

## Next Steps

After Phase 1 is complete:
- Phase 2: Add self-reflection and adaptive strategy
- Phase 3: Add memory and learning

## Troubleshooting

### Compilation Errors

**Error**: `cannot find type ExecutionPlan in this scope`
**Solution**: Make sure you added the import in Step 1

**Error**: `no field planning_enabled on type AgentConfig`
**Solution**: Make sure you added the fields in Step 2

### Test Failures

**Error**: Tests fail with "No more mock responses"
**Solution**: Add more mock responses to match the number of LLM calls

## Files Modified

- `src/lib.rs` - Added `pub mod planning;`
- `src/agent.rs` - Added planning and goal tracking functionality

## Files Created

- `src/planning.rs` - Core structures
- `src/agent_phase1_additions.rs` - Reference implementation
- `docs/phase1-implementation-guide.md` - This file

---

**Status**: Ready for Integration
**Estimated Time**: 30-60 minutes
**Complexity**: Medium
