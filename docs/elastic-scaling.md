# Elastic Scaling Design

## Overview

A3S Box Elastic Scaling is a Knative-inspired autoscaling system designed specifically for AI coding agent workloads. It provides intelligent scaling based on request patterns, queue depth, and session state, with support for scale-to-zero and sub-second cold starts.

## Design Goals

1. **Cost Efficiency**: Scale to zero when idle, minimize resource waste
2. **Low Latency**: Sub-second cold starts, warm instance pooling
3. **Session Awareness**: Respect session affinity, handle stateful workloads
4. **Queue Integration**: Scale based on lane-based queue metrics
5. **Predictive Scaling**: Anticipate demand spikes before they happen
6. **Graceful Degradation**: Handle overload without dropping requests

## Architecture

### System Overview

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           A3S Box Autoscaler                                 │
│                                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │  Metrics        │  │  Scaler         │  │  Box Pool Manager            │ │
│  │  Collector      │  │  Controller     │  │                             │ │
│  │                 │  │                 │  │  ┌───────┐ ┌───────┐       │ │
│  │  - Queue depth  │─►│  - Scale up     │─►│  │ Warm  │ │ Warm  │ ...   │ │
│  │  - Concurrency  │  │  - Scale down   │  │  │ Box 1 │ │ Box 2 │       │ │
│  │  - Latency      │  │  - Predictive   │  │  └───────┘ └───────┘       │ │
│  │  - Session count│  │                 │  │                             │ │
│  └─────────────────┘  └─────────────────┘  └─────────────────────────────┘ │
│                                                                             │
│  ┌─────────────────┐  ┌─────────────────┐  ┌─────────────────────────────┐ │
│  │  Session        │  │  Load           │  │  Instance Lifecycle          │ │
│  │  Router         │  │  Balancer       │  │                             │ │
│  │                 │  │                 │  │  Cold → Warm → Hot → Drain  │ │
│  │  - Affinity     │  │  - Least conn   │  │                             │ │
│  │  - Migration    │  │  - Queue aware  │  │  ┌───────┐ ┌───────┐       │ │
│  │                 │  │                 │  │  │ Hot   │ │ Drain │       │ │
│  └─────────────────┘  └─────────────────┘  │  │ Box   │ │ Box   │       │ │
│                                            │  └───────┘ └───────┘       │ │
└─────────────────────────────────────────────┴─────────────────────────────┘
                                    │
                    ┌───────────────┼───────────────┐
                    ▼               ▼               ▼
            ┌─────────────┐ ┌─────────────┐ ┌─────────────┐
            │   Box 1     │ │   Box 2     │ │   Box N     │
            │  (Active)   │ │  (Active)   │ │  (Standby)  │
            │             │ │             │ │             │
            │ Sessions: 3 │ │ Sessions: 2 │ │ Sessions: 0 │
            │ Queue: 5    │ │ Queue: 3    │ │ Queue: 0    │
            └─────────────┘ └─────────────┘ └─────────────┘
```

### Components

#### 1. Metrics Collector

Collects real-time metrics from all Box instances:

```rust
pub struct BoxMetrics {
    /// Instance identifier
    pub box_id: String,

    /// Active session count
    pub session_count: u32,

    /// Queue metrics per lane
    pub queue_metrics: HashMap<LaneId, LaneMetrics>,

    /// Request latency percentiles (p50, p90, p99)
    pub latency_percentiles: LatencyPercentiles,

    /// Resource utilization
    pub cpu_percent: f32,
    pub memory_bytes: u64,

    /// Instance state
    pub state: BoxState,

    /// Last activity timestamp
    pub last_activity: DateTime<Utc>,
}

pub struct LaneMetrics {
    pub pending: usize,
    pub active: usize,
    pub completed_per_second: f64,
    pub avg_latency_ms: f64,
}
```

#### 2. Scaler Controller

Makes scaling decisions based on metrics:

```rust
pub struct ScalerConfig {
    /// Target concurrency per instance
    pub target_concurrency: u32,

    /// Maximum instances
    pub max_instances: u32,

    /// Minimum instances (0 = scale to zero)
    pub min_instances: u32,

    /// Scale up threshold (queue depth ratio)
    pub scale_up_threshold: f64,

    /// Scale down threshold (queue depth ratio)
    pub scale_down_threshold: f64,

    /// Cooldown period after scale up
    pub scale_up_cooldown: Duration,

    /// Cooldown period after scale down
    pub scale_down_cooldown: Duration,

    /// Grace period before scale to zero
    pub scale_to_zero_grace: Duration,

    /// Enable predictive scaling
    pub predictive_scaling: bool,
}
```

#### 3. Box Pool Manager

Manages warm instance pool for fast scaling:

```rust
pub struct BoxPoolConfig {
    /// Warm pool size (pre-started instances)
    pub warm_pool_size: u32,

    /// Maximum pool size
    pub max_pool_size: u32,

    /// Instance idle timeout
    pub idle_timeout: Duration,

    /// Prewarming schedule (cron expression)
    pub prewarm_schedule: Option<String>,
}

pub enum BoxState {
    /// Cold - not started
    Cold,
    /// Starting - VM booting
    Starting,
    /// Warm - started, no sessions
    Warm,
    /// Hot - active sessions
    Hot,
    /// Draining - graceful shutdown
    Draining,
    /// Stopped - terminated
    Stopped,
}
```

#### 4. Session Router

Routes requests with session affinity:

```rust
pub struct SessionRouter {
    /// Session to Box mapping
    session_map: HashMap<SessionId, BoxId>,

    /// Box to Sessions mapping (reverse index)
    box_sessions: HashMap<BoxId, HashSet<SessionId>>,

    /// Session migration queue
    migration_queue: VecDeque<MigrationTask>,
}

pub struct MigrationTask {
    pub session_id: SessionId,
    pub from_box: BoxId,
    pub to_box: BoxId,
    pub reason: MigrationReason,
}

pub enum MigrationReason {
    /// Source box draining
    Drain,
    /// Load balancing
    Rebalance,
    /// Affinity violation
    Affinity,
}
```

## Scaling Algorithms

### 1. Reactive Scaling

Based on current queue depth and concurrency:

```
desired_instances = ceil(total_pending / target_concurrency)
desired_instances = clamp(desired_instances, min_instances, max_instances)

if current_instances < desired_instances:
    scale_up(desired_instances - current_instances)
elif current_instances > desired_instances:
    scale_down(current_instances - desired_instances)
```

### 2. Predictive Scaling

Uses historical patterns to anticipate demand:

```
┌──────────────────────────────────────────────────────────────────┐
│  Request Rate Over Time                                          │
│                                                                  │
│  ▲                                                               │
│  │                    ╭─────╮                                    │
│  │                   ╱       ╲        Predicted                  │
│  │                  ╱         ╲       ┌─────────╮                │
│  │         ╭──────╮╱           ╲     ╱           ╲               │
│  │        ╱        ╲            ╲───╱             ╲              │
│  │  ─────╱          ╲────────────────              ╲─────        │
│  │                                                               │
│  └──────────────────────────────────────────────────────────────►│
│        Past              Now                Future                │
│                                                                  │
│  Historical Pattern → Predict → Pre-scale                        │
└──────────────────────────────────────────────────────────────────┘
```

Predictive factors:
- **Time-of-day patterns**: Business hours vs. off-hours
- **Day-of-week patterns**: Weekday vs. weekend
- **Seasonal patterns**: Month-end processing, holidays
- **Trend analysis**: Growing or declining usage

### 3. Queue-Aware Scaling

Integrates with lane-based queue for intelligent scaling:

```rust
fn calculate_scale_factor(&self, metrics: &AggregatedMetrics) -> f64 {
    let mut scale_factor = 1.0;

    // Weight by lane priority (higher priority = more weight)
    for (lane_id, lane_metrics) in &metrics.lanes {
        let priority_weight = match lane_id.as_str() {
            "system" => 3.0,   // Critical
            "control" => 2.5,  // Important
            "query" => 1.5,    // Normal
            "session" => 1.2,  // Normal
            "skill" => 1.0,    // Normal
            "prompt" => 2.0,   // Expensive but important
            _ => 1.0,
        };

        let queue_pressure = lane_metrics.pending as f64 / lane_metrics.max as f64;
        scale_factor += queue_pressure * priority_weight;
    }

    scale_factor
}
```

### 4. Scale-to-Zero

Graceful scale down to zero instances:

```
┌─────────────────────────────────────────────────────────────────┐
│                    Scale-to-Zero Timeline                        │
│                                                                 │
│  Active   │ Last    │ Grace   │ Drain   │ Zero                  │
│  Sessions │ Request │ Period  │ Period  │ Instances             │
│           │         │         │         │                       │
│  ────●────│────●────│────●────│────●────│────●────              │
│     t=0       t=30s    t=5min    t=6min    t=7min               │
│                                                                 │
│  • t=0: Last session destroyed                                  │
│  • t=30s: No new requests                                       │
│  • t=5min: Grace period ends, initiate drain                    │
│  • t=6min: State checkpoint saved                               │
│  • t=7min: Instance terminated                                  │
└─────────────────────────────────────────────────────────────────┘
```

Configuration:
```rust
pub struct ScaleToZeroConfig {
    /// Enable scale to zero
    pub enabled: bool,

    /// Grace period after last activity
    pub grace_period: Duration,

    /// Drain timeout for active sessions
    pub drain_timeout: Duration,

    /// Save state before termination
    pub checkpoint_state: bool,
}
```

## Instance Lifecycle

### State Machine

```
                    ┌─────────┐
                    │  Cold   │
                    └────┬────┘
                         │ start()
                         ▼
                    ┌─────────┐
           ┌───────│Starting │
           │       └────┬────┘
           │            │ ready
           │            ▼
           │       ┌─────────┐
           │   ┌───│  Warm   │◄──────────────┐
           │   │   └────┬────┘               │
           │   │        │ first session      │ last session
           │   │        ▼                    │ destroyed
           │   │   ┌─────────┐               │
           │   │   │   Hot   │───────────────┘
           │   │   └────┬────┘
           │   │        │ drain()
           │   │        ▼
           │   │   ┌─────────┐
           │   └──►│Draining │
           │       └────┬────┘
           │            │ drained
           │            ▼
           │       ┌─────────┐
           └──────►│ Stopped │
                   └─────────┘
```

### Warm Pool Management

Pre-started instances for instant availability:

```rust
impl BoxPoolManager {
    /// Ensure warm pool has minimum instances
    async fn maintain_warm_pool(&self) {
        let warm_count = self.count_by_state(BoxState::Warm);
        let deficit = self.config.warm_pool_size.saturating_sub(warm_count);

        for _ in 0..deficit {
            self.start_warm_instance().await;
        }
    }

    /// Get a warm instance or start a new one
    async fn acquire_instance(&self) -> Result<BoxId> {
        // Try warm pool first
        if let Some(box_id) = self.pop_warm_instance().await {
            return Ok(box_id);
        }

        // Start new instance (cold start)
        self.start_instance().await
    }
}
```

### Cold Start Optimization

Techniques to minimize cold start latency:

| Technique | Description | Impact |
|-----------|-------------|--------|
| **Warm Pool** | Pre-started instances | Eliminates cold start |
| **Image Caching** | Pre-pulled OCI images | -2-5s |
| **Snapshot/Restore** | VM memory snapshots | -500ms |
| **Lazy Loading** | Defer non-critical init | -200ms |
| **Connection Pooling** | Pre-established LLM connections | -100ms |

Target cold start times:
- **Warm pool hit**: < 50ms
- **Cached image**: < 500ms
- **Full cold start**: < 2s

## Session Management

### Session Affinity

Sessions are pinned to instances for consistency:

```rust
impl SessionRouter {
    /// Route a request to the appropriate Box
    async fn route(&self, session_id: &SessionId) -> Result<BoxId> {
        // Check existing affinity
        if let Some(box_id) = self.session_map.get(session_id) {
            if self.is_healthy(box_id).await {
                return Ok(box_id.clone());
            }
        }

        // New session - find best Box
        self.select_box_for_new_session().await
    }

    /// Select Box for new session (least connections)
    async fn select_box_for_new_session(&self) -> Result<BoxId> {
        let boxes = self.pool.list_hot_instances().await;

        boxes
            .into_iter()
            .min_by_key(|b| self.box_sessions.get(&b.id).map(|s| s.len()).unwrap_or(0))
            .map(|b| b.id)
            .ok_or(BoxError::NoAvailableInstance)
    }
}
```

### Session Migration

Move sessions between instances during drain or rebalance:

```rust
impl SessionMigrator {
    /// Migrate a session to another Box
    async fn migrate(&self, task: MigrationTask) -> Result<()> {
        // 1. Checkpoint session state
        let state = self.checkpoint_session(&task.session_id, &task.from_box).await?;

        // 2. Restore on target Box
        self.restore_session(&task.session_id, &task.to_box, state).await?;

        // 3. Update routing
        self.router.update_affinity(&task.session_id, &task.to_box).await;

        // 4. Clean up source
        self.cleanup_session(&task.session_id, &task.from_box).await?;

        Ok(())
    }
}

pub struct SessionState {
    /// Session ID
    pub id: SessionId,

    /// Conversation history
    pub messages: Vec<Message>,

    /// Active skill
    pub active_skill: Option<String>,

    /// Custom context
    pub context: serde_json::Value,
}
```

## Kubernetes Integration

### Custom Resource Definitions

```yaml
apiVersion: a3s.io/v1
kind: BoxAutoscaler
metadata:
  name: coding-agent-autoscaler
spec:
  # Target Box deployment
  targetRef:
    apiVersion: a3s.io/v1
    kind: BoxDeployment
    name: coding-agent

  # Scaling bounds
  minReplicas: 0
  maxReplicas: 100

  # Scaling metrics
  metrics:
    # Target concurrency per instance
    - type: Concurrency
      target:
        averageValue: 10

    # Queue depth based scaling
    - type: Queue
      queue:
        name: prompt
        targetPendingPerInstance: 5

    # CPU based scaling
    - type: Resource
      resource:
        name: cpu
        target:
          averageUtilization: 70

  # Scaling behavior
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 0
      policies:
        - type: Percent
          value: 100
          periodSeconds: 15
        - type: Pods
          value: 4
          periodSeconds: 15
      selectPolicy: Max

    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
        - type: Percent
          value: 10
          periodSeconds: 60

  # Scale to zero
  scaleToZero:
    enabled: true
    gracePeriodSeconds: 300

  # Warm pool
  warmPool:
    size: 2
    maxSize: 10
```

### BoxDeployment CRD

```yaml
apiVersion: a3s.io/v1
kind: BoxDeployment
metadata:
  name: coding-agent
spec:
  # Box template
  template:
    spec:
      codingAgent:
        kind: a3s_code
        image: ghcr.io/a3s-lab/code:latest

      llm:
        provider: anthropic
        model: claude-sonnet-4-20250514

      resources:
        requests:
          memory: "2Gi"
          cpu: "1"
        limits:
          memory: "4Gi"
          cpu: "2"

      skills:
        - name: web-search
          path: /skills/web-search

  # Revision history
  revisionHistoryLimit: 5

  # Traffic routing
  traffic:
    - revisionName: coding-agent-v2
      percent: 90
    - revisionName: coding-agent-v1
      percent: 10
```

### Operator Architecture

```
┌─────────────────────────────────────────────────────────────────┐
│                    A3S Box Operator                              │
│                                                                 │
│  ┌─────────────────────────────────────────────────────────┐   │
│  │                   Controller Manager                     │   │
│  │                                                         │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │   │
│  │  │BoxDeployment│  │BoxAutoscaler│  │  BoxRevision    │ │   │
│  │  │ Controller  │  │ Controller  │  │  Controller     │ │   │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘ │   │
│  │                                                         │   │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────┐ │   │
│  │  │  BoxPool    │  │  Session    │  │   Metrics       │ │   │
│  │  │ Controller  │  │ Controller  │  │   Collector     │ │   │
│  │  └─────────────┘  └─────────────┘  └─────────────────┘ │   │
│  └─────────────────────────────────────────────────────────┘   │
│                              │                                  │
│  ┌───────────────────────────┼───────────────────────────────┐ │
│  │              Kubernetes API Server                         │ │
│  └───────────────────────────────────────────────────────────┘ │
└─────────────────────────────────────────────────────────────────┘
```

## Observability

### Metrics

Prometheus metrics for scaling observability:

```
# Instance metrics
a3s_box_instances_total{state="warm|hot|draining"}
a3s_box_instances_target
a3s_box_scale_up_total
a3s_box_scale_down_total

# Session metrics
a3s_box_sessions_total
a3s_box_sessions_per_instance{box_id="..."}
a3s_box_session_migrations_total{reason="drain|rebalance"}

# Queue metrics
a3s_box_queue_depth{lane="system|control|query|session|skill|prompt"}
a3s_box_queue_latency_seconds{lane="...", quantile="0.5|0.9|0.99"}

# Cold start metrics
a3s_box_cold_start_duration_seconds{type="warm_pool|cached|full"}
a3s_box_warm_pool_size
a3s_box_warm_pool_hits_total
a3s_box_warm_pool_misses_total

# Scaling decision metrics
a3s_box_scaling_decision{action="scale_up|scale_down|no_change"}
a3s_box_scaling_cooldown_remaining_seconds
```

### Events

```yaml
# Scale up event
apiVersion: v1
kind: Event
metadata:
  name: coding-agent.scaling.178a2b3c
reason: ScaleUp
message: "Scaled up from 2 to 5 instances due to queue pressure"
type: Normal
involvedObject:
  kind: BoxAutoscaler
  name: coding-agent-autoscaler

# Scale to zero event
apiVersion: v1
kind: Event
metadata:
  name: coding-agent.scaling.289c4d5e
reason: ScaleToZero
message: "Scaled to zero after 5 minutes of inactivity"
type: Normal
```

## Configuration Examples

### Development Environment

```yaml
apiVersion: a3s.io/v1
kind: BoxAutoscaler
metadata:
  name: dev-autoscaler
spec:
  targetRef:
    kind: BoxDeployment
    name: coding-agent
  minReplicas: 1  # Always keep 1 running
  maxReplicas: 3
  metrics:
    - type: Concurrency
      target:
        averageValue: 5
  scaleToZero:
    enabled: false  # Disable for fast iteration
```

### Production Environment

```yaml
apiVersion: a3s.io/v1
kind: BoxAutoscaler
metadata:
  name: prod-autoscaler
spec:
  targetRef:
    kind: BoxDeployment
    name: coding-agent
  minReplicas: 0
  maxReplicas: 100
  metrics:
    - type: Concurrency
      target:
        averageValue: 10
    - type: Queue
      queue:
        name: prompt
        targetPendingPerInstance: 3
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 0
      policies:
        - type: Percent
          value: 100
          periodSeconds: 15
    scaleDown:
      stabilizationWindowSeconds: 300
      policies:
        - type: Percent
          value: 10
          periodSeconds: 60
  scaleToZero:
    enabled: true
    gracePeriodSeconds: 300
  warmPool:
    size: 5
    maxSize: 20
```

### Cost-Optimized Environment

```yaml
apiVersion: a3s.io/v1
kind: BoxAutoscaler
metadata:
  name: cost-optimized-autoscaler
spec:
  targetRef:
    kind: BoxDeployment
    name: coding-agent
  minReplicas: 0
  maxReplicas: 50
  metrics:
    - type: Concurrency
      target:
        averageValue: 20  # Higher density
  behavior:
    scaleUp:
      stabilizationWindowSeconds: 30  # Slower scale up
      policies:
        - type: Pods
          value: 2
          periodSeconds: 30
    scaleDown:
      stabilizationWindowSeconds: 60  # Faster scale down
      policies:
        - type: Percent
          value: 25
          periodSeconds: 30
  scaleToZero:
    enabled: true
    gracePeriodSeconds: 60  # Aggressive scale to zero
  warmPool:
    size: 1  # Minimal warm pool
    maxSize: 5
```

## Comparison with Knative

| Feature | Knative | A3S Box Autoscaler |
|---------|---------|-------------------|
| Scale to Zero | Yes | Yes |
| Request-based Scaling | Yes | Yes (via queue) |
| Concurrency Target | Yes | Yes |
| Warm Pool | No (activator proxy) | Yes (native) |
| Session Affinity | No | Yes (built-in) |
| Queue-aware Scaling | No | Yes (lane integration) |
| Predictive Scaling | No | Yes (optional) |
| Session Migration | No | Yes (stateful) |
| Cold Start | ~1-5s | < 500ms (warm pool) |

### Key Differences

1. **Session Awareness**: A3S Box understands stateful sessions, Knative is stateless
2. **Queue Integration**: Native integration with lane-based queue for smarter scaling
3. **Warm Pool**: Built-in warm instance pool instead of activator proxy
4. **Predictive**: Optional predictive scaling based on historical patterns
5. **Migration**: Can migrate sessions between instances during drain

## Implementation Phases

### Phase 1: Core Autoscaler

- [ ] Metrics collector implementation
- [ ] Basic reactive scaling algorithm
- [ ] Scale up/down with cooldown
- [ ] Integration with CRI runtime

### Phase 2: Warm Pool

- [ ] Box pool manager
- [ ] Warm instance lifecycle
- [ ] Pool size management
- [ ] Cold start optimization

### Phase 3: Session Management

- [ ] Session router with affinity
- [ ] Session state checkpointing
- [ ] Session migration during drain
- [ ] Load balancing improvements

### Phase 4: Scale to Zero

- [ ] Grace period handling
- [ ] State preservation
- [ ] Fast resume from zero
- [ ] Activation queue

### Phase 5: Advanced Features

- [ ] Predictive scaling
- [ ] Queue-aware algorithms
- [ ] Traffic splitting (revisions)
- [ ] A/B testing support

### Phase 6: Kubernetes Operator

- [ ] BoxAutoscaler CRD
- [ ] BoxDeployment CRD
- [ ] Operator controller
- [ ] Helm charts

## Summary

The A3S Box Elastic Scaling system provides:

1. **Intelligent Scaling**: Queue-aware, session-aware autoscaling
2. **Cost Efficiency**: Scale to zero with fast resume
3. **Low Latency**: Warm pool for sub-second cold starts
4. **Stateful Support**: Session affinity and migration
5. **Kubernetes Native**: CRDs and operator pattern
6. **Observability**: Comprehensive metrics and events

This design extends Knative concepts with AI workload-specific features like session awareness, queue integration, and predictive scaling.
