# a3s-effect

`a3s-effect` is the actor runtime for an A3S Code harness. It keeps two
ideas separate, and the rest of the crate follows from that split.

A transition is a program value. It names its success, its expected error,
and the services it needs. Nothing in that description calls a model, a tool,
or a compactor. [`Effect::run`](src/effect.rs) is the edge that executes it.

A thread is an immutable fact log. Components fold the log into a view and
the transitions that view enables. The runtime runs those transitions, appends
the facts they return, and folds again. Resume reads the same log. A
confirmation and a question stay parked until an answer fact arrives. Neither
wait is a process-local timer.

This crate is not a port of another agent framework and it does not embed the
TypeScript Effect runtime. The charts below are the four claims the code
already implements.

## A program is a value until it is run

[`Effect`](src/effect.rs) is a description. Building one, mapping it, or
wrapping it in `retry` does not enter the service. The construct test counts
the service call at zero until `Effect::run`, then one.

Fourteen public methods return another `Effect`: `succeed`, `fail`, `die`,
`from_async`, `map`, `and_then`, `catch_fail`, `retry`, `timeout`,
`with_span`, `zip_par`, `race`, `bracket`, and `provide`. One method executes
the description.

[The program is a value until Effect::run](docs/charts/program-is-a-value.html)

## A defect is not retried

[`Exit`](src/exit.rs) has three stops. `Exit::Fail` carries an [`ActorError`](src/error.rs)
and is the expected failure. `Exit::Die` is a defect. `Exit::Interrupt` is
cancellation from a parent scope.

`retry` repeats `Exit::Fail` only. In the operator test, a failure with one
retry left runs twice. A defect with three retries left runs once. `catch_fail`
handles `Exit::Fail` and lets `Exit::Die` through.

[A defect is not retried](docs/charts/retry-skips-defects.html)

## The log decides the next transition

[`resume`](src/actor.rs) folds the facts already stored. Each component's
`step` consumes one fact. `output` returns the view and the transitions that
state enables. A transition whose key is already a `cause` on some fact is
not selected again, so a second resume of a finished text turn adds nothing
and does not call the model.

If two components enable the same key, projection returns
`ActorError::DuplicateTransition` before either effect body runs.

A text turn stops after `model.turn`. A confirmation thread grows through the
answer and the tool. A question thread grows through `question.answered`.
A duplicate key stops at the ingress fact.

[The log decides the next transition](docs/charts/log-fold.html)

## Both waits end on a fact

The coding scheduler parks in `CodingPhase::Confirm` until a
`confirmation.answered` fact, and in `CodingPhase::Question` until a
`question.answered` fact. The question view keeps `allow_free_text` from the
model decision. While parked, the fold enables no transition, so `resume`
takes zero steps. There is no timeout in that path.

After `confirmation.answered` with `approved: true`, the tool transition
runs once. After `question.answered`, the next model turn runs once. A denial
completes the turn without calling the tool.

[Both waits end on a fact, not a timer](docs/charts/park-until-fact.html)

## What else the same rules cover

These are the same two mechanisms, not extra philosophies.

| Mechanism | Where it lives |
| --- | --- |
| Retries, sibling cancellation, and release on every exit | `Effect::retry`, `Effect::zip_par`, `Effect::bracket` |
| Services in the type, swapped at the edge | the `S` parameter and `Effect::provide` |
| Spans recorded by the runtime | `Effect::with_span` |
| A log line that is not a fact | `parse_fact_json` |
| A harness that cannot run | `HarnessConfig::new` rejects a zero step limit or zero model attempts |

`coding_actor` mounts three components on one log: instructions, the tool
catalog, and the scheduler. The scheduler is the fold that enables a model
turn, a tool call, a compaction, or a budget denial.

```rust
let config = HarnessConfig::new(4, 8_000, 32, 2, vec!["You are a coding harness.".into()], vec![])?;
let actor = coding_actor(config);
```

Run the tests from this directory:

```bash
cargo test
```
