//! Shared Runtime-use directives for agent-driven TUI workflows.

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RuntimePolicy {
    /// OS Runtime evidence is required before the final answer.
    Required,
    /// The workflow intentionally stays local.
    LocalOnly,
}

impl RuntimePolicy {
    pub(crate) fn directive(self) -> &'static str {
        match self {
            RuntimePolicy::Required => {
                "Runtime evidence is required before the final answer: call the signed-in \
                 OS A3S Runtime through `runtime`, use local `parallel_task` only for \
                 host-side local fan-out, or execute an OS progressive operation with \
                 `\"shaped\":true` / `shaped:true` that returns `.view` or `viewUrl`. \
                 If this cannot be done, explicitly explain why and do not claim Runtime \
                 or RemoteUI success."
            }
            RuntimePolicy::LocalOnly => {
                "Local-only runtime policy: stay local; do not call OS Runtime, open \
                 RemoteUI/WebIDE/browser pages, or claim `.view`/`viewUrl` output."
            }
        }
    }
}
