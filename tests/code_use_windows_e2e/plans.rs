use std::path::Path;

use serde_json::{json, Value};

#[derive(Clone, Debug)]
pub(super) struct PlannedToolCall {
    pub(super) name: String,
    pub(super) arguments: Value,
}

impl PlannedToolCall {
    fn new(name: impl Into<String>, arguments: Value) -> Self {
        Self {
            name: name.into(),
            arguments,
        }
    }
}

fn browser(name: &str, arguments: Value) -> PlannedToolCall {
    PlannedToolCall::new(format!("mcp__use_browser__agent_browser_{name}"), arguments)
}

fn office(name: &str, arguments: Value) -> PlannedToolCall {
    PlannedToolCall::new(format!("mcp__use_office__office_{name}"), arguments)
}

pub(super) fn browser_plan(url: &str, screenshot: &Path) -> Vec<PlannedToolCall> {
    let session = "code-windows-e2e";
    let namespace = "code-windows-e2e";
    let scoped = || json!({"session":session,"namespace":namespace});

    vec![
        browser("tools_profiles", scoped()),
        browser("install", scoped()),
        browser(
            "doctor",
            json!({
                "session":session,
                "namespace":namespace,
                "quick":true,
                "offline":true
            }),
        ),
        browser(
            "open",
            json!({"session":session,"namespace":namespace,"url":url}),
        ),
        browser(
            "wait_for_load",
            json!({
                "session":session,
                "namespace":namespace,
                "state":"domcontentloaded",
                "waitTimeoutMs":10_000
            }),
        ),
        browser(
            "snapshot",
            json!({"session":session,"namespace":namespace,"interactive":true}),
        ),
        browser(
            "fill",
            json!({
                "session":session,
                "namespace":namespace,
                "selector":"#name",
                "text":"A3S"
            }),
        ),
        browser(
            "type",
            json!({
                "session":session,
                "namespace":namespace,
                "selector":"#name",
                "text":" Code",
                "clear":false
            }),
        ),
        browser(
            "press",
            json!({"session":session,"namespace":namespace,"key":"End"}),
        ),
        browser(
            "check",
            json!({"session":session,"namespace":namespace,"selector":"#enabled"}),
        ),
        browser(
            "uncheck",
            json!({"session":session,"namespace":namespace,"selector":"#enabled"}),
        ),
        browser(
            "select",
            json!({
                "session":session,
                "namespace":namespace,
                "selector":"#choice",
                "values":["two"]
            }),
        ),
        browser(
            "scroll",
            json!({
                "session":session,
                "namespace":namespace,
                "direction":"down",
                "amount":120
            }),
        ),
        browser(
            "wait_ms",
            json!({"session":session,"namespace":namespace,"ms":20}),
        ),
        browser(
            "wait_for_selector",
            json!({
                "session":session,
                "namespace":namespace,
                "selector":"#status",
                "waitTimeoutMs":10_000
            }),
        ),
        browser(
            "click",
            json!({"session":session,"namespace":namespace,"selector":"#run"}),
        ),
        browser(
            "wait_for_text",
            json!({
                "session":session,
                "namespace":namespace,
                "text":"clicked",
                "waitTimeoutMs":10_000
            }),
        ),
        browser(
            "get_text",
            json!({"session":session,"namespace":namespace,"selector":"#status"}),
        ),
        browser("get_url", scoped()),
        browser("get_title", scoped()),
        browser(
            "eval",
            json!({
                "session":session,
                "namespace":namespace,
                "script":"history.pushState({}, '', '/first'); history.pushState({}, '', '/second'); document.querySelector('#status').textContent = 'evaluated'; 'eval-ok'"
            }),
        ),
        browser(
            "screenshot",
            json!({
                "session":session,
                "namespace":namespace,
                "path":screenshot,
                "fullPage":true
            }),
        ),
        browser(
            "read",
            json!({
                "session":session,
                "namespace":namespace,
                "url":url,
                "readTimeoutMs":10_000
            }),
        ),
        browser("back", scoped()),
        browser("forward", scoped()),
        browser("reload", scoped()),
        browser(
            "tab_new",
            json!({
                "session":session,
                "namespace":namespace,
                "url":"about:blank",
                "label":"second"
            }),
        ),
        browser("tab_list", scoped()),
        browser(
            "tab_switch",
            json!({"session":session,"namespace":namespace,"tab":"second"}),
        ),
        browser(
            "tab_close",
            json!({"session":session,"namespace":namespace,"tab":"second"}),
        ),
        browser("close", scoped()),
    ]
}

pub(super) fn office_plan(
    document: &Path,
    merged: &Path,
    screenshot: &Path,
) -> Vec<PlannedToolCall> {
    let session = "office-e2e";
    let mut calls = vec![
        office("create", json!({"session":session,"file":document})),
        office(
            "apply_batch",
            json!({
                "session":session,
                "mutations":[
                    {
                        "operation":"set-text",
                        "path":"/body/p[1]",
                        "text":"Hello {{name}}"
                    },
                    {
                        "operation":"add-paragraph",
                        "parent":"/body",
                        "text":"Native Office on Windows"
                    }
                ]
            }),
        ),
        office(
            "get",
            json!({"session":session,"path":"/body/p[1]","depth":1}),
        ),
        office(
            "query",
            json!({"session":session,"selector":"p","limit":10}),
        ),
    ];
    for view in [
        "text",
        "annotated",
        "outline",
        "stats",
        "issues",
        "html",
        "svg",
    ] {
        calls.push(office("view", json!({"session":session,"view":view})));
    }
    calls.extend([
        office(
            "view",
            json!({
                "session":session,
                "view":"screenshot",
                "output":screenshot,
                "timeoutMs":120_000
            }),
        ),
        office(
            "raw_xml",
            json!({"session":session,"part":"/word/document.xml"}),
        ),
        office(
            "merge_template",
            json!({
                "session":session,
                "output":merged,
                "data":{"name":"Windows"}
            }),
        ),
        office("save", json!({"session":session})),
        office("validate", json!({"file":document})),
        office("list", json!({})),
        office("close", json!({"session":session})),
        office(
            "open",
            json!({"session":"office-opened","file":merged,"readOnly":true}),
        ),
        office("close", json!({"session":"office-opened"})),
        office("install_compat", json!({})),
    ]);
    calls
}

pub(super) fn ocr_plan(image: &Path) -> Vec<PlannedToolCall> {
    vec![
        PlannedToolCall::new("mcp__use_ocr__ocr_doctor", json!({})),
        PlannedToolCall::new("mcp__use_ocr__ocr_install", json!({})),
        PlannedToolCall::new("mcp__use_ocr__ocr_extract", json!({"path":image})),
    ]
}

pub(super) fn office_compat_plan() -> Vec<PlannedToolCall> {
    vec![PlannedToolCall::new(
        "mcp__use_office-compat__officecli",
        json!({"command":"help"}),
    )]
}
