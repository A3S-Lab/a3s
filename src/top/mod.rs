//! `a3s top` — a ctop-style monitor for boxes, coding agents, and diagnostics.
//!
//! The first milestone intentionally keeps collectors independent from the TUI
//! layer. That lets the same snapshot model feed `--json` and remote
//! dashboards without coupling collection to presentation.

use std::collections::{HashMap, HashSet};
use std::io::SeekFrom;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use a3s_tui::cmd::{self, Cmd};
use a3s_tui::components::{
    divider_line, CellAlign, Confirm, DataColumn, DataRow, DataTable, DetailPanel, HelpPanel,
    HelpSection, LogView, LogViewState, MenuItem, MenuPanel, Meter, MetricTrend, MultiSelect,
    MultiSelectMsg, SectionHeader, Select, SelectMsg, Sparkline, StatusBar, TabSegment, Tabs, Tree,
    TreeNode, WrappedPrefixBlock,
};
use a3s_tui::event::KeyEvent;
use a3s_tui::keymap::{KeyBinding, Keymap};
use a3s_tui::layout::{Constraint, Layout};
use a3s_tui::style::{Color, Style};
use a3s_tui::{Event, KeyCode, KeyModifiers, Model, ProgramBuilder};
use futures::stream::{self, StreamExt};
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio::process::Command;

use crate::cli::args::OutputMode;
use crate::cli::output::{render_value, write_jsonl};

mod collect;
mod view;
pub(crate) use collect::{collect_processes, AgentKind, ProcessRow, Risk};
#[cfg(test)]
use collect::{detect_agent, parse_lsof_cwd, parse_process_line, process_risk};
pub(crate) use view::{render_process_table, ProcessTableView};

const ACCENT: Color = Color::Rgb(122, 162, 247);
const GREEN: Color = Color::Rgb(158, 206, 106);
const YELLOW: Color = Color::Rgb(224, 175, 104);
const RED: Color = Color::Rgb(247, 118, 142);
const CYAN: Color = Color::Rgb(125, 207, 255);
const ORANGE: Color = Color::Rgb(255, 158, 100);
const HISTORY_LIMIT: usize = 30;
const A3S_BOX_INSPECT_LIMIT: usize = 32;
const OBSERVER_EVENT_LIMIT: usize = 200;
const OBSERVER_AUTO_MAX_FILES_PER_AGENT: usize = 8;
const OBSERVER_AUTO_MAX_SCAN_FILES: usize = 512;
const OBSERVER_AUTO_INITIAL_TAIL_BYTES: u64 = 256 * 1024;
const A3S_BOX_PIDS_CONCURRENCY: usize = 4;
const A3S_BOX_PIDS_TIMEOUT: Duration = Duration::from_millis(900);
const AGENTS_TREE_SESSION_LIMIT: usize = 4;
const AGENTS_TREE_PROCESS_LIMIT: usize = 5;
const AGENTS_TREE_EVENT_LIMIT: usize = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Tab {
    Agents,
    Containers,
    Sessions,
    Events,
    Processes,
}

impl Tab {
    const PRIMARY: [Tab; 2] = [Tab::Agents, Tab::Containers];
    const ALL: [Tab; 5] = [
        Tab::Agents,
        Tab::Containers,
        Tab::Sessions,
        Tab::Events,
        Tab::Processes,
    ];

    fn primary_index(self) -> usize {
        Self::PRIMARY.iter().position(|t| *t == self).unwrap_or(0)
    }

    fn label(self) -> &'static str {
        match self {
            Tab::Agents => "Agents",
            Tab::Containers => "Containers",
            Tab::Sessions => "Sessions",
            Tab::Events => "Events",
            Tab::Processes => "Processes",
        }
    }

    fn next(self) -> Self {
        Self::PRIMARY[(self.primary_index() + 1) % Self::PRIMARY.len()]
    }

    fn prev(self) -> Self {
        Self::PRIMARY[(self.primary_index() + Self::PRIMARY.len() - 1) % Self::PRIMARY.len()]
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SortBy {
    Cpu,
    Mem,
    Net,
    Block,
    Pids,
    State,
    Id,
    Uptime,
    Name,
    Tokens,
}

impl SortBy {
    #[cfg(test)]
    fn next(self) -> Self {
        match self {
            SortBy::Cpu => SortBy::Mem,
            SortBy::Mem => SortBy::Net,
            SortBy::Net => SortBy::Block,
            SortBy::Block => SortBy::Pids,
            SortBy::Pids => SortBy::State,
            SortBy::State => SortBy::Id,
            SortBy::Id => SortBy::Uptime,
            SortBy::Uptime => SortBy::Name,
            SortBy::Name => SortBy::Tokens,
            SortBy::Tokens => SortBy::Cpu,
        }
    }

    fn label(self) -> &'static str {
        match self {
            SortBy::Cpu => "cpu",
            SortBy::Mem => "mem",
            SortBy::Net => "net",
            SortBy::Block => "block",
            SortBy::Pids => "pids",
            SortBy::State => "state",
            SortBy::Id => "id",
            SortBy::Uptime => "uptime",
            SortBy::Name => "name",
            SortBy::Tokens => "tokens",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        let normalized = label.trim().to_ascii_lowercase();
        match normalized.as_str() {
            "cpu" => Some(SortBy::Cpu),
            "mem" | "mem%" | "mem %" | "mem-pct" | "mem_pct" | "memory" => Some(SortBy::Mem),
            "net" | "network" => Some(SortBy::Net),
            "block" | "blk" | "disk" | "io" => Some(SortBy::Block),
            "pid" | "pids" | "processes" => Some(SortBy::Pids),
            "state" | "status" => Some(SortBy::State),
            "id" | "container-id" | "container_id" => Some(SortBy::Id),
            "uptime" | "up" | "started" => Some(SortBy::Uptime),
            "name" => Some(SortBy::Name),
            "tok" | "token" | "tokens" | "llm" => Some(SortBy::Tokens),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerConnector {
    A3sBox,
    Docker,
    RunC,
}

impl ContainerConnector {
    fn label(self) -> &'static str {
        match self {
            ContainerConnector::A3sBox => "a3s-box",
            ContainerConnector::Docker => "docker",
            ContainerConnector::RunC => "runc",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "a3s-box" | "a3sbox" | "box" => Some(ContainerConnector::A3sBox),
            "docker" => Some(ContainerConnector::Docker),
            "runc" => Some(ContainerConnector::RunC),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RiskFilter {
    All,
    Medium,
    High,
}

impl RiskFilter {
    fn next(self) -> Self {
        match self {
            RiskFilter::All => RiskFilter::Medium,
            RiskFilter::Medium => RiskFilter::High,
            RiskFilter::High => RiskFilter::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            RiskFilter::All => "all",
            RiskFilter::Medium => "medium",
            RiskFilter::High => "high",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "all" => Some(RiskFilter::All),
            "medium" | "med" => Some(RiskFilter::Medium),
            "high" => Some(RiskFilter::High),
            _ => None,
        }
    }

    fn matches(self, risk: Risk) -> bool {
        match self {
            RiskFilter::All => true,
            RiskFilter::Medium => matches!(risk, Risk::Medium | Risk::High),
            RiskFilter::High => risk == Risk::High,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum KindFilter {
    All,
    Tool,
    Security,
    File,
    Egress,
    Llm,
    Other,
}

impl KindFilter {
    fn next(self) -> Self {
        match self {
            KindFilter::All => KindFilter::Tool,
            KindFilter::Tool => KindFilter::Security,
            KindFilter::Security => KindFilter::File,
            KindFilter::File => KindFilter::Egress,
            KindFilter::Egress => KindFilter::Llm,
            KindFilter::Llm => KindFilter::Other,
            KindFilter::Other => KindFilter::All,
        }
    }

    fn label(self) -> &'static str {
        match self {
            KindFilter::All => "all",
            KindFilter::Tool => "tool",
            KindFilter::Security => "security",
            KindFilter::File => "file",
            KindFilter::Egress => "egress",
            KindFilter::Llm => "llm",
            KindFilter::Other => "other",
        }
    }

    fn from_label(label: &str) -> Option<Self> {
        match label {
            "all" => Some(KindFilter::All),
            "tool" | "tools" | "tool-exec" | "toolexec" => Some(KindFilter::Tool),
            "security" | "security-action" | "securityaction" => Some(KindFilter::Security),
            "file" | "files" => Some(KindFilter::File),
            "egress" | "network" => Some(KindFilter::Egress),
            "llm" | "model" | "tokens" => Some(KindFilter::Llm),
            "other" => Some(KindFilter::Other),
            _ => None,
        }
    }

    fn matches(self, kind: &str) -> bool {
        match self {
            KindFilter::All => true,
            KindFilter::Tool => kind == "ToolExec",
            KindFilter::Security => kind == "SecurityAction",
            KindFilter::File => matches!(kind, "FileAccess" | "FileDelete"),
            KindFilter::Egress => kind == "Egress",
            KindFilter::Llm => is_llm_event_kind(kind),
            KindFilter::Other => {
                !matches!(
                    kind,
                    "ToolExec" | "SecurityAction" | "FileAccess" | "FileDelete" | "Egress"
                ) && !is_llm_event_kind(kind)
            }
        }
    }
}

#[derive(Debug, Clone)]
struct ContainerRow {
    connector: ContainerConnector,
    id: String,
    name: String,
    image: String,
    status: String,
    inspect: ContainerInspect,
    cpu_pct: Option<f32>,
    cpu_count: Option<u32>,
    cpu_usage_total_ns: Option<u64>,
    mem_pct: Option<f32>,
    mem_usage: String,
    net_io: String,
    block_io: String,
    pids: String,
    ports: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ContainerInspect {
    health: String,
    restarts: String,
    restart_policy: String,
    created: String,
    started: String,
    exit: String,
    mounts: String,
    env: String,
    labels: String,
    networks: String,
}

impl Default for ContainerInspect {
    fn default() -> Self {
        Self {
            health: "-".into(),
            restarts: "-".into(),
            restart_policy: "-".into(),
            created: "-".into(),
            started: "-".into(),
            exit: "-".into(),
            mounts: "-".into(),
            env: "-".into(),
            labels: "-".into(),
            networks: "-".into(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ContainerStateSummary {
    total: usize,
    running: usize,
    restarting: usize,
    paused: usize,
    exited: usize,
    created: usize,
    dead: usize,
    other: usize,
}

#[derive(Debug, Clone, Default)]
struct RuncStats {
    cpu_usage_total_ns: Option<u64>,
    memory_usage: Option<u64>,
    memory_limit: Option<u64>,
    pids_current: Option<u64>,
    net_rx: u64,
    net_tx: u64,
    block_read: u64,
    block_write: u64,
}

#[derive(Debug, Clone, Default)]
struct MetricHistory {
    cpu: Vec<f32>,
    mem: Vec<f32>,
    net_io_bytes: Vec<f64>,
    block_io_bytes: Vec<f64>,
    cpu_usage_total_ns: Option<u64>,
    raw_sample_at: Option<Instant>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct ProcessTreeUsage {
    cpu_pct: f32,
    mem_pct: f32,
    descendants: usize,
}

#[derive(Debug, Clone)]
struct EventRow {
    ts: String,
    source: String,
    session: Option<String>,
    task: Option<String>,
    pid: Option<u32>,
    ppid: Option<u32>,
    kind: String,
    message: String,
    details: Vec<(String, String)>,
    risk: Risk,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct AgentActivity {
    events: usize,
    sessions: usize,
    tools: usize,
    security: usize,
    files: usize,
    egress: usize,
    llm: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    model: String,
    provider: String,
    latency_ms: u64,
    latency_samples: u64,
    ttft_ms: u64,
    ttft_samples: u64,
    req_bytes: u64,
    resp_bytes: u64,
    high_risk: usize,
}

#[derive(Debug, Clone)]
struct AgentTreeGroup {
    agent: AgentKind,
    sessions: Vec<SessionRow>,
    processes: Vec<ProcessRow>,
    events: Vec<EventRow>,
    activity: AgentActivity,
    usage: ProcessTreeUsage,
}

impl AgentTreeGroup {
    fn active(&self) -> bool {
        !self.sessions.is_empty() || !self.processes.is_empty() || !self.events.is_empty()
    }

    fn risk(&self) -> Risk {
        self.processes
            .iter()
            .map(|process| process.risk)
            .chain(self.events.iter().map(|event| event.risk))
            .chain(self.sessions.iter().map(|session| session.risk))
            .max_by_key(|risk| risk_rank(*risk))
            .unwrap_or(Risk::Low)
    }
}

#[derive(Debug, Clone)]
struct SessionRow {
    source: String,
    session: String,
    task: String,
    workspace: String,
    events: usize,
    tools: usize,
    security: usize,
    files: usize,
    egress: usize,
    llm: usize,
    prompt_tokens: u64,
    completion_tokens: u64,
    total_tokens: u64,
    model: String,
    provider: String,
    latency_ms: u64,
    latency_samples: u64,
    ttft_ms: u64,
    ttft_samples: u64,
    req_bytes: u64,
    resp_bytes: u64,
    high_risk: usize,
    risk: Risk,
    last_kind: String,
    last_message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SessionFocus {
    source: String,
    session: String,
}

impl SessionFocus {
    fn from_row(row: &SessionRow) -> Self {
        Self {
            source: row.source.clone(),
            session: row.session.clone(),
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct TokenUsageSummary {
    prompt: u64,
    completion: u64,
    total: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct LlmNetworkSummary {
    provider: Option<String>,
    latency_ms: Option<u64>,
    ttft_ms: Option<u64>,
    req_bytes: u64,
    resp_bytes: u64,
}

#[derive(Debug, Clone, Default)]
struct TopSnapshot {
    processes: Vec<ProcessRow>,
    containers: Vec<ContainerRow>,
    events: Vec<EventRow>,
    errors: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct ObserverState {
    paths: Vec<PathBuf>,
    auto_paths: HashSet<PathBuf>,
    files: HashMap<PathBuf, ObserverFileState>,
    events: Vec<EventRow>,
}

#[derive(Debug, Clone, Default)]
struct ObserverFileState {
    offset: u64,
    pending: String,
    json_events_seen: usize,
}

#[derive(Debug, Clone)]
enum Action {
    KillProcess(u32, String),
    StartContainer(ContainerConnector, String, String),
    StopContainer(ContainerConnector, String, String),
    RestartContainer(ContainerConnector, String, String),
    PauseContainer(ContainerConnector, String, String),
    UnpauseContainer(ContainerConnector, String, String),
    RemoveContainer(ContainerConnector, String, String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ContainerMenuAction {
    Focus,
    Logs,
    ExecShell,
    OpenBrowser,
    Start,
    Stop,
    Restart,
    Pause,
    Unpause,
    Remove,
}

#[derive(Debug, Clone)]
struct ContainerMenuItem {
    action: ContainerMenuAction,
    key: char,
    label: String,
}

#[derive(Debug, Clone)]
enum ExternalAction {
    ContainerShell {
        connector: ContainerConnector,
        id: String,
        name: String,
    },
    OpenBrowser {
        url: String,
        name: String,
    },
}

#[derive(Debug, Clone)]
struct TopConfig {
    show_all_containers: bool,
    show_header: bool,
    reverse_sort: bool,
    sort_by: SortBy,
    risk_filter: RiskFilter,
    kind_filter: KindFilter,
    connector: ContainerConnector,
    filter: String,
    hidden_columns: HashSet<String>,
}

impl Default for TopConfig {
    fn default() -> Self {
        Self {
            show_all_containers: false,
            show_header: true,
            reverse_sort: false,
            sort_by: SortBy::Cpu,
            risk_filter: RiskFilter::All,
            kind_filter: KindFilter::All,
            connector: ContainerConnector::A3sBox,
            filter: String::new(),
            hidden_columns: default_hidden_columns(),
        }
    }
}

fn default_hidden_columns() -> HashSet<String> {
    [
        "agents.pid",
        "agents.cpu_history",
        "agents.mem_history",
        "agents.sessions",
        "agents.session",
        "agents.task",
        "agents.tools",
        "agents.security",
        "agents.files",
        "agents.egress",
        "agents.llm",
        "agents.model",
        "agents.provider",
        "agents.latency",
        "agents.high_risk",
        "agents.children",
        "agents.elapsed",
        "agents.cwd",
        "sessions.task",
        "sessions.tools",
        "sessions.security",
        "sessions.files",
        "sessions.egress",
        "sessions.model",
        "sessions.provider",
        "sessions.latency",
        "sessions.high_risk",
        "processes.ppid",
        "processes.cpu_history",
        "processes.mem_history",
        "processes.elapsed",
        "processes.cwd",
        "containers.cpus",
        "containers.image",
        "containers.mem_usage",
        "containers.ports",
        "containers.health",
        "events.session",
        "events.task",
        "events.pid",
        "events.ppid",
    ]
    .into_iter()
    .map(String::from)
    .collect()
}

#[derive(Debug, Clone)]
struct LogPanel {
    connector: ContainerConnector,
    container_id: String,
    container_name: String,
    text: String,
    scroll: usize,
    timestamps: bool,
    loading: bool,
    refreshing: bool,
    follow: bool,
}

#[derive(Debug, Clone, Default)]
struct ContainerProcessRow {
    pid: String,
    ppid: String,
    cpu_pct: Option<f32>,
    mem_pct: Option<f32>,
    elapsed: String,
    command: String,
}

#[derive(Debug, Clone)]
struct ContainerProcessPanel {
    container_id: String,
    container_name: String,
    rows: Vec<ContainerProcessRow>,
    scroll: usize,
    error: Option<String>,
    loading: bool,
}

#[derive(Debug, Clone)]
struct ColumnChoice {
    id: &'static str,
    label: &'static str,
}

struct ColumnPanel {
    tab: Tab,
    choices: Vec<ColumnChoice>,
    select: MultiSelect,
}

struct SortPanel {
    choices: Vec<SortBy>,
    select: Select,
}

struct ConnectorPanel {
    choices: Vec<ContainerConnector>,
    select: Select,
}

struct ContainerMenu {
    container: ContainerRow,
    items: Vec<ContainerMenuItem>,
    select: Select,
}

#[derive(Debug, Clone)]
enum Msg {
    Term(Event),
    Snapshot {
        connector: ContainerConnector,
        snapshot: TopSnapshot,
        observer: ObserverState,
    },
    Tick,
    ActionDone(String),
    ContainerLogs {
        connector: ContainerConnector,
        id: String,
        name: String,
        timestamps: bool,
        result: Result<String, String>,
    },
    ContainerProcesses {
        id: String,
        name: String,
        result: Result<Vec<ContainerProcessRow>, String>,
    },
    ConfigSaved(Result<PathBuf, String>),
}

impl From<Event> for Msg {
    fn from(event: Event) -> Self {
        Msg::Term(event)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TopKey {
    Quit,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    NextTab,
    PrevTab,
    Filter,
    Sort,
    TogglePause,
    ToggleAll,
    ToggleHeader,
    ToggleReverse,
    ToggleRiskFilter,
    ToggleKindFilter,
    Detail,
    Open,
    Logs,
    OpenBrowser,
    Columns,
    Connector,
    ExecShell,
    SaveConfig,
    Help,
    Kill,
}

struct TopApp {
    snapshot: TopSnapshot,
    observer: ObserverState,
    runtime_events: Vec<EventRow>,
    history: HashMap<String, MetricHistory>,
    tab: Tab,
    sort_by: SortBy,
    selected: usize,
    scroll: usize,
    filter: String,
    editing_filter: bool,
    filter_before_edit: Option<String>,
    help: bool,
    detail: bool,
    paused: bool,
    show_all_containers: bool,
    show_header: bool,
    reverse_sort: bool,
    risk_filter: RiskFilter,
    kind_filter: KindFilter,
    invert_colors: bool,
    connector: ContainerConnector,
    hidden_columns: HashSet<String>,
    focused_container: Option<String>,
    focused_agent_pid: Option<u32>,
    focused_session: Option<SessionFocus>,
    confirm: Option<Action>,
    log: Option<LogPanel>,
    container_processes: Option<ContainerProcessPanel>,
    column_panel: Option<ColumnPanel>,
    sort_panel: Option<SortPanel>,
    connector_panel: Option<ConnectorPanel>,
    container_menu: Option<ContainerMenu>,
    external_action: Arc<Mutex<Option<ExternalAction>>>,
    note: Option<String>,
    interval: Duration,
    width: u16,
    height: u16,
    last_refresh: Option<Instant>,
    keymap: Keymap<TopKey>,
}

impl TopApp {
    fn new(options: TopOptions) -> Self {
        let (width, height) = a3s_tui::terminal::Terminal::size().unwrap_or((100, 30));
        let mut app = Self {
            snapshot: TopSnapshot::default(),
            observer: ObserverState::default(),
            runtime_events: Vec::new(),
            history: HashMap::new(),
            tab: options.tab,
            sort_by: options.config.sort_by,
            selected: 0,
            scroll: 0,
            filter: options.config.filter,
            editing_filter: false,
            filter_before_edit: None,
            help: options.start_help,
            detail: false,
            paused: false,
            show_all_containers: options.config.show_all_containers,
            show_header: options.config.show_header,
            reverse_sort: options.config.reverse_sort,
            risk_filter: options.config.risk_filter,
            kind_filter: options.config.kind_filter,
            invert_colors: options.invert_colors,
            connector: options.config.connector,
            hidden_columns: options.config.hidden_columns,
            focused_container: options.container_query.clone(),
            focused_agent_pid: None,
            focused_session: None,
            confirm: None,
            log: None,
            container_processes: None,
            column_panel: None,
            sort_panel: None,
            connector_panel: None,
            container_menu: None,
            external_action: options.external_action,
            note: None,
            interval: options.interval,
            width,
            height,
            last_refresh: None,
            keymap: top_keymap(),
        };
        app.ensure_visible_columns();
        app
    }

    fn apply_snapshot(
        &mut self,
        mut snapshot: TopSnapshot,
        observer: ObserverState,
        connector: ContainerConnector,
    ) {
        if self.last_refresh.is_some() {
            push_runtime_events(
                &mut self.runtime_events,
                container_lifecycle_events(
                    connector,
                    &self.snapshot.containers,
                    &snapshot.containers,
                ),
            );
        }
        prepend_runtime_events(&mut snapshot.events, &self.runtime_events);
        self.record_history(&mut snapshot);
        self.snapshot = snapshot;
        self.observer = observer;
        self.last_refresh = Some(Instant::now());
        self.clamp_selection();
    }

    fn reset_position(&mut self) {
        self.selected = 0;
        self.scroll = 0;
        self.detail = false;
    }

    fn visible_len(&self) -> usize {
        match self.tab {
            Tab::Agents if self.focused_agent_pid.is_some() => self.filtered_agents().len(),
            Tab::Agents => self.agent_tree_groups().len(),
            Tab::Sessions if self.focused_session.is_some() => self.focused_session_events().len(),
            Tab::Sessions => self.filtered_sessions().len(),
            Tab::Containers => self.filtered_containers().len(),
            Tab::Processes => self.filtered_processes().len(),
            Tab::Events => self.filtered_events().len(),
        }
    }

    fn visible_height(&self) -> usize {
        let reserved = if self.detail { 12 } else { 5 };
        (self.height as usize).saturating_sub(reserved).max(3)
    }

    fn log_visible_height(&self) -> usize {
        (self.height as usize).saturating_sub(7).max(3)
    }

    fn record_history(&mut self, snapshot: &mut TopSnapshot) {
        let now = Instant::now();
        let mut live_keys = HashSet::new();
        for process in &snapshot.processes {
            let key = process_history_key(process.pid);
            live_keys.insert(key.clone());
            push_history(
                self.history.entry(key).or_default(),
                process.cpu_pct,
                process.mem_pct,
            );
        }
        let agent_tree_usages = snapshot
            .processes
            .iter()
            .filter(|process| process.agent.is_some())
            .map(|process| {
                (
                    agent_tree_history_key(process.pid),
                    process_tree_usage(&snapshot.processes, process.pid),
                )
            })
            .collect::<Vec<_>>();
        for (key, usage) in agent_tree_usages {
            live_keys.insert(key.clone());
            push_history(
                self.history.entry(key).or_default(),
                usage.cpu_pct,
                usage.mem_pct,
            );
        }
        for container in &mut snapshot.containers {
            let key = container_history_key(&container.id);
            live_keys.insert(key.clone());
            let history = self.history.entry(key).or_default();
            let cpu_pct = container.cpu_pct.unwrap_or_else(|| {
                observe_raw_cpu_pct(history, container.cpu_usage_total_ns, now).unwrap_or_default()
            });
            if container.cpu_pct.is_none() && container.cpu_usage_total_ns.is_some() {
                container.cpu_pct = Some(cpu_pct);
            }
            push_history(history, cpu_pct, container.mem_pct.unwrap_or_default());
            push_io_history(
                history,
                container_net_total(container),
                container_block_total(container),
            );
        }
        self.history.retain(|key, _| live_keys.contains(key));
    }

    fn metric_history(&self, key: &str) -> MetricHistory {
        self.history.get(key).cloned().unwrap_or_default()
    }

    fn sparkline(&self, values: &[f32], color: Color) -> String {
        Sparkline::new(values.iter().copied().map(f64::from))
            .width(8)
            .range(0.0, 100.0)
            .fg(color)
            .view()
    }

    fn metric_cell(&self, pct: Option<f32>, values: &[f32], color: Color) -> String {
        MetricTrend::new(pct.map(f64::from), values.iter().copied().map(f64::from))
            .width(15)
            .trend_width(8)
            .range(0.0, 100.0)
            .fg(color)
            .view()
    }

    fn scaled_cpu_metric_cell(
        &self,
        pct: Option<f32>,
        values: &[f32],
        cpus: Option<u32>,
    ) -> String {
        let history = values
            .iter()
            .copied()
            .map(|value| scale_cpu_pct_for_cpus(value, cpus))
            .collect::<Vec<_>>();
        self.metric_cell(
            pct.map(|value| scale_cpu_pct_for_cpus(value, cpus)),
            &history,
            CYAN,
        )
    }

    fn clamp_selection(&mut self) {
        let len = self.visible_len();
        if len == 0 {
            self.selected = 0;
            self.scroll = 0;
            return;
        }
        self.selected = self.selected.min(len - 1);
        let body = self.visible_height();
        if self.selected < self.scroll {
            self.scroll = self.selected;
        } else if self.selected >= self.scroll + body {
            self.scroll = self.selected + 1 - body;
        }
    }

    fn selected_action(&self) -> Option<Action> {
        match self.tab {
            Tab::Agents | Tab::Processes => self
                .current_process()
                .map(|p| Action::KillProcess(p.pid, display_cmd(&p.command))),
            Tab::Containers => self.current_container().map(|c| {
                Action::StopContainer(
                    c.connector,
                    c.id.clone(),
                    format!("{} ({})", c.name, short_id(&c.id)),
                )
            }),
            Tab::Events => None,
            Tab::Sessions => None,
        }
    }

    fn current_process(&self) -> Option<ProcessRow> {
        match self.tab {
            Tab::Agents if self.focused_agent_pid.is_some() => {
                self.filtered_agents().first().cloned()
            }
            Tab::Agents => self
                .current_agent_group()
                .and_then(|group| group.processes.first().cloned()),
            Tab::Processes => self.filtered_processes().get(self.selected).cloned(),
            _ => None,
        }
    }

    fn current_agent_group(&self) -> Option<AgentTreeGroup> {
        if self.tab != Tab::Agents || self.focused_agent_pid.is_some() {
            return None;
        }
        self.agent_tree_groups().get(self.selected).cloned()
    }

    fn current_session(&self) -> Option<SessionRow> {
        (self.tab == Tab::Sessions)
            .then(|| self.filtered_sessions().get(self.selected).cloned())
            .flatten()
    }

    fn current_event(&self) -> Option<EventRow> {
        match self.tab {
            Tab::Events => self.filtered_events().get(self.selected).cloned(),
            Tab::Sessions if self.focused_session.is_some() => {
                self.focused_session_events().get(self.selected).cloned()
            }
            _ => None,
        }
    }

    fn current_container(&self) -> Option<ContainerRow> {
        self.filtered_containers().get(self.selected).cloned()
    }

    fn descendant_count(&self, pid: u32) -> usize {
        self.process_tree_usage(pid).descendants
    }

    fn process_tree_usage(&self, pid: u32) -> ProcessTreeUsage {
        process_tree_usage(&self.snapshot.processes, pid)
    }

    fn process_tree(&self, pid: u32) -> Option<TreeNode> {
        let mut visited = HashSet::new();
        self.process_tree_node(pid, &mut visited)
    }

    fn process_tree_node(&self, pid: u32, visited: &mut HashSet<u32>) -> Option<TreeNode> {
        if !visited.insert(pid) {
            return None;
        }
        let process = self.snapshot.processes.iter().find(|p| p.pid == pid)?;
        let mut children = self
            .snapshot
            .processes
            .iter()
            .filter(|p| p.ppid == pid)
            .collect::<Vec<_>>();
        children.sort_by_key(|p| p.pid);
        let children = children
            .into_iter()
            .filter_map(|child| self.process_tree_node(child.pid, visited))
            .collect::<Vec<_>>();
        let label = format!(
            "{}  cpu {:.1}% mem {:.1}%  {}",
            process.pid,
            process.cpu_pct,
            process.mem_pct,
            display_cmd(&process.command)
        );
        if children.is_empty() {
            Some(TreeNode::leaf(label))
        } else {
            Some(TreeNode::branch(label, children))
        }
    }

    fn recent_agent_events_for_process(&self, process: &ProcessRow) -> Vec<EventRow> {
        if process.agent.is_none() {
            return Vec::new();
        }
        self.snapshot
            .events
            .iter()
            .filter(|event| self.event_matches_agent_process(process, event))
            .take(3)
            .cloned()
            .collect()
    }

    #[cfg(test)]
    fn recent_agent_events(&self, agent: AgentKind) -> Vec<EventRow> {
        self.snapshot
            .events
            .iter()
            .filter(|event| agent_matches_source(agent, &event.source))
            .take(3)
            .cloned()
            .collect()
    }

    #[cfg(test)]
    fn agent_activity(&self, agent: AgentKind) -> AgentActivity {
        self.agent_activity_from_events(
            self.snapshot
                .events
                .iter()
                .filter(|event| agent_matches_source(agent, &event.source)),
        )
    }

    fn agent_activity_for_process(&self, process: &ProcessRow) -> AgentActivity {
        if process.agent.is_none() {
            return AgentActivity::default();
        }
        self.agent_activity_from_events(
            self.snapshot
                .events
                .iter()
                .filter(|event| self.event_matches_agent_process(process, event)),
        )
    }

    fn agent_activity_from_events<'a>(
        &self,
        events: impl IntoIterator<Item = &'a EventRow>,
    ) -> AgentActivity {
        let mut sessions = HashSet::new();
        let mut activity = AgentActivity::default();
        for event in events {
            activity.events += 1;
            if event.kind == "ToolExec" {
                activity.tools += 1;
            }
            if event.kind == "SecurityAction" {
                activity.security += 1;
            }
            if matches!(event.kind.as_str(), "FileAccess" | "FileDelete") {
                activity.files += 1;
            }
            if event.kind == "Egress" {
                activity.egress += 1;
            }
            if is_llm_event_kind(&event.kind) {
                activity.llm += 1;
            }
            if let Some(tokens) = event_token_usage(event) {
                activity.prompt_tokens += tokens.prompt;
                activity.completion_tokens += tokens.completion;
                activity.total_tokens += tokens.total;
            }
            if activity.model.is_empty() {
                if let Some(model) = event_model(event) {
                    activity.model = model;
                }
            }
            if let Some(network) = event_llm_network(event) {
                if activity.provider.is_empty() {
                    if let Some(provider) = network.provider {
                        activity.provider = provider;
                    }
                }
                if let Some(latency_ms) = network.latency_ms {
                    activity.latency_ms += latency_ms;
                    activity.latency_samples += 1;
                }
                if let Some(ttft_ms) = network.ttft_ms {
                    activity.ttft_ms += ttft_ms;
                    activity.ttft_samples += 1;
                }
                activity.req_bytes += network.req_bytes;
                activity.resp_bytes += network.resp_bytes;
            }
            if event.risk == Risk::High {
                activity.high_risk += 1;
            }
            if let Some(session) = event.session.as_ref().or(event.task.as_ref()) {
                sessions.insert(session.clone());
            }
        }
        activity.sessions = sessions.len();
        activity
    }

    fn event_matches_agent_process(&self, process: &ProcessRow, event: &EventRow) -> bool {
        let Some(agent) = process.agent else {
            return false;
        };
        if !agent_matches_source(agent, &event.source) {
            return false;
        }
        if let Some(pid) = event.pid {
            if self.pid_belongs_to_process(process.pid, pid) {
                return true;
            }
        }
        if let Some(ppid) = event.ppid {
            if self.pid_belongs_to_process(process.pid, ppid) {
                return true;
            }
        }
        if event.pid.is_some() || event.ppid.is_some() {
            return false;
        }
        self.event_workspace_matches_process(process, event)
            || self.agent_source_is_unambiguous(agent)
    }

    fn event_workspace_matches_process(&self, process: &ProcessRow, event: &EventRow) -> bool {
        let Some(agent) = process.agent else {
            return false;
        };
        let Some(workspace) = event_workspace(event) else {
            return false;
        };
        if !process
            .cwd
            .as_deref()
            .is_some_and(|cwd| workspace_paths_overlap(cwd, &workspace))
        {
            return false;
        }

        self.snapshot
            .processes
            .iter()
            .filter(|candidate| candidate.agent == Some(agent))
            .filter(|candidate| {
                candidate
                    .cwd
                    .as_deref()
                    .is_some_and(|cwd| workspace_paths_overlap(cwd, &workspace))
            })
            .take(2)
            .count()
            == 1
    }

    fn pid_belongs_to_process(&self, root_pid: u32, pid: u32) -> bool {
        if pid == root_pid {
            return true;
        }
        let mut current = pid;
        let mut visited = HashSet::new();
        while visited.insert(current) {
            let Some(process) = self.snapshot.processes.iter().find(|p| p.pid == current) else {
                return false;
            };
            if process.ppid == root_pid {
                return true;
            }
            if process.ppid == 0 || process.ppid == current {
                return false;
            }
            current = process.ppid;
        }
        false
    }

    fn agent_source_is_unambiguous(&self, agent: AgentKind) -> bool {
        self.snapshot
            .processes
            .iter()
            .filter(|process| process.agent == Some(agent))
            .count()
            == 1
    }

    fn process_matches_risk_filter(&self, process: &ProcessRow) -> bool {
        if self.risk_filter.matches(process.risk) {
            return true;
        }
        self.risk_filter == RiskFilter::High
            && process.agent.is_some()
            && self.agent_activity_for_process(process).high_risk > 0
    }

    fn process_token_total(&self, process: &ProcessRow) -> u64 {
        self.agent_activity_for_process(process).total_tokens
    }

    fn process_cpu_total(&self, process: &ProcessRow) -> f32 {
        if process.agent.is_some() {
            self.process_tree_usage(process.pid).cpu_pct
        } else {
            process.cpu_pct
        }
    }

    fn process_mem_total(&self, process: &ProcessRow) -> f32 {
        if process.agent.is_some() {
            self.process_tree_usage(process.pid).mem_pct
        } else {
            process.mem_pct
        }
    }

    fn sort_process_rows(&self, rows: &mut [ProcessRow]) {
        match self.sort_by {
            SortBy::Cpu => rows.sort_by(|a, b| {
                b.agent
                    .is_some()
                    .cmp(&a.agent.is_some())
                    .then(
                        self.process_cpu_total(b)
                            .partial_cmp(&self.process_cpu_total(a))
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
                    .then(a.command.cmp(&b.command))
            }),
            SortBy::Mem => rows.sort_by(|a, b| {
                b.agent
                    .is_some()
                    .cmp(&a.agent.is_some())
                    .then(
                        self.process_mem_total(b)
                            .partial_cmp(&self.process_mem_total(a))
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
                    .then(a.command.cmp(&b.command))
            }),
            SortBy::Net => rows.sort_by(|a, b| {
                self.agent_activity_for_process(b)
                    .egress
                    .cmp(&self.agent_activity_for_process(a).egress)
                    .then(b.agent.is_some().cmp(&a.agent.is_some()))
                    .then(a.command.cmp(&b.command))
            }),
            SortBy::Pids => rows.sort_by(|a, b| {
                self.process_tree_usage(b.pid)
                    .descendants
                    .cmp(&self.process_tree_usage(a.pid).descendants)
                    .then(b.agent.is_some().cmp(&a.agent.is_some()))
                    .then(a.command.cmp(&b.command))
            }),
            SortBy::Id => rows.sort_by(|a, b| a.pid.cmp(&b.pid).then(a.command.cmp(&b.command))),
            SortBy::Block | SortBy::State | SortBy::Uptime | SortBy::Name => {
                rows.sort_by(|a, b| a.command.cmp(&b.command))
            }
            SortBy::Tokens => rows.sort_by(|a, b| {
                self.process_token_total(b)
                    .cmp(&self.process_token_total(a))
                    .then(b.agent.is_some().cmp(&a.agent.is_some()))
                    .then(
                        self.process_cpu_total(b)
                            .partial_cmp(&self.process_cpu_total(a))
                            .unwrap_or(std::cmp::Ordering::Equal),
                    )
                    .then(a.command.cmp(&b.command))
            }),
        }
    }

    fn column_visible(&self, id: &str) -> bool {
        !self.hidden_columns.contains(id)
    }

    fn configured_column(&self, id: &str, column: DataColumn) -> DataColumn {
        if self.column_visible(id) {
            column
        } else {
            column.hidden()
        }
    }

    fn configured_container_column(&self, id: &str, column: DataColumn) -> DataColumn {
        let column = if container_sort_column_id(self.sort_by) == Some(id) {
            column.header_suffix(if self.reverse_sort { "↑" } else { "↓" })
        } else {
            column
        };
        self.configured_column(id, column)
    }

    fn column_choices(&self, tab: Tab) -> Vec<ColumnChoice> {
        match tab {
            Tab::Agents => vec![
                ColumnChoice {
                    id: "agents.pid",
                    label: "PID",
                },
                ColumnChoice {
                    id: "agents.agent",
                    label: "Agent",
                },
                ColumnChoice {
                    id: "agents.cpu",
                    label: "CPU %",
                },
                ColumnChoice {
                    id: "agents.cpu_history",
                    label: "CPU trend",
                },
                ColumnChoice {
                    id: "agents.mem",
                    label: "Memory %",
                },
                ColumnChoice {
                    id: "agents.mem_history",
                    label: "Memory trend",
                },
                ColumnChoice {
                    id: "agents.risk",
                    label: "Risk",
                },
                ColumnChoice {
                    id: "agents.events",
                    label: "Events",
                },
                ColumnChoice {
                    id: "agents.sessions",
                    label: "Sessions",
                },
                ColumnChoice {
                    id: "agents.session",
                    label: "Top session",
                },
                ColumnChoice {
                    id: "agents.task",
                    label: "Top task",
                },
                ColumnChoice {
                    id: "agents.tools",
                    label: "Tools",
                },
                ColumnChoice {
                    id: "agents.security",
                    label: "Security",
                },
                ColumnChoice {
                    id: "agents.files",
                    label: "Files",
                },
                ColumnChoice {
                    id: "agents.egress",
                    label: "Egress",
                },
                ColumnChoice {
                    id: "agents.llm",
                    label: "LLM",
                },
                ColumnChoice {
                    id: "agents.tokens",
                    label: "Tokens",
                },
                ColumnChoice {
                    id: "agents.model",
                    label: "Model",
                },
                ColumnChoice {
                    id: "agents.provider",
                    label: "Provider",
                },
                ColumnChoice {
                    id: "agents.latency",
                    label: "Latency",
                },
                ColumnChoice {
                    id: "agents.high_risk",
                    label: "High-risk",
                },
                ColumnChoice {
                    id: "agents.children",
                    label: "Children",
                },
                ColumnChoice {
                    id: "agents.elapsed",
                    label: "Elapsed",
                },
                ColumnChoice {
                    id: "agents.cwd",
                    label: "Working directory",
                },
                ColumnChoice {
                    id: "agents.command",
                    label: "Command",
                },
            ],
            Tab::Sessions => vec![
                ColumnChoice {
                    id: "sessions.source",
                    label: "Agent",
                },
                ColumnChoice {
                    id: "sessions.session",
                    label: "Session",
                },
                ColumnChoice {
                    id: "sessions.task",
                    label: "Task",
                },
                ColumnChoice {
                    id: "sessions.workspace",
                    label: "Workspace",
                },
                ColumnChoice {
                    id: "sessions.events",
                    label: "Events",
                },
                ColumnChoice {
                    id: "sessions.tools",
                    label: "Tools",
                },
                ColumnChoice {
                    id: "sessions.security",
                    label: "Security",
                },
                ColumnChoice {
                    id: "sessions.files",
                    label: "Files",
                },
                ColumnChoice {
                    id: "sessions.egress",
                    label: "Egress",
                },
                ColumnChoice {
                    id: "sessions.llm",
                    label: "LLM",
                },
                ColumnChoice {
                    id: "sessions.tokens",
                    label: "Tokens",
                },
                ColumnChoice {
                    id: "sessions.model",
                    label: "Model",
                },
                ColumnChoice {
                    id: "sessions.provider",
                    label: "Provider",
                },
                ColumnChoice {
                    id: "sessions.latency",
                    label: "Latency",
                },
                ColumnChoice {
                    id: "sessions.high_risk",
                    label: "High-risk",
                },
                ColumnChoice {
                    id: "sessions.risk",
                    label: "Risk",
                },
                ColumnChoice {
                    id: "sessions.kind",
                    label: "Last kind",
                },
                ColumnChoice {
                    id: "sessions.message",
                    label: "Last message",
                },
            ],
            Tab::Processes => vec![
                ColumnChoice {
                    id: "processes.pid",
                    label: "PID",
                },
                ColumnChoice {
                    id: "processes.ppid",
                    label: "PPID",
                },
                ColumnChoice {
                    id: "processes.cpu",
                    label: "CPU %",
                },
                ColumnChoice {
                    id: "processes.cpu_history",
                    label: "CPU trend",
                },
                ColumnChoice {
                    id: "processes.mem",
                    label: "Memory %",
                },
                ColumnChoice {
                    id: "processes.mem_history",
                    label: "Memory trend",
                },
                ColumnChoice {
                    id: "processes.risk",
                    label: "Risk",
                },
                ColumnChoice {
                    id: "processes.elapsed",
                    label: "Elapsed",
                },
                ColumnChoice {
                    id: "processes.cwd",
                    label: "Working directory",
                },
                ColumnChoice {
                    id: "processes.command",
                    label: "Command",
                },
            ],
            Tab::Containers => vec![
                ColumnChoice {
                    id: "containers.status",
                    label: "Status",
                },
                ColumnChoice {
                    id: "containers.name",
                    label: "Name",
                },
                ColumnChoice {
                    id: "containers.id",
                    label: "CID",
                },
                ColumnChoice {
                    id: "containers.cpu",
                    label: "CPU",
                },
                ColumnChoice {
                    id: "containers.cpus",
                    label: "CPU scaled",
                },
                ColumnChoice {
                    id: "containers.mem",
                    label: "Memory",
                },
                ColumnChoice {
                    id: "containers.net",
                    label: "Net I/O",
                },
                ColumnChoice {
                    id: "containers.block",
                    label: "IO",
                },
                ColumnChoice {
                    id: "containers.pids",
                    label: "PIDs",
                },
                ColumnChoice {
                    id: "containers.uptime",
                    label: "Uptime",
                },
                ColumnChoice {
                    id: "containers.image",
                    label: "Image",
                },
                ColumnChoice {
                    id: "containers.mem_usage",
                    label: "Memory usage",
                },
                ColumnChoice {
                    id: "containers.ports",
                    label: "Ports",
                },
                ColumnChoice {
                    id: "containers.health",
                    label: "Health",
                },
            ],
            Tab::Events => vec![
                ColumnChoice {
                    id: "events.time",
                    label: "Time",
                },
                ColumnChoice {
                    id: "events.source",
                    label: "Source",
                },
                ColumnChoice {
                    id: "events.session",
                    label: "Session",
                },
                ColumnChoice {
                    id: "events.task",
                    label: "Task",
                },
                ColumnChoice {
                    id: "events.pid",
                    label: "PID",
                },
                ColumnChoice {
                    id: "events.ppid",
                    label: "PPID",
                },
                ColumnChoice {
                    id: "events.kind",
                    label: "Kind",
                },
                ColumnChoice {
                    id: "events.risk",
                    label: "Risk",
                },
                ColumnChoice {
                    id: "events.message",
                    label: "Message",
                },
            ],
        }
    }

    fn ensure_visible_columns(&mut self) {
        for tab in Tab::ALL {
            let choices = self.column_choices(tab);
            if !choices.is_empty()
                && choices
                    .iter()
                    .all(|choice| self.hidden_columns.contains(choice.id))
            {
                self.hidden_columns.remove(choices[0].id);
            }
        }
    }

    fn filtered_agents(&self) -> Vec<ProcessRow> {
        let mut rows: Vec<_> = self
            .snapshot
            .processes
            .iter()
            .filter(|p| p.agent.is_some())
            .filter(|p| self.focused_agent_pid.is_none_or(|pid| p.pid == pid))
            .filter(|p| self.process_matches_risk_filter(p))
            .filter(|p| self.agent_matches_filter(p))
            .cloned()
            .collect();
        self.sort_process_rows(&mut rows);
        if self.reverse_sort {
            rows.reverse();
        }
        rows
    }

    fn filtered_processes(&self) -> Vec<ProcessRow> {
        let mut rows: Vec<_> = self
            .snapshot
            .processes
            .iter()
            .filter(|p| self.process_matches_risk_filter(p))
            .filter(|p| {
                self.matches_filter(&p.command)
                    || self.matches_filter(&p.pid.to_string())
                    || p.cwd.as_ref().is_some_and(|cwd| self.matches_filter(cwd))
            })
            .cloned()
            .collect();
        self.sort_process_rows(&mut rows);
        if self.reverse_sort {
            rows.reverse();
        }
        rows
    }

    fn filtered_sessions(&self) -> Vec<SessionRow> {
        let events = self
            .snapshot
            .events
            .iter()
            .filter(|event| self.event_matches_scope_filters(event))
            .cloned()
            .collect::<Vec<_>>();
        let mut rows = session_rows(&events)
            .into_iter()
            .filter(|row| {
                self.matches_filter(&row.source)
                    || self.matches_filter(&row.session)
                    || self.matches_filter(&row.task)
                    || self.matches_filter(&row.workspace)
                    || self.matches_filter(&row.last_kind)
                    || self.matches_filter(&row.last_message)
            })
            .collect::<Vec<_>>();
        sort_sessions(&mut rows, self.sort_by);
        if self.reverse_sort {
            rows.reverse();
        }
        rows
    }

    fn filtered_containers(&self) -> Vec<ContainerRow> {
        let mut rows: Vec<_> = self
            .snapshot
            .containers
            .iter()
            .filter(|c| {
                self.focused_container
                    .as_ref()
                    .is_none_or(|query| container_matches_query(c, query))
                    && (self.matches_filter(&c.name)
                        || self.matches_filter(&c.id)
                        || self.matches_filter(short_id(&c.id))
                        || self.matches_filter(&c.image)
                        || self.matches_filter(&c.status)
                        || self.matches_filter(&c.ports)
                        || self.matches_filter(&c.inspect.health)
                        || self.matches_filter(&c.inspect.restart_policy)
                        || self.matches_filter(&c.inspect.mounts)
                        || self.matches_filter(&c.inspect.labels)
                        || self.matches_filter(&c.inspect.networks))
            })
            .cloned()
            .collect();
        match self.sort_by {
            SortBy::Cpu => rows.sort_by(|a, b| {
                b.cpu_pct
                    .unwrap_or_default()
                    .partial_cmp(&a.cpu_pct.unwrap_or_default())
                    .unwrap_or(std::cmp::Ordering::Equal)
            }),
            SortBy::Mem => rows.sort_by(|a, b| {
                b.mem_pct
                    .unwrap_or_default()
                    .partial_cmp(&a.mem_pct.unwrap_or_default())
                    .unwrap_or(std::cmp::Ordering::Equal)
                    .then(a.name.cmp(&b.name))
            }),
            SortBy::Net => rows.sort_by(|a, b| {
                container_net_total(b)
                    .cmp(&container_net_total(a))
                    .then(a.name.cmp(&b.name))
            }),
            SortBy::Block => rows.sort_by(|a, b| {
                container_block_total(b)
                    .cmp(&container_block_total(a))
                    .then(a.name.cmp(&b.name))
            }),
            SortBy::Pids => rows.sort_by(|a, b| {
                container_pid_count(b)
                    .cmp(&container_pid_count(a))
                    .then(a.name.cmp(&b.name))
            }),
            SortBy::State => rows.sort_by(|a, b| {
                container_state_rank(b)
                    .cmp(&container_state_rank(a))
                    .then(a.name.cmp(&b.name))
            }),
            SortBy::Id => rows.sort_by(|a, b| a.id.cmp(&b.id).then(a.name.cmp(&b.name))),
            SortBy::Uptime => rows.sort_by(|a, b| {
                container_uptime_seconds(b)
                    .cmp(&container_uptime_seconds(a))
                    .then(a.name.cmp(&b.name))
            }),
            SortBy::Name | SortBy::Tokens => rows.sort_by(|a, b| a.name.cmp(&b.name)),
        }
        if self.reverse_sort {
            rows.reverse();
        }
        rows
    }

    fn filtered_events(&self) -> Vec<EventRow> {
        self.snapshot
            .events
            .iter()
            .filter(|event| self.event_matches_scope_filters(event))
            .filter(|e| self.event_matches_filter(e))
            .cloned()
            .collect()
    }

    fn focused_session_events(&self) -> Vec<EventRow> {
        let Some(focus) = &self.focused_session else {
            return Vec::new();
        };
        self.snapshot
            .events
            .iter()
            .filter(|event| session_focus_matches_event(focus, event))
            .filter(|event| self.event_matches_scope_filters(event))
            .filter(|event| self.event_matches_filter(event))
            .cloned()
            .collect()
    }

    fn focused_session_row(&self) -> Option<SessionRow> {
        let focus = self.focused_session.as_ref()?;
        let events = self
            .snapshot
            .events
            .iter()
            .filter(|event| self.event_matches_scope_filters(event))
            .cloned()
            .collect::<Vec<_>>();
        session_rows(&events)
            .into_iter()
            .find(|row| row.source == focus.source && row.session == focus.session)
    }

    fn event_matches_scope_filters(&self, event: &EventRow) -> bool {
        self.risk_filter.matches(event.risk) && self.kind_filter.matches(&event.kind)
    }

    fn event_matches_filter(&self, event: &EventRow) -> bool {
        self.matches_filter(&event.source)
            || event
                .session
                .as_ref()
                .is_some_and(|session| self.matches_filter(session))
            || event
                .task
                .as_ref()
                .is_some_and(|task| self.matches_filter(task))
            || self.matches_filter(&event.kind)
            || self.matches_filter(&event.message)
            || event
                .details
                .iter()
                .any(|(key, value)| self.matches_filter(key) || self.matches_filter(value))
    }

    fn agent_matches_filter(&self, process: &ProcessRow) -> bool {
        if self.filter.is_empty() {
            return true;
        }
        if self.matches_filter(&process.command)
            || self.matches_filter(&process.pid.to_string())
            || process
                .cwd
                .as_ref()
                .is_some_and(|cwd| self.matches_filter(cwd))
            || process
                .agent
                .is_some_and(|agent| self.matches_filter(agent.label()))
        {
            return true;
        }

        let activity = self.agent_activity_for_process(process);
        if self.matches_filter(&activity.model)
            || self.matches_filter(&activity.provider)
            || self.matches_filter(&format_count(activity.total_tokens))
        {
            return true;
        }

        agent_session_rows(self, process).iter().any(|session| {
            self.matches_filter(&session.source)
                || self.matches_filter(&session.session)
                || self.matches_filter(&session.task)
                || self.matches_filter(&session.workspace)
                || self.matches_filter(&session.model)
                || self.matches_filter(&session.provider)
                || self.matches_filter(&session.last_kind)
                || self.matches_filter(&session.last_message)
        }) || self
            .snapshot
            .events
            .iter()
            .filter(|event| self.event_matches_agent_process(process, event))
            .any(|event| self.event_matches_filter(event))
    }

    fn matches_filter(&self, text: &str) -> bool {
        self.filter.is_empty() || text.to_lowercase().contains(&self.filter.to_lowercase())
    }

    fn header(&self) -> String {
        let agents = self
            .snapshot
            .processes
            .iter()
            .filter(|p| p.agent.is_some())
            .count();
        let containers = container_state_summary(&self.snapshot.containers);
        let processes = self.snapshot.processes.len();
        let high_events = self
            .snapshot
            .events
            .iter()
            .filter(|event| event.risk == Risk::High)
            .count();
        let llm_events = self
            .snapshot
            .events
            .iter()
            .filter(|event| is_llm_event_kind(&event.kind))
            .count();
        let total_tokens = self
            .snapshot
            .events
            .iter()
            .filter_map(event_token_usage)
            .map(|tokens| tokens.total)
            .sum::<u64>();
        let title = format!(
            " a3s top  boxes:{}  agents:{agents}  processes:{processes}  events:{} high:{high_events} llm:{llm_events} tok:{} ",
            containers.header_label(),
            self.snapshot.events.len(),
            format_count(total_tokens)
        );
        let right = if self.paused {
            "paused".to_string()
        } else {
            self.last_refresh
                .map(|t| format!("refreshed {:.1}s ago", t.elapsed().as_secs_f32()))
                .unwrap_or_else(|| "loading".to_string())
        };
        StatusBar::new()
            .left(title)
            .right(right)
            .fg(Color::BrightWhite)
            .bg(Color::Rgb(35, 40, 60))
            .bold(true)
            .view(self.width)
    }

    fn tabs(&self) -> String {
        let mut tabs = Tab::PRIMARY.to_vec();
        if !tabs.contains(&self.tab) {
            tabs.push(self.tab);
        }
        let active = tabs.iter().position(|tab| *tab == self.tab).unwrap_or(0);
        let labels = tabs.iter().map(|tab| tab.label()).collect::<Vec<_>>();
        let mut panel = Tabs::new(labels)
            .active_colors(Color::Black, ACCENT)
            .inactive_color(Color::BrightBlack)
            .suffix_color(Color::BrightBlack)
            .gap(1);
        panel.set_active(active);

        let filter = if self.editing_filter {
            format!("/{}_", self.filter)
        } else if self.filter.is_empty() {
            "/ filter".to_string()
        } else {
            format!("/{}", self.filter)
        };
        panel = panel.segment(TabSegment::new(filter));
        if let Some(id) = &self.focused_container {
            let label = self
                .snapshot
                .containers
                .iter()
                .find(|c| c.id == *id || short_id(&c.id) == id)
                .map(|c| c.name.as_str())
                .unwrap_or(id);
            panel = panel
                .segment(TabSegment::new(format!("focus:{}", truncate(label, 20))).color(CYAN));
        }
        if let Some(pid) = self.focused_agent_pid {
            let label = self
                .snapshot
                .processes
                .iter()
                .find(|p| p.pid == pid)
                .and_then(|p| p.agent.map(|agent| agent.label()))
                .unwrap_or("agent");
            panel = panel.segment(TabSegment::new(format!("agent:{label}/{pid}")).color(ACCENT));
        }
        if let Some(focus) = &self.focused_session {
            panel = panel.segment(
                TabSegment::new(format!(
                    "session:{}/{}",
                    focus.source,
                    truncate(&focus.session, 18)
                ))
                .color(ORANGE),
            );
        }
        panel.view(self.width)
    }

    fn table(&self) -> String {
        if self.column_panel.is_some() {
            return self.column_panel_view();
        }
        if self.sort_panel.is_some() {
            return self.sort_panel_view();
        }
        if self.connector_panel.is_some() {
            return self.connector_panel_view();
        }
        if self.log.is_some() {
            return self.logs_view();
        }
        if self.container_menu.is_some() {
            return self.container_menu_view();
        }
        if self.help {
            return self.help_view();
        }
        match self.tab {
            Tab::Agents if self.focused_agent_pid.is_some() => {
                self.agent_focus_view(self.filtered_agents())
            }
            Tab::Agents => self.agents_tree_view(),
            Tab::Sessions if self.focused_session.is_some() => {
                self.session_focus_view(self.focused_session_events())
            }
            Tab::Sessions => self.sessions_table(self.filtered_sessions()),
            Tab::Processes => self.render_process_tab(),
            Tab::Containers if self.focused_container.is_some() => {
                self.container_focus_view(self.filtered_containers())
            }
            Tab::Containers => self.container_table(self.filtered_containers()),
            Tab::Events => self.events_table(self.filtered_events()),
        }
    }

    fn agents_tree_view(&self) -> String {
        let root = self.agents_tree_root();
        Tree::new(root)
            .branch_color(ACCENT)
            .leaf_color(Color::BrightBlack)
            .view(self.width, self.visible_height() + 2)
    }

    fn agents_tree_root(&self) -> TreeNode {
        let groups = self.agent_tree_groups();
        let mut children = groups
            .iter()
            .enumerate()
            .map(|(idx, group)| self.agent_tree_agent_node(group, idx == self.selected))
            .collect::<Vec<_>>();

        if children.is_empty() {
            children.push(TreeNode::leaf(
                "no coding-agent activity matched the current filters",
            ));
        }

        TreeNode::branch(self.agent_tree_summary_label(&groups), children)
    }

    fn agent_tree_groups(&self) -> Vec<AgentTreeGroup> {
        let sessions = self.filtered_sessions();
        let processes = self.filtered_agents();
        let events = self.filtered_events();
        let mut groups = AgentKind::ALL
            .iter()
            .copied()
            .filter_map(|agent| {
                let sessions = sessions
                    .iter()
                    .filter(|session| agent_matches_source(agent, &session.source))
                    .cloned()
                    .collect::<Vec<_>>();
                let processes = processes
                    .iter()
                    .filter(|process| process.agent == Some(agent))
                    .cloned()
                    .collect::<Vec<_>>();
                let events = events
                    .iter()
                    .filter(|event| agent_matches_source(agent, &event.source))
                    .cloned()
                    .collect::<Vec<_>>();
                if !self.filter.is_empty()
                    && sessions.is_empty()
                    && processes.is_empty()
                    && events.is_empty()
                {
                    return None;
                }
                let activity = self.agent_activity_from_events(events.iter());
                let usage = self.agent_tree_usage(&processes);
                Some(AgentTreeGroup {
                    agent,
                    sessions,
                    processes,
                    events,
                    activity,
                    usage,
                })
            })
            .collect::<Vec<_>>();
        groups.sort_by(|a, b| {
            b.active()
                .cmp(&a.active())
                .then(risk_rank(b.risk()).cmp(&risk_rank(a.risk())))
                .then(
                    b.usage
                        .cpu_pct
                        .partial_cmp(&a.usage.cpu_pct)
                        .unwrap_or(std::cmp::Ordering::Equal),
                )
                .then(b.activity.total_tokens.cmp(&a.activity.total_tokens))
                .then(b.events.len().cmp(&a.events.len()))
                .then(agent_order(a.agent).cmp(&agent_order(b.agent)))
        });
        groups
    }

    fn agent_tree_summary_label(&self, groups: &[AgentTreeGroup]) -> String {
        let active = groups.iter().filter(|group| group.active()).count();
        let sessions = groups
            .iter()
            .map(|group| group.sessions.len())
            .sum::<usize>();
        let processes = groups
            .iter()
            .map(|group| self.agent_system_processes(group).len())
            .sum::<usize>();
        let events = groups.iter().map(|group| group.events.len()).sum::<usize>();
        let high = groups
            .iter()
            .map(|group| group.activity.high_risk)
            .sum::<usize>();
        let tokens = groups
            .iter()
            .map(|group| group.activity.total_tokens)
            .sum::<u64>();
        format!(
            "Agents · active {active}/{} · sessions {sessions} · processes {processes} · events {events} · high {high} · tok {}",
            groups.len(),
            format_count(tokens)
        )
    }

    fn agent_tree_agent_node(&self, group: &AgentTreeGroup, selected: bool) -> TreeNode {
        let activity = &group.activity;
        let usage = group.usage;
        let state = agent_tree_state_label(group);
        let system_processes = self.agent_system_processes(group);
        let prefix = if selected { "> " } else { "  " };
        let label = agent_tree_label(
            group.agent,
            format!(
                "{prefix}{} [{state}] · S{} P{} E{} · CPU {:.1}% · MEM {:.1}% · TOK {}",
                group.agent.label(),
                group.sessions.len(),
                system_processes.len(),
                group.events.len(),
                usage.cpu_pct,
                usage.mem_pct,
                format_count(activity.total_tokens)
            ),
        );
        TreeNode::branch(
            label,
            vec![
                self.agent_tree_resources_node(group),
                self.agent_tree_sessions_node(group.agent, &group.sessions),
                self.agent_tree_processes_node(
                    group.agent,
                    &system_processes,
                    &group.processes,
                    selected,
                ),
                self.agent_tree_events_node(group.agent, &group.events),
            ],
        )
    }

    fn agent_tree_resources_node(&self, group: &AgentTreeGroup) -> TreeNode {
        let history = self.agent_group_history(&group.processes);
        let activity = &group.activity;
        let mut children = vec![
            TreeNode::leaf(agent_tree_label(
                group.agent,
                format!(
                    "CPU {:>5.1}%  {}",
                    group.usage.cpu_pct,
                    self.sparkline(&history.cpu, metric_color(group.usage.cpu_pct))
                ),
            )),
            TreeNode::leaf(agent_tree_label(
                group.agent,
                format!(
                    "MEM {:>5.1}%  {}",
                    group.usage.mem_pct,
                    self.sparkline(&history.mem, metric_color(group.usage.mem_pct))
                ),
            )),
            TreeNode::leaf(agent_tree_label(
                group.agent,
                format!("children {}", group.usage.descendants),
            )),
        ];
        let model = display_model(&activity.model);
        let provider = display_provider(&activity.provider);
        let latency = format_avg_ms(activity.latency_ms, activity.latency_samples);
        let ttft = format_avg_ms(activity.ttft_ms, activity.ttft_samples);
        if activity.events > 0 || activity.total_tokens > 0 {
            children.push(TreeNode::leaf(agent_tree_label(group.agent, format!(
                "activity evt {} · tool {} · sec {} · file {} · net {} · llm {} · tok {} · model {} · provider {} · lat {} · ttft {} · high {}",
                activity.events,
                activity.tools,
                activity.security,
                activity.files,
                activity.egress,
                activity.llm,
                format_count(activity.total_tokens),
                model,
                provider,
                latency,
                ttft,
                activity.high_risk
            ))));
        }
        TreeNode::branch(agent_tree_label(group.agent, "Resources"), children)
    }

    fn agent_group_history(&self, processes: &[ProcessRow]) -> MetricHistory {
        let mut history = MetricHistory::default();
        for process in self.agent_group_root_processes(processes) {
            let process_history = self.metric_history(&agent_tree_history_key(process.pid));
            add_aligned_f32_history(&mut history.cpu, &process_history.cpu);
            add_aligned_f32_history(&mut history.mem, &process_history.mem);
        }
        history
    }

    fn agent_tree_usage(&self, processes: &[ProcessRow]) -> ProcessTreeUsage {
        let mut usage = ProcessTreeUsage::default();
        for process in self.agent_group_root_processes(processes) {
            let tree = self.process_tree_usage(process.pid);
            usage.cpu_pct += tree.cpu_pct;
            usage.mem_pct += tree.mem_pct;
            usage.descendants += tree.descendants;
        }
        usage
    }

    fn agent_group_root_processes(&self, processes: &[ProcessRow]) -> Vec<ProcessRow> {
        let agent_pids = processes
            .iter()
            .map(|process| process.pid)
            .collect::<HashSet<_>>();
        processes
            .iter()
            .filter(|process| !self.has_ancestor_in_set(process.pid, &agent_pids))
            .cloned()
            .collect()
    }

    fn has_ancestor_in_set(&self, pid: u32, ancestors: &HashSet<u32>) -> bool {
        let mut current = pid;
        let mut visited = HashSet::new();
        while visited.insert(current) {
            let Some(process) = self
                .snapshot
                .processes
                .iter()
                .find(|process| process.pid == current)
            else {
                return false;
            };
            if ancestors.contains(&process.ppid) {
                return true;
            }
            if process.ppid == 0 || process.ppid == current {
                return false;
            }
            current = process.ppid;
        }
        false
    }

    fn agent_system_processes(&self, group: &AgentTreeGroup) -> Vec<ProcessRow> {
        let mut roots = self.agent_group_root_processes(&group.processes);
        roots.sort_by_key(|process| process.pid);
        let mut rows = Vec::new();
        let mut visited = roots
            .iter()
            .map(|process| process.pid)
            .collect::<HashSet<_>>();
        for root in roots {
            self.collect_agent_system_processes(group.agent, root.pid, &mut visited, &mut rows);
        }
        rows
    }

    fn collect_agent_system_processes(
        &self,
        agent: AgentKind,
        parent_pid: u32,
        visited: &mut HashSet<u32>,
        rows: &mut Vec<ProcessRow>,
    ) {
        let mut children = self
            .snapshot
            .processes
            .iter()
            .filter(|process| process.ppid == parent_pid)
            .cloned()
            .collect::<Vec<_>>();
        children.sort_by_key(|process| process.pid);

        for child in children {
            if !visited.insert(child.pid) {
                continue;
            }
            match child.agent {
                Some(child_agent) if child_agent != agent => continue,
                Some(_) => {
                    self.collect_agent_system_processes(agent, child.pid, visited, rows);
                }
                None => {
                    rows.push(child.clone());
                    self.collect_agent_system_processes(agent, child.pid, visited, rows);
                }
            }
        }
    }

    fn agent_tree_sessions_node(&self, agent: AgentKind, sessions: &[SessionRow]) -> TreeNode {
        let mut children = sessions
            .iter()
            .take(AGENTS_TREE_SESSION_LIMIT)
            .map(|session| {
                let latency = format_avg_ms(session.latency_ms, session.latency_samples);
                TreeNode::leaf(agent_tree_label(agent, format!(
                    "{} · task {} · cwd {} · evt {} · tools {} · llm {} · tok {} · model {} · lat {} · risk {}",
                    session.session,
                    session.task,
                    display_workspace(Some(&session.workspace)),
                    session.events,
                    session.tools,
                    session.llm,
                    format_count(session.total_tokens),
                    display_model(&session.model),
                    latency,
                    session.risk.label()
                )))
            })
            .collect::<Vec<_>>();
        push_agent_more_leaf(
            &mut children,
            sessions.len(),
            AGENTS_TREE_SESSION_LIMIT,
            "sessions",
            agent,
        );
        if children.is_empty() {
            children.push(TreeNode::leaf(agent_tree_label(agent, "no sessions")));
        }
        TreeNode::branch(
            agent_tree_label(agent, format!("Sessions ({})", sessions.len())),
            children,
        )
    }

    fn agent_tree_processes_node(
        &self,
        agent: AgentKind,
        system_processes: &[ProcessRow],
        agent_processes: &[ProcessRow],
        selected_group: bool,
    ) -> TreeNode {
        let mut children = system_processes
            .iter()
            .enumerate()
            .take(AGENTS_TREE_PROCESS_LIMIT)
            .map(|(idx, process)| {
                let marker = if selected_group && idx == 0 {
                    "> "
                } else {
                    "  "
                };
                TreeNode::leaf(agent_tree_label(
                    agent,
                    format!(
                        "{marker}pid {} · ppid {} · CPU {:.1}% · MEM {:.1}% · {}",
                        process.pid,
                        process.ppid,
                        process.cpu_pct,
                        process.mem_pct,
                        display_cmd(&process.command)
                    ),
                ))
            })
            .collect::<Vec<_>>();
        push_agent_more_leaf(
            &mut children,
            system_processes.len(),
            AGENTS_TREE_PROCESS_LIMIT,
            "processes",
            agent,
        );
        if children.is_empty() {
            children.push(TreeNode::leaf(agent_tree_label(
                agent,
                "no system processes from this agent",
            )));
        }
        TreeNode::branch(
            agent_tree_label(
                agent,
                format!(
                    "Processes ({} system · {} agent)",
                    system_processes.len(),
                    agent_processes.len()
                ),
            ),
            children,
        )
    }

    fn agent_tree_events_node(&self, agent: AgentKind, events: &[EventRow]) -> TreeNode {
        let mut children = events
            .iter()
            .take(AGENTS_TREE_EVENT_LIMIT)
            .map(|event| {
                TreeNode::leaf(agent_tree_label(
                    agent,
                    format!(
                        "{} · {} · {} · {} · {}",
                        event.ts,
                        event.kind,
                        event.risk.label(),
                        event_scope_label(event),
                        event.message
                    ),
                ))
            })
            .collect::<Vec<_>>();
        push_agent_more_leaf(
            &mut children,
            events.len(),
            AGENTS_TREE_EVENT_LIMIT,
            "events",
            agent,
        );
        if children.is_empty() {
            children.push(TreeNode::leaf(agent_tree_label(agent, "no recent events")));
        }
        TreeNode::branch(
            agent_tree_label(agent, format!("Events ({})", events.len())),
            children,
        )
    }

    /// Render the Processes tab via the shared process-table renderer, feeding
    /// it this app's live CPU/MEM history for the sparkline columns.
    fn render_process_tab(&self) -> String {
        let rows = self.filtered_processes();
        let history = |pid: u32| {
            let h = self.metric_history(&process_history_key(pid));
            (h.cpu, h.mem)
        };
        render_process_table(
            &rows,
            &ProcessTableView {
                selected: self.selected,
                scroll: self.scroll,
                width: self.width,
                height: self.visible_height() + 2,
                hidden: &self.hidden_columns,
                history: Some(&history),
            },
        )
    }

    fn sessions_table(&self, rows: Vec<SessionRow>) -> String {
        let mut table = DataTable::new(vec![
            self.configured_column("sessions.source", DataColumn::new("AGENT").width(12)),
            self.configured_column("sessions.session", DataColumn::new("SESSION").width(14)),
            self.configured_column("sessions.task", DataColumn::new("TASK").width(12)),
            self.configured_column("sessions.workspace", DataColumn::new("CWD").width(18)),
            self.configured_column(
                "sessions.events",
                DataColumn::new("EVT").width(5).align(CellAlign::Right),
            ),
            self.configured_column(
                "sessions.tools",
                DataColumn::new("TOOL").width(5).align(CellAlign::Right),
            ),
            self.configured_column(
                "sessions.security",
                DataColumn::new("SEC").width(5).align(CellAlign::Right),
            ),
            self.configured_column(
                "sessions.files",
                DataColumn::new("FILE").width(5).align(CellAlign::Right),
            ),
            self.configured_column(
                "sessions.egress",
                DataColumn::new("NET").width(5).align(CellAlign::Right),
            ),
            self.configured_column(
                "sessions.llm",
                DataColumn::new("LLM").width(5).align(CellAlign::Right),
            ),
            self.configured_column(
                "sessions.tokens",
                DataColumn::new("TOK").width(7).align(CellAlign::Right),
            ),
            self.configured_column("sessions.model", DataColumn::new("MODEL").width(14)),
            self.configured_column("sessions.provider", DataColumn::new("PROV").width(10)),
            self.configured_column(
                "sessions.latency",
                DataColumn::new("LAT").width(7).align(CellAlign::Right),
            ),
            self.configured_column(
                "sessions.high_risk",
                DataColumn::new("HIGH").width(5).align(CellAlign::Right),
            ),
            self.configured_column("sessions.risk", DataColumn::new("RISK").width(5)),
            self.configured_column("sessions.kind", DataColumn::new("LAST").width(12)),
            self.configured_column("sessions.message", DataColumn::new("MESSAGE").min_width(18)),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .selected((!rows.is_empty()).then_some(self.selected))
        .scroll(self.scroll)
        .empty("no agent sessions yet - auto-discovery watches Claude, Codex, and A3S Code logs");

        for row in rows {
            table.add_row(
                DataRow::new(vec![
                    row.source,
                    row.session,
                    row.task,
                    row.workspace,
                    row.events.to_string(),
                    row.tools.to_string(),
                    row.security.to_string(),
                    row.files.to_string(),
                    row.egress.to_string(),
                    row.llm.to_string(),
                    format_count(row.total_tokens),
                    display_model(&row.model),
                    display_provider(&row.provider),
                    format_avg_ms(row.latency_ms, row.latency_samples),
                    row.high_risk.to_string(),
                    row.risk.label().to_string(),
                    row.last_kind,
                    row.last_message,
                ])
                .fg(row.risk.color()),
            );
        }
        table.view(self.width, self.visible_height() + 2)
    }

    fn session_focus_view(&self, events: Vec<EventRow>) -> String {
        let width = self.width as usize;
        let Some(focus) = &self.focused_session else {
            return String::new();
        };

        let row = self.focused_session_row();
        let risk = row.as_ref().map(|row| row.risk).unwrap_or(Risk::Low);
        let mut out = Vec::new();
        let mut header =
            SectionHeader::new(format!("session view {} · {}", focus.source, focus.session))
                .title_color(risk.color())
                .metadata_color(Color::BrightBlack)
                .muted_color(Color::BrightBlack)
                .separator_color(Color::BrightBlack);
        if let Some(row) = &row {
            header = header.metadata(format!(
                "task {} · cwd {} · events {} · tools {} · sec {} · files {} · net {} · llm {} · tokens {} · model {} · provider {} · latency {} · ttft {} · wire {} / {} · high {} · risk {}",
                row.task,
                row.workspace,
                row.events,
                row.tools,
                row.security,
                row.files,
                row.egress,
                row.llm,
                format_count(row.total_tokens),
                display_model(&row.model),
                display_provider(&row.provider),
                format_avg_ms(row.latency_ms, row.latency_samples),
                format_avg_ms(row.ttft_ms, row.ttft_samples),
                format_bytes(row.req_bytes),
                format_bytes(row.resp_bytes),
                row.high_risk,
                row.risk.label()
            ));
        } else {
            header = header.muted("focused session has no events");
        }
        out.extend(header.view(self.width, 3).lines().map(ToString::to_string));

        let mut table = DataTable::new(vec![
            DataColumn::new("TIME").width(9),
            DataColumn::new("KIND").width(12),
            DataColumn::new("RISK").width(5),
            DataColumn::new("TASK").width(12),
            DataColumn::new("PID").width(7).align(CellAlign::Right),
            DataColumn::new("PPID").width(7).align(CellAlign::Right),
            DataColumn::new("MESSAGE").min_width(18),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .selected((!events.is_empty()).then_some(self.selected))
        .scroll(self.scroll)
        .empty("no events match this session");

        for event in events {
            table.add_row(
                DataRow::new(vec![
                    event.ts,
                    event.kind,
                    event.risk.label().to_string(),
                    event.task.unwrap_or_else(|| "-".to_string()),
                    format_event_pid(event.pid),
                    format_event_pid(event.ppid),
                    event.message,
                ])
                .fg(event.risk.color()),
            );
        }

        let height = self.visible_height() + 2;
        let table_height = height.saturating_sub(out.len()).max(3);
        out.push(table.view(self.width, table_height));
        out.join("\n")
            .lines()
            .take(height)
            .map(|line| pad_line(line, width))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn agent_focus_view(&self, rows: Vec<ProcessRow>) -> String {
        let width = self.width as usize;
        let Some(row) = rows.first() else {
            return Style::new()
                .fg(Color::BrightBlack)
                .italic()
                .render(&pad_plain(" focused agent is no longer present", width));
        };
        let agent = row.agent.unwrap_or(AgentKind::A3sCode);
        let color = agent.color();
        let usage = self.process_tree_usage(row.pid);
        let history = self.metric_history(&agent_tree_history_key(row.pid));
        let activity = self.agent_activity_for_process(row);
        let mut out = Vec::new();

        out.extend(
            SectionHeader::new(format!("agent view {} · pid {}", agent.label(), row.pid))
                .metadata(format!(
                    "ppid {} · elapsed {} · children {} · subtree cpu {:.1}% mem {:.1}% · risk {} · cwd {}",
                    row.ppid,
                    row.elapsed,
                    usage.descendants,
                    usage.cpu_pct,
                    usage.mem_pct,
                    row.risk.label(),
                    display_workspace(row.cwd.as_deref())
                ))
                .title_color(color)
                .metadata_color(Color::BrightBlack)
                .separator_color(Color::BrightBlack)
                .view(self.width, 3)
                .lines()
                .map(ToString::to_string),
        );
        out.push(
            Meter::new(usage.cpu_pct as f64)
                .label("CPU")
                .width(width)
                .fg(metric_color(usage.cpu_pct))
                .view(),
        );
        out.push(self.focus_trend_line("CPU trend", &history.cpu, metric_color(usage.cpu_pct)));
        out.push(
            Meter::new(usage.mem_pct as f64)
                .label("MEM")
                .width(width)
                .fg(metric_color(usage.mem_pct))
                .view(),
        );
        out.push(self.focus_trend_line("MEM trend", &history.mem, metric_color(usage.mem_pct)));
        out.push(divider_line(self.width));
        out.extend(
            WrappedPrefixBlock::new(format!(
                "activity events {} · sessions {} · tools {} · sec {} · files {} · net {} · llm {} · tokens {} · model {} · provider {} · latency {} · ttft {} · wire {} / {} · high {}",
                activity.events,
                activity.sessions,
                activity.tools,
                activity.security,
                activity.files,
                activity.egress,
                activity.llm,
                format_count(activity.total_tokens),
                display_model(&activity.model),
                display_provider(&activity.provider),
                format_avg_ms(activity.latency_ms, activity.latency_samples),
                format_avg_ms(activity.ttft_ms, activity.ttft_samples),
                format_bytes(activity.req_bytes),
                format_bytes(activity.resp_bytes),
                activity.high_risk
            ))
            .first_prefix(" ")
            .continuation_prefix(" ")
            .width(width)
            .view()
            .lines()
            .map(ToString::to_string),
        );
        let height = self.visible_height() + 2;
        let recent_events = self.recent_agent_events_for_process(row);
        if !recent_events.is_empty() {
            out.push(divider_line(self.width));
            let event_height = recent_events.len().saturating_add(2).min(5);
            out.extend(
                self.agent_events_table(&recent_events, event_height)
                    .lines()
                    .map(ToString::to_string),
            );
        }
        out.extend(
            WrappedPrefixBlock::new(format!("command {}", row.command))
                .first_prefix(" ")
                .continuation_prefix(" ")
                .width(width)
                .view()
                .lines()
                .map(ToString::to_string),
        );

        let remaining = height.saturating_sub(out.len());
        if remaining > 7 {
            let reserve_tree = self.process_tree(row.pid).map_or(0, |_| 4);
            let session_height = remaining.saturating_sub(1 + reserve_tree).min(7);
            if session_height >= 3 {
                out.push(divider_line(self.width));
                let sessions = agent_session_rows(self, row);
                out.extend(
                    self.agent_sessions_table(&sessions, session_height)
                        .lines()
                        .map(ToString::to_string),
                );
            }
        }

        let remaining = height.saturating_sub(out.len());
        if remaining > 3 {
            out.push(divider_line(self.width));
            if let Some(tree) = self.process_tree(row.pid) {
                let tree = Tree::new(tree)
                    .branch_color(color)
                    .leaf_color(Color::BrightBlack)
                    .view(self.width, remaining.saturating_sub(1));
                out.extend(tree.lines().map(ToString::to_string));
            }
        }

        out.into_iter()
            .take(height)
            .map(|line| pad_line(&line, width))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn agent_sessions_table(&self, sessions: &[SessionRow], height: usize) -> String {
        let mut table = DataTable::new(vec![
            DataColumn::new("SESSION").width(14),
            DataColumn::new("TASK").width(12),
            DataColumn::new("EVT").width(5).align(CellAlign::Right),
            DataColumn::new("SEC").width(5).align(CellAlign::Right),
            DataColumn::new("LLM").width(5).align(CellAlign::Right),
            DataColumn::new("TOK").width(7).align(CellAlign::Right),
            DataColumn::new("RISK").width(5),
            DataColumn::new("LAST").min_width(16),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .empty("no sessions matched this agent");

        for row in sessions {
            table.add_row(
                DataRow::new(vec![
                    row.session.clone(),
                    row.task.clone(),
                    row.events.to_string(),
                    row.security.to_string(),
                    row.llm.to_string(),
                    format_count(row.total_tokens),
                    row.risk.label().to_string(),
                    format!("{} {}", row.last_kind, row.last_message),
                ])
                .fg(row.risk.color()),
            );
        }

        table.view(self.width, height)
    }

    fn agent_events_table(&self, events: &[EventRow], height: usize) -> String {
        let mut table = DataTable::new(vec![
            DataColumn::new("TIME").width(9),
            DataColumn::new("KIND").width(12),
            DataColumn::new("RISK").width(5),
            DataColumn::new("SESSION").width(12),
            DataColumn::new("TASK").width(12),
            DataColumn::new("PID").width(7).align(CellAlign::Right),
            DataColumn::new("MESSAGE").min_width(18),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .empty("no recent events matched this agent");

        for event in events {
            table.add_row(
                DataRow::new(vec![
                    event.ts.clone(),
                    event.kind.clone(),
                    event.risk.label().to_string(),
                    event.session.clone().unwrap_or_else(|| "-".to_string()),
                    event.task.clone().unwrap_or_else(|| "-".to_string()),
                    format_event_pid(event.pid),
                    event.message.clone(),
                ])
                .fg(event.risk.color()),
            );
        }

        table.view(self.width, height)
    }

    fn container_table(&self, rows: Vec<ContainerRow>) -> String {
        let mut table = DataTable::new(vec![
            self.configured_container_column(
                "containers.status",
                DataColumn::new("STATUS")
                    .width(9)
                    .min_width(7)
                    .priority(100),
            ),
            self.configured_container_column(
                "containers.name",
                DataColumn::new("NAME").width(16).min_width(8).priority(98),
            ),
            self.configured_container_column(
                "containers.id",
                DataColumn::new("CID").width(12).min_width(12).priority(99),
            ),
            self.configured_container_column(
                "containers.cpu",
                DataColumn::new("CPU")
                    .width(15)
                    .min_width(15)
                    .align(CellAlign::Right)
                    .priority(97),
            ),
            self.configured_container_column(
                "containers.cpus",
                DataColumn::new("CPUS")
                    .width(15)
                    .min_width(15)
                    .align(CellAlign::Right)
                    .priority(40),
            ),
            self.configured_container_column(
                "containers.mem",
                DataColumn::new("MEM")
                    .width(15)
                    .min_width(15)
                    .align(CellAlign::Right)
                    .priority(96),
            ),
            self.configured_container_column(
                "containers.net",
                DataColumn::new("NET I/O")
                    .width(17)
                    .min_width(11)
                    .priority(80),
            ),
            self.configured_container_column(
                "containers.block",
                DataColumn::new("IO").width(17).min_width(9).priority(79),
            ),
            self.configured_container_column(
                "containers.pids",
                DataColumn::new("PIDS")
                    .width(5)
                    .align(CellAlign::Right)
                    .priority(70),
            ),
            self.configured_container_column(
                "containers.uptime",
                DataColumn::new("UPTIME")
                    .width(14)
                    .min_width(8)
                    .priority(60),
            ),
            self.configured_container_column(
                "containers.image",
                DataColumn::new("IMAGE").width(18).priority(30),
            ),
            self.configured_container_column(
                "containers.mem_usage",
                DataColumn::new("MEM USAGE").width(17).priority(25),
            ),
            self.configured_container_column(
                "containers.ports",
                DataColumn::new("PORTS").width(24).priority(20),
            ),
            self.configured_container_column(
                "containers.health",
                DataColumn::new("HEALTH").width(10).priority(10),
            ),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .selected((!rows.is_empty()).then_some(self.selected))
        .scroll(self.scroll)
        .empty(if self.focused_container.is_some() {
            "focused container is no longer present".to_string()
        } else if self.show_all_containers {
            format!(
                "no containers found or {} is unavailable",
                self.connector.label()
            )
        } else {
            format!(
                "no running containers found or {} is unavailable",
                self.connector.label()
            )
        });

        for row in rows {
            let history = self.metric_history(&container_history_key(&row.id));
            let status = container_state_label(&row.status);
            let state_color = container_state_color(&status);
            let uptime = container_uptime_label(&row);
            let cid = short_id(&row.id).to_string();
            table.add_row(
                DataRow::new(vec![
                    status,
                    row.name,
                    cid,
                    self.metric_cell(row.cpu_pct, &history.cpu, CYAN),
                    self.scaled_cpu_metric_cell(row.cpu_pct, &history.cpu, row.cpu_count),
                    self.metric_cell(row.mem_pct, &history.mem, YELLOW),
                    row.net_io,
                    row.block_io,
                    row.pids,
                    uptime,
                    row.image,
                    row.mem_usage,
                    display_ports(&row.ports),
                    row.inspect.health,
                ])
                .cell_fg(0, state_color),
            );
        }
        table.view(self.width, self.visible_height() + 2)
    }

    fn container_focus_view(&self, rows: Vec<ContainerRow>) -> String {
        let width = self.width as usize;
        let Some(row) = rows.first() else {
            return Style::new()
                .fg(Color::BrightBlack)
                .italic()
                .render(&pad_plain(" focused container is no longer present", width));
        };

        let history = self.metric_history(&container_history_key(&row.id));
        let cpu = row.cpu_pct.unwrap_or_default();
        let mem = row.mem_pct.unwrap_or_default();
        let cpu_color = metric_color(cpu);
        let mem_color = metric_color(mem);
        let mut out = Vec::new();

        out.extend(
            SectionHeader::new(format!("single view {} ({})", row.name, short_id(&row.id)))
                .metadata(format!(
                    "status {} · health {} · image {}",
                    row.status, row.inspect.health, row.image
                ))
                .title_color(CYAN)
                .metadata_color(Color::BrightBlack)
                .separator_color(Color::BrightBlack)
                .view(self.width, 3)
                .lines()
                .map(ToString::to_string),
        );
        out.push(
            Meter::new(cpu as f64)
                .label("CPU")
                .width(width)
                .fg(cpu_color)
                .view(),
        );
        out.push(self.focus_trend_line("CPU trend", &history.cpu, cpu_color));
        out.push(
            Meter::new(mem as f64)
                .label("MEM")
                .width(width)
                .fg(mem_color)
                .view(),
        );
        out.push(self.focus_trend_line("MEM trend", &history.mem, mem_color));
        out.push(self.focus_bytes_trend_line(
            "NET trend",
            container_net_total(row),
            &history.net_io_bytes,
            GREEN,
        ));
        out.push(self.focus_bytes_trend_line(
            "IO trend",
            container_block_total(row),
            &history.block_io_bytes,
            ORANGE,
        ));
        out.push(divider_line(self.width));
        out.extend(
            self.container_resource_table(row, 9)
                .lines()
                .map(ToString::to_string),
        );

        let height = self.visible_height() + 2;
        let remaining = height.saturating_sub(out.len());
        if remaining > 6 {
            out.push(divider_line(self.width));
            let inspect_height = remaining.saturating_sub(1).min(8);
            out.extend(
                self.container_inspect_table(row, inspect_height)
                    .lines()
                    .map(ToString::to_string),
            );
        }

        let remaining = height.saturating_sub(out.len());
        if remaining > 4 {
            out.push(divider_line(self.width));
            out.extend(
                self.container_processes_table(row, remaining.saturating_sub(1))
                    .lines()
                    .map(ToString::to_string),
            );
        }

        out.into_iter()
            .take(height)
            .map(|line| pad_line(&line, width))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn container_inspect_table(&self, container: &ContainerRow, height: usize) -> String {
        let inspect = &container.inspect;
        let rows = [
            ("HEALTH", inspect.health.as_str()),
            ("RESTARTS", inspect.restarts.as_str()),
            ("RESTART POLICY", inspect.restart_policy.as_str()),
            ("CREATED", inspect.created.as_str()),
            ("STARTED", inspect.started.as_str()),
            ("EXIT", inspect.exit.as_str()),
            ("MOUNTS", inspect.mounts.as_str()),
            ("NETWORKS", inspect.networks.as_str()),
            ("ENV", inspect.env.as_str()),
            ("LABELS", inspect.labels.as_str()),
        ];
        let mut table = DataTable::new(vec![
            DataColumn::new("INSPECT").width(14),
            DataColumn::new("VALUE").min_width(18),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .empty("inspect metadata unavailable");

        for (key, value) in rows {
            if value != "-" {
                table.add_row(DataRow::new(vec![key.to_string(), value.to_string()]));
            }
        }

        table.view(self.width, height)
    }

    fn container_resource_table(&self, container: &ContainerRow, height: usize) -> String {
        let cpu_count = container
            .cpu_count
            .map(|cpus| cpus.to_string())
            .unwrap_or_else(|| "-".to_string());
        let rows = vec![
            ("MEMORY".to_string(), container.mem_usage.clone()),
            ("NET I/O".to_string(), container.net_io.clone()),
            ("IO".to_string(), container.block_io.clone()),
            ("PIDS".to_string(), container.pids.clone()),
            ("CPUS".to_string(), cpu_count),
            ("UPTIME".to_string(), container_uptime_label(container)),
            ("PORTS".to_string(), display_ports(&container.ports)),
        ];
        let mut table = DataTable::new(vec![
            DataColumn::new("RESOURCE").width(10),
            DataColumn::new("VALUE").min_width(18),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .empty("resource metadata unavailable");

        for (key, value) in rows {
            table.add_row(DataRow::new(vec![key, value]));
        }

        table.view(self.width, height)
    }

    fn container_processes_table(&self, container: &ContainerRow, height: usize) -> String {
        let panel = self
            .container_processes
            .as_ref()
            .filter(|panel| panel.container_id == container.id);
        let empty = match panel {
            Some(panel) if panel.loading && panel.rows.is_empty() => {
                format!("loading processes for {}", panel.container_name)
            }
            Some(panel) if panel.error.is_some() && panel.rows.is_empty() => format!(
                "container processes unavailable: {}",
                panel.error.as_deref().unwrap_or("unknown error")
            ),
            Some(_) => "no processes returned for this container".to_string(),
            None => "loading container processes...".to_string(),
        };
        let mut table = DataTable::new(vec![
            DataColumn::new("PID").width(7).align(CellAlign::Right),
            DataColumn::new("PPID").width(7).align(CellAlign::Right),
            DataColumn::new("CPU%").width(6).align(CellAlign::Right),
            DataColumn::new("MEM%").width(6).align(CellAlign::Right),
            DataColumn::new("ELAPSED").width(9),
            DataColumn::new("COMMAND").min_width(18),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .scroll(panel.map(|panel| panel.scroll).unwrap_or_default())
        .empty(empty);

        if let Some(panel) = panel {
            for row in &panel.rows {
                table.add_row(DataRow::new(vec![
                    row.pid.clone(),
                    row.ppid.clone(),
                    format_optional_pct(row.cpu_pct),
                    format_optional_pct(row.mem_pct),
                    row.elapsed.clone(),
                    row.command.clone(),
                ]));
            }
        }

        table.view(self.width, height)
    }

    fn focus_trend_line(&self, label: &str, values: &[f32], color: Color) -> String {
        let width = self.width as usize;
        let prefix = format!(" {label:<9} ");
        let chart_width = width
            .saturating_sub(a3s_tui::style::visible_len(&prefix))
            .max(1);
        let chart = Sparkline::new(values.iter().copied().map(f64::from))
            .width(chart_width)
            .range(0.0, 100.0)
            .fg(color)
            .view();
        format!("{prefix}{chart}")
    }

    fn focus_bytes_trend_line(
        &self,
        label: &str,
        value: u64,
        values: &[f64],
        color: Color,
    ) -> String {
        let width = self.width as usize;
        let value_label = format_bytes(value);
        let prefix = format!(" {label:<9} {value_label:>10} ");
        let chart_width = width
            .saturating_sub(a3s_tui::style::visible_len(&prefix))
            .max(1);
        let max = values.iter().copied().fold(value as f64, f64::max).max(1.0);
        let chart = Sparkline::new(values.iter().copied())
            .width(chart_width)
            .range(0.0, max)
            .fg(color)
            .view();
        format!("{prefix}{chart}")
    }

    fn column_panel_view(&self) -> String {
        let Some(panel) = &self.column_panel else {
            return String::new();
        };
        column_panel_lines(panel, self.width, self.visible_height()).join("\n")
    }

    fn logs_view(&self) -> String {
        let Some(log) = &self.log else {
            return String::new();
        };
        let state = if log.loading {
            LogViewState::Loading
        } else if log.refreshing {
            LogViewState::Refreshing
        } else {
            LogViewState::Ready
        };
        let timestamps = if log.timestamps {
            "timestamps:on"
        } else {
            "timestamps:off"
        };
        let follow = if log.follow {
            "follow:on"
        } else {
            "follow:off"
        };

        let mut view = LogView::new(format!(
            "logs {} ({})",
            log.container_name,
            short_id(&log.container_id)
        ))
        .text(&log.text)
        .scroll(log.scroll)
        .state(state)
        .loading_text("loading container logs...")
        .empty_text("no logs returned for this container")
        .title_color(CYAN)
        .metadata_color(Color::BrightBlack)
        .text_color(Color::BrightWhite)
        .muted_color(Color::BrightBlack)
        .separator_color(Color::BrightBlack);
        if state == LogViewState::Ready {
            view = view.metadata("tail 200");
        }
        view = view.metadata(timestamps).metadata(follow);

        view.view(self.width, self.log_visible_height().saturating_add(2))
    }

    fn container_menu_view(&self) -> String {
        let Some(menu) = &self.container_menu else {
            return String::new();
        };
        container_menu_lines(menu, self.width, self.visible_height()).join("\n")
    }

    fn help_view(&self) -> String {
        let rows = [
            ("Tab / Shift+Tab", "switch tabs: agents and containers"),
            ("↑/↓ or j/k", "select row"),
            ("Home / End", "jump to first or last row"),
            ("/ or f / Esc", "filter rows / clear filter"),
            ("s / r", "select sort field / reverse"),
            ("!", "cycle risk filter"),
            ("g", "cycle event kind filter"),
            ("--sessions", "start on agent session activity"),
            ("a / H / Space/p", "all containers / header / pause"),
            ("Enter", "container menu on Containers tab"),
            ("x", "toggle detail panel"),
            ("o", "single container, agent, or session focus"),
            ("← / →", "logs / container view"),
            ("l / r / t / f", "logs / refresh / timestamps / follow"),
            ("e", "exec shell in container"),
            ("w", "open first published web port"),
            ("r", "reverse sort order"),
            ("c", "configure columns"),
            ("c then d", "restore compact columns"),
            ("C", "switch container connector"),
            ("S", "save current top configuration"),
            ("K", "terminate process or stop container"),
            ("Esc", "clear filter or close panel/focus"),
            ("q", "quit"),
        ];
        let mut section = HelpSection::new("Keys");
        for (key, action) in rows {
            section.add_row(key, action);
        }
        HelpPanel::without_title()
            .section(section)
            .key_width(18)
            .indent(0)
            .gap(2)
            .section_color(Color::BrightBlack)
            .key_color(Color::BrightWhite)
            .description_color(Color::BrightBlack)
            .view(self.width, self.visible_height() + 2)
    }

    fn sort_panel_view(&self) -> String {
        let Some(panel) = &self.sort_panel else {
            return String::new();
        };
        sort_panel_lines(panel, self.width, self.visible_height()).join("\n")
    }

    fn connector_panel_view(&self) -> String {
        let Some(panel) = &self.connector_panel else {
            return String::new();
        };
        connector_panel_lines(panel, self.width, self.visible_height()).join("\n")
    }

    fn events_table(&self, rows: Vec<EventRow>) -> String {
        let mut table = DataTable::new(vec![
            self.configured_column("events.time", DataColumn::new("TIME").width(9)),
            self.configured_column("events.source", DataColumn::new("SOURCE").width(12)),
            self.configured_column("events.session", DataColumn::new("SESSION").width(12)),
            self.configured_column("events.task", DataColumn::new("TASK").width(10)),
            self.configured_column(
                "events.pid",
                DataColumn::new("PID").width(7).align(CellAlign::Right),
            ),
            self.configured_column(
                "events.ppid",
                DataColumn::new("PPID").width(7).align(CellAlign::Right),
            ),
            self.configured_column("events.kind", DataColumn::new("KIND").width(12)),
            self.configured_column("events.risk", DataColumn::new("RISK").width(5)),
            self.configured_column("events.message", DataColumn::new("MESSAGE").min_width(18)),
        ])
        .header_fg(Color::BrightBlack)
        .separator_fg(Color::BrightBlack)
        .selected((!rows.is_empty()).then_some(self.selected))
        .scroll(self.scroll)
        .empty("no events yet - container changes appear live; agent logs are auto-discovered when present");

        for row in rows {
            table.add_row(
                DataRow::new(vec![
                    row.ts,
                    row.source,
                    row.session.unwrap_or_else(|| "-".to_string()),
                    row.task.unwrap_or_else(|| "-".to_string()),
                    format_event_pid(row.pid),
                    format_event_pid(row.ppid),
                    row.kind,
                    row.risk.label().to_string(),
                    row.message,
                ])
                .fg(row.risk.color()),
            );
        }
        table.view(self.width, self.visible_height() + 2)
    }

    fn details(&self) -> String {
        if !self.detail {
            return String::new();
        }
        let mut lines = Vec::new();
        lines.push(divider_line(self.width));
        match self.tab {
            Tab::Agents if self.focused_agent_pid.is_none() => {
                if let Some(group) = self.current_agent_group() {
                    let activity = group.activity.clone();
                    let usage = group.usage;
                    lines.push(Style::new().fg(group.agent.color()).bold().render(&format!(
                        " agent {} · state {} · risk {}",
                        group.agent.label(),
                        agent_tree_state_label(&group),
                        group.risk().label()
                    )));
                    lines.push(format!(
                        " resources cpu {:.1}% · mem {:.1}% · system processes {} · agent processes {} · children {}",
                        usage.cpu_pct,
                        usage.mem_pct,
                        self.agent_system_processes(&group).len(),
                        group.processes.len(),
                        usage.descendants
                    ));
                    lines.push(format!(
                        " activity sessions {} · events {} · tools {} · sec {} · files {} · net {} · llm {} · tokens {} · model {} · provider {} · latency {} · high {}",
                        group.sessions.len(),
                        activity.events,
                        activity.tools,
                        activity.security,
                        activity.files,
                        activity.egress,
                        activity.llm,
                        format_count(activity.total_tokens),
                        display_model(&activity.model),
                        display_provider(&activity.provider),
                        format_avg_ms(activity.latency_ms, activity.latency_samples),
                        activity.high_risk
                    ));
                    if let Some(session) = group.sessions.first() {
                        lines.push(format!(
                            " top session {} · task {} · cwd {} · risk {}",
                            session.session,
                            session.task,
                            session.workspace,
                            session.risk.label()
                        ));
                    }
                    if let Some(process) = group.processes.first() {
                        lines.push(format!(
                            " top process pid {} · elapsed {} · cwd {}",
                            process.pid,
                            process.elapsed,
                            display_workspace(process.cwd.as_deref())
                        ));
                    }
                    if let Some(event) = group.events.first() {
                        lines.push(format!(
                            " latest event {} · {} · {}",
                            event.kind,
                            event_scope_label(event),
                            event.message
                        ));
                    }
                    lines.push(
                        " actions o focus top process/session · / filter · ! risk · g kind"
                            .to_string(),
                    );
                }
            }
            Tab::Agents | Tab::Processes => {
                if let Some(row) = self.current_process() {
                    lines.push(
                        Style::new()
                            .fg(row.agent.map(|a| a.color()).unwrap_or(ACCENT))
                            .bold()
                            .render(&format!(
                                " process {} · ppid {} · risk {}",
                                row.pid,
                                row.ppid,
                                row.risk.label()
                            )),
                    );
                    lines.push(format!(
                        " cpu {:.1}% · mem {:.1}% · elapsed {} · children {}",
                        row.cpu_pct,
                        row.mem_pct,
                        row.elapsed,
                        self.descendant_count(row.pid)
                    ));
                    lines.push(format!(
                        " agent {}",
                        row.agent.map(|a| a.label()).unwrap_or("none")
                    ));
                    if row.agent.is_some() {
                        let activity = self.agent_activity_for_process(&row);
                        lines.push(format!(
                            " activity events {} · sessions {} · tools {} · sec {} · files {} · net {} · llm {} · tokens {} · model {} · provider {} · latency {} · high {}",
                            activity.events,
                            activity.sessions,
                            activity.tools,
                            activity.security,
                            activity.files,
                            activity.egress,
                            activity.llm,
                            format_count(activity.total_tokens),
                            display_model(&activity.model),
                            display_provider(&activity.provider),
                            format_avg_ms(activity.latency_ms, activity.latency_samples),
                            activity.high_risk
                        ));
                        for event in self.recent_agent_events_for_process(&row) {
                            lines.push(format!(
                                " event {} · {} · {} · {}",
                                event.kind,
                                event.risk.label(),
                                event_scope_label(&event),
                                event.message
                            ));
                        }
                    }
                    lines.push(format!(" cwd {}", display_workspace(row.cwd.as_deref())));
                    lines.push(format!(" command {}", row.command));
                }
            }
            Tab::Sessions => {
                if self.focused_session.is_some() {
                    if let Some(row) = self.current_event() {
                        self.push_event_detail(&row, &mut lines);
                    }
                } else if let Some(row) = self.current_session() {
                    lines.push(
                        Style::new()
                            .fg(row.risk.color())
                            .bold()
                            .render(&format!(" session {} · {}", row.source, row.session)),
                    );
                    lines.push(format!(" task {}", row.task));
                    lines.push(format!(" cwd {}", row.workspace));
                    lines.push(format!(
                        " activity events {} · tools {} · sec {} · files {} · net {} · llm {} · tokens {} · model {} · provider {} · latency {} · high {} · risk {}",
                        row.events,
                        row.tools,
                        row.security,
                        row.files,
                        row.egress,
                        row.llm,
                        format_count(row.total_tokens),
                        display_model(&row.model),
                        display_provider(&row.provider),
                        format_avg_ms(row.latency_ms, row.latency_samples),
                        row.high_risk,
                        row.risk.label()
                    ));
                    lines.push(format!(" latest {} · {}", row.last_kind, row.last_message));
                }
            }
            Tab::Containers => {
                if let Some(row) = self.current_container() {
                    let panel =
                        DetailPanel::new(format!("container {} ({})", row.name, short_id(&row.id)))
                            .show_separator(false)
                            .unlimited_rows()
                            .title_color(CYAN)
                            .value_color(Color::BrightWhite)
                            .action_color(ACCENT)
                            .text(format!("image {}", row.image))
                            .text(format!("status {}", row.status))
                            .text(format!(
                                "cpu {} · mem {} · net {} · block {} · pids {} · ports {}",
                                row.cpu_pct
                                    .map(|v| format!("{v:.1}%"))
                                    .unwrap_or_else(|| "-".to_string()),
                                row.mem_usage,
                                row.net_io,
                                row.block_io,
                                row.pids,
                                display_ports(&row.ports)
                            ))
                            .action("Enter menu · l logs · e shell · o focus · K stop");
                    lines.extend(panel.view(self.width, 5).lines().map(ToString::to_string));
                }
            }
            Tab::Events => {
                if let Some(row) = self.current_event() {
                    self.push_event_detail(&row, &mut lines);
                }
            }
        }
        lines
            .into_iter()
            .take(10)
            .map(|line| pad_line(&line, self.width as usize))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn push_event_detail(&self, row: &EventRow, lines: &mut Vec<String>) {
        let mut panel = DetailPanel::new(format!(
            "event {} · {} · risk {}",
            row.source,
            row.kind,
            row.risk.label()
        ))
        .show_separator(false)
        .unlimited_rows()
        .title_color(row.risk.color())
        .value_color(Color::BrightWhite)
        .action_color(ACCENT);

        panel = panel.text(format!("time {} · {}", row.ts, event_scope_label(row)));
        panel = panel.text(format!(
            "source {} · session {} · task {}",
            row.source,
            row.session.as_deref().unwrap_or("-"),
            row.task.as_deref().unwrap_or("-")
        ));
        if row.pid.is_some() || row.ppid.is_some() {
            panel = panel.text(format!(
                "process pid {} · ppid {}",
                row.pid
                    .map(|pid| pid.to_string())
                    .unwrap_or_else(|| "-".to_string()),
                row.ppid
                    .map(|pid| pid.to_string())
                    .unwrap_or_else(|| "-".to_string())
            ));
        }
        panel = panel.text(format!("message {}", row.message));
        for (key, value) in row.details.iter().take(4) {
            let budget = (self.width as usize)
                .saturating_sub(key.chars().count() + " detail  ".len())
                .max(12);
            panel = panel.text(format!("detail {key} {}", truncate(value, budget)));
        }
        if session_key_for_event(row).is_some() {
            panel = panel.action("o session focus · / filter · ! risk · g kind");
        } else {
            panel = panel.action("/ filter · ! risk · g kind");
        }
        lines.extend(panel.view(self.width, 9).lines().map(ToString::to_string));
    }

    fn confirm_view(&self) -> String {
        let Some(action) = &self.confirm else {
            return String::new();
        };
        let (title, target) = match action {
            Action::KillProcess(pid, label) => {
                ("Terminate process?", format!("PID {pid} · {label}"))
            }
            Action::StartContainer(_, _, name) => ("Start container?", name.clone()),
            Action::StopContainer(_, _, name) => ("Stop container?", name.clone()),
            Action::RestartContainer(_, _, name) => ("Restart container?", name.clone()),
            Action::PauseContainer(_, _, name) => ("Pause container?", name.clone()),
            Action::UnpauseContainer(_, _, name) => ("Unpause container?", name.clone()),
            Action::RemoveContainer(_, _, name) => ("Remove container?", name.clone()),
        };
        Confirm::new(target)
            .title(title)
            .with_labels("y", "n")
            .max_width(58)
            .colors(Color::BrightWhite, Some(RED))
            .box_view(self.width)
    }
}

impl Model for TopApp {
    type Msg = Msg;

    fn init(&mut self) -> Option<Cmd<Msg>> {
        Some(refresh_cmd(
            self.connector,
            self.show_all_containers,
            self.focused_container.clone(),
            self.observer.clone(),
        ))
    }

    fn update(&mut self, msg: Msg) -> Option<Cmd<Msg>> {
        match msg {
            Msg::Snapshot {
                connector,
                snapshot,
                observer,
            } => {
                self.apply_snapshot(snapshot, observer, connector);
                let mut cmds = vec![cmd::tick(self.interval, Msg::Tick)];
                if let Some(cmd) = self.focused_container_process_refresh_cmd() {
                    cmds.push(cmd);
                }
                if let Some(cmd) = self.open_log_refresh_cmd() {
                    cmds.push(cmd);
                }
                Some(single_or_batch(cmds))
            }
            Msg::Tick => {
                if self.paused {
                    Some(cmd::tick(self.interval, Msg::Tick))
                } else {
                    Some(refresh_cmd(
                        self.connector,
                        self.show_all_containers,
                        self.focused_container.clone(),
                        self.observer.clone(),
                    ))
                }
            }
            Msg::ActionDone(note) => {
                self.note = Some(note);
                Some(refresh_cmd(
                    self.connector,
                    self.show_all_containers,
                    self.focused_container.clone(),
                    self.observer.clone(),
                ))
            }
            Msg::ContainerLogs {
                connector,
                id,
                name,
                timestamps,
                result,
            } => {
                if self.log.as_ref().map(|log| log.container_id.as_str()) != Some(id.as_str()) {
                    return None;
                }
                let follow = self.log.as_ref().map(|log| log.follow).unwrap_or(true);
                let previous_scroll = self.log.as_ref().map(|log| log.scroll).unwrap_or_default();
                let text = match result {
                    Ok(text) => text,
                    Err(err) => format!("{} logs failed: {err}", connector.label()),
                };
                let max_scroll = text
                    .lines()
                    .count()
                    .saturating_sub(self.log_visible_height());
                self.log = Some(LogPanel {
                    connector,
                    container_id: id,
                    container_name: name,
                    text,
                    scroll: if follow {
                        max_scroll
                    } else {
                        previous_scroll.min(max_scroll)
                    },
                    timestamps,
                    loading: false,
                    refreshing: false,
                    follow,
                });
                None
            }
            Msg::ContainerProcesses { id, name, result } => {
                if self.focused_container.as_deref() != Some(id.as_str()) {
                    return None;
                }
                let (rows, error) = match result {
                    Ok(rows) => (rows, None),
                    Err(err) => (Vec::new(), Some(err)),
                };
                let scroll = self
                    .container_processes
                    .as_ref()
                    .filter(|panel| panel.container_id == id)
                    .map(|panel| panel.scroll)
                    .unwrap_or_default();
                self.container_processes = Some(ContainerProcessPanel {
                    container_id: id,
                    container_name: name,
                    rows,
                    scroll,
                    error,
                    loading: false,
                });
                None
            }
            Msg::ConfigSaved(result) => {
                self.note = Some(match result {
                    Ok(path) => format!("saved top config to {}", path.display()),
                    Err(err) => format!("failed to save top config: {err}"),
                });
                None
            }
            Msg::Term(Event::Resize { width, height }) => {
                self.width = width;
                self.height = height;
                self.clamp_selection();
                None
            }
            Msg::Term(Event::Key(key)) => self.handle_key(key),
            Msg::Term(_) => None,
        }
    }

    fn view(&self) -> String {
        let mut body = Vec::new();
        if self.show_header {
            body.push(self.header());
        }
        body.push(self.tabs());
        body.push(self.table());
        let details = if self.help
            || self.column_panel.is_some()
            || self.sort_panel.is_some()
            || self.connector_panel.is_some()
            || self.log.is_some()
            || self.container_menu.is_some()
            || (self.tab == Tab::Containers && self.focused_container.is_some())
            || (self.tab == Tab::Agents && self.focused_agent_pid.is_some())
        {
            String::new()
        } else {
            self.details()
        };
        if !details.is_empty() {
            body.push(details);
        }
        if let Some(note) = &self.note {
            body.push(Style::new().fg(YELLOW).render(&format!(" {note}")));
        } else if !self.snapshot.errors.is_empty() {
            body.push(
                Style::new()
                    .fg(YELLOW)
                    .render(&format!(" {}", self.snapshot.errors.join(" · "))),
            );
        }

        let main = body.join("\n");
        let help = self.footer_help_text();
        let sort_dir = if self.reverse_sort { "↑" } else { "↓" };
        let scope = if self.show_all_containers {
            format!("{}:all", self.connector.label())
        } else {
            format!("{}:running", self.connector.label())
        };
        let observer = observer_status_label(&self.observer);
        let status = StatusBar::new()
            .left(format!(
                " {} · sort:{}{} · risk:{} · kind:{} · {} · {} · {} rows",
                self.tab.label(),
                self.sort_by.label(),
                sort_dir,
                self.risk_filter.label(),
                self.kind_filter.label(),
                scope,
                observer,
                self.visible_len()
            ))
            .center(help)
            .right(if self.paused { "paused" } else { "live" })
            .fg(Color::BrightWhite)
            .bg(Color::Rgb(35, 40, 60))
            .view(self.width);

        let mut screen = Layout::vertical()
            .item(&main, Constraint::Fill)
            .item(&status, Constraint::Fixed(1))
            .render(self.height);

        let confirm = self.confirm_view();
        if !confirm.is_empty() {
            screen.push('\n');
            screen.push_str(&confirm);
        }
        if self.invert_colors {
            invert_screen(&screen)
        } else {
            screen
        }
    }
}

impl TopApp {
    fn footer_help_text(&self) -> &'static str {
        if self.column_panel.is_some() {
            "Space toggle · Enter apply · d compact · Esc cancel"
        } else if self.sort_panel.is_some() {
            "Enter apply sort · ↑/↓ select · Esc cancel"
        } else if self.connector_panel.is_some() {
            "Enter switch connector · ↑/↓ select · Esc cancel"
        } else if self.log.is_some() {
            "Esc close logs · ↑/↓ scroll · r refresh · t timestamps · f follow"
        } else if self.container_menu.is_some() {
            "Enter run action · ↑/↓ select · Esc close menu"
        } else if self.help {
            "h/Esc close help"
        } else if self.tab == Tab::Containers && self.focused_container.is_some() {
            "Esc list · ↑/↓ proc · Enter actions(start/stop/restart/pause) · l logs · e shell · w browser · K stop"
        } else if self.tab == Tab::Sessions && self.focused_session.is_some() {
            "Esc close session · ↑/↓ event · / filter · q quit"
        } else if self.editing_filter {
            "type filter · Enter apply · Esc cancel · Ctrl+U clear · Ctrl+W word"
        } else if self.tab == Tab::Agents {
            "↑/↓ agent · o focus process/session · x details · / filter · Tab containers · q quit"
        } else {
            "Tab switch · / filter · ! risk · g kind · C connector · h help · Enter menu · c columns · S save · o focus · l logs · q quit"
        }
    }

    fn handle_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        if self.editing_filter {
            return self.handle_filter_key(key);
        }
        if self.confirm.is_some() {
            return self.handle_confirm_key(key);
        }
        if self.help {
            return self.handle_help_key(key);
        }
        if self.container_menu.is_some() {
            return self.handle_container_menu_key(key);
        }
        if self.sort_panel.is_some() {
            return self.handle_sort_key(key);
        }
        if self.connector_panel.is_some() {
            return self.handle_connector_key(key);
        }
        if self.column_panel.is_some() {
            return self.handle_column_key(key);
        }
        if self.log.is_some() {
            return self.handle_log_key(key);
        }
        if key.code == KeyCode::Esc {
            if self.focused_container.take().is_some() {
                self.container_processes = None;
                self.reset_position();
                return None;
            }
            if self.focused_agent_pid.take().is_some() {
                self.reset_position();
                return None;
            }
            if self.focused_session.take().is_some() {
                self.reset_position();
                return None;
            }
            if self.detail {
                self.detail = false;
                return None;
            }
        }
        if key.modifiers.is_empty() && self.tab == Tab::Containers {
            match key.code {
                KeyCode::Right => return self.toggle_container_focus(),
                KeyCode::Left => return self.open_container_logs(),
                _ => {}
            }
        }
        if key.modifiers.is_empty()
            && self.tab == Tab::Containers
            && self.focused_container.is_some()
            && self.handle_focused_container_scroll_key(&key)
        {
            return None;
        }

        let action = self.keymap.resolve(&key);
        match action {
            Some(TopKey::Quit) => Some(cmd::quit()),
            Some(TopKey::Up) => {
                self.selected = self.selected.saturating_sub(1);
                self.clamp_selection();
                None
            }
            Some(TopKey::Down) => {
                self.selected = (self.selected + 1).min(self.visible_len().saturating_sub(1));
                self.clamp_selection();
                None
            }
            Some(TopKey::Home) => {
                self.selected = 0;
                self.clamp_selection();
                None
            }
            Some(TopKey::End) => {
                self.selected = self.visible_len().saturating_sub(1);
                self.clamp_selection();
                None
            }
            Some(TopKey::PageUp) => {
                self.selected = self.selected.saturating_sub(10);
                self.clamp_selection();
                None
            }
            Some(TopKey::PageDown) => {
                self.selected = (self.selected + 10).min(self.visible_len().saturating_sub(1));
                self.clamp_selection();
                None
            }
            Some(TopKey::NextTab) => {
                self.tab = self.tab.next();
                self.reset_position();
                None
            }
            Some(TopKey::PrevTab) => {
                self.tab = self.tab.prev();
                self.reset_position();
                None
            }
            Some(TopKey::Filter) => {
                self.filter_before_edit = Some(self.filter.clone());
                self.editing_filter = true;
                None
            }
            Some(TopKey::Sort) => {
                self.open_sort_panel();
                None
            }
            Some(TopKey::TogglePause) => {
                self.paused = !self.paused;
                None
            }
            Some(TopKey::ToggleAll) => {
                self.show_all_containers = !self.show_all_containers;
                self.reset_position();
                Some(refresh_cmd(
                    self.connector,
                    self.show_all_containers,
                    self.focused_container.clone(),
                    self.observer.clone(),
                ))
            }
            Some(TopKey::ToggleHeader) => {
                self.show_header = !self.show_header;
                None
            }
            Some(TopKey::ToggleReverse) => {
                self.reverse_sort = !self.reverse_sort;
                self.clamp_selection();
                None
            }
            Some(TopKey::ToggleRiskFilter) => {
                self.risk_filter = self.risk_filter.next();
                self.reset_position();
                None
            }
            Some(TopKey::ToggleKindFilter) => {
                self.kind_filter = self.kind_filter.next();
                self.reset_position();
                None
            }
            Some(TopKey::Detail) => {
                if self.tab == Tab::Containers && key.code == KeyCode::Enter {
                    self.open_container_menu();
                } else {
                    self.detail = !self.detail;
                }
                None
            }
            Some(TopKey::Open) => {
                match self.tab {
                    Tab::Containers => return self.toggle_container_focus(),
                    Tab::Agents | Tab::Processes => self.toggle_agent_focus(),
                    Tab::Sessions => self.toggle_session_focus(),
                    Tab::Events => self.open_event_focus(),
                }
                None
            }
            Some(TopKey::Logs) => self.open_container_logs(),
            Some(TopKey::OpenBrowser) => self.open_container_browser(),
            Some(TopKey::Columns) => {
                self.open_column_panel();
                None
            }
            Some(TopKey::Connector) => {
                self.open_connector_panel();
                None
            }
            Some(TopKey::ExecShell) => self.open_container_shell(),
            Some(TopKey::SaveConfig) => Some(save_config_cmd(self.current_config())),
            Some(TopKey::Help) => {
                self.help = true;
                None
            }
            Some(TopKey::Kill) => {
                self.confirm = self.selected_action();
                None
            }
            None => None,
        }
    }

    fn handle_focused_container_scroll_key(&mut self, key: &KeyEvent) -> bool {
        let step = self.visible_height().saturating_div(2).max(1);
        let Some(panel) = &mut self.container_processes else {
            return false;
        };
        let max_scroll = panel.rows.len();
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                panel.scroll = panel.scroll.saturating_sub(1);
                true
            }
            KeyCode::Down | KeyCode::Char('j') => {
                panel.scroll = (panel.scroll + 1).min(max_scroll);
                true
            }
            KeyCode::PageUp => {
                panel.scroll = panel.scroll.saturating_sub(step);
                true
            }
            KeyCode::PageDown => {
                panel.scroll = (panel.scroll + step).min(max_scroll);
                true
            }
            KeyCode::Home => {
                panel.scroll = 0;
                true
            }
            KeyCode::End => {
                panel.scroll = max_scroll;
                true
            }
            _ => false,
        }
    }

    fn handle_help_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        if matches!(self.keymap.resolve(&key), Some(TopKey::Quit)) {
            return Some(cmd::quit());
        }
        match key.code {
            KeyCode::Esc | KeyCode::Char('h') => {
                self.help = false;
                None
            }
            _ => None,
        }
    }

    fn handle_container_menu_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        if matches!(self.keymap.resolve(&key), Some(TopKey::Quit)) {
            return Some(cmd::quit());
        }
        if key.code == KeyCode::Esc {
            self.container_menu = None;
            return None;
        }

        let Some(menu) = &mut self.container_menu else {
            return None;
        };
        if let KeyCode::Char(ch) = key.code {
            let ch = ch.to_ascii_lowercase();
            if let Some(item) = menu.items.iter().find(|item| item.key == ch).cloned() {
                let container = menu.container.clone();
                self.container_menu = None;
                return self.run_container_menu_action(container, item.action);
            }
        }
        match menu.select.handle_key(&key) {
            Some(SelectMsg::Selected(idx, _)) => {
                let item = menu.items.get(idx).cloned()?;
                let container = menu.container.clone();
                self.container_menu = None;
                self.run_container_menu_action(container, item.action)
            }
            None => None,
        }
    }

    fn handle_column_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        if matches!(self.keymap.resolve(&key), Some(TopKey::Quit)) {
            return Some(cmd::quit());
        }
        if key.code == KeyCode::Esc {
            self.column_panel = None;
            return None;
        }
        if matches!(key.code, KeyCode::Char('d') | KeyCode::Char('D')) {
            if let Some(tab) = self.column_panel.as_ref().map(|panel| panel.tab) {
                self.reset_tab_columns_to_default(tab);
                self.column_panel = None;
            }
            return None;
        }

        let Some(panel) = &mut self.column_panel else {
            return None;
        };
        match panel.select.handle_key(&key) {
            Some(MultiSelectMsg::Submit(indices)) => {
                if indices.is_empty() {
                    self.note = Some("at least one column must stay visible".to_string());
                    return None;
                }

                let visible: HashSet<usize> = indices.into_iter().collect();
                for (idx, choice) in panel.choices.iter().enumerate() {
                    if visible.contains(&idx) {
                        self.hidden_columns.remove(choice.id);
                    } else {
                        self.hidden_columns.insert(choice.id.to_string());
                    }
                }
                self.column_panel = None;
                None
            }
            Some(MultiSelectMsg::Toggle(_)) | None => None,
        }
    }

    fn handle_sort_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        if matches!(self.keymap.resolve(&key), Some(TopKey::Quit)) {
            return Some(cmd::quit());
        }
        if key.code == KeyCode::Esc {
            self.sort_panel = None;
            return None;
        }

        let selected = {
            let Some(panel) = &mut self.sort_panel else {
                return None;
            };
            match panel.select.handle_key(&key) {
                Some(SelectMsg::Selected(idx, _)) => panel.choices.get(idx).copied(),
                None => return None,
            }
        };

        if let Some(sort_by) = selected {
            self.sort_by = sort_by;
            self.clamp_selection();
        }
        self.sort_panel = None;
        None
    }

    fn handle_connector_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        if matches!(self.keymap.resolve(&key), Some(TopKey::Quit)) {
            return Some(cmd::quit());
        }
        if key.code == KeyCode::Esc {
            self.connector_panel = None;
            return None;
        }

        let selected = {
            let Some(panel) = &mut self.connector_panel else {
                return None;
            };
            match panel.select.handle_key(&key) {
                Some(SelectMsg::Selected(idx, _)) => panel.choices.get(idx).copied(),
                None => return None,
            }
        };

        self.connector_panel = None;
        if let Some(connector) = selected {
            self.connector = connector;
            self.focused_container = None;
            self.container_processes = None;
            self.container_menu = None;
            self.log = None;
            self.reset_position();
            return Some(refresh_cmd(
                self.connector,
                self.show_all_containers,
                self.focused_container.clone(),
                self.observer.clone(),
            ));
        }
        None
    }

    fn handle_filter_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        match key.code {
            KeyCode::Esc => {
                self.filter_before_edit = None;
                self.filter.clear();
                self.editing_filter = false;
                self.reset_position();
            }
            KeyCode::Enter => {
                self.filter_before_edit = None;
                self.editing_filter = false;
                self.reset_position();
            }
            KeyCode::Backspace => {
                self.filter.pop();
                self.reset_position();
            }
            KeyCode::Char('u') | KeyCode::Char('U')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.filter.clear();
                self.reset_position();
            }
            KeyCode::Char('w') | KeyCode::Char('W')
                if key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                delete_filter_word(&mut self.filter);
                self.reset_position();
            }
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.filter.push(c);
                self.reset_position();
            }
            _ => {}
        }
        None
    }

    fn handle_log_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        if matches!(self.keymap.resolve(&key), Some(TopKey::Quit)) {
            return Some(cmd::quit());
        }

        let max_scroll = self
            .log
            .as_ref()
            .map(|log| {
                log.text
                    .lines()
                    .count()
                    .saturating_sub(self.log_visible_height())
            })
            .unwrap_or_default();

        match key.code {
            KeyCode::Esc => {
                self.log = None;
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(log) = &mut self.log {
                    log.scroll = log.scroll.saturating_sub(1);
                    log.follow = false;
                }
                None
            }
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(log) = &mut self.log {
                    log.scroll = (log.scroll + 1).min(max_scroll);
                    log.follow = log.scroll >= max_scroll;
                }
                None
            }
            KeyCode::PageUp => {
                let step = self.log_visible_height();
                if let Some(log) = &mut self.log {
                    log.scroll = log.scroll.saturating_sub(step);
                    log.follow = false;
                }
                None
            }
            KeyCode::PageDown => {
                let step = self.log_visible_height();
                if let Some(log) = &mut self.log {
                    log.scroll = (log.scroll + step).min(max_scroll);
                    log.follow = log.scroll >= max_scroll;
                }
                None
            }
            KeyCode::Home => {
                if let Some(log) = &mut self.log {
                    log.scroll = 0;
                    log.follow = false;
                }
                None
            }
            KeyCode::End => {
                if let Some(log) = &mut self.log {
                    log.scroll = max_scroll;
                    log.follow = true;
                }
                None
            }
            KeyCode::Char('r') | KeyCode::Char('R') => self.open_log_refresh_cmd(),
            KeyCode::Char('f') | KeyCode::Char('F') => {
                if let Some(log) = &mut self.log {
                    if log.follow {
                        log.follow = false;
                    } else {
                        log.scroll = max_scroll;
                        log.follow = true;
                    }
                }
                None
            }
            KeyCode::Char('t') => {
                let Some(log) = &mut self.log else {
                    return None;
                };
                log.timestamps = !log.timestamps;
                log.loading = true;
                log.refreshing = true;
                log.follow = true;
                log.text.clear();
                Some(container_logs_cmd(
                    log.connector,
                    log.container_id.clone(),
                    log.container_name.clone(),
                    log.timestamps,
                ))
            }
            _ => None,
        }
    }

    fn handle_confirm_key(&mut self, key: KeyEvent) -> Option<Cmd<Msg>> {
        match key.code {
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') => {
                self.confirm = None;
                None
            }
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => {
                let action = self.confirm.take()?;
                Some(cmd::cmd(move || async move {
                    Msg::ActionDone(run_action(action).await)
                }))
            }
            _ => None,
        }
    }

    fn toggle_container_focus(&mut self) -> Option<Cmd<Msg>> {
        let Some(container) = self.current_container() else {
            self.note = Some("select a container before opening focus view".to_string());
            return None;
        };
        if self.focused_container.as_deref() == Some(container.id.as_str()) {
            self.focused_container = None;
            self.container_processes = None;
            self.reset_position();
            None
        } else {
            self.focused_container = Some(container.id.clone());
            self.reset_position();
            self.detail = false;
            self.start_container_process_refresh(container)
        }
    }

    fn start_container_process_refresh(&mut self, container: ContainerRow) -> Option<Cmd<Msg>> {
        let (rows, scroll) = self
            .container_processes
            .as_ref()
            .filter(|panel| panel.container_id == container.id)
            .map(|panel| (panel.rows.clone(), panel.scroll))
            .unwrap_or_default();
        self.container_processes = Some(ContainerProcessPanel {
            container_id: container.id.clone(),
            container_name: container.name.clone(),
            rows,
            scroll,
            error: None,
            loading: true,
        });
        Some(container_processes_cmd(container))
    }

    fn focused_container_process_refresh_cmd(&mut self) -> Option<Cmd<Msg>> {
        let container = self.current_container()?;
        if self
            .container_processes
            .as_ref()
            .is_some_and(|panel| panel.container_id == container.id && panel.loading)
        {
            return None;
        }
        self.start_container_process_refresh(container)
    }

    fn toggle_agent_focus(&mut self) {
        if self.tab == Tab::Agents && self.focused_agent_pid.is_none() {
            let Some(group) = self.current_agent_group() else {
                self.note = Some("select an agent before opening focus view".to_string());
                return;
            };
            if let Some(process) = group.processes.first() {
                self.focused_agent_pid = Some(process.pid);
                self.reset_position();
                self.detail = false;
                return;
            }
            if let Some(session) = group.sessions.first() {
                self.focused_session = Some(SessionFocus::from_row(session));
                self.tab = Tab::Sessions;
                self.reset_position();
                self.detail = false;
                return;
            }
            self.note = Some(format!(
                "{} has no live process or session to focus",
                group.agent.label()
            ));
            return;
        }

        let Some(process) = self.current_process() else {
            self.note = Some("select an agent before opening focus view".to_string());
            return;
        };
        if process.agent.is_none() {
            self.note = Some("focus view is available for coding-agent processes".to_string());
            return;
        }
        if self.focused_agent_pid == Some(process.pid) {
            self.focused_agent_pid = None;
            self.reset_position();
        } else {
            self.focused_agent_pid = Some(process.pid);
            self.tab = Tab::Agents;
            self.reset_position();
            self.detail = false;
        }
    }

    fn toggle_session_focus(&mut self) {
        if self.focused_session.take().is_some() {
            self.reset_position();
            return;
        }

        let Some(row) = self.current_session() else {
            self.note = Some("select a session before opening focus view".to_string());
            return;
        };
        self.focused_session = Some(SessionFocus::from_row(&row));
        self.reset_position();
        self.detail = false;
    }

    fn open_event_focus(&mut self) {
        let Some(event) = self.current_event() else {
            self.note = Some("select an event before opening focus view".to_string());
            return;
        };
        let Some((source, session)) = session_key_for_event(&event) else {
            self.note = Some("event focus is available for coding-agent events".to_string());
            return;
        };
        self.focused_session = Some(SessionFocus { source, session });
        self.tab = Tab::Sessions;
        self.reset_position();
        self.detail = false;
    }

    fn open_container_menu(&mut self) {
        if self.tab != Tab::Containers {
            return;
        }
        let Some(container) = self.current_container() else {
            self.note = Some("select a container before opening the container menu".to_string());
            return;
        };
        let items = container_menu_items(&container);
        let labels = items
            .iter()
            .map(|item| format!("{}  {}", item.key, item.label))
            .collect::<Vec<_>>();
        self.container_menu = Some(ContainerMenu {
            container,
            items,
            select: Select::new(labels),
        });
    }

    fn run_container_menu_action(
        &mut self,
        container: ContainerRow,
        action: ContainerMenuAction,
    ) -> Option<Cmd<Msg>> {
        match action {
            ContainerMenuAction::Focus => {
                self.focused_container = Some(container.id.clone());
                self.reset_position();
                self.detail = false;
                self.start_container_process_refresh(container)
            }
            ContainerMenuAction::Logs => self.open_container_logs_for(container),
            ContainerMenuAction::ExecShell => self.open_container_shell_for(container),
            ContainerMenuAction::OpenBrowser => self.open_container_browser_for(container),
            ContainerMenuAction::Start
            | ContainerMenuAction::Stop
            | ContainerMenuAction::Restart
            | ContainerMenuAction::Pause
            | ContainerMenuAction::Unpause
            | ContainerMenuAction::Remove => {
                self.confirm = Some(container_action(container, action));
                None
            }
        }
    }

    fn open_container_browser(&mut self) -> Option<Cmd<Msg>> {
        if self.tab != Tab::Containers {
            self.note = Some("browser open is available on the Containers tab".to_string());
            return None;
        }
        let Some(container) = self.current_container() else {
            self.note = Some("select a container before opening a browser".to_string());
            return None;
        };
        self.open_container_browser_for(container)
    }

    fn open_container_browser_for(&mut self, container: ContainerRow) -> Option<Cmd<Msg>> {
        let Some(url) = container_web_url(&container) else {
            self.note = Some(format!(
                "{} has no published web port to open",
                container.name
            ));
            return None;
        };
        if let Ok(mut slot) = self.external_action.lock() {
            *slot = Some(ExternalAction::OpenBrowser {
                url,
                name: container.name,
            });
            Some(cmd::quit())
        } else {
            self.note = Some("failed to prepare browser open".to_string());
            None
        }
    }

    fn open_container_logs(&mut self) -> Option<Cmd<Msg>> {
        if self.tab != Tab::Containers {
            self.note = Some("logs are available on the Containers tab".to_string());
            return None;
        }
        let Some(container) = self.current_container() else {
            self.note = Some("select a container before opening logs".to_string());
            return None;
        };
        self.open_container_logs_for(container)
    }

    fn open_container_logs_for(&mut self, container: ContainerRow) -> Option<Cmd<Msg>> {
        if !matches!(
            container.connector,
            ContainerConnector::A3sBox | ContainerConnector::Docker
        ) {
            self.note =
                Some("logs are currently available for a3s-box and Docker containers".to_string());
            return None;
        }
        let timestamps = self.log.as_ref().map(|log| log.timestamps).unwrap_or(false);
        self.log = Some(LogPanel {
            connector: container.connector,
            container_id: container.id.clone(),
            container_name: container.name.clone(),
            text: String::new(),
            scroll: 0,
            timestamps,
            loading: true,
            refreshing: true,
            follow: true,
        });
        Some(container_logs_cmd(
            container.connector,
            container.id,
            container.name,
            timestamps,
        ))
    }

    fn open_log_refresh_cmd(&mut self) -> Option<Cmd<Msg>> {
        let log = self.log.as_mut()?;
        if log.loading || log.refreshing {
            return None;
        }
        log.refreshing = true;
        Some(container_logs_cmd(
            log.connector,
            log.container_id.clone(),
            log.container_name.clone(),
            log.timestamps,
        ))
    }

    fn open_container_shell(&mut self) -> Option<Cmd<Msg>> {
        if self.tab != Tab::Containers {
            self.note = Some("exec shell is available on the Containers tab".to_string());
            return None;
        }
        let Some(container) = self.current_container() else {
            self.note = Some("select a container before opening a shell".to_string());
            return None;
        };
        self.open_container_shell_for(container)
    }

    fn open_container_shell_for(&mut self, container: ContainerRow) -> Option<Cmd<Msg>> {
        if !matches!(
            container.connector,
            ContainerConnector::A3sBox | ContainerConnector::Docker
        ) {
            self.note = Some(
                "exec shell is currently available for a3s-box and Docker containers".to_string(),
            );
            return None;
        }
        if let Ok(mut slot) = self.external_action.lock() {
            *slot = Some(ExternalAction::ContainerShell {
                connector: container.connector,
                id: container.id,
                name: container.name,
            });
            Some(cmd::quit())
        } else {
            self.note = Some("failed to prepare container shell".to_string());
            None
        }
    }

    fn open_sort_panel(&mut self) {
        let choices = sort_choices_for_tab(self.tab);
        if choices.is_empty() {
            self.note = Some("events stay newest-first; use /, !, or g to narrow them".to_string());
            return;
        }
        let labels = choices
            .iter()
            .map(|sort_by| sort_choice_label(*sort_by))
            .collect::<Vec<_>>();
        let selected = choices
            .iter()
            .position(|sort_by| *sort_by == self.sort_by)
            .unwrap_or(0);
        self.sort_panel = Some(SortPanel {
            choices,
            select: Select::new(labels)
                .with_selected(selected)
                .with_number_shortcuts(),
        });
    }

    fn open_connector_panel(&mut self) {
        let choices = vec![
            ContainerConnector::A3sBox,
            ContainerConnector::Docker,
            ContainerConnector::RunC,
        ];
        let labels = choices
            .iter()
            .map(|connector| connector_choice_label(*connector))
            .collect::<Vec<_>>();
        let selected = choices
            .iter()
            .position(|connector| *connector == self.connector)
            .unwrap_or(0);
        self.connector_panel = Some(ConnectorPanel {
            choices,
            select: Select::new(labels)
                .with_selected(selected)
                .with_number_shortcuts(),
        });
    }

    fn open_column_panel(&mut self) {
        let choices = self.column_choices(self.tab);
        let labels = choices.iter().map(|c| c.label).collect::<Vec<_>>();
        let checked = choices
            .iter()
            .map(|c| self.column_visible(c.id))
            .collect::<Vec<_>>();
        self.column_panel = Some(ColumnPanel {
            tab: self.tab,
            choices,
            select: MultiSelect::new(labels)
                .with_checked(checked)
                .with_number_shortcuts(),
        });
    }

    fn reset_tab_columns_to_default(&mut self, tab: Tab) {
        let default_hidden = default_hidden_columns();
        for choice in self.column_choices(tab) {
            if default_hidden.contains(choice.id) {
                self.hidden_columns.insert(choice.id.to_string());
            } else {
                self.hidden_columns.remove(choice.id);
            }
        }
        self.ensure_visible_columns();
    }

    fn current_config(&self) -> TopConfig {
        TopConfig {
            show_all_containers: self.show_all_containers,
            show_header: self.show_header,
            reverse_sort: self.reverse_sort,
            sort_by: self.sort_by,
            risk_filter: self.risk_filter,
            kind_filter: self.kind_filter,
            connector: self.connector,
            filter: self.filter.clone(),
            hidden_columns: self.hidden_columns.clone(),
        }
    }
}

#[derive(Debug, Clone)]
struct TopOptions {
    tab: Tab,
    interval: Duration,
    force_all_containers: bool,
    force_active_containers: bool,
    container_query: Option<String>,
    filter: Option<String>,
    sort_by: Option<SortBy>,
    risk_filter: Option<RiskFilter>,
    kind_filter: Option<KindFilter>,
    connector: Option<ContainerConnector>,
    reverse_sort: Option<bool>,
    show_header: Option<bool>,
    compact_columns: bool,
    json: bool,
    watch: bool,
    json_count: Option<usize>,
    invert_colors: bool,
    start_help: bool,
    config: TopConfig,
    external_action: Arc<Mutex<Option<ExternalAction>>>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MachineOutput {
    Json,
    Jsonl,
}

#[derive(Debug)]
pub(crate) struct TopInterrupted {
    next_sequence: u64,
}

impl TopInterrupted {
    pub(crate) fn next_sequence(&self) -> u64 {
        self.next_sequence
    }
}

impl std::fmt::Display for TopInterrupted {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("top monitoring cancelled")
    }
}

impl std::error::Error for TopInterrupted {}

impl Default for TopOptions {
    fn default() -> Self {
        Self {
            tab: Tab::Agents,
            interval: Duration::from_millis(1500),
            force_all_containers: false,
            force_active_containers: false,
            container_query: None,
            filter: None,
            sort_by: None,
            risk_filter: None,
            kind_filter: None,
            connector: None,
            reverse_sort: None,
            show_header: None,
            compact_columns: false,
            json: false,
            watch: false,
            json_count: None,
            invert_colors: false,
            start_help: false,
            config: TopConfig::default(),
            external_action: Arc::new(Mutex::new(None)),
        }
    }
}

pub async fn run(args: Vec<String>) -> anyhow::Result<()> {
    run_with_output(args, None, tokio_util::sync::CancellationToken::new()).await
}

pub(crate) async fn run_machine(
    args: Vec<String>,
    output: MachineOutput,
    cancellation: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    run_with_output(args, Some(output), cancellation).await
}

async fn run_with_output(
    args: Vec<String>,
    forced_output: Option<MachineOutput>,
    cancellation: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let mut base_options = parse_options(args)?;
    let machine_output = forced_output.or_else(|| base_options.json.then_some(MachineOutput::Json));
    let force_all_containers = base_options.force_all_containers;
    let force_active_containers = base_options.force_active_containers;
    base_options.config = load_top_config().unwrap_or_default();
    apply_cli_overrides(
        &mut base_options,
        force_all_containers,
        force_active_containers,
    );
    if let Some(output) = machine_output {
        run_machine_snapshots(base_options, output, cancellation).await?;
        return Ok(());
    }
    loop {
        let external_action = Arc::new(Mutex::new(None));
        let mut options = base_options.clone();
        options.external_action = external_action.clone();
        ProgramBuilder::new(TopApp::new(options))
            .with_alt_screen()
            .with_fps(30)
            .run()
            .await?;

        let action = external_action.lock().ok().and_then(|mut slot| slot.take());
        let Some(action) = action else {
            break;
        };
        run_external_action(action).await?;
        base_options.config = load_top_config().unwrap_or_default();
        apply_cli_overrides(
            &mut base_options,
            force_all_containers,
            force_active_containers,
        );
    }
    Ok(())
}

fn apply_cli_overrides(
    options: &mut TopOptions,
    force_all_containers: bool,
    force_active_containers: bool,
) {
    if options.container_query.is_some() {
        options.tab = Tab::Containers;
        if !force_all_containers && !force_active_containers {
            options.config.show_all_containers = true;
        }
    }
    if force_all_containers {
        options.config.show_all_containers = true;
    }
    if force_active_containers {
        options.config.show_all_containers = false;
    }
    if let Some(filter) = &options.filter {
        options.config.filter = filter.clone();
    }
    if let Some(sort_by) = options.sort_by {
        options.config.sort_by = sort_by;
    }
    if let Some(risk_filter) = options.risk_filter {
        options.config.risk_filter = risk_filter;
    }
    if let Some(kind_filter) = options.kind_filter {
        options.config.kind_filter = kind_filter;
    }
    if let Some(connector) = options.connector.or_else(env_connector) {
        options.config.connector = connector;
    }
    if let Some(reverse_sort) = options.reverse_sort {
        options.config.reverse_sort = reverse_sort;
    }
    if let Some(show_header) = options.show_header {
        options.config.show_header = show_header;
    }
    if options.compact_columns {
        options.config.hidden_columns = default_hidden_columns();
    }
}

async fn run_machine_snapshots(
    options: TopOptions,
    output: MachineOutput,
    cancellation: tokio_util::sync::CancellationToken,
) -> anyhow::Result<()> {
    let connector = options.config.connector;
    let show_all_containers = options.config.show_all_containers;
    let interval = options.interval;
    let container_target = options.container_query.clone();
    let max_snapshots = match output {
        MachineOutput::Json => 1,
        MachineOutput::Jsonl => options.json_count.unwrap_or(usize::MAX),
    };
    let mut app = TopApp::new(options);
    let mut observer = ObserverState::default();
    let mut emitted = 0usize;

    while emitted < max_snapshots {
        let (snapshot, next_observer) = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(TopInterrupted {
                    next_sequence: emitted as u64 + 1,
                }
                .into());
            }
            snapshot = collect_snapshot(
                connector,
                show_all_containers,
                container_target.clone(),
                observer,
            ) => snapshot,
        };
        observer = next_observer.clone();
        app.apply_snapshot(snapshot, next_observer, connector);

        let data = top_snapshot_json(&app, unix_millis());
        match output {
            MachineOutput::Json => render_value(OutputMode::Json, "top", data, || {})?,
            MachineOutput::Jsonl => write_jsonl(&serde_json::json!({
                "schemaVersion": 1,
                "command": "top",
                "type": "snapshot",
                "sequence": emitted as u64 + 1,
                "data": data,
            }))?,
        }
        emitted += 1;

        if emitted >= max_snapshots {
            break;
        }
        tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                return Err(TopInterrupted {
                    next_sequence: emitted as u64 + 1,
                }
                .into());
            }
            _ = tokio::time::sleep(interval) => {}
        }
    }
    if output == MachineOutput::Jsonl {
        write_jsonl(&serde_json::json!({
            "schemaVersion": 1,
            "command": "top",
            "type": "result",
            "sequence": emitted as u64 + 1,
            "ok": true,
            "data": {"snapshots": emitted},
        }))?;
    }
    Ok(())
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

fn top_snapshot_json(app: &TopApp, collected_at_unix_ms: u128) -> serde_json::Value {
    let agents = app.filtered_agents();
    let sessions = app.filtered_sessions();
    let containers = app.filtered_containers();
    let container_states = container_state_summary(&containers);
    let raw_container_states = container_state_summary(&app.snapshot.containers);
    let processes = app.filtered_processes();
    let events = app.filtered_events();
    let high_events = app
        .snapshot
        .events
        .iter()
        .filter(|event| event.risk == Risk::High)
        .count();
    let total_tokens = app
        .snapshot
        .events
        .iter()
        .filter_map(event_token_usage)
        .map(|tokens| tokens.total)
        .sum::<u64>();

    serde_json::json!({
        "schema": "a3s.top.snapshot.v1",
        "collected_at_unix_ms": collected_at_unix_ms,
        "config": {
            "tab": app.tab.label(),
            "sort_by": app.sort_by.label(),
            "reverse_sort": app.reverse_sort,
            "risk_filter": app.risk_filter.label(),
            "kind_filter": app.kind_filter.label(),
            "connector": app.connector.label(),
            "show_all_containers": app.show_all_containers,
            "filter": &app.filter,
            "container": app.focused_container.as_deref(),
        },
        "observer": {
            "status": observer_status_label(&app.observer),
            "paths": app.observer.paths.iter().map(|path| path.display().to_string()).collect::<Vec<_>>(),
        },
        "summary": {
            "agents": agents.len(),
            "sessions": sessions.len(),
            "containers": containers.len(),
            "container_states": container_state_summary_json(container_states),
            "processes": processes.len(),
            "events": events.len(),
            "raw_processes": app.snapshot.processes.len(),
            "raw_containers": app.snapshot.containers.len(),
            "raw_container_states": container_state_summary_json(raw_container_states),
            "raw_events": app.snapshot.events.len(),
            "high_events": high_events,
            "total_tokens": total_tokens,
            "errors": app.snapshot.errors.len(),
        },
        "agents": agents.iter().map(|row| agent_json(app, row)).collect::<Vec<_>>(),
        "sessions": sessions.iter().map(session_json).collect::<Vec<_>>(),
        "containers": containers
            .iter()
            .map(|row| container_json(app, row))
            .collect::<Vec<_>>(),
        "processes": processes
            .iter()
            .map(|row| process_json(app, row))
            .collect::<Vec<_>>(),
        "events": events.iter().map(event_json).collect::<Vec<_>>(),
        "errors": &app.snapshot.errors,
    })
}

fn agent_json(app: &TopApp, row: &ProcessRow) -> serde_json::Value {
    let usage = app.process_tree_usage(row.pid);
    let activity = app.agent_activity_for_process(row);
    let history = app.metric_history(&agent_tree_history_key(row.pid));
    let sessions = agent_session_rows(app, row);
    let top_session = sessions.first();
    serde_json::json!({
        "pid": row.pid,
        "ppid": row.ppid,
        "agent": row.agent.map(|agent| agent.label()),
        "cpu_pct": row.cpu_pct,
        "mem_pct": row.mem_pct,
        "subtree": {
            "cpu_pct": usage.cpu_pct,
            "mem_pct": usage.mem_pct,
            "descendants": usage.descendants,
        },
        "process_tree": process_tree_json(app, row.pid),
        "history": metric_history_json(&history),
        "elapsed": &row.elapsed,
        "cwd": row.cwd.as_deref(),
        "risk": row.risk.label(),
        "command": &row.command,
        "activity": activity_json(&activity),
        "top_session": top_session.map(|row| row.session.as_str()),
        "top_task": top_session.and_then(|row| empty_dash_to_null(&row.task)),
        "sessions": sessions.iter().map(session_json).collect::<Vec<_>>(),
        "recent_events": app
            .recent_agent_events_for_process(row)
            .iter()
            .map(event_json)
            .collect::<Vec<_>>(),
    })
}

fn agent_session_rows(app: &TopApp, row: &ProcessRow) -> Vec<SessionRow> {
    if row.agent.is_none() {
        return Vec::new();
    }
    let events = app
        .snapshot
        .events
        .iter()
        .filter(|event| app.event_matches_agent_process(row, event))
        .cloned()
        .collect::<Vec<_>>();
    session_rows(&events)
}

fn process_tree_json(app: &TopApp, root_pid: u32) -> serde_json::Value {
    let mut visited = HashSet::new();
    process_tree_node_json(app, root_pid, &mut visited).unwrap_or(serde_json::Value::Null)
}

fn process_tree_node_json(
    app: &TopApp,
    pid: u32,
    visited: &mut HashSet<u32>,
) -> Option<serde_json::Value> {
    if !visited.insert(pid) {
        return None;
    }
    let process = app
        .snapshot
        .processes
        .iter()
        .find(|process| process.pid == pid)?;
    let mut children = app
        .snapshot
        .processes
        .iter()
        .filter(|candidate| candidate.ppid == pid)
        .map(|candidate| candidate.pid)
        .collect::<Vec<_>>();
    children.sort_unstable();
    let children = children
        .into_iter()
        .filter_map(|child_pid| process_tree_node_json(app, child_pid, visited))
        .collect::<Vec<_>>();

    Some(serde_json::json!({
        "pid": process.pid,
        "ppid": process.ppid,
        "cpu_pct": process.cpu_pct,
        "mem_pct": process.mem_pct,
        "elapsed": &process.elapsed,
        "cwd": process.cwd.as_deref(),
        "agent": process.agent.map(|agent| agent.label()),
        "risk": process.risk.label(),
        "command": &process.command,
        "children": children,
    }))
}

fn activity_json(activity: &AgentActivity) -> serde_json::Value {
    serde_json::json!({
        "events": activity.events,
        "sessions": activity.sessions,
        "tools": activity.tools,
        "security": activity.security,
        "files": activity.files,
        "egress": activity.egress,
        "llm": activity.llm,
        "tokens": {
            "prompt": activity.prompt_tokens,
            "completion": activity.completion_tokens,
            "total": activity.total_tokens,
        },
        "model": empty_dash_to_null(&activity.model),
        "provider": empty_dash_to_null(&activity.provider),
        "latency_ms_avg": average_u64(activity.latency_ms, activity.latency_samples),
        "ttft_ms_avg": average_u64(activity.ttft_ms, activity.ttft_samples),
        "wire": {
            "request_bytes": activity.req_bytes,
            "response_bytes": activity.resp_bytes,
        },
        "high_risk": activity.high_risk,
    })
}

fn session_json(row: &SessionRow) -> serde_json::Value {
    serde_json::json!({
        "source": &row.source,
        "session": &row.session,
        "task": empty_dash_to_null(&row.task),
        "workspace": empty_dash_to_null(&row.workspace),
        "events": row.events,
        "tools": row.tools,
        "security": row.security,
        "files": row.files,
        "egress": row.egress,
        "llm": row.llm,
        "tokens": {
            "prompt": row.prompt_tokens,
            "completion": row.completion_tokens,
            "total": row.total_tokens,
        },
        "model": empty_dash_to_null(&row.model),
        "provider": empty_dash_to_null(&row.provider),
        "latency_ms_avg": average_u64(row.latency_ms, row.latency_samples),
        "ttft_ms_avg": average_u64(row.ttft_ms, row.ttft_samples),
        "wire": {
            "request_bytes": row.req_bytes,
            "response_bytes": row.resp_bytes,
        },
        "high_risk": row.high_risk,
        "risk": row.risk.label(),
        "last_kind": &row.last_kind,
        "last_message": &row.last_message,
    })
}

fn container_json(app: &TopApp, row: &ContainerRow) -> serde_json::Value {
    let history = app.metric_history(&container_history_key(&row.id));
    serde_json::json!({
        "connector": row.connector.label(),
        "id": &row.id,
        "cid": short_id(&row.id),
        "short_id": short_id(&row.id),
        "name": &row.name,
        "image": &row.image,
        "status": &row.status,
        "cpu_pct": row.cpu_pct,
        "cpu_count": row.cpu_count,
        "cpu_usage_total_ns": row.cpu_usage_total_ns,
        "mem_pct": row.mem_pct,
        "mem_usage": &row.mem_usage,
        "net_io": &row.net_io,
        "block_io": &row.block_io,
        "pids": &row.pids,
        "ports": &row.ports,
        "history": metric_history_json(&history),
        "inspect": {
            "health": empty_dash_to_null(&row.inspect.health),
            "restarts": empty_dash_to_null(&row.inspect.restarts),
            "restart_policy": empty_dash_to_null(&row.inspect.restart_policy),
            "created": empty_dash_to_null(&row.inspect.created),
            "started": empty_dash_to_null(&row.inspect.started),
            "exit": empty_dash_to_null(&row.inspect.exit),
            "mounts": empty_dash_to_null(&row.inspect.mounts),
            "env": empty_dash_to_null(&row.inspect.env),
            "labels": empty_dash_to_null(&row.inspect.labels),
            "networks": empty_dash_to_null(&row.inspect.networks),
        },
    })
}

fn container_matches_query(container: &ContainerRow, query: &str) -> bool {
    let query = query.trim();
    if query.is_empty() {
        return true;
    }
    container.name == query
        || container.id == query
        || short_id(&container.id) == query
        || container.id.starts_with(query)
        || short_id(&container.id).starts_with(query)
}

fn container_target_filters(target: Option<&str>) -> Vec<Option<String>> {
    let Some(target) = target.map(str::trim).filter(|target| !target.is_empty()) else {
        return vec![None];
    };
    vec![Some(format!("name={target}")), Some(format!("id={target}"))]
}

fn delete_filter_word(filter: &mut String) {
    while filter.chars().last().is_some_and(|ch| ch.is_whitespace()) {
        filter.pop();
    }
    while filter.chars().last().is_some_and(|ch| !ch.is_whitespace()) {
        filter.pop();
    }
}

fn dedupe_container_rows(rows: &mut Vec<ContainerRow>) {
    let mut seen = HashSet::new();
    rows.retain(|row| seen.insert(row.id.clone()));
}

fn process_json(app: &TopApp, row: &ProcessRow) -> serde_json::Value {
    let history = app.metric_history(&process_history_key(row.pid));
    serde_json::json!({
        "pid": row.pid,
        "ppid": row.ppid,
        "cpu_pct": row.cpu_pct,
        "mem_pct": row.mem_pct,
        "history": metric_history_json(&history),
        "elapsed": &row.elapsed,
        "cwd": row.cwd.as_deref(),
        "agent": row.agent.map(|agent| agent.label()),
        "risk": row.risk.label(),
        "command": &row.command,
    })
}

fn metric_history_json(history: &MetricHistory) -> serde_json::Value {
    serde_json::json!({
        "cpu_pct": &history.cpu,
        "mem_pct": &history.mem,
        "net_io_bytes": &history.net_io_bytes,
        "block_io_bytes": &history.block_io_bytes,
    })
}

fn event_json(row: &EventRow) -> serde_json::Value {
    serde_json::json!({
        "ts": &row.ts,
        "source": &row.source,
        "session": row.session.as_deref(),
        "task": row.task.as_deref(),
        "pid": row.pid,
        "ppid": row.ppid,
        "kind": &row.kind,
        "message": &row.message,
        "risk": row.risk.label(),
        "details": row
            .details
            .iter()
            .map(|(key, value)| serde_json::json!({ "key": key, "value": value }))
            .collect::<Vec<_>>(),
    })
}

fn average_u64(total: u64, samples: u64) -> Option<u64> {
    total.checked_div(samples)
}

fn empty_dash_to_null(value: &str) -> Option<&str> {
    let value = value.trim();
    (!value.is_empty() && value != "-").then_some(value)
}

fn parse_options(args: Vec<String>) -> anyhow::Result<TopOptions> {
    let mut options = TopOptions::default();
    let mut it = args.into_iter();
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--agents" => options.tab = Tab::Agents,
            "--sessions" => options.tab = Tab::Sessions,
            "--containers" => options.tab = Tab::Containers,
            "--processes" => options.tab = Tab::Processes,
            "--events" => options.tab = Tab::Events,
            "-a" | "--active" | "--active-only" => {
                options.force_active_containers = true;
                options.force_all_containers = false;
            }
            "--all" => {
                options.force_all_containers = true;
                options.force_active_containers = false;
            }
            "-f" | "--filter" => {
                options.filter = Some(
                    it.next()
                        .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?,
                );
            }
            "-s" | "--sort" => {
                let value = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?;
                options.sort_by = Some(SortBy::from_label(&value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown top sort field '{value}' (expected cpu, mem, net, block, pids, state, id, uptime, name, or tokens)"
                    )
                })?);
            }
            "--risk" => {
                let value = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?;
                options.risk_filter = Some(RiskFilter::from_label(&value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown top risk filter '{value}' (expected all, medium, or high)"
                    )
                })?);
            }
            "--kind" => {
                let value = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?;
                options.kind_filter = Some(KindFilter::from_label(&value).ok_or_else(|| {
                    anyhow::anyhow!(
                        "unknown top event kind filter '{value}' (expected all, tool, security, file, egress, llm, or other)"
                    )
                })?);
            }
            "--connector" => {
                let value = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?;
                options.connector =
                    Some(ContainerConnector::from_label(&value).ok_or_else(|| {
                        anyhow::anyhow!(
                            "unknown top connector '{value}' (expected a3s-box, docker, or runc)"
                        )
                    })?);
            }
            "--container" => {
                let value = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?;
                options.container_query = Some(value);
                options.tab = Tab::Containers;
            }
            "-r" | "--reverse" => options.reverse_sort = Some(true),
            "-i" | "--invert" => options.invert_colors = true,
            "--compact" | "--compact-columns" => options.compact_columns = true,
            "--json" => options.json = true,
            "--count" => {
                let value = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?;
                let count = value.parse::<usize>()?;
                if count == 0 {
                    return Err(anyhow::anyhow!("--count must be greater than zero"));
                }
                options.json_count = Some(count);
            }
            "--no-header" => options.show_header = Some(false),
            "--watch" | "--interval" => {
                let value = it
                    .next()
                    .ok_or_else(|| anyhow::anyhow!("{arg} requires a value"))?;
                options.interval = parse_duration(&value)?;
                options.watch = true;
            }
            "-h" => options.start_help = true,
            "--help" | "-v" | "-V" | "--version" => {
                return Err(anyhow::anyhow!(
                    "help and version flags must be handled by the root CLI parser"
                ));
            }
            other if other.starts_with('-') => {
                return Err(anyhow::anyhow!("unknown a3s top option '{other}'"));
            }
            other => {
                if options.container_query.is_some() {
                    return Err(anyhow::anyhow!("a3s top accepts only one container target"));
                }
                options.container_query = Some(other.to_string());
                options.tab = Tab::Containers;
            }
        }
    }
    Ok(options)
}

fn env_connector() -> Option<ContainerConnector> {
    std::env::var("A3S_TOP_CONNECTOR")
        .ok()
        .and_then(|value| ContainerConnector::from_label(&value))
}

fn top_keymap() -> Keymap<TopKey> {
    let mut km = Keymap::new();
    km.register(KeyBinding::new(KeyCode::Char('q')), TopKey::Quit, "quit");
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('c'), KeyModifiers::CONTROL),
        TopKey::Quit,
        "quit",
    );
    km.register(KeyBinding::new(KeyCode::Up), TopKey::Up, "select up");
    km.register(KeyBinding::new(KeyCode::Char('k')), TopKey::Up, "select up");
    km.register(KeyBinding::new(KeyCode::Down), TopKey::Down, "select down");
    km.register(
        KeyBinding::new(KeyCode::Char('j')),
        TopKey::Down,
        "select down",
    );
    km.register(KeyBinding::new(KeyCode::Home), TopKey::Home, "select first");
    km.register(KeyBinding::new(KeyCode::End), TopKey::End, "select last");
    km.register(KeyBinding::new(KeyCode::PageUp), TopKey::PageUp, "page up");
    km.register(
        KeyBinding::new(KeyCode::PageDown),
        TopKey::PageDown,
        "page down",
    );
    km.register(KeyBinding::new(KeyCode::Tab), TopKey::NextTab, "next tab");
    km.register(KeyBinding::new(KeyCode::Right), TopKey::NextTab, "next tab");
    km.register(
        KeyBinding::new(KeyCode::BackTab),
        TopKey::PrevTab,
        "previous tab",
    );
    km.register(
        KeyBinding::new(KeyCode::Left),
        TopKey::PrevTab,
        "previous tab",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('/')),
        TopKey::Filter,
        "filter",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('f')),
        TopKey::Filter,
        "filter",
    );
    km.register(KeyBinding::new(KeyCode::Char('h')), TopKey::Help, "help");
    km.register(
        KeyBinding::new(KeyCode::Char('c')),
        TopKey::Columns,
        "columns",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('C')),
        TopKey::Connector,
        "connector",
    );
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('C'), KeyModifiers::SHIFT),
        TopKey::Connector,
        "connector",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('S')),
        TopKey::SaveConfig,
        "save config",
    );
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('S'), KeyModifiers::SHIFT),
        TopKey::SaveConfig,
        "save config",
    );
    km.register(KeyBinding::new(KeyCode::Char('s')), TopKey::Sort, "sort");
    km.register(
        KeyBinding::new(KeyCode::Char(' ')),
        TopKey::TogglePause,
        "pause",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('p')),
        TopKey::TogglePause,
        "pause",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('a')),
        TopKey::ToggleAll,
        "toggle all containers",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('H')),
        TopKey::ToggleHeader,
        "toggle header",
    );
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('H'), KeyModifiers::SHIFT),
        TopKey::ToggleHeader,
        "toggle header",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('r')),
        TopKey::ToggleReverse,
        "reverse sort",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('R')),
        TopKey::ToggleReverse,
        "reverse sort",
    );
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('R'), KeyModifiers::SHIFT),
        TopKey::ToggleReverse,
        "reverse sort",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('!')),
        TopKey::ToggleRiskFilter,
        "risk filter",
    );
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('!'), KeyModifiers::SHIFT),
        TopKey::ToggleRiskFilter,
        "risk filter",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('g')),
        TopKey::ToggleKindFilter,
        "event kind filter",
    );
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('G'), KeyModifiers::SHIFT),
        TopKey::ToggleKindFilter,
        "event kind filter",
    );
    km.register(KeyBinding::new(KeyCode::Enter), TopKey::Detail, "detail");
    km.register(
        KeyBinding::new(KeyCode::Char('x')),
        TopKey::Detail,
        "detail",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('o')),
        TopKey::Open,
        "focus container",
    );
    km.register(KeyBinding::new(KeyCode::Char('l')), TopKey::Logs, "logs");
    km.register(
        KeyBinding::new(KeyCode::Char('w')),
        TopKey::OpenBrowser,
        "open browser",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('e')),
        TopKey::ExecShell,
        "exec shell",
    );
    km.register(
        KeyBinding::new(KeyCode::Char('K')),
        TopKey::Kill,
        "terminate",
    );
    km.register(
        KeyBinding::with_modifiers(KeyCode::Char('K'), KeyModifiers::SHIFT),
        TopKey::Kill,
        "terminate",
    );
    km
}

fn refresh_cmd(
    connector: ContainerConnector,
    show_all_containers: bool,
    container_target: Option<String>,
    observer: ObserverState,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let (snapshot, observer) =
            collect_snapshot(connector, show_all_containers, container_target, observer).await;
        Msg::Snapshot {
            connector,
            snapshot,
            observer,
        }
    })
}

fn single_or_batch(mut cmds: Vec<Cmd<Msg>>) -> Cmd<Msg> {
    if cmds.len() == 1 {
        cmds.remove(0)
    } else {
        cmd::batch(cmds)
    }
}

fn container_logs_cmd(
    connector: ContainerConnector,
    id: String,
    name: String,
    timestamps: bool,
) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let result = collect_container_logs(connector, &id, timestamps).await;
        Msg::ContainerLogs {
            connector,
            id,
            name,
            timestamps,
            result,
        }
    })
}

fn container_processes_cmd(container: ContainerRow) -> Cmd<Msg> {
    cmd::cmd(move || async move {
        let id = container.id.clone();
        let name = container.name.clone();
        let result = collect_container_processes(container.connector, &container.id).await;
        Msg::ContainerProcesses { id, name, result }
    })
}

fn save_config_cmd(config: TopConfig) -> Cmd<Msg> {
    cmd::cmd(move || async move { Msg::ConfigSaved(save_top_config(config).await) })
}

fn top_config_path() -> PathBuf {
    if let Some(path) = std::env::var_os("A3S_TOP_CONFIG") {
        return PathBuf::from(path);
    }
    if let Some(config_home) = std::env::var_os("XDG_CONFIG_HOME") {
        return PathBuf::from(config_home).join("a3s").join("top.json");
    }
    if let Some(home) = crate::user_paths::user_home_dir() {
        return home.join(".config").join("a3s").join("top.json");
    }
    PathBuf::from(".a3s-top.json")
}

fn load_top_config() -> Result<TopConfig, String> {
    let path = top_config_path();
    if !path.exists() {
        return Ok(TopConfig::default());
    }
    let text =
        std::fs::read_to_string(&path).map_err(|err| format!("read {}: {err}", path.display()))?;
    parse_top_config(&text)
}

fn parse_top_config(text: &str) -> Result<TopConfig, String> {
    let value: serde_json::Value = serde_json::from_str(text).map_err(|err| err.to_string())?;
    let mut config = TopConfig::default();

    if let Some(v) = value.get("show_all_containers").and_then(|v| v.as_bool()) {
        config.show_all_containers = v;
    }
    if let Some(v) = value.get("show_header").and_then(|v| v.as_bool()) {
        config.show_header = v;
    }
    if let Some(v) = value.get("reverse_sort").and_then(|v| v.as_bool()) {
        config.reverse_sort = v;
    }
    if let Some(sort) = value
        .get("sort_by")
        .and_then(|v| v.as_str())
        .and_then(SortBy::from_label)
    {
        config.sort_by = sort;
    }
    if let Some(risk_filter) = value
        .get("risk_filter")
        .and_then(|v| v.as_str())
        .and_then(RiskFilter::from_label)
    {
        config.risk_filter = risk_filter;
    }
    if let Some(kind_filter) = value
        .get("kind_filter")
        .and_then(|v| v.as_str())
        .and_then(KindFilter::from_label)
    {
        config.kind_filter = kind_filter;
    }
    if let Some(connector) = value
        .get("connector")
        .and_then(|v| v.as_str())
        .and_then(ContainerConnector::from_label)
    {
        config.connector = connector;
    }
    if let Some(filter) = value.get("filter").and_then(|v| v.as_str()) {
        config.filter = filter.to_string();
    }
    if let Some(columns) = value.get("hidden_columns").and_then(|v| v.as_array()) {
        config.hidden_columns = columns
            .iter()
            .filter_map(|v| v.as_str().map(ToString::to_string))
            .collect();
    }

    Ok(config)
}

fn top_config_json(config: &TopConfig) -> String {
    let mut hidden_columns = config.hidden_columns.iter().cloned().collect::<Vec<_>>();
    hidden_columns.sort();
    serde_json::to_string_pretty(&serde_json::json!({
        "show_all_containers": config.show_all_containers,
        "show_header": config.show_header,
        "reverse_sort": config.reverse_sort,
        "sort_by": config.sort_by.label(),
        "risk_filter": config.risk_filter.label(),
        "kind_filter": config.kind_filter.label(),
        "connector": config.connector.label(),
        "filter": &config.filter,
        "hidden_columns": hidden_columns,
    }))
    .unwrap_or_else(|_| "{}".to_string())
}

async fn save_top_config(config: TopConfig) -> Result<PathBuf, String> {
    let path = top_config_path();
    if let Some(parent) = path.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(|err| format!("create {}: {err}", parent.display()))?;
    }
    tokio::fs::write(&path, top_config_json(&config))
        .await
        .map_err(|err| format!("write {}: {err}", path.display()))?;
    Ok(path)
}

async fn run_external_action(action: ExternalAction) -> anyhow::Result<()> {
    match action {
        ExternalAction::ContainerShell {
            connector,
            id,
            name,
        } => {
            println!(
                "opening shell in {} container {name} ({})",
                connector.label(),
                short_id(&id)
            );
            match connector {
                ContainerConnector::A3sBox => {
                    let a3s_box = a3s::components::resolve_or_install("box").await?;
                    let status = std::process::Command::new(a3s_box)
                        .args(["shell", &id])
                        .status()?;
                    if !status.success() {
                        eprintln!("a3s-box shell exited with status {status}");
                    }
                }
                ContainerConnector::Docker => {
                    let shell = detect_container_shell(&id);
                    let status = std::process::Command::new("docker")
                        .args(["exec", "-it", &id, &shell])
                        .status()?;
                    if !status.success() {
                        eprintln!("docker exec exited with status {status}");
                    }
                }
                ContainerConnector::RunC => {
                    eprintln!("runc shell is not supported from a3s top yet");
                }
            }
        }
        ExternalAction::OpenBrowser { url, name } => {
            println!("opening browser for container {name}: {url}");
            if let Err(err) = open_url(&url) {
                eprintln!("failed to open browser: {err}");
            }
        }
    }
    Ok(())
}

fn open_url(url: &str) -> anyhow::Result<()> {
    let status = {
        #[cfg(target_os = "macos")]
        {
            std::process::Command::new("open").arg(url).status()
        }
        #[cfg(target_os = "windows")]
        {
            std::process::Command::new("cmd")
                .args(["/C", "start", "", url])
                .status()
        }
        #[cfg(all(not(target_os = "macos"), not(target_os = "windows")))]
        {
            std::process::Command::new("xdg-open").arg(url).status()
        }
    }?;

    if status.success() {
        Ok(())
    } else {
        Err(anyhow::anyhow!(
            "browser opener exited with status {status}"
        ))
    }
}

fn detect_container_shell(id: &str) -> String {
    let output = std::process::Command::new("docker")
        .args([
            "exec",
            id,
            "sh",
            "-c",
            "command -v bash 2>/dev/null || command -v sh 2>/dev/null",
        ])
        .output();
    let Ok(output) = output else {
        return "sh".to_string();
    };
    if !output.status.success() {
        return "sh".to_string();
    }
    let shell = String::from_utf8_lossy(&output.stdout)
        .lines()
        .next()
        .unwrap_or("sh")
        .trim()
        .to_string();
    if shell.is_empty() {
        "sh".to_string()
    } else {
        shell
    }
}

fn runc_command() -> Command {
    let mut command = Command::new("runc");
    for arg in runc_global_args() {
        command.arg(arg);
    }
    command
}

fn runc_global_args() -> Vec<String> {
    let root = std::env::var("RUNC_ROOT").unwrap_or_else(|_| "/run/runc".to_string());
    let mut args = vec!["--root".to_string(), root];
    if env_flag("RUNC_SYSTEMD_CGROUP") {
        args.push("--systemd-cgroup".to_string());
    }
    args
}

fn env_flag(key: &str) -> bool {
    std::env::var(key)
        .ok()
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "on"
            )
        })
        .unwrap_or(false)
}

async fn collect_snapshot(
    connector: ContainerConnector,
    show_all_containers: bool,
    container_target: Option<String>,
    observer: ObserverState,
) -> (TopSnapshot, ObserverState) {
    let (processes, containers, observer) = tokio::join!(
        collect_processes(),
        collect_containers(connector, show_all_containers, container_target.as_deref()),
        collect_observer_events(observer),
    );
    let (processes, process_error) = match processes {
        Ok(rows) => (rows, None),
        Err(err) => (Vec::new(), Some(format!("process collector: {err}"))),
    };
    let (containers, container_error) = match containers {
        Ok(rows) => (rows, None),
        Err(err) => (Vec::new(), Some(format!("container collector: {err}"))),
    };

    let mut errors = Vec::new();
    errors.extend(process_error);
    errors.extend(container_error);
    let mut events = observer.events.clone();
    if events.is_empty() {
        events.extend(errors.iter().map(|err| EventRow {
            ts: "now".into(),
            source: "collector".into(),
            session: None,
            task: None,
            pid: None,
            ppid: None,
            kind: "warning".into(),
            message: err.clone(),
            details: vec![("error".into(), err.clone())],
            risk: Risk::Medium,
        }));
    }

    (
        TopSnapshot {
            processes,
            containers,
            events,
            errors,
        },
        observer,
    )
}

async fn collect_containers(
    connector: ContainerConnector,
    show_all: bool,
    target: Option<&str>,
) -> anyhow::Result<Vec<ContainerRow>> {
    match connector {
        ContainerConnector::A3sBox => collect_a3s_box_containers(show_all, target).await,
        ContainerConnector::Docker => collect_docker_containers(show_all, target).await,
        ContainerConnector::RunC => collect_runc_containers(show_all, target).await,
    }
}

async fn collect_a3s_box_containers(
    show_all: bool,
    target: Option<&str>,
) -> anyhow::Result<Vec<ContainerRow>> {
    let a3s_box = ensure_a3s_box_binary().await?;
    let mut containers = Vec::new();
    for filter in container_target_filters(target) {
        containers.extend(a3s_box_ps_rows(&a3s_box, show_all, filter.as_deref()).await?);
    }
    dedupe_container_rows(&mut containers);
    if containers.is_empty() {
        return Ok(containers);
    }

    if let Some(by_id) = a3s_box_stats_rows(&a3s_box).await {
        for container in &mut containers {
            if let Some(stats) = by_id
                .get(&container.id)
                .or_else(|| by_id.get(short_id(&container.id)))
                .or_else(|| by_id.get(&container.name))
            {
                container.cpu_pct = stats.cpu_pct;
                container.cpu_count = stats.cpu_count;
                container.mem_pct = stats.mem_pct;
                container.mem_usage = stats.mem_usage.clone();
                container.net_io = stats.net_io.clone();
                container.block_io = stats.block_io.clone();
                if let Some(pids) = stats.pids_current {
                    container.pids = pids.to_string();
                }
            }
        }
    }
    enrich_a3s_box_process_counts(&mut containers, &a3s_box).await;
    enrich_a3s_box_inspect(&mut containers, &a3s_box).await;
    Ok(containers)
}

async fn enrich_a3s_box_process_counts(containers: &mut [ContainerRow], a3s_box: &Path) {
    let targets = containers
        .iter()
        .filter(|container| container_is_running(&container.status))
        .filter(|container| container.pids == "-")
        .map(|container| container.id.clone())
        .collect::<Vec<_>>();
    if targets.is_empty() {
        return;
    }

    let counts = stream::iter(targets.into_iter().map(|id| {
        let a3s_box = a3s_box.to_path_buf();
        async move {
            let count = a3s_box_container_process_count(&a3s_box, &id).await;
            (id, count)
        }
    }))
    .buffer_unordered(A3S_BOX_PIDS_CONCURRENCY)
    .collect::<Vec<_>>()
    .await
    .into_iter()
    .filter_map(|(id, count)| count.map(|count| (id, count)))
    .collect::<HashMap<_, _>>();

    for container in containers {
        if let Some(count) = counts.get(&container.id) {
            container.pids = count.to_string();
        }
    }
}

async fn a3s_box_container_process_count(a3s_box: &Path, id: &str) -> Option<usize> {
    let rows = tokio::time::timeout(
        A3S_BOX_PIDS_TIMEOUT,
        collect_a3s_box_container_processes_with_binary(a3s_box, id),
    )
    .await
    .ok()?
    .ok()?;
    Some(a3s_box_process_count_from_rows(&rows))
}

async fn a3s_box_ps_rows(
    a3s_box: &Path,
    show_all: bool,
    filter: Option<&str>,
) -> anyhow::Result<Vec<ContainerRow>> {
    let mut ps_args = vec!["ps".to_string(), "--format".to_string(), "json".to_string()];
    if show_all {
        ps_args.insert(1, "-a".to_string());
    }
    if let Some(filter) = filter {
        ps_args.push("--filter".to_string());
        ps_args.push(filter.to_string());
    }
    let json_command_label = format!("a3s-box {}", ps_args.join(" "));

    let json_error = match Command::new(a3s_box).args(ps_args).output().await {
        Ok(ps) if ps.status.success() => {
            let text = String::from_utf8_lossy(&ps.stdout);
            let rows = parse_a3s_box_ps_json(&text);
            if !rows.is_empty() || text.trim() == "[]" {
                return Ok(rows);
            }
            Some(format!("{json_command_label} returned unparseable JSON"))
        }
        Ok(ps) => Some(command_output_error(&json_command_label, &ps)),
        Err(err) => Some(format!("failed to run {json_command_label}: {err}")),
    };

    match a3s_box_ps_output(a3s_box, show_all, filter).await {
        Ok(output) => Ok(parse_a3s_box_ps(&output)),
        Err(err) => Err(anyhow::anyhow!(
            "{}; fallback table failed: {err}",
            json_error.unwrap_or_else(|| "a3s-box ps JSON output was unavailable".to_string())
        )),
    }
}

async fn a3s_box_ps_output(
    a3s_box: &Path,
    show_all: bool,
    filter: Option<&str>,
) -> anyhow::Result<String> {
    let mut ps_args = vec![
        "ps".to_string(),
        "--format".to_string(),
        "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}\t{{.Created}}\t{{.Command}}"
            .to_string(),
    ];
    if show_all {
        ps_args.insert(1, "-a".to_string());
    }
    if let Some(filter) = filter {
        ps_args.push("--filter".to_string());
        ps_args.push(filter.to_string());
    }
    let command_label = format!("a3s-box {}", ps_args.join(" "));

    let ps = Command::new(a3s_box).args(ps_args).output().await;
    let ps = ps.map_err(|err| anyhow::anyhow!("failed to run {command_label}: {err}"))?;
    if !ps.status.success() {
        return Err(anyhow::anyhow!(command_output_error(&command_label, &ps)));
    }
    Ok(String::from_utf8_lossy(&ps.stdout).into_owned())
}

fn command_output_error(command: &str, output: &std::process::Output) -> String {
    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    let stdout = String::from_utf8_lossy(&output.stdout).trim().to_string();
    let detail = if !stderr.is_empty() {
        stderr
    } else if !stdout.is_empty() {
        stdout
    } else {
        "no output".to_string()
    };
    format!("{command} exited with status {}: {detail}", output.status)
}

async fn a3s_box_stats_rows(a3s_box: &Path) -> Option<HashMap<String, A3sBoxStatsRow>> {
    let json = Command::new(a3s_box)
        .args(["stats", "--no-stream", "--format", "json"])
        .output()
        .await
        .ok();
    if let Some(json) = json.filter(|output| output.status.success()) {
        let text = String::from_utf8_lossy(&json.stdout);
        let rows = parse_a3s_box_stats_json(&text);
        if !rows.is_empty() || text.trim() == "[]" {
            return Some(rows);
        }
    }

    let table = Command::new(a3s_box)
        .args(["stats", "--no-stream"])
        .output()
        .await
        .ok()?;
    if !table.status.success() {
        return None;
    }
    Some(parse_a3s_box_stats(&String::from_utf8_lossy(&table.stdout)))
}

async fn ensure_a3s_box_binary() -> anyhow::Result<PathBuf> {
    // Resolving the binary scans $PATH + stats several candidate paths; the
    // location is stable for the session, so memoize the first success (errors
    // are not cached, so a missing binary is retried next refresh).
    static A3S_BOX_BIN: tokio::sync::OnceCell<PathBuf> = tokio::sync::OnceCell::const_new();
    A3S_BOX_BIN
        .get_or_try_init(|| a3s::components::resolve_or_install("box"))
        .await
        .cloned()
}

#[derive(Debug, Clone, PartialEq)]
struct A3sBoxStatsRow {
    cpu_pct: Option<f32>,
    cpu_count: Option<u32>,
    mem_pct: Option<f32>,
    mem_usage: String,
    net_io: String,
    block_io: String,
    pid: Option<String>,
    pids_current: Option<u64>,
}

fn parse_a3s_box_ps(text: &str) -> Vec<ContainerRow> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(7, '\t');
            let id = parts.next()?.trim();
            if id.is_empty() {
                return None;
            }
            let name = parts.next().unwrap_or(id).trim();
            let image = parts.next().unwrap_or("-").trim();
            let status = parts.next().unwrap_or("-").trim();
            let ports = parts.next().unwrap_or("").trim();
            let created = parts.next().unwrap_or("-").trim();
            let command = parts.next().unwrap_or("").trim();
            let mut inspect = ContainerInspect {
                created: created.to_string(),
                started: created.to_string(),
                ..ContainerInspect::default()
            };
            if !command.is_empty() {
                inspect.labels = format!("command={}", truncate(command, 80));
            }
            Some(ContainerRow {
                connector: ContainerConnector::A3sBox,
                id: id.to_string(),
                name: if name.is_empty() {
                    id.to_string()
                } else {
                    name.to_string()
                },
                image: if image.is_empty() {
                    "-".into()
                } else {
                    image.to_string()
                },
                status: if status.is_empty() {
                    "-".into()
                } else {
                    status.to_string()
                },
                inspect,
                ports: normalize_ports(ports),
                cpu_pct: None,
                cpu_count: None,
                cpu_usage_total_ns: None,
                mem_pct: None,
                mem_usage: "-".into(),
                net_io: "-".into(),
                block_io: "-".into(),
                pids: "-".into(),
            })
        })
        .collect()
}

fn parse_a3s_box_ps_json(text: &str) -> Vec<ContainerRow> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };
    let items = match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(mut object) => ["containers", "boxes", "items"]
            .iter()
            .find_map(|key| object.remove(*key))
            .and_then(|value| value.as_array().cloned())
            .unwrap_or_default(),
        _ => return Vec::new(),
    };

    items.iter().filter_map(parse_a3s_box_ps_json_row).collect()
}

fn parse_a3s_box_ps_json_row(item: &serde_json::Value) -> Option<ContainerRow> {
    let id = json_string(item, &["id", "Id", "ID"])
        .or_else(|| json_string(item, &["short_id", "shortId", "cid", "CID"]))?;
    let name = json_string(item, &["name", "names", "Names"]).unwrap_or_else(|| id.clone());
    let image = json_string(item, &["image", "Image"]).unwrap_or_else(|| "-".into());
    let status = json_string(item, &["raw_status", "rawStatus", "state", "State"])
        .or_else(|| json_string(item, &["status", "Status"]))
        .unwrap_or_else(|| "-".into());
    let ports = json_ports_text(item);
    let created_at = json_string(item, &["created_at", "createdAt"]);
    let started_at = json_string(item, &["started_at", "startedAt"]);
    let created = json_string(item, &["created", "Created"])
        .or_else(|| {
            created_at
                .as_deref()
                .map(|value| compact_timestamp(Some(value)))
        })
        .unwrap_or_else(|| "-".into());
    let started = started_at
        .as_deref()
        .map(|value| compact_timestamp(Some(value)))
        .unwrap_or_else(|| created.clone());
    let command = json_string(item, &["command", "Command"]).unwrap_or_default();
    let health = json_string(item, &["health", "Health"])
        .filter(|value| !value.eq_ignore_ascii_case("none"))
        .unwrap_or_else(|| "-".into());
    let labels = a3s_box_ps_labels_summary(item.get("labels"), &command);

    Some(ContainerRow {
        connector: ContainerConnector::A3sBox,
        id,
        name,
        image,
        status,
        inspect: ContainerInspect {
            health,
            created,
            started,
            labels,
            ..ContainerInspect::default()
        },
        ports: normalize_ports(&ports),
        cpu_pct: None,
        cpu_count: None,
        cpu_usage_total_ns: None,
        mem_pct: None,
        mem_usage: "-".into(),
        net_io: "-".into(),
        block_io: "-".into(),
        pids: "-".into(),
    })
}

fn json_ports_text(value: &serde_json::Value) -> String {
    if let Some(ports) = json_string(value, &["ports_text", "portsText"]) {
        return ports;
    }
    if let Some(ports) = value
        .get("ports")
        .or_else(|| value.get("Ports"))
        .and_then(serde_json::Value::as_array)
    {
        return ports
            .iter()
            .filter_map(|port| match port {
                serde_json::Value::String(value) => Some(value.trim().to_string()),
                serde_json::Value::Number(value) => Some(value.to_string()),
                _ => None,
            })
            .filter(|value| !value.is_empty())
            .collect::<Vec<_>>()
            .join(", ");
    }
    json_string(value, &["ports", "Ports"]).unwrap_or_default()
}

fn a3s_box_ps_labels_summary(labels: Option<&serde_json::Value>, command: &str) -> String {
    let mut parts = Vec::new();
    if !command.trim().is_empty() {
        parts.push(format!("command={}", truncate(command, 80)));
    }
    let labels = docker_label_summary(labels);
    if labels != "-" {
        parts.push(labels);
    }
    if parts.is_empty() {
        "-".into()
    } else {
        parts.join(", ")
    }
}

fn parse_a3s_box_stats(text: &str) -> HashMap<String, A3sBoxStatsRow> {
    let mut rows = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty()
            || line.starts_with("BOX ID")
            || line.starts_with("No active boxes")
            || line.starts_with("NAME ")
        {
            continue;
        }
        let Some((id, name, stats)) = parse_a3s_box_stats_line(line) else {
            continue;
        };
        rows.insert(id.clone(), stats.clone());
        rows.insert(name, stats);
    }
    rows
}

fn parse_a3s_box_stats_json(text: &str) -> HashMap<String, A3sBoxStatsRow> {
    let mut rows = HashMap::new();
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return rows;
    };
    let items = match value {
        serde_json::Value::Array(items) => items,
        serde_json::Value::Object(mut object) => match object.remove("containers") {
            Some(serde_json::Value::Array(items)) => items,
            _ => return rows,
        },
        _ => return rows,
    };

    for item in items {
        let Some((id, name, stats)) = parse_a3s_box_stats_json_row(&item) else {
            continue;
        };
        rows.insert(id.clone(), stats.clone());
        if let Some(short) = json_string(&item, &["short_id", "shortId"]).filter(|s| s != &id) {
            rows.insert(short, stats.clone());
        }
        rows.insert(name, stats);
    }

    rows
}

fn parse_a3s_box_stats_json_row(
    item: &serde_json::Value,
) -> Option<(String, String, A3sBoxStatsRow)> {
    let id = json_string(item, &["id", "short_id", "shortId"])?;
    let name = json_string(item, &["name", "names"]).unwrap_or_else(|| id.clone());
    let memory_bytes = json_u64_field(item, &["memory_bytes", "memoryBytes"])?;
    let memory_limit_bytes = json_u64_field(item, &["memory_limit_bytes", "memoryLimitBytes"])?;
    let network_rx = json_u64_field(item, &["network_rx_bytes", "networkRxBytes"]).unwrap_or(0);
    let network_tx = json_u64_field(item, &["network_tx_bytes", "networkTxBytes"]).unwrap_or(0);
    let block_read = json_u64_field(item, &["block_read_bytes", "blockReadBytes"]).unwrap_or(0);
    let block_write = json_u64_field(item, &["block_write_bytes", "blockWriteBytes"]).unwrap_or(0);
    let mem_pct = json_f64_field(item, &["memory_percent", "memoryPercent"]).map(|v| v as f32);
    let cpu_count = json_u64_field(item, &["cpus", "cpu_count", "cpuCount"])
        .and_then(|value| u32::try_from(value).ok())
        .filter(|value| *value > 0);
    let pids_current = json_u64_field(item, &["pids_current", "pidsCurrent"])
        .or_else(|| item.pointer("/pids/current").and_then(json_u64));

    Some((
        id,
        name,
        A3sBoxStatsRow {
            cpu_pct: json_f64_field(item, &["cpu_percent", "cpuPercent"]).map(|v| v as f32),
            cpu_count,
            mem_pct,
            mem_usage: format_byte_pair_decimal(memory_bytes, memory_limit_bytes),
            net_io: format_byte_pair_decimal(network_rx, network_tx),
            block_io: format_byte_pair_decimal(block_read, block_write),
            pid: json_u64_field(item, &["pid"]).map(|pid| pid.to_string()),
            pids_current,
        },
    ))
}

fn json_string(value: &serde_json::Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        value
            .get(*field)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToString::to_string)
    })
}

fn json_u64_field(value: &serde_json::Value, fields: &[&str]) -> Option<u64> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?;
        value
            .as_u64()
            .or_else(|| value.as_str()?.trim().parse::<u64>().ok())
    })
}

fn json_f64_field(value: &serde_json::Value, fields: &[&str]) -> Option<f64> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?;
        value.as_f64().or_else(|| {
            value
                .as_str()?
                .trim()
                .trim_end_matches('%')
                .parse::<f64>()
                .ok()
        })
    })
}

fn format_byte_pair_decimal(first: u64, second: u64) -> String {
    format!(
        "{} / {}",
        format_bytes_decimal(first),
        format_bytes_decimal(second)
    )
}

fn format_bytes_decimal(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * KB;
    const GB: u64 = 1024 * MB;

    if bytes >= GB {
        format!("{:.1} GB", bytes as f64 / GB as f64)
    } else if bytes >= MB {
        format!("{:.1} MB", bytes as f64 / MB as f64)
    } else if bytes >= KB {
        format!("{:.1} KB", bytes as f64 / KB as f64)
    } else {
        format!("{bytes} B")
    }
}

fn parse_a3s_box_stats_line(line: &str) -> Option<(String, String, A3sBoxStatsRow)> {
    let mut parts = line.split_whitespace();
    let id = parts.next()?.to_string();
    let name = parts.next()?.to_string();
    let _status = parts.next()?;
    let cpu_pct = parts.next().and_then(parse_percent);
    let rest = parts.collect::<Vec<_>>();
    let mem_pct_idx = rest.iter().rposition(|part| part.ends_with('%'))?;
    let mem_pct = parse_percent(rest[mem_pct_idx]);
    let pid = rest.get(mem_pct_idx + 1).map(|value| (*value).to_string());
    let (net_io, block_io) = split_a3s_box_stats_io(rest.get(mem_pct_idx + 2..).unwrap_or(&[]));
    let mem_usage = rest[..mem_pct_idx].join(" ");
    Some((
        id,
        name,
        A3sBoxStatsRow {
            cpu_pct,
            cpu_count: None,
            mem_pct,
            mem_usage: if mem_usage.is_empty() {
                "-".into()
            } else {
                mem_usage
            },
            net_io,
            block_io,
            pid,
            pids_current: None,
        },
    ))
}

fn split_a3s_box_stats_io(parts: &[&str]) -> (String, String) {
    if parts.is_empty() {
        return ("-".into(), "-".into());
    }

    let slash_positions = parts
        .iter()
        .enumerate()
        .filter_map(|(idx, part)| (*part == "/").then_some(idx))
        .collect::<Vec<_>>();

    if slash_positions.len() >= 2 && slash_positions[1] >= 2 {
        let second_pair_start = slash_positions[1] - 2;
        let net_io = parts[..second_pair_start].join(" ");
        let block_io = parts[second_pair_start..].join(" ");
        return (non_empty_or_dash(net_io), non_empty_or_dash(block_io));
    }

    ("-".into(), parts.join(" "))
}

fn non_empty_or_dash(value: String) -> String {
    if value.is_empty() {
        "-".into()
    } else {
        value
    }
}

// ponytail: process-global inspect cache. `a3s-box inspect` output is
// near-static (created/started/labels/health), so a short TTL eliminates the
// per-tick fan-out (was up to A3S_BOX_INSPECT_LIMIT subprocess spawns every
// refresh). Keyed by the requested container id; pruned to live ids each pass.
static INSPECT_CACHE: std::sync::OnceLock<Mutex<HashMap<String, (Instant, ContainerInspect)>>> =
    std::sync::OnceLock::new();
const INSPECT_TTL: Duration = Duration::from_secs(10);

async fn enrich_a3s_box_inspect(containers: &mut [ContainerRow], a3s_box: &Path) {
    if containers.is_empty() {
        return;
    }
    let cache = INSPECT_CACHE.get_or_init(|| Mutex::new(HashMap::new()));
    let now = Instant::now();

    // Only inspect ids whose cached entry is missing or past its TTL.
    let stale_ids = {
        let map = cache.lock().unwrap();
        containers
            .iter()
            .take(A3S_BOX_INSPECT_LIMIT)
            .filter(|container| match map.get(&container.id) {
                Some((at, _)) => now.duration_since(*at) >= INSPECT_TTL,
                None => true,
            })
            .map(|container| container.id.clone())
            .collect::<Vec<_>>()
    };

    if !stale_ids.is_empty() {
        let fresh: Vec<(String, Option<ContainerInspect>)> = stream::iter(stale_ids)
            .map(|id| {
                let a3s_box = a3s_box.to_path_buf();
                async move {
                    let inspect = collect_a3s_box_inspect(&a3s_box, &id).await;
                    (id, inspect)
                }
            })
            .buffer_unordered(8)
            .collect()
            .await;
        let mut map = cache.lock().unwrap();
        for (id, inspect) in fresh {
            if let Some(inspect) = inspect {
                map.insert(id, (now, inspect));
            }
        }
    }

    let live_ids: HashSet<String> = containers.iter().map(|c| c.id.clone()).collect();
    let mut map = cache.lock().unwrap();
    map.retain(|id, _| live_ids.contains(id));
    for container in containers.iter_mut() {
        if let Some((_, inspect)) = map.get(&container.id) {
            container.inspect = inspect.clone();
        }
    }
}

async fn collect_a3s_box_inspect(a3s_box: &Path, id: &str) -> Option<ContainerInspect> {
    let output = tokio::time::timeout(
        Duration::from_millis(1200),
        Command::new(a3s_box).args(["inspect", id]).output(),
    )
    .await
    .ok()?
    .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_a3s_box_inspect(&String::from_utf8_lossy(&output.stdout))
        .into_values()
        .next()
}

fn parse_a3s_box_inspect(text: &str) -> HashMap<String, ContainerInspect> {
    let mut by_id = HashMap::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    if let Some((id, inspect)) = parse_a3s_box_inspect_record(&item) {
                        by_id.insert(short_id(&id).to_string(), inspect.clone());
                        by_id.insert(id, inspect);
                    }
                }
                return by_id;
            }
            value => {
                if let Some((id, inspect)) = parse_a3s_box_inspect_record(&value) {
                    by_id.insert(short_id(&id).to_string(), inspect.clone());
                    by_id.insert(id, inspect);
                    return by_id;
                }
            }
        }
    }

    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some((id, inspect)) = parse_a3s_box_inspect_record(&value) {
            by_id.insert(short_id(&id).to_string(), inspect.clone());
            by_id.insert(id, inspect);
        }
    }
    by_id
}

fn parse_a3s_box_inspect_record(value: &serde_json::Value) -> Option<(String, ContainerInspect)> {
    let id = value
        .get("id")
        .or_else(|| value.get("Id"))
        .and_then(|v| v.as_str())?
        .to_string();
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or_default();
    let status_detail = value
        .get("status_detail")
        .unwrap_or(&serde_json::Value::Null);
    let state = value.get("State").unwrap_or(&serde_json::Value::Null);

    Some((
        id,
        ContainerInspect {
            health: a3s_box_health_label(value, status_detail),
            restarts: value
                .get("restart_count")
                .and_then(json_u64)
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".into()),
            restart_policy: a3s_box_restart_policy(value),
            created: compact_timestamp(value.get("created_at").and_then(|v| v.as_str())),
            started: compact_timestamp(value.get("started_at").and_then(|v| v.as_str())),
            exit: a3s_box_exit_label(value, state, status),
            mounts: a3s_box_mounts_summary(value),
            env: a3s_box_env_summary(value.get("env")),
            labels: docker_label_summary(value.get("labels")),
            networks: a3s_box_network_summary(value),
        },
    ))
}

fn a3s_box_health_label(value: &serde_json::Value, status_detail: &serde_json::Value) -> String {
    status_detail
        .get("health")
        .or_else(|| value.get("health_status"))
        .and_then(|v| v.as_str())
        .filter(|health| !health.is_empty() && *health != "none")
        .map(ToString::to_string)
        .unwrap_or_else(|| "-".into())
}

fn a3s_box_restart_policy(value: &serde_json::Value) -> String {
    let policy = value
        .get("restart_policy")
        .and_then(|v| v.as_str())
        .unwrap_or("no");
    let max = value
        .get("max_restart_count")
        .and_then(json_u64)
        .unwrap_or(0);
    if policy == "on-failure" && max > 0 {
        format!("{policy}:{max}")
    } else if policy.is_empty() {
        "-".into()
    } else {
        policy.to_string()
    }
}

fn a3s_box_exit_label(
    value: &serde_json::Value,
    state: &serde_json::Value,
    status: &str,
) -> String {
    let running = state
        .get("Running")
        .and_then(|v| v.as_bool())
        .unwrap_or(matches!(status, "running" | "paused"));
    if running {
        return "-".into();
    }
    value
        .get("exit_code")
        .or_else(|| state.get("ExitCode"))
        .and_then(|v| v.as_i64())
        .map(|code| code.to_string())
        .unwrap_or_else(|| "-".into())
}

fn a3s_box_mounts_summary(value: &serde_json::Value) -> String {
    let mut labels = Vec::new();
    collect_json_string_array(value.get("volumes"), &mut labels);
    collect_json_string_array(value.get("volume_names"), &mut labels);
    collect_json_string_array(value.get("tmpfs"), &mut labels);
    collect_json_string_array(value.get("anonymous_volumes"), &mut labels);
    if labels.is_empty() {
        return "-".into();
    }
    let total = labels.len();
    summarize_named_count(
        total,
        "mount",
        labels
            .into_iter()
            .take(3)
            .map(|label| truncate(&label, 32))
            .collect(),
    )
}

fn a3s_box_env_summary(value: Option<&serde_json::Value>) -> String {
    let Some(env) = value.and_then(|v| v.as_object()) else {
        return "-".into();
    };
    if env.is_empty() {
        "-".into()
    } else if env.len() == 1 {
        "1 var".into()
    } else {
        format!("{} vars", env.len())
    }
}

fn a3s_box_network_summary(value: &serde_json::Value) -> String {
    let mut labels = Vec::new();
    if let Some(name) = value.get("network_name").and_then(|v| v.as_str()) {
        if !name.is_empty() {
            labels.push(name.to_string());
        }
    }
    if labels.is_empty() {
        if let Some(mode) = value.get("network_mode") {
            if !mode.is_null() {
                labels.push(compact_json_value(mode));
            }
        }
    }
    collect_json_string_array(value.get("add_host"), &mut labels);
    if labels.is_empty() {
        return "-".into();
    }
    let total = labels.len();
    summarize_named_count(
        total,
        "net",
        labels
            .into_iter()
            .take(3)
            .map(|label| truncate(&label, 36))
            .collect(),
    )
}

fn collect_json_string_array(value: Option<&serde_json::Value>, out: &mut Vec<String>) {
    let Some(items) = value.and_then(|v| v.as_array()) else {
        return;
    };
    out.extend(
        items
            .iter()
            .filter_map(|item| item.as_str())
            .filter(|item| !item.is_empty())
            .map(ToString::to_string),
    );
}

async fn collect_docker_containers(
    show_all: bool,
    target: Option<&str>,
) -> anyhow::Result<Vec<ContainerRow>> {
    let mut containers = Vec::new();
    for filter in container_target_filters(target) {
        containers.extend(parse_docker_container_list(
            &docker_ps_output(show_all, filter.as_deref()).await,
        ));
    }
    dedupe_container_rows(&mut containers);
    if containers.is_empty() {
        return Ok(containers);
    }

    let stats = Command::new("docker")
        .args([
            "stats",
            "--no-stream",
            "--format",
            "{{.ID}}\t{{.CPUPerc}}\t{{.MemPerc}}\t{{.MemUsage}}\t{{.NetIO}}\t{{.BlockIO}}\t{{.PIDs}}",
        ])
        .output()
        .await;
    if let Ok(stats) = stats {
        if stats.status.success() {
            let stats_text = String::from_utf8_lossy(&stats.stdout);
            type DockerStatsRow = (Option<f32>, Option<f32>, String, String, String, String);
            let mut by_id: HashMap<String, DockerStatsRow> = HashMap::new();
            for line in stats_text.lines() {
                let mut parts = line.split('\t');
                let Some(id) = parts.next() else { continue };
                let cpu = parts.next().and_then(parse_percent);
                let mem_pct = parts.next().and_then(parse_percent);
                let mem = parts.next().unwrap_or("-").to_string();
                let net = parts.next().unwrap_or("-").to_string();
                let block = parts.next().unwrap_or("-").to_string();
                let pids = parts.next().unwrap_or("-").to_string();
                by_id.insert(id.to_string(), (cpu, mem_pct, mem, net, block, pids));
            }
            for c in &mut containers {
                let short = short_id(&c.id);
                let stats = by_id.get(&c.id).or_else(|| by_id.get(short)).cloned();
                if let Some((cpu, mem_pct, mem, net, block, pids)) = stats {
                    c.cpu_pct = cpu;
                    c.mem_pct = mem_pct;
                    c.mem_usage = mem;
                    c.net_io = net;
                    c.block_io = block;
                    c.pids = pids;
                }
            }
        }
    }
    enrich_docker_inspect(&mut containers).await;
    Ok(containers)
}

async fn docker_ps_output(show_all: bool, filter: Option<&str>) -> String {
    let mut ps_args = vec![
        "ps".to_string(),
        "--no-trunc".to_string(),
        "--format".to_string(),
        "{{.ID}}\t{{.Names}}\t{{.Image}}\t{{.Status}}\t{{.Ports}}".to_string(),
    ];
    if show_all {
        ps_args.insert(1, "-a".to_string());
    }
    if let Some(filter) = filter {
        ps_args.push("--filter".to_string());
        ps_args.push(filter.to_string());
    }

    let ps = Command::new("docker").args(ps_args).output().await;
    let Ok(ps) = ps else {
        return String::new();
    };
    if !ps.status.success() {
        return String::new();
    }
    String::from_utf8_lossy(&ps.stdout).into_owned()
}

fn parse_docker_container_list(text: &str) -> Vec<ContainerRow> {
    text.lines()
        .filter_map(|line| {
            let mut parts = line.splitn(5, '\t');
            Some(ContainerRow {
                connector: ContainerConnector::Docker,
                id: parts.next()?.to_string(),
                name: parts.next()?.to_string(),
                image: parts.next()?.to_string(),
                status: parts.next().unwrap_or_default().to_string(),
                inspect: ContainerInspect::default(),
                ports: normalize_ports(parts.next().unwrap_or_default()),
                cpu_pct: None,
                cpu_count: None,
                cpu_usage_total_ns: None,
                mem_pct: None,
                mem_usage: "-".into(),
                net_io: "-".into(),
                block_io: "-".into(),
                pids: "-".into(),
            })
        })
        .collect()
}

async fn enrich_docker_inspect(containers: &mut [ContainerRow]) {
    if containers.is_empty() {
        return;
    }

    let mut command = Command::new("docker");
    command.args(["inspect", "--type", "container"]);
    for container in containers.iter() {
        command.arg(&container.id);
    }

    let output = tokio::time::timeout(Duration::from_millis(1200), command.output())
        .await
        .ok()
        .and_then(Result::ok);
    let Some(output) = output else {
        return;
    };
    if !output.status.success() {
        return;
    }

    let inspect_by_id = parse_docker_inspect(&String::from_utf8_lossy(&output.stdout));
    for container in containers {
        let short = short_id(&container.id);
        if let Some(inspect) = inspect_by_id
            .get(&container.id)
            .or_else(|| inspect_by_id.get(short))
        {
            container.inspect = inspect.clone();
        }
    }
}

fn parse_docker_inspect(text: &str) -> HashMap<String, ContainerInspect> {
    let mut by_id = HashMap::new();
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        match value {
            serde_json::Value::Array(items) => {
                for item in items {
                    if let Some((id, inspect)) = parse_docker_inspect_container(&item) {
                        by_id.insert(short_id(&id).to_string(), inspect.clone());
                        by_id.insert(id, inspect);
                    }
                }
                return by_id;
            }
            value => {
                if let Some((id, inspect)) = parse_docker_inspect_container(&value) {
                    by_id.insert(short_id(&id).to_string(), inspect.clone());
                    by_id.insert(id, inspect);
                    return by_id;
                }
            }
        }
    }

    for line in text.lines() {
        let Ok(value) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        if let Some((id, inspect)) = parse_docker_inspect_container(&value) {
            by_id.insert(short_id(&id).to_string(), inspect.clone());
            by_id.insert(id, inspect);
        }
    }
    by_id
}

fn parse_docker_inspect_container(value: &serde_json::Value) -> Option<(String, ContainerInspect)> {
    let id = value.get("Id").and_then(|v| v.as_str())?.to_string();
    let state = value.get("State").unwrap_or(&serde_json::Value::Null);
    let config = value.get("Config").unwrap_or(&serde_json::Value::Null);
    let host_config = value.get("HostConfig").unwrap_or(&serde_json::Value::Null);
    let network_settings = value
        .get("NetworkSettings")
        .unwrap_or(&serde_json::Value::Null);

    Some((
        id,
        ContainerInspect {
            health: docker_health_label(state),
            restarts: value
                .get("RestartCount")
                .and_then(json_u64)
                .map(|count| count.to_string())
                .unwrap_or_else(|| "-".into()),
            restart_policy: docker_restart_policy(host_config.get("RestartPolicy")),
            created: compact_timestamp(value.get("Created").and_then(|v| v.as_str())),
            started: compact_timestamp(state.get("StartedAt").and_then(|v| v.as_str())),
            exit: docker_exit_label(state),
            mounts: docker_mounts_summary(value.get("Mounts")),
            env: docker_env_summary(config.get("Env")),
            labels: docker_label_summary(config.get("Labels")),
            networks: docker_network_summary(network_settings.get("Networks")),
        },
    ))
}

fn docker_health_label(state: &serde_json::Value) -> String {
    if let Some(status) = state.pointer("/Health/Status").and_then(|v| v.as_str()) {
        return status.to_string();
    }
    if state
        .get("OOMKilled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return "oom-killed".into();
    }
    if state.get("Dead").and_then(|v| v.as_bool()).unwrap_or(false) {
        return "dead".into();
    }
    "-".into()
}

fn docker_restart_policy(value: Option<&serde_json::Value>) -> String {
    let Some(value) = value else {
        return "-".into();
    };
    let name = value.get("Name").and_then(|v| v.as_str()).unwrap_or("-");
    if name.is_empty() || name == "no" {
        return name.to_string();
    }
    let max_retry = value
        .get("MaximumRetryCount")
        .and_then(json_u64)
        .unwrap_or(0);
    if max_retry > 0 {
        format!("{name}:{max_retry}")
    } else {
        name.to_string()
    }
}

fn docker_exit_label(state: &serde_json::Value) -> String {
    let status = state.get("Status").and_then(|v| v.as_str()).unwrap_or("");
    if matches!(status, "running" | "restarting" | "paused") {
        return "-".into();
    }

    let code = state
        .get("ExitCode")
        .and_then(|v| v.as_i64())
        .map(|code| code.to_string())
        .unwrap_or_else(|| "-".into());
    let error = state
        .get("Error")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .trim();
    if !error.is_empty() {
        return truncate(&format!("{code} {error}"), 80);
    }
    if state
        .get("OOMKilled")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        return format!("{code} oom-killed");
    }
    code
}

fn docker_mounts_summary(value: Option<&serde_json::Value>) -> String {
    let Some(items) = value.and_then(|v| v.as_array()) else {
        return "-".into();
    };
    if items.is_empty() {
        return "-".into();
    }
    let labels = items
        .iter()
        .take(3)
        .filter_map(|item| {
            let target = item
                .get("Destination")
                .or_else(|| item.get("Target"))
                .and_then(|v| v.as_str())
                .or_else(|| item.get("Name").and_then(|v| v.as_str()))
                .or_else(|| item.get("Source").and_then(|v| v.as_str()))?;
            let mode = if item.get("RW").and_then(|v| v.as_bool()).unwrap_or(true) {
                "rw"
            } else {
                "ro"
            };
            Some(format!("{}({mode})", truncate(target, 28)))
        })
        .collect::<Vec<_>>();
    summarize_named_count(items.len(), "mount", labels)
}

fn docker_env_summary(value: Option<&serde_json::Value>) -> String {
    let Some(items) = value.and_then(|v| v.as_array()) else {
        return "-".into();
    };
    if items.is_empty() {
        "-".into()
    } else if items.len() == 1 {
        "1 var".into()
    } else {
        format!("{} vars", items.len())
    }
}

fn docker_label_summary(value: Option<&serde_json::Value>) -> String {
    let Some(labels) = value.and_then(|v| v.as_object()) else {
        return "-".into();
    };
    if labels.is_empty() {
        return "-".into();
    }
    let mut keys = labels.keys().cloned().collect::<Vec<_>>();
    keys.sort();
    summarize_named_count(
        keys.len(),
        "label",
        keys.into_iter()
            .take(3)
            .map(|key| truncate(&key, 32))
            .collect(),
    )
}

fn docker_network_summary(value: Option<&serde_json::Value>) -> String {
    let Some(networks) = value.and_then(|v| v.as_object()) else {
        return "-".into();
    };
    if networks.is_empty() {
        return "-".into();
    }
    let mut labels = networks
        .iter()
        .map(|(name, detail)| {
            let ip = detail
                .get("IPAddress")
                .and_then(|v| v.as_str())
                .filter(|ip| !ip.is_empty());
            match ip {
                Some(ip) => format!("{name} {ip}"),
                None => name.clone(),
            }
        })
        .collect::<Vec<_>>();
    labels.sort();
    summarize_named_count(
        labels.len(),
        "net",
        labels
            .into_iter()
            .take(3)
            .map(|label| truncate(&label, 36))
            .collect(),
    )
}

fn summarize_named_count(total: usize, singular: &str, labels: Vec<String>) -> String {
    let plural = if total == 1 {
        singular.to_string()
    } else {
        format!("{singular}s")
    };
    if labels.is_empty() {
        return format!("{total} {plural}");
    }
    let suffix = if total > labels.len() {
        format!(", +{}", total - labels.len())
    } else {
        String::new()
    };
    format!("{total} {plural}: {}{suffix}", labels.join(", "))
}

fn compact_timestamp(value: Option<&str>) -> String {
    let Some(value) = value.map(str::trim).filter(|value| !value.is_empty()) else {
        return "-".into();
    };
    if value.starts_with("0001-") {
        return "-".into();
    }
    let value = value.trim_end_matches('Z');
    let value = value.split('.').next().unwrap_or(value).replace('T', " ");
    truncate(&value, 32)
}

async fn collect_runc_containers(
    show_all: bool,
    target: Option<&str>,
) -> anyhow::Result<Vec<ContainerRow>> {
    let output = runc_command()
        .args(["list", "--format", "json"])
        .output()
        .await;
    let Ok(output) = output else {
        return Ok(Vec::new());
    };
    if !output.status.success() {
        return Ok(Vec::new());
    }
    let text = String::from_utf8_lossy(&output.stdout);
    let mut containers = parse_runc_list(&text)?;
    filter_runc_container_rows(&mut containers, show_all, target);
    let ids = containers
        .iter()
        .filter(|container| container_is_running(&container.status))
        .map(|container| container.id.clone())
        .collect::<Vec<_>>();
    let stats = futures::future::join_all(ids.iter().map(|id| collect_runc_stats(id))).await;
    let stats_by_id = ids
        .into_iter()
        .zip(stats)
        .filter_map(|(id, stats)| stats.map(|stats| (id, stats)))
        .collect::<HashMap<_, _>>();
    for container in &mut containers {
        if let Some(stats) = stats_by_id.get(&container.id) {
            apply_runc_stats(container, stats);
        }
    }
    Ok(containers)
}

fn filter_runc_container_rows(rows: &mut Vec<ContainerRow>, show_all: bool, target: Option<&str>) {
    let target = target.map(str::trim).filter(|target| !target.is_empty());
    rows.retain(|row| {
        (show_all || container_is_running(&row.status))
            && target.is_none_or(|query| container_matches_query(row, query))
    });
}

fn parse_runc_list(text: &str) -> anyhow::Result<Vec<ContainerRow>> {
    let value: serde_json::Value = serde_json::from_str(text)?;
    let Some(items) = value.as_array() else {
        return Ok(Vec::new());
    };
    Ok(items.iter().filter_map(parse_runc_container).collect())
}

fn parse_runc_container(value: &serde_json::Value) -> Option<ContainerRow> {
    let id = value.get("id").and_then(|v| v.as_str())?.to_string();
    let status = value
        .get("status")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();
    let bundle = value.get("bundle").and_then(|v| v.as_str()).unwrap_or("-");
    let rootfs = value.get("rootfs").and_then(|v| v.as_str()).unwrap_or("-");
    let pid = value
        .get("pid")
        .and_then(|v| v.as_i64())
        .filter(|pid| *pid > 0)
        .map(|pid| pid.to_string())
        .unwrap_or_else(|| "-".to_string());
    Some(ContainerRow {
        connector: ContainerConnector::RunC,
        id: id.clone(),
        name: id,
        image: if rootfs == "-" {
            format!("bundle:{bundle}")
        } else {
            format!("rootfs:{rootfs}")
        },
        status,
        inspect: ContainerInspect::default(),
        cpu_pct: None,
        cpu_count: None,
        cpu_usage_total_ns: None,
        mem_pct: None,
        mem_usage: "-".into(),
        net_io: "-".into(),
        block_io: "-".into(),
        pids: pid,
        ports: "-".into(),
    })
}

async fn collect_runc_stats(id: &str) -> Option<RuncStats> {
    let mut command = runc_command();
    command.args(["events", "--stats", id]);
    let output = tokio::time::timeout(Duration::from_millis(800), command.output())
        .await
        .ok()?
        .ok()?;
    if !output.status.success() {
        return None;
    }
    parse_runc_stats_event(&String::from_utf8_lossy(&output.stdout))
}

fn apply_runc_stats(container: &mut ContainerRow, stats: &RuncStats) {
    container.cpu_usage_total_ns = stats.cpu_usage_total_ns;
    if let Some(usage) = stats.memory_usage {
        if let Some(limit) = stats.memory_limit.filter(|limit| *limit > 0) {
            container.mem_pct = Some((usage as f32 / limit as f32) * 100.0);
            container.mem_usage = format!("{} / {}", format_bytes(usage), format_bytes(limit));
        } else {
            container.mem_usage = format_bytes(usage);
        }
    }
    if stats.net_rx > 0 || stats.net_tx > 0 {
        container.net_io = format_byte_pair(stats.net_rx, stats.net_tx);
    }
    if stats.block_read > 0 || stats.block_write > 0 {
        container.block_io = format_byte_pair(stats.block_read, stats.block_write);
    }
    if let Some(pids) = stats.pids_current {
        container.pids = pids.to_string();
    }
}

fn parse_runc_stats_event(text: &str) -> Option<RuncStats> {
    let value = serde_json::from_str::<serde_json::Value>(text)
        .ok()
        .or_else(|| {
            text.lines()
                .find_map(|line| serde_json::from_str::<serde_json::Value>(line).ok())
        })?;
    let data = value.get("data").unwrap_or(&value);
    let mut stats = RuncStats {
        cpu_usage_total_ns: data.pointer("/cpu/usage/total").and_then(json_u64),
        memory_usage: data
            .pointer("/memory/usage/usage")
            .and_then(json_u64)
            .or_else(|| data.pointer("/memory/raw/usage").and_then(json_u64)),
        memory_limit: data
            .pointer("/memory/usage/limit")
            .and_then(json_u64)
            .or_else(|| data.pointer("/memory/raw/limit").and_then(json_u64)),
        pids_current: data.pointer("/pids/current").and_then(json_u64),
        ..RuncStats::default()
    };

    if let Some(interfaces) = data
        .get("network_interfaces")
        .or_else(|| data.get("networkInterfaces"))
        .and_then(|v| v.as_array())
    {
        for interface in interfaces {
            stats.net_rx += json_field_u64(interface, &["RxBytes", "rxBytes", "rx_bytes"]);
            stats.net_tx += json_field_u64(interface, &["TxBytes", "txBytes", "tx_bytes"]);
        }
    }

    if let Some(entries) = data
        .pointer("/blkio/ioServiceBytesRecursive")
        .or_else(|| data.pointer("/blkio/io_service_bytes_recursive"))
        .and_then(|v| v.as_array())
    {
        for entry in entries {
            let op = entry
                .get("op")
                .or_else(|| entry.get("Op"))
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let value = json_field_u64(entry, &["value", "Value"]);
            if op == "read" {
                stats.block_read += value;
            } else if op == "write" {
                stats.block_write += value;
            }
        }
    }

    Some(stats)
}

fn container_lifecycle_events(
    connector: ContainerConnector,
    previous: &[ContainerRow],
    current: &[ContainerRow],
) -> Vec<EventRow> {
    let previous_by_id = previous
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<HashMap<_, _>>();
    let current_by_id = current
        .iter()
        .map(|row| (row.id.as_str(), row))
        .collect::<HashMap<_, _>>();
    let mut events = Vec::new();

    for row in current {
        let previous = previous_by_id.get(row.id.as_str()).copied();
        let current_state = container_state_label(&row.status);
        let previous_state = previous.map(|row| container_state_label(&row.status));
        if previous_state.as_deref() != Some(current_state.as_str()) {
            let action = container_transition_action(previous_state.as_deref(), &current_state);
            events.push(container_event_row(
                connector,
                action,
                previous_state.as_deref(),
                row,
            ));
            continue;
        }

        if let Some(previous) = previous {
            let previous_health = previous.inspect.health.as_str();
            let current_health = row.inspect.health.as_str();
            if current_health != "-" && previous_health != current_health && previous_health != "-"
            {
                events.push(container_event_row(
                    connector,
                    "health_status",
                    Some(previous_health),
                    row,
                ));
            }
        }
    }

    for row in previous {
        if !current_by_id.contains_key(row.id.as_str()) {
            let previous_state = container_state_label(&row.status);
            events.push(container_event_row(
                connector,
                "destroy",
                Some(&previous_state),
                row,
            ));
        }
    }

    events.sort_by(|a, b| a.message.cmp(&b.message));
    events
}

fn container_transition_action(previous: Option<&str>, current: &str) -> &'static str {
    match (previous, current) {
        (None, "created") => "create",
        (None, "running") => "start",
        (None, "paused") => "pause",
        (None, "exited" | "dead") => "die",
        (Some("created"), "running") => "start",
        (Some("running"), "paused") => "pause",
        (Some("paused"), "running") => "unpause",
        (Some("exited" | "dead"), "running") => "restart",
        (Some(old), "exited" | "dead") if old != current => "die",
        (Some(old), _) if old != current => "update",
        _ => "update",
    }
}

fn container_event_row(
    connector: ContainerConnector,
    action: &'static str,
    previous_status: Option<&str>,
    row: &ContainerRow,
) -> EventRow {
    let status = container_state_label(&row.status);
    let mut details = vec![
        ("connector".into(), connector.label().into()),
        ("action".into(), action.into()),
        ("id".into(), row.id.clone()),
        ("cid".into(), short_id(&row.id).to_string()),
        ("name".into(), row.name.clone()),
        ("image".into(), row.image.clone()),
        ("status".into(), status.clone()),
    ];
    if let Some(previous_status) = previous_status {
        details.push(("previous_status".into(), previous_status.to_string()));
    }
    if row.inspect.health != "-" {
        details.push(("health".into(), row.inspect.health.clone()));
    }
    if row.pids != "-" {
        details.push(("pids".into(), row.pids.clone()));
    }

    EventRow {
        ts: "now".into(),
        source: connector.label().into(),
        session: None,
        task: None,
        pid: None,
        ppid: None,
        kind: "Container".into(),
        message: format!(
            "{action} {} ({}) · status {status}",
            row.name,
            short_id(&row.id)
        ),
        details,
        risk: match action {
            "die" | "destroy" | "pause" | "restart" | "update" => Risk::Medium,
            _ => Risk::Low,
        },
    }
}

fn push_runtime_events(current: &mut Vec<EventRow>, mut rows: Vec<EventRow>) {
    if rows.is_empty() {
        return;
    }
    rows.append(current);
    rows.truncate(200);
    *current = rows;
}

fn prepend_runtime_events(events: &mut Vec<EventRow>, runtime_events: &[EventRow]) {
    if runtime_events.is_empty() {
        return;
    }
    let mut merged = runtime_events.to_vec();
    merged.append(events);
    merged.truncate(200);
    *events = merged;
}

async fn collect_observer_events(mut state: ObserverState) -> ObserverState {
    let sources = observer_paths();
    if sources.paths.is_empty() {
        return ObserverState::default();
    }

    if state.paths != sources.paths || state.auto_paths != sources.auto_paths {
        state.paths = sources.paths.clone();
        state.auto_paths = sources.auto_paths.clone();
        state.files.retain(|path, _| sources.paths.contains(path));
    }

    for path in sources.paths {
        collect_observer_path(&mut state, path).await;
    }
    trim_observer_events(&mut state.events);
    state
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ObserverPaths {
    paths: Vec<PathBuf>,
    auto_paths: HashSet<PathBuf>,
}

fn observer_paths() -> ObserverPaths {
    let explicit = observer_paths_from_env();
    let mut paths = Vec::new();
    let mut auto_paths = HashSet::new();

    for path in explicit {
        push_unique_path(&mut paths, path);
    }

    if observer_auto_enabled() {
        for path in observer_paths_from_auto() {
            let inserted = push_unique_path(&mut paths, path.clone());
            if inserted {
                auto_paths.insert(path);
            }
        }
    }

    ObserverPaths { paths, auto_paths }
}

fn observer_paths_from_env() -> Vec<PathBuf> {
    let Some(value) = std::env::var_os("A3S_TOP_OBSERVER_LOGS")
        .or_else(|| std::env::var_os("A3S_TOP_OBSERVER_LOG"))
    else {
        return Vec::new();
    };
    let text = value.to_string_lossy();
    let raw_paths = if text.contains(',') {
        text.split(',')
            .map(|part| PathBuf::from(part.trim()))
            .collect::<Vec<_>>()
    } else {
        std::env::split_paths(&value).collect::<Vec<_>>()
    };
    raw_paths
        .into_iter()
        .filter(|path| !path.as_os_str().is_empty())
        .collect()
}

fn observer_auto_enabled() -> bool {
    std::env::var("A3S_TOP_OBSERVER_AUTO")
        .map(|value| {
            let value = value.trim().to_ascii_lowercase();
            !matches!(value.as_str(), "0" | "false" | "off" | "no")
        })
        .unwrap_or(true)
}

fn observer_paths_from_auto() -> Vec<PathBuf> {
    let Some(home) = crate::user_paths::user_home_dir() else {
        return Vec::new();
    };

    let mut paths = Vec::new();
    for path in [
        home.join(".a3s").join("observer").join("events.ndjson"),
        home.join(".a3s").join("observer").join("events.jsonl"),
        home.join(".a3s").join("top").join("events.ndjson"),
        home.join(".a3s").join("top").join("events.jsonl"),
        home.join(".a3s").join("events.ndjson"),
        home.join(".a3s").join("events.jsonl"),
        home.join(".codex").join("history.jsonl"),
    ] {
        if path.is_file() {
            push_unique_path(&mut paths, path);
        }
    }

    for path in recent_files_with_extension(
        &home.join(".claude").join("projects"),
        "jsonl",
        3,
        OBSERVER_AUTO_MAX_FILES_PER_AGENT,
    ) {
        push_unique_path(&mut paths, path);
    }

    for path in recent_files_with_extension(
        &home.join(".codex").join("sessions"),
        "jsonl",
        5,
        OBSERVER_AUTO_MAX_FILES_PER_AGENT,
    ) {
        push_unique_path(&mut paths, path);
    }

    for path in recent_files_with_extension(
        &home.join(".a3s").join("tui").join("sessions").join("runs"),
        "json",
        1,
        OBSERVER_AUTO_MAX_FILES_PER_AGENT,
    ) {
        push_unique_path(&mut paths, path);
    }

    for path in recent_files_with_extension(
        &home.join(".a3s").join("runtime-workspaces"),
        "json",
        4,
        OBSERVER_AUTO_MAX_FILES_PER_AGENT,
    )
    .into_iter()
    .filter(|path| path_contains_component(path, "runs"))
    {
        push_unique_path(&mut paths, path);
    }

    for path in recent_files_with_extension(
        &home.join(".a3s").join("workspace"),
        "json",
        6,
        OBSERVER_AUTO_MAX_FILES_PER_AGENT,
    )
    .into_iter()
    .filter(|path| path_contains_component(path, "runs"))
    {
        push_unique_path(&mut paths, path);
    }

    paths
}

fn recent_files_with_extension(
    root: &Path,
    extension: &str,
    max_depth: usize,
    limit: usize,
) -> Vec<PathBuf> {
    if !root.is_dir() {
        return Vec::new();
    }

    let mut found = Vec::new();
    let mut visited = 0usize;
    collect_recent_files(root, extension, max_depth, &mut visited, &mut found);
    found.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));
    found
        .into_iter()
        .take(limit)
        .map(|(_, path)| path)
        .collect()
}

fn collect_recent_files(
    dir: &Path,
    extension: &str,
    depth: usize,
    visited: &mut usize,
    found: &mut Vec<(u128, PathBuf)>,
) {
    if *visited >= OBSERVER_AUTO_MAX_SCAN_FILES {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };

    for entry in entries.flatten() {
        if *visited >= OBSERVER_AUTO_MAX_SCAN_FILES {
            return;
        }
        *visited += 1;
        let path = entry.path();
        let Ok(file_type) = entry.file_type() else {
            continue;
        };
        if file_type.is_dir() {
            if depth > 0 && !observer_scan_dir_is_ignored(&path) {
                collect_recent_files(&path, extension, depth - 1, visited, found);
            }
            continue;
        }
        if !file_type.is_file()
            || path
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| !ext.eq_ignore_ascii_case(extension))
                .unwrap_or(true)
        {
            continue;
        }
        let modified = entry
            .metadata()
            .ok()
            .and_then(|metadata| metadata.modified().ok())
            .and_then(|modified| modified.duration_since(UNIX_EPOCH).ok())
            .map(|duration| duration.as_millis())
            .unwrap_or(0);
        found.push((modified, path));
    }
}

fn observer_scan_dir_is_ignored(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .map(|name| {
            matches!(
                name,
                ".git" | "cache" | "memory" | "buildcache" | "artifacts"
            )
        })
        .unwrap_or(false)
}

fn path_contains_component(path: &Path, component: &str) -> bool {
    path.components().any(|part| {
        part.as_os_str()
            .to_str()
            .map(|value| value == component)
            .unwrap_or(false)
    })
}

fn push_unique_path(paths: &mut Vec<PathBuf>, path: PathBuf) -> bool {
    if paths.iter().any(|existing| existing == &path) {
        false
    } else {
        paths.push(path);
        true
    }
}

fn observer_status_label(state: &ObserverState) -> String {
    match state.paths.as_slice() {
        [] => "obs:off".to_string(),
        [path] if state.auto_paths.contains(path) => format!(
            "obs:auto:{}",
            path.file_name().and_then(|s| s.to_str()).unwrap_or("log")
        ),
        [path] => format!(
            "obs:{}",
            path.file_name().and_then(|s| s.to_str()).unwrap_or("log")
        ),
        paths if paths.iter().all(|path| state.auto_paths.contains(path)) => {
            format!("obs:auto:{}", paths.len())
        }
        paths => format!("obs:{} files", paths.len()),
    }
}

async fn collect_observer_path(state: &mut ObserverState, path: PathBuf) {
    let mut file_state = state.files.remove(&path).unwrap_or_default();
    let is_auto = state.auto_paths.contains(&path);

    let metadata = match tokio::fs::metadata(&path).await {
        Ok(metadata) => metadata,
        Err(err) => {
            push_observer_event(state, observer_error_event(&path, "stat", err.to_string()));
            state.files.insert(path, file_state);
            return;
        }
    };
    if is_observer_json_document(&path) {
        collect_observer_json_document_path(state, path, file_state, &metadata).await;
        return;
    }
    if metadata.len() < file_state.offset {
        file_state.offset = 0;
        file_state.pending.clear();
    }
    if is_auto && file_state.offset == 0 && metadata.len() > OBSERVER_AUTO_INITIAL_TAIL_BYTES {
        file_state.offset = metadata
            .len()
            .saturating_sub(OBSERVER_AUTO_INITIAL_TAIL_BYTES);
        file_state.pending.clear();
    }

    let mut file = match tokio::fs::File::open(&path).await {
        Ok(file) => file,
        Err(err) => {
            push_observer_event(state, observer_error_event(&path, "read", err.to_string()));
            state.files.insert(path, file_state);
            return;
        }
    };
    if let Err(err) = file.seek(SeekFrom::Start(file_state.offset)).await {
        push_observer_event(state, observer_error_event(&path, "seek", err.to_string()));
        state.files.insert(path, file_state);
        return;
    }

    let mut chunk = String::new();
    match file.read_to_string(&mut chunk).await {
        Ok(_) => {
            file_state.offset = file_state.offset.saturating_add(chunk.len() as u64);
            let rows = append_observer_chunk(&mut file_state, &chunk);
            push_observer_rows(state, rows);
        }
        Err(err) => {
            push_observer_event(state, observer_error_event(&path, "read", err.to_string()))
        }
    }
    state.files.insert(path, file_state);
}

async fn collect_observer_json_document_path(
    state: &mut ObserverState,
    path: PathBuf,
    mut file_state: ObserverFileState,
    metadata: &std::fs::Metadata,
) {
    let text = match tokio::fs::read_to_string(&path).await {
        Ok(text) => text,
        Err(err) => {
            push_observer_event(state, observer_error_event(&path, "read", err.to_string()));
            state.files.insert(path, file_state);
            return;
        }
    };
    let mut rows = parse_observer_json_document(&text);
    if metadata.len() < file_state.offset || rows.len() < file_state.json_events_seen {
        file_state.json_events_seen = 0;
    }
    let seen = file_state.json_events_seen.min(rows.len());
    file_state.json_events_seen = rows.len();
    file_state.offset = metadata.len();
    if seen < rows.len() {
        rows.drain(..seen);
        rows.reverse();
        push_observer_rows(state, rows);
    }
    state.files.insert(path, file_state);
}

fn is_observer_json_document(path: &Path) -> bool {
    path.extension()
        .and_then(|ext| ext.to_str())
        .map(|ext| ext.eq_ignore_ascii_case("json"))
        .unwrap_or(false)
}

fn observer_error_event(path: &std::path::Path, action: &str, error: String) -> EventRow {
    EventRow {
        ts: "now".into(),
        source: "observer".into(),
        session: None,
        task: None,
        pid: None,
        ppid: None,
        kind: "error".into(),
        message: format!(
            "failed to {action} observer log {}: {error}",
            path.display()
        ),
        details: vec![
            ("path".into(), path.display().to_string()),
            ("error".into(), error),
        ],
        risk: Risk::Medium,
    }
}

fn append_observer_chunk(file: &mut ObserverFileState, chunk: &str) -> Vec<EventRow> {
    if chunk.is_empty() && file.pending.is_empty() {
        return Vec::new();
    }
    let mut text = String::new();
    text.push_str(&file.pending);
    text.push_str(chunk);

    let complete = if text.ends_with('\n') {
        file.pending.clear();
        text.as_str()
    } else if let Some((head, tail)) = text.rsplit_once('\n') {
        file.pending = tail.to_string();
        head
    } else {
        file.pending = text;
        ""
    };

    let mut rows = complete
        .lines()
        .filter_map(parse_observer_line)
        .collect::<Vec<_>>();
    rows.reverse();
    rows
}

fn push_observer_rows(state: &mut ObserverState, mut rows: Vec<EventRow>) {
    if rows.is_empty() {
        return;
    }
    rows.append(&mut state.events);
    state.events = rows;
}

fn push_observer_event(state: &mut ObserverState, event: EventRow) {
    state.events.insert(0, event);
}

fn trim_observer_events(events: &mut Vec<EventRow>) {
    if events.len() <= OBSERVER_EVENT_LIMIT {
        events.sort_by_key(|event| std::cmp::Reverse(observer_event_sort_key(event)));
        return;
    }
    events.sort_by_key(|event| std::cmp::Reverse(observer_event_sort_key(event)));
    events.truncate(OBSERVER_EVENT_LIMIT);
}

fn observer_event_sort_key(event: &EventRow) -> u128 {
    observer_timestamp_sort_key(&event.ts).unwrap_or(0)
}

fn observer_timestamp_sort_key(ts: &str) -> Option<u128> {
    let ts = ts.trim();
    if ts.is_empty() || matches!(ts, "recent" | "now") {
        return Some(u128::MAX);
    }
    if ts.chars().all(|c| c.is_ascii_digit()) {
        let value = ts.parse::<u128>().ok()?;
        return Some(match ts.len() {
            0..=10 => value.saturating_mul(1000),
            11..=13 => value,
            14..=16 => value / 1000,
            _ => value / 1_000_000,
        });
    }
    parse_iso_timestamp_millis(ts)
}

fn parse_iso_timestamp_millis(ts: &str) -> Option<u128> {
    let mut digits = ts
        .chars()
        .filter(|c| c.is_ascii_digit())
        .map(|c| (c as u8 - b'0') as i32);
    let year = take_digits(&mut digits, 4)?;
    let month = take_digits(&mut digits, 2)?;
    let day = take_digits(&mut digits, 2)?;
    let hour = take_digits(&mut digits, 2).unwrap_or(0);
    let minute = take_digits(&mut digits, 2).unwrap_or(0);
    let second = take_digits(&mut digits, 2).unwrap_or(0);
    let millis = take_digits(&mut digits, 3).unwrap_or(0);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || !(0..=23).contains(&hour)
        || !(0..=59).contains(&minute)
        || !(0..=60).contains(&second)
    {
        return None;
    }
    let days = days_from_civil(year, month, day)?;
    Some(
        (days as u128)
            .saturating_mul(86_400_000)
            .saturating_add((hour as u128).saturating_mul(3_600_000))
            .saturating_add((minute as u128).saturating_mul(60_000))
            .saturating_add((second as u128).saturating_mul(1000))
            .saturating_add(millis as u128),
    )
}

fn take_digits(digits: &mut impl Iterator<Item = i32>, count: usize) -> Option<i32> {
    let mut value = 0;
    for _ in 0..count {
        value = value * 10 + digits.next()?;
    }
    Some(value)
}

fn days_from_civil(year: i32, month: i32, day: i32) -> Option<i64> {
    let year = year - (month <= 2) as i32;
    let era = if year >= 0 { year } else { year - 399 } / 400;
    let yoe = year - era * 400;
    let month = month + if month > 2 { -3 } else { 9 };
    let doy = (153 * month + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era as i64 * 146_097 + doe as i64 - 719_468;
    (days >= 0).then_some(days)
}

async fn collect_container_logs(
    connector: ContainerConnector,
    id: &str,
    timestamps: bool,
) -> Result<String, String> {
    let mut command = match connector {
        ContainerConnector::A3sBox => {
            let a3s_box = ensure_a3s_box_binary()
                .await
                .map_err(|err| err.to_string())?;
            let mut command = Command::new(a3s_box);
            command.args(["logs", "--tail", "200"]);
            command
        }
        ContainerConnector::Docker => {
            let mut command = Command::new("docker");
            command.args(["logs", "--tail", "200"]);
            command
        }
        ContainerConnector::RunC => {
            return Err("runc logs are not supported".to_string());
        }
    };
    if timestamps {
        command.arg("--timestamps");
    }
    command.arg(id);

    let output = command.output().await.map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        let mut text = String::new();
        text.push_str(stdout.trim_end());
        if !stderr.trim().is_empty() {
            if !text.is_empty() {
                text.push('\n');
            }
            text.push_str(stderr.trim_end());
        }
        Ok(text)
    } else {
        let reason = if stderr.trim().is_empty() {
            output.status.to_string()
        } else {
            stderr.trim().to_string()
        };
        Err(reason)
    }
}

async fn collect_container_processes(
    connector: ContainerConnector,
    id: &str,
) -> Result<Vec<ContainerProcessRow>, String> {
    match connector {
        ContainerConnector::A3sBox => collect_a3s_box_container_processes(id).await,
        ContainerConnector::Docker => collect_docker_container_processes(id).await,
        ContainerConnector::RunC => collect_runc_container_processes(id).await,
    }
}

async fn collect_a3s_box_container_processes(id: &str) -> Result<Vec<ContainerProcessRow>, String> {
    let a3s_box = ensure_a3s_box_binary()
        .await
        .map_err(|err| err.to_string())?;
    collect_a3s_box_container_processes_with_binary(&a3s_box, id).await
}

async fn collect_a3s_box_container_processes_with_binary(
    a3s_box: &Path,
    id: &str,
) -> Result<Vec<ContainerProcessRow>, String> {
    let json = Command::new(a3s_box)
        .args(["top", id, "--format", "json"])
        .output()
        .await
        .map_err(|err| err.to_string())?;
    if json.status.success() {
        let stdout = String::from_utf8_lossy(&json.stdout);
        if let Some(rows) = parse_a3s_box_top_json(&stdout) {
            return Ok(filter_a3s_box_probe_processes(rows));
        }
    }

    let output = Command::new(a3s_box)
        .args(["top", id, "--", "-eo", "pid,ppid,pcpu,pmem,etime,args"])
        .output()
        .await
        .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(filter_a3s_box_probe_processes(
            parse_container_process_table(&stdout),
        ))
    } else if stderr.trim().is_empty() {
        Err(output.status.to_string())
    } else {
        Err(stderr.trim().to_string())
    }
}

fn parse_a3s_box_top_json(text: &str) -> Option<Vec<ContainerProcessRow>> {
    let value: serde_json::Value = serde_json::from_str(text.trim()).ok()?;
    let items = value.as_array()?;
    Some(
        items
            .iter()
            .filter_map(parse_a3s_box_top_json_row)
            .collect(),
    )
}

fn parse_a3s_box_top_json_row(item: &serde_json::Value) -> Option<ContainerProcessRow> {
    Some(ContainerProcessRow {
        pid: json_value_string(item, &["pid", "PID"])?,
        ppid: json_value_string(item, &["ppid", "parent_pid", "PPID"])
            .unwrap_or_else(|| "-".to_string()),
        cpu_pct: json_f64_field(item, &["cpu_percent", "cpu_pct", "pcpu", "%cpu", "CPU"])
            .map(|value| value as f32),
        mem_pct: json_f64_field(
            item,
            &[
                "memory_percent",
                "mem_percent",
                "mem_pct",
                "pmem",
                "%mem",
                "MEM",
            ],
        )
        .map(|value| value as f32),
        elapsed: json_value_string(item, &["elapsed", "etime", "time", "ELAPSED"])
            .unwrap_or_else(|| "-".to_string()),
        command: json_value_string(item, &["command", "cmd", "args", "COMMAND"])
            .unwrap_or_else(|| "-".to_string()),
    })
}

fn json_value_string(value: &serde_json::Value, fields: &[&str]) -> Option<String> {
    fields.iter().find_map(|field| {
        let value = value.get(*field)?;
        if let Some(text) = value.as_str() {
            return Some(text.trim().to_string()).filter(|text| !text.is_empty());
        }
        if let Some(number) = value.as_u64() {
            return Some(number.to_string());
        }
        if let Some(number) = value.as_i64() {
            return Some(number.to_string());
        }
        if let Some(number) = value.as_f64() {
            return Some(number.to_string());
        }
        None
    })
}

fn filter_a3s_box_probe_processes(mut rows: Vec<ContainerProcessRow>) -> Vec<ContainerProcessRow> {
    if let Some(idx) = rows
        .iter()
        .position(|row| is_a3s_box_process_probe(&row.command))
    {
        rows.remove(idx);
    }
    rows
}

fn a3s_box_process_count_from_rows(rows: &[ContainerProcessRow]) -> usize {
    rows.iter()
        .filter(|row| !is_a3s_box_process_probe(&row.command))
        .count()
}

fn is_a3s_box_process_probe(command: &str) -> bool {
    let command = command.trim();
    command == "ps"
        || command.starts_with("ps -eo pid,ppid,pcpu,pmem,etime,args")
        || command.starts_with("/bin/ps -eo pid,ppid,pcpu,pmem,etime,args")
        || command.starts_with("/usr/bin/ps -eo pid,ppid,pcpu,pmem,etime,args")
}

async fn collect_docker_container_processes(id: &str) -> Result<Vec<ContainerProcessRow>, String> {
    let output = Command::new("docker")
        .args(["top", id, "-eo", "pid,ppid,pcpu,pmem,etime,args"])
        .output()
        .await
        .map_err(|err| err.to_string())?;
    if output.status.success() {
        let rows = parse_container_process_table(&String::from_utf8_lossy(&output.stdout));
        if !rows.is_empty() {
            return Ok(rows);
        }
    }

    let fallback = Command::new("docker")
        .args(["top", id])
        .output()
        .await
        .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&fallback.stdout);
    let stderr = String::from_utf8_lossy(&fallback.stderr);
    if fallback.status.success() {
        Ok(parse_container_process_table(&stdout))
    } else if stderr.trim().is_empty() {
        Err(fallback.status.to_string())
    } else {
        Err(stderr.trim().to_string())
    }
}

async fn collect_runc_container_processes(id: &str) -> Result<Vec<ContainerProcessRow>, String> {
    let mut command = runc_command();
    let output = command
        .args(["ps", id])
        .output()
        .await
        .map_err(|err| err.to_string())?;
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    if output.status.success() {
        Ok(parse_runc_processes(&stdout))
    } else if stderr.trim().is_empty() {
        Err(output.status.to_string())
    } else {
        Err(stderr.trim().to_string())
    }
}

fn parse_container_process_table(text: &str) -> Vec<ContainerProcessRow> {
    let mut lines = text.lines().filter(|line| !line.trim().is_empty());
    let Some(header) = lines.next() else {
        return Vec::new();
    };
    let headers = header
        .split_whitespace()
        .map(|part| part.trim().to_ascii_uppercase())
        .collect::<Vec<_>>();
    let pid_idx = headers.iter().position(|part| part == "PID");
    let ppid_idx = headers.iter().position(|part| part == "PPID");
    let cpu_idx = headers
        .iter()
        .position(|part| matches!(part.as_str(), "%CPU" | "PCPU" | "CPU%"));
    let mem_idx = headers
        .iter()
        .position(|part| matches!(part.as_str(), "%MEM" | "PMEM" | "MEM%"));
    let elapsed_idx = headers
        .iter()
        .position(|part| matches!(part.as_str(), "ELAPSED" | "ETIME" | "TIME"));
    let command_idx = headers
        .iter()
        .position(|part| matches!(part.as_str(), "COMMAND" | "CMD" | "ARGS"));

    lines
        .filter_map(|line| {
            parse_container_process_line(
                line,
                pid_idx?,
                ppid_idx,
                cpu_idx,
                mem_idx,
                elapsed_idx,
                command_idx,
            )
        })
        .collect()
}

fn parse_container_process_line(
    line: &str,
    pid_idx: usize,
    ppid_idx: Option<usize>,
    cpu_idx: Option<usize>,
    mem_idx: Option<usize>,
    elapsed_idx: Option<usize>,
    command_idx: Option<usize>,
) -> Option<ContainerProcessRow> {
    let parts = line.split_whitespace().collect::<Vec<_>>();
    let pid = parts.get(pid_idx)?.to_string();
    let ppid = ppid_idx
        .and_then(|idx| parts.get(idx))
        .map(|part| (*part).to_string())
        .unwrap_or_else(|| "-".to_string());
    let command = command_idx
        .and_then(|idx| (idx < parts.len()).then(|| parts[idx..].join(" ")))
        .unwrap_or_else(|| "-".to_string());
    Some(ContainerProcessRow {
        pid,
        ppid,
        cpu_pct: cpu_idx
            .and_then(|idx| parts.get(idx))
            .and_then(|value| parse_percent(value)),
        mem_pct: mem_idx
            .and_then(|idx| parts.get(idx))
            .and_then(|value| parse_percent(value)),
        elapsed: elapsed_idx
            .and_then(|idx| parts.get(idx))
            .map(|part| (*part).to_string())
            .unwrap_or_else(|| "-".to_string()),
        command,
    })
}

fn parse_runc_processes(text: &str) -> Vec<ContainerProcessRow> {
    text.lines()
        .filter_map(|line| {
            let pid = line.split_whitespace().next()?.trim();
            if pid.parse::<u32>().is_err() {
                return None;
            }
            Some(ContainerProcessRow {
                pid: pid.to_string(),
                ppid: "-".to_string(),
                cpu_pct: None,
                mem_pct: None,
                elapsed: "-".to_string(),
                command: "runc process".to_string(),
            })
        })
        .collect()
}

fn container_menu_items(container: &ContainerRow) -> Vec<ContainerMenuItem> {
    let running = container_is_running(&container.status);
    let paused = container_is_paused(&container.status);
    let mut items = vec![ContainerMenuItem {
        action: ContainerMenuAction::Focus,
        key: 'o',
        label: "Open single view".to_string(),
    }];

    if matches!(
        container.connector,
        ContainerConnector::A3sBox | ContainerConnector::Docker
    ) {
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::Logs,
            key: 'l',
            label: "View logs".to_string(),
        });
    }
    if container_web_url(container).is_some() {
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::OpenBrowser,
            key: 'w',
            label: "Open browser".to_string(),
        });
    }

    if running && !paused {
        if matches!(
            container.connector,
            ContainerConnector::A3sBox | ContainerConnector::Docker
        ) {
            items.push(ContainerMenuItem {
                action: ContainerMenuAction::ExecShell,
                key: 'e',
                label: "Exec shell".to_string(),
            });
        }
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::Pause,
            key: 'p',
            label: "Pause container".to_string(),
        });
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::Stop,
            key: 's',
            label: "Stop container".to_string(),
        });
        if matches!(
            container.connector,
            ContainerConnector::A3sBox | ContainerConnector::Docker
        ) {
            items.push(ContainerMenuItem {
                action: ContainerMenuAction::Restart,
                key: 'r',
                label: "Restart container".to_string(),
            });
        }
    } else if paused {
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::Unpause,
            key: 'u',
            label: "Unpause container".to_string(),
        });
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::Stop,
            key: 's',
            label: "Stop container".to_string(),
        });
        if matches!(
            container.connector,
            ContainerConnector::A3sBox | ContainerConnector::Docker
        ) {
            items.push(ContainerMenuItem {
                action: ContainerMenuAction::Restart,
                key: 'r',
                label: "Restart container".to_string(),
            });
        }
    } else {
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::Start,
            key: 's',
            label: "Start container".to_string(),
        });
        items.push(ContainerMenuItem {
            action: ContainerMenuAction::Remove,
            key: 'd',
            label: "Remove container".to_string(),
        });
    }

    items
}

fn container_action(container: ContainerRow, action: ContainerMenuAction) -> Action {
    let name = format!("{} ({})", container.name, short_id(&container.id));
    match action {
        ContainerMenuAction::Start => {
            Action::StartContainer(container.connector, container.id, name)
        }
        ContainerMenuAction::Stop => Action::StopContainer(container.connector, container.id, name),
        ContainerMenuAction::Restart => {
            Action::RestartContainer(container.connector, container.id, name)
        }
        ContainerMenuAction::Pause => {
            Action::PauseContainer(container.connector, container.id, name)
        }
        ContainerMenuAction::Unpause => {
            Action::UnpauseContainer(container.connector, container.id, name)
        }
        ContainerMenuAction::Remove => {
            Action::RemoveContainer(container.connector, container.id, name)
        }
        ContainerMenuAction::Focus
        | ContainerMenuAction::Logs
        | ContainerMenuAction::ExecShell
        | ContainerMenuAction::OpenBrowser => {
            unreachable!("non-confirmable menu actions are handled before confirmation")
        }
    }
}

fn container_menu_lines(menu: &ContainerMenu, width: u16, height: usize) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let container = &menu.container;
    let items = menu
        .items
        .iter()
        .map(|item| MenuItem::new(item.key.to_string()).description(item.label.clone()))
        .collect::<Vec<_>>();
    MenuPanel::new(format!(
        "container menu {} ({})",
        container.name,
        short_id(&container.id)
    ))
    .subtitle(format!(
        "image {} · status {}",
        container.image, container.status
    ))
    .items(items)
    .selected(menu.select.selected_index())
    .max_items(height)
    .show_scroll(true)
    .indent(1)
    .marker(">")
    .title_color(CYAN)
    .subtitle_color(Color::BrightBlack)
    .text_color(Color::BrightWhite)
    .muted_color(Color::BrightBlack)
    .selected_colors(Color::BrightWhite, ACCENT)
    .view(width, height.saturating_add(2))
    .lines()
    .map(str::to_string)
    .collect()
}

fn column_panel_lines(panel: &ColumnPanel, width: u16, height: usize) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    let items = panel
        .choices
        .iter()
        .enumerate()
        .map(|(idx, choice)| MenuItem::new(choice.label).checked(panel.select.is_checked(idx)))
        .collect::<Vec<_>>();

    MenuPanel::new(format!("columns · {}", panel.tab.label()))
        .items(items)
        .selected(panel.select.cursor())
        .max_items(height)
        .show_scroll(true)
        .number_shortcuts(true)
        .indent(1)
        .marker(">")
        .title_color(ACCENT)
        .text_color(Color::BrightWhite)
        .muted_color(Color::BrightBlack)
        .checked_color(Color::Green)
        .selected_colors(Color::BrightWhite, ACCENT)
        .view(width, height.saturating_add(2))
        .lines()
        .map(str::to_string)
        .collect()
}

fn sort_panel_lines(panel: &SortPanel, width: u16, height: usize) -> Vec<String> {
    let items = panel
        .choices
        .iter()
        .enumerate()
        .map(|(idx, sort_by)| MenuItem::new(numbered_label(idx, sort_choice_label(*sort_by))))
        .collect::<Vec<_>>();
    option_menu_lines(
        "sort by",
        "choose the primary ordering for the current top view",
        items,
        panel.select.selected_index(),
        width,
        height,
    )
}

fn connector_panel_lines(panel: &ConnectorPanel, width: u16, height: usize) -> Vec<String> {
    let items = panel
        .choices
        .iter()
        .enumerate()
        .map(|(idx, connector)| {
            MenuItem::new(numbered_label(idx, connector_choice_label(*connector)))
        })
        .collect::<Vec<_>>();
    option_menu_lines(
        "container connector",
        "choose which runtime feeds the Containers tab",
        items,
        panel.select.selected_index(),
        width,
        height,
    )
}

fn option_menu_lines(
    title: &str,
    subtitle: &str,
    items: Vec<MenuItem>,
    selected: usize,
    width: u16,
    height: usize,
) -> Vec<String> {
    if width == 0 || height == 0 {
        return Vec::new();
    }

    MenuPanel::new(title)
        .subtitle(subtitle)
        .items(items)
        .selected(selected)
        .max_items(height)
        .show_scroll(true)
        .indent(1)
        .marker(">")
        .title_color(CYAN)
        .subtitle_color(Color::BrightBlack)
        .text_color(Color::BrightWhite)
        .muted_color(Color::BrightBlack)
        .selected_colors(Color::BrightWhite, ACCENT)
        .view(width, height.saturating_add(2))
        .lines()
        .map(str::to_string)
        .collect()
}

fn numbered_label(index: usize, label: &str) -> String {
    number_shortcut_label(index)
        .map(|shortcut| format!("{shortcut} {label}"))
        .unwrap_or_else(|| format!("  {label}"))
}

fn number_shortcut_label(index: usize) -> Option<char> {
    match index {
        0..=8 => Some((b'1' + index as u8) as char),
        9 => Some('0'),
        _ => None,
    }
}

fn container_is_running(status: &str) -> bool {
    let lower = status.to_lowercase();
    lower.starts_with("up") || lower.contains("running") || lower.contains("paused")
}

fn container_is_paused(status: &str) -> bool {
    status.to_lowercase().contains("paused")
}

fn parse_observer_line(line: &str) -> Option<EventRow> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    parse_a3s_observer_value(&value)
        .or_else(|| parse_claude_jsonl_value(&value))
        .or_else(|| parse_codex_jsonl_value(&value))
}

fn parse_observer_json_document(text: &str) -> Vec<EventRow> {
    let Ok(value) = serde_json::from_str::<serde_json::Value>(text) else {
        return Vec::new();
    };

    match value {
        serde_json::Value::Array(items) => items
            .iter()
            .flat_map(parse_observer_json_document_item)
            .collect(),
        item => parse_observer_json_document_item(&item),
    }
}

fn parse_observer_json_document_item(value: &serde_json::Value) -> Vec<EventRow> {
    if let Some(rows) = parse_a3s_run_record_value(value) {
        return rows;
    }
    if let Some(row) = parse_a3s_trace_value(value) {
        return vec![row];
    }
    parse_a3s_observer_value(value)
        .or_else(|| parse_claude_jsonl_value(value))
        .or_else(|| parse_codex_jsonl_value(value))
        .into_iter()
        .collect()
}

fn parse_a3s_observer_value(value: &serde_json::Value) -> Option<EventRow> {
    let identity = value.get("identity")?;
    let ts = observer_event_timestamp(value).unwrap_or_else(|| "recent".into());
    let source = identity
        .get("agent")
        .and_then(|v| v.as_str())
        .unwrap_or("agent")
        .to_string();
    let session = identity.get("session").and_then(compact_identity_value);
    let task = identity.get("task").and_then(compact_identity_value);
    let event = value.get("event")?.as_object()?;
    let (kind, payload) = event.iter().next()?;
    let mut details = event_payload_details(payload);
    if let Some(provider) = value.get("provider").and_then(compact_provider_value) {
        details.insert(0, ("provider".into(), provider));
    }
    let pid = event_detail_u32(&details, &["pid"]);
    let ppid = event_detail_u32(&details, &["ppid", "parent_pid", "parentPid"]);
    let message = event_payload_message(payload, &details);
    let risk = match kind.as_str() {
        "SecurityAction" | "FileDelete" => Risk::High,
        "Egress" | "FileAccess" | "ToolExec" => Risk::Medium,
        _ => Risk::Low,
    };
    Some(EventRow {
        ts,
        source,
        session,
        task,
        pid,
        ppid,
        kind: kind.clone(),
        message,
        details,
        risk,
    })
}

fn parse_claude_jsonl_value(value: &serde_json::Value) -> Option<EventRow> {
    let type_name = value.get("type").and_then(|v| v.as_str())?;
    if value.get("message").is_none()
        && !matches!(
            type_name,
            "mode" | "permission-mode" | "pr-link" | "last-prompt" | "ai-title" | "system"
        )
    {
        return None;
    }

    let mut details = Vec::new();
    push_json_detail(&mut details, "type", value.get("type"));
    push_json_detail(&mut details, "cwd", value.get("cwd"));
    push_json_detail(&mut details, "git_branch", value.get("gitBranch"));
    push_json_detail(&mut details, "version", value.get("version"));
    let session = value
        .get("sessionId")
        .and_then(compact_identity_value)
        .or_else(|| value.get("session_id").and_then(compact_identity_value));
    let task = value.get("uuid").and_then(compact_identity_value);
    let ts = observer_event_timestamp(value).unwrap_or_else(|| "recent".into());

    let (kind, message, risk) = match type_name {
        "assistant" => {
            let message_value = value.get("message")?;
            push_json_detail(&mut details, "model", message_value.get("model"));
            if let Some(usage) = message_value.get("usage") {
                push_usage_details(&mut details, usage);
            }
            if let Some(tool) = claude_tool_use(message_value) {
                push_json_detail(&mut details, "tool", tool.get("name"));
                if let Some(input) = tool.get("input") {
                    push_json_detail(&mut details, "command", input.get("command"));
                    push_json_detail(&mut details, "description", input.get("description"));
                }
                (
                    "ToolExec".to_string(),
                    claude_tool_message(tool),
                    Risk::Medium,
                )
            } else {
                (
                    "LlmCall".to_string(),
                    claude_message_text(message_value)
                        .unwrap_or_else(|| "assistant message".into()),
                    Risk::Low,
                )
            }
        }
        "user" => (
            "UserPrompt".to_string(),
            value
                .get("message")
                .and_then(claude_message_text)
                .unwrap_or_else(|| "user prompt".into()),
            Risk::Low,
        ),
        "permission-mode" => (
            "SecurityAction".to_string(),
            format!(
                "permission mode {}",
                value
                    .get("permissionMode")
                    .and_then(|v| v.as_str())
                    .unwrap_or("changed")
            ),
            Risk::Medium,
        ),
        "pr-link" => (
            "ToolExec".to_string(),
            format!(
                "opened PR {}",
                value
                    .get("prUrl")
                    .and_then(|v| v.as_str())
                    .unwrap_or("link")
            ),
            Risk::Low,
        ),
        "mode" | "last-prompt" | "ai-title" | "system" => (
            "AgentEvent".to_string(),
            claude_generic_message(value, type_name),
            Risk::Low,
        ),
        other => (
            "AgentEvent".to_string(),
            format!("claude {other}"),
            Risk::Low,
        ),
    };

    Some(EventRow {
        ts,
        source: "claude".into(),
        session,
        task,
        pid: None,
        ppid: None,
        kind,
        message,
        details,
        risk,
    })
}

fn parse_codex_jsonl_value(value: &serde_json::Value) -> Option<EventRow> {
    let type_name = value.get("type").and_then(|v| v.as_str())?;
    let looks_like_codex = value.get("payload").is_some()
        || value.get("session_id").is_some()
        || matches!(type_name, "event_msg" | "response_item");
    if !looks_like_codex {
        return None;
    }

    let payload = value.get("payload").unwrap_or(value);
    let payload_type = payload
        .get("type")
        .and_then(|v| v.as_str())
        .unwrap_or(type_name);
    let mut details = vec![("type".into(), payload_type.to_string())];
    let session = value
        .get("session_id")
        .and_then(compact_identity_value)
        .or_else(|| payload.get("session_id").and_then(compact_identity_value));
    let task = payload
        .get("turn_id")
        .and_then(compact_identity_value)
        .or_else(|| {
            payload
                .get("internal_chat_message_metadata_passthrough")
                .and_then(|v| v.get("turn_id"))
                .and_then(compact_identity_value)
        });
    let ts = observer_event_timestamp(value)
        .or_else(|| observer_event_timestamp(payload))
        .unwrap_or_else(|| "recent".into());

    let (kind, message, risk) = match payload_type {
        "token_count" => {
            if let Some(info) = payload.get("info") {
                push_codex_token_usage(&mut details, info);
            }
            ("LlmCall".into(), "token usage update".into(), Risk::Low)
        }
        "message" => {
            push_json_detail(&mut details, "role", payload.get("role"));
            let phase = payload
                .get("phase")
                .and_then(|v| v.as_str())
                .unwrap_or("message");
            (
                "LlmCall".into(),
                codex_message_text(payload).unwrap_or_else(|| format!("codex {phase}")),
                Risk::Low,
            )
        }
        "task_complete" => {
            push_json_detail(&mut details, "duration_ms", payload.get("duration_ms"));
            push_json_detail(
                &mut details,
                "time_to_first_token_ms",
                payload.get("time_to_first_token_ms"),
            );
            (
                "AgentEvent".into(),
                payload
                    .get("last_agent_message")
                    .and_then(|v| v.as_str())
                    .map(|value| truncate(value, 120))
                    .unwrap_or_else(|| "task complete".into()),
                Risk::Low,
            )
        }
        "exec_command_begin" | "exec_command_end" | "tool_call" => {
            push_json_detail(&mut details, "cmd", payload.get("cmd"));
            push_json_detail(&mut details, "command", payload.get("command"));
            (
                "ToolExec".into(),
                format!("codex {payload_type}"),
                Risk::Medium,
            )
        }
        other => ("AgentEvent".into(), format!("codex {other}"), Risk::Low),
    };

    Some(EventRow {
        ts,
        source: "codex".into(),
        session,
        task,
        pid: None,
        ppid: None,
        kind,
        message,
        details,
        risk,
    })
}

fn parse_a3s_run_record_value(value: &serde_json::Value) -> Option<Vec<EventRow>> {
    let snapshot = value.get("snapshot")?;
    let events = value.get("events")?.as_array()?;
    let session = snapshot
        .get("session_id")
        .and_then(compact_identity_value)
        .or_else(|| snapshot.get("id").and_then(compact_identity_value));
    let task = snapshot.get("id").and_then(compact_identity_value);
    let workspace = snapshot
        .get("workspace")
        .or_else(|| snapshot.get("cwd"))
        .and_then(|v| v.as_str())
        .map(str::to_string);

    Some(
        events
            .iter()
            .filter_map(|entry| {
                parse_a3s_run_event(entry, session.clone(), task.clone(), &workspace)
            })
            .collect(),
    )
}

fn parse_a3s_run_event(
    value: &serde_json::Value,
    session: Option<String>,
    task: Option<String>,
    workspace: &Option<String>,
) -> Option<EventRow> {
    let event = value.get("event")?;
    let event_type = event.get("type").and_then(|v| v.as_str())?;
    let mut details = event_payload_details(event);
    if let Some(workspace) = workspace {
        details.push(("workspace".into(), workspace.clone()));
    }
    push_json_detail(&mut details, "sequence", value.get("sequence"));
    let ts = value
        .get("timestamp_ms")
        .and_then(compact_timestamp_field)
        .unwrap_or_else(|| "recent".into());
    let kind = a3s_event_kind(event_type);
    let risk = match kind {
        "ToolExec" => Risk::Medium,
        "SecurityAction" => Risk::High,
        _ => Risk::Low,
    };

    Some(EventRow {
        ts,
        source: "a3s-code".into(),
        session,
        task,
        pid: None,
        ppid: None,
        kind: kind.into(),
        message: a3s_event_message(event_type, event, &details),
        details,
        risk,
    })
}

fn parse_a3s_trace_value(value: &serde_json::Value) -> Option<EventRow> {
    if value.get("schema").and_then(|v| v.as_str()) != Some("a3s.trace_event.v1") {
        return None;
    }
    let kind_value = value
        .get("kind")
        .and_then(|v| v.as_str())
        .unwrap_or("trace");
    let mut details = event_payload_details(value);
    Some(EventRow {
        ts: "recent".into(),
        source: "a3s-code".into(),
        session: None,
        task: None,
        pid: None,
        ppid: None,
        kind: a3s_event_kind(kind_value).into(),
        message: a3s_event_message(kind_value, value, &details),
        details: {
            details.truncate(8);
            details
        },
        risk: if a3s_event_kind(kind_value) == "ToolExec" {
            Risk::Medium
        } else {
            Risk::Low
        },
    })
}

fn observer_event_timestamp(value: &serde_json::Value) -> Option<String> {
    [
        "ts",
        "time",
        "timestamp",
        "created_at",
        "createdAt",
        "observed_at",
        "observedAt",
    ]
    .iter()
    .find_map(|key| value.get(*key).and_then(compact_timestamp_field))
}

fn compact_timestamp_field(value: &serde_json::Value) -> Option<String> {
    let text = match value {
        serde_json::Value::Null => return None,
        serde_json::Value::String(s) => s.trim().to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        _ => return None,
    };
    (!text.is_empty()).then(|| truncate(&text, 40))
}

fn event_payload_message(payload: &serde_json::Value, details: &[(String, String)]) -> String {
    if matches!(payload, serde_json::Value::Object(_)) {
        details
            .iter()
            .take(4)
            .map(|(k, v)| format!("{k}={}", truncate(v, 80)))
            .collect::<Vec<_>>()
            .join(" ")
    } else {
        compact_json_value(payload)
    }
}

fn event_payload_details(payload: &serde_json::Value) -> Vec<(String, String)> {
    match payload {
        serde_json::Value::Object(map) => map
            .iter()
            .take(8)
            .map(|(k, v)| (truncate(k, 32), compact_json_value(v)))
            .collect(),
        serde_json::Value::Null => Vec::new(),
        other => vec![("value".into(), compact_json_value(other))],
    }
}

fn compact_provider_value(value: &serde_json::Value) -> Option<String> {
    match value {
        serde_json::Value::Null => None,
        serde_json::Value::String(s) => {
            let s = s.trim();
            (!s.is_empty()).then(|| s.to_string())
        }
        serde_json::Value::Object(map) => map
            .get("Other")
            .and_then(|v| v.as_str())
            .map(ToString::to_string)
            .or_else(|| Some(compact_json_value(value))),
        other => Some(compact_json_value(other)),
    }
}

fn compact_identity_value(value: &serde_json::Value) -> Option<String> {
    let text = match value {
        serde_json::Value::Null => return None,
        serde_json::Value::String(s) => s.trim().to_string(),
        other => other.to_string(),
    };
    (!text.is_empty()).then(|| truncate(&text, 18))
}

fn push_json_detail(
    details: &mut Vec<(String, String)>,
    key: &str,
    value: Option<&serde_json::Value>,
) {
    let Some(value) = value else {
        return;
    };
    if value.is_null() {
        return;
    }
    let compact = compact_json_value(value);
    if compact.trim().is_empty() || compact == "null" {
        return;
    }
    details.push((key.into(), compact));
}

fn push_usage_details(details: &mut Vec<(String, String)>, usage: &serde_json::Value) {
    for (detail_key, json_keys) in [
        (
            "input_tokens",
            &["input_tokens", "prompt_tokens", "cache_read_input_tokens"][..],
        ),
        (
            "output_tokens",
            &[
                "output_tokens",
                "completion_tokens",
                "reasoning_output_tokens",
            ][..],
        ),
        ("total_tokens", &["total_tokens"][..]),
        ("latency_ms", &["latency_ms", "duration_ms"][..]),
    ] {
        if let Some(value) = json_keys.iter().find_map(|key| usage.get(*key)) {
            push_json_detail(details, detail_key, Some(value));
        }
    }
}

fn push_codex_token_usage(details: &mut Vec<(String, String)>, info: &serde_json::Value) {
    if let Some(last) = info.get("last_token_usage") {
        for (detail_key, json_key) in [
            ("prompt_tokens", "input_tokens"),
            ("completion_tokens", "output_tokens"),
            ("total_tokens", "total_tokens"),
        ] {
            if let Some(value) = last.get(json_key) {
                push_json_detail(details, detail_key, Some(value));
            }
        }
    }
    if let Some(total) = info.get("total_token_usage") {
        for (detail_key, json_key) in [
            ("lifetime_input_tokens", "input_tokens"),
            ("lifetime_output_tokens", "output_tokens"),
            ("lifetime_total_tokens", "total_tokens"),
        ] {
            if let Some(value) = total.get(json_key) {
                push_json_detail(details, detail_key, Some(value));
            }
        }
    }
    push_json_detail(details, "context_window", info.get("model_context_window"));
}

fn claude_tool_use(message: &serde_json::Value) -> Option<&serde_json::Value> {
    message
        .get("content")
        .and_then(|content| content.as_array())
        .and_then(|items| {
            items.iter().find(|item| {
                item.get("type")
                    .and_then(|value| value.as_str())
                    .map(|value| value == "tool_use")
                    .unwrap_or(false)
            })
        })
}

fn claude_tool_message(tool: &serde_json::Value) -> String {
    let name = tool
        .get("name")
        .and_then(|value| value.as_str())
        .unwrap_or("tool");
    let input = tool.get("input");
    let detail = input
        .and_then(|value| value.get("description"))
        .or_else(|| input.and_then(|value| value.get("command")))
        .and_then(|value| value.as_str())
        .map(|value| truncate(value, 120))
        .unwrap_or_else(|| "invoked".to_string());
    format!("{name}: {detail}")
}

fn claude_message_text(message: &serde_json::Value) -> Option<String> {
    message_content_text(message.get("content")?)
}

fn codex_message_text(message: &serde_json::Value) -> Option<String> {
    message_content_text(message.get("content")?)
}

fn message_content_text(content: &serde_json::Value) -> Option<String> {
    match content {
        serde_json::Value::String(text) => non_empty_truncated(text, 120),
        serde_json::Value::Array(items) => {
            let text = items
                .iter()
                .filter_map(|item| {
                    item.get("text")
                        .or_else(|| item.get("content"))
                        .and_then(|value| value.as_str())
                })
                .collect::<Vec<_>>()
                .join(" ");
            non_empty_truncated(&text, 120)
        }
        other => non_empty_truncated(&compact_json_value(other), 120),
    }
}

fn non_empty_truncated(value: &str, max: usize) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| truncate(value, max))
}

fn claude_generic_message(value: &serde_json::Value, type_name: &str) -> String {
    for key in ["mode", "lastPrompt", "aiTitle", "subtype", "message"] {
        if let Some(text) = value.get(key).and_then(|v| v.as_str()) {
            return format!("{type_name}: {}", truncate(text, 120));
        }
    }
    format!("claude {type_name}")
}

fn a3s_event_kind(event_type: &str) -> &'static str {
    let lower = event_type.to_ascii_lowercase();
    if lower.contains("tool")
        || lower.contains("bash")
        || lower.contains("command")
        || lower.contains("program_execution")
        || lower.contains("tool_execution")
    {
        "ToolExec"
    } else if lower.contains("llm")
        || lower.contains("token")
        || lower.contains("turn_end")
        || lower.contains("generate")
    {
        "LlmCall"
    } else if lower.contains("permission")
        || lower.contains("security")
        || lower.contains("deny")
        || lower.contains("error")
    {
        "SecurityAction"
    } else if lower.contains("file") {
        "FileAccess"
    } else {
        "AgentEvent"
    }
}

fn a3s_event_message(
    event_type: &str,
    value: &serde_json::Value,
    details: &[(String, String)],
) -> String {
    value
        .get("name")
        .or_else(|| value.get("tool"))
        .or_else(|| value.get("message"))
        .or_else(|| value.get("prompt"))
        .and_then(|value| value.as_str())
        .map(|value| format!("{event_type}: {}", truncate(value, 120)))
        .unwrap_or_else(|| {
            if details.is_empty() {
                event_type.to_string()
            } else {
                format!(
                    "{event_type}: {}",
                    details
                        .iter()
                        .take(3)
                        .map(|(key, value)| format!("{key}={}", truncate(value, 60)))
                        .collect::<Vec<_>>()
                        .join(" ")
                )
            }
        })
}

fn event_scope_label(event: &EventRow) -> String {
    match (&event.session, &event.task) {
        (Some(session), Some(task)) => format!("session {session} task {task}"),
        (Some(session), None) => format!("session {session}"),
        (None, Some(task)) => format!("task {task}"),
        (None, None) => "no session".to_string(),
    }
}

fn event_workspace(event: &EventRow) -> Option<String> {
    event
        .details
        .iter()
        .find(|(key, value)| {
            let key = key.to_ascii_lowercase();
            matches!(
                key.as_str(),
                "cwd" | "workdir" | "working_dir" | "workingdirectory" | "workspace"
            ) && !value.trim().is_empty()
        })
        .map(|(_, value)| value.clone())
}

fn workspace_paths_overlap(a: &str, b: &str) -> bool {
    let Some(a) = normalize_workspace_path(a) else {
        return false;
    };
    let Some(b) = normalize_workspace_path(b) else {
        return false;
    };
    if a == b {
        return true;
    }

    let a_path = Path::new(&a);
    let b_path = Path::new(&b);
    a_path.is_absolute()
        && b_path.is_absolute()
        && (a_path.starts_with(b_path) || b_path.starts_with(a_path))
}

fn normalize_workspace_path(path: &str) -> Option<String> {
    let path = path.trim().trim_matches('"').trim();
    if path.is_empty() || path == "-" {
        None
    } else {
        Some(if path == "/" {
            path.to_string()
        } else {
            path.trim_end_matches('/').to_string()
        })
    }
}

fn is_llm_event_kind(kind: &str) -> bool {
    matches!(kind, "LlmCall" | "LlmApi" | "SslContent")
}

fn event_model(event: &EventRow) -> Option<String> {
    event_detail_value(event, &["model", "model_name", "modelName"])
}

fn event_provider(event: &EventRow) -> Option<String> {
    event_detail_value(event, &["provider", "provider_name", "providerName"])
}

fn event_llm_network(event: &EventRow) -> Option<LlmNetworkSummary> {
    if !is_llm_event_kind(&event.kind) {
        return None;
    }
    Some(LlmNetworkSummary {
        provider: event_provider(event),
        latency_ms: event_detail_millis(event, &["latency", "latency_ms", "latencyMs"]),
        ttft_ms: event_detail_millis(event, &["ttft", "ttft_ms", "ttftMs"]),
        req_bytes: event_detail_u64(event, &["req_bytes", "request_bytes", "reqBytes"])
            .unwrap_or(0),
        resp_bytes: event_detail_u64(event, &["resp_bytes", "response_bytes", "respBytes"])
            .unwrap_or(0),
    })
}

fn event_token_usage(event: &EventRow) -> Option<TokenUsageSummary> {
    let prompt = event_detail_u64(
        event,
        &[
            "prompt_tokens",
            "input_tokens",
            "promptTokens",
            "inputTokens",
        ],
    )
    .unwrap_or(0);
    let completion = event_detail_u64(
        event,
        &[
            "completion_tokens",
            "output_tokens",
            "completionTokens",
            "outputTokens",
        ],
    )
    .unwrap_or(0);
    let total = event_detail_u64(event, &["total_tokens", "totalTokens"])
        .unwrap_or_else(|| prompt.saturating_add(completion));
    (prompt > 0 || completion > 0 || total > 0).then_some(TokenUsageSummary {
        prompt,
        completion,
        total,
    })
}

fn event_detail_value(event: &EventRow, keys: &[&str]) -> Option<String> {
    event
        .details
        .iter()
        .find(|(key, value)| {
            keys.iter().any(|wanted| key.eq_ignore_ascii_case(wanted)) && !value.trim().is_empty()
        })
        .map(|(_, value)| value.trim().to_string())
}

fn event_detail_u64(event: &EventRow, keys: &[&str]) -> Option<u64> {
    event_detail_value(event, keys)
        .and_then(|value| value.trim_matches('"').replace('_', "").parse::<u64>().ok())
}

fn event_detail_u32(details: &[(String, String)], keys: &[&str]) -> Option<u32> {
    details
        .iter()
        .find(|(key, value)| {
            keys.iter().any(|wanted| key.eq_ignore_ascii_case(wanted)) && !value.trim().is_empty()
        })
        .and_then(|(_, value)| value.trim_matches('"').replace('_', "").parse::<u32>().ok())
}

fn event_detail_millis(event: &EventRow, keys: &[&str]) -> Option<u64> {
    event_detail_value(event, keys).and_then(|value| parse_millis_value(&value))
}

fn parse_millis_value(value: &str) -> Option<u64> {
    let value = value.trim().trim_matches('"');
    if value.is_empty() {
        return None;
    }
    if let Some(ms) = value.strip_suffix("ms") {
        return ms
            .trim()
            .parse::<f64>()
            .ok()
            .map(|ms| ms.max(0.0).round() as u64);
    }
    if let Some(sec) = value.strip_suffix('s') {
        return sec
            .trim()
            .parse::<f64>()
            .ok()
            .map(|sec| (sec.max(0.0) * 1000.0).round() as u64);
    }
    if let Ok(number) = value.parse::<f64>() {
        return Some(number.max(0.0).round() as u64);
    }
    let parsed = serde_json::from_str::<serde_json::Value>(value).ok()?;
    let secs = parsed
        .get("secs")
        .or_else(|| parsed.get("seconds"))
        .and_then(json_u64)
        .unwrap_or(0);
    let nanos = parsed
        .get("nanos")
        .or_else(|| parsed.get("nanoseconds"))
        .and_then(json_u64)
        .unwrap_or(0);
    Some(
        secs.saturating_mul(1000)
            .saturating_add((nanos as f64 / 1_000_000.0).round() as u64),
    )
}

async fn run_action(action: Action) -> String {
    match action {
        Action::KillProcess(pid, label) => {
            let status = Command::new("kill")
                .arg("-TERM")
                .arg(pid.to_string())
                .status()
                .await;
            match status {
                Ok(s) if s.success() => format!("sent SIGTERM to PID {pid} · {label}"),
                Ok(s) => format!("kill failed for PID {pid}: {s}"),
                Err(err) => format!("kill failed for PID {pid}: {err}"),
            }
        }
        Action::StartContainer(connector, id, name) => {
            run_container_runtime_action(connector, "start", &id, &name).await
        }
        Action::StopContainer(connector, id, name) => {
            run_container_runtime_action(connector, "stop", &id, &name).await
        }
        Action::RestartContainer(connector, id, name) => {
            run_container_runtime_action(connector, "restart", &id, &name).await
        }
        Action::PauseContainer(connector, id, name) => {
            run_container_runtime_action(connector, "pause", &id, &name).await
        }
        Action::UnpauseContainer(connector, id, name) => {
            run_container_runtime_action(connector, "unpause", &id, &name).await
        }
        Action::RemoveContainer(connector, id, name) => {
            run_container_runtime_action(connector, "rm", &id, &name).await
        }
    }
}

async fn run_container_runtime_action(
    connector: ContainerConnector,
    action: &str,
    id: &str,
    name: &str,
) -> String {
    match connector {
        ContainerConnector::A3sBox => run_a3s_box_container_action(action, id, name).await,
        ContainerConnector::Docker => run_docker_container_action(action, id, name).await,
        ContainerConnector::RunC => run_runc_container_action(action, id, name).await,
    }
}

async fn run_a3s_box_container_action(action: &str, id: &str, name: &str) -> String {
    let command = match action {
        "start" => "start",
        "stop" => "stop",
        "restart" => "restart",
        "pause" => "pause",
        "unpause" => "unpause",
        "rm" => "rm",
        _ => return format!("a3s-box {action} is not supported for {name}"),
    };
    let a3s_box = match ensure_a3s_box_binary().await {
        Ok(path) => path,
        Err(err) => return format!("a3s-box {command} failed for {name}: {err}"),
    };
    let status = Command::new(a3s_box).args([command, id]).status().await;
    match status {
        Ok(s) if s.success() => format!("a3s-box {command} succeeded for {name}"),
        Ok(s) => format!("a3s-box {command} failed for {name}: {s}"),
        Err(err) => format!("a3s-box {command} failed for {name}: {err}"),
    }
}

async fn run_docker_container_action(action: &str, id: &str, name: &str) -> String {
    let status = Command::new("docker").args([action, id]).status().await;
    match status {
        Ok(s) if s.success() => format!("docker {action} succeeded for {name}"),
        Ok(s) => format!("docker {action} failed for {name}: {s}"),
        Err(err) => format!("docker {action} failed for {name}: {err}"),
    }
}

async fn run_runc_container_action(action: &str, id: &str, name: &str) -> String {
    let mut command = runc_command();
    match action {
        "start" => {
            command.args(["start", id]);
        }
        "stop" => {
            command.args(["kill", id, "TERM"]);
        }
        "pause" => {
            command.args(["pause", id]);
        }
        "unpause" => {
            command.args(["resume", id]);
        }
        "rm" => {
            command.args(["delete", id]);
        }
        "restart" => {
            return format!("runc restart is not supported for {name}");
        }
        _ => return format!("runc {action} is not supported for {name}"),
    }

    let status = command.status().await;
    match status {
        Ok(s) if s.success() => format!("runc {action} succeeded for {name}"),
        Ok(s) => format!("runc {action} failed for {name}: {s}"),
        Err(err) => format!("runc {action} failed for {name}: {err}"),
    }
}

fn agent_matches_source(agent: AgentKind, source: &str) -> bool {
    let source = source.to_lowercase().replace(['_', ' '], "-");
    match agent {
        AgentKind::A3sCode => source == "a3s" || source.contains("a3s-code"),
        AgentKind::ClaudeCode => source.contains("claude"),
        AgentKind::Codex => source.contains("codex"),
        AgentKind::Cursor => source.contains("cursor"),
        AgentKind::Gemini => source.contains("gemini"),
        AgentKind::WorkBuddy => source.contains("workbuddy") || source.contains("codebuddy"),
    }
}

fn agent_order(agent: AgentKind) -> usize {
    AgentKind::ALL
        .iter()
        .position(|item| *item == agent)
        .unwrap_or(usize::MAX)
}

fn agent_tree_state_label(group: &AgentTreeGroup) -> &'static str {
    if group.activity.high_risk > 0 || group.risk() == Risk::High {
        "HIGH"
    } else if !group.processes.is_empty() {
        "RUN"
    } else if !group.sessions.is_empty() || !group.events.is_empty() {
        "RECENT"
    } else {
        "IDLE"
    }
}

fn agent_tree_label(agent: AgentKind, text: impl AsRef<str>) -> String {
    Style::new().fg(agent.color()).render(text.as_ref())
}

fn agent_kind_for_source(source: &str) -> Option<AgentKind> {
    AgentKind::ALL
        .iter()
        .copied()
        .find(|agent| agent_matches_source(*agent, source))
}

fn sort_choices_for_tab(tab: Tab) -> Vec<SortBy> {
    match tab {
        Tab::Agents => vec![
            SortBy::Cpu,
            SortBy::Mem,
            SortBy::Net,
            SortBy::Pids,
            SortBy::Name,
            SortBy::Tokens,
        ],
        Tab::Sessions => vec![
            SortBy::Cpu,
            SortBy::Mem,
            SortBy::Net,
            SortBy::Name,
            SortBy::Tokens,
        ],
        Tab::Containers => vec![
            SortBy::Cpu,
            SortBy::Mem,
            SortBy::Net,
            SortBy::Block,
            SortBy::Pids,
            SortBy::State,
            SortBy::Id,
            SortBy::Uptime,
            SortBy::Name,
        ],
        Tab::Processes => vec![
            SortBy::Cpu,
            SortBy::Mem,
            SortBy::Pids,
            SortBy::Id,
            SortBy::Name,
        ],
        Tab::Events => Vec::new(),
    }
}

fn sort_choice_label(sort_by: SortBy) -> &'static str {
    match sort_by {
        SortBy::Cpu => "cpu      CPU usage",
        SortBy::Mem => "mem      memory usage",
        SortBy::Net => "net      network I/O",
        SortBy::Block => "block    block I/O",
        SortBy::Pids => "pids     process count",
        SortBy::State => "state    container state",
        SortBy::Id => "id       container id / pid",
        SortBy::Uptime => "uptime   running duration",
        SortBy::Name => "name     name",
        SortBy::Tokens => "tokens   LLM token usage",
    }
}

fn container_sort_column_id(sort_by: SortBy) -> Option<&'static str> {
    match sort_by {
        SortBy::Cpu => Some("containers.cpu"),
        SortBy::Mem => Some("containers.mem"),
        SortBy::Net => Some("containers.net"),
        SortBy::Block => Some("containers.block"),
        SortBy::Pids => Some("containers.pids"),
        SortBy::State => Some("containers.status"),
        SortBy::Id => Some("containers.id"),
        SortBy::Uptime => Some("containers.uptime"),
        SortBy::Name => Some("containers.name"),
        SortBy::Tokens => None,
    }
}

fn connector_choice_label(connector: ContainerConnector) -> &'static str {
    match connector {
        ContainerConnector::A3sBox => "a3s-box A3S Box runtime",
        ContainerConnector::Docker => "docker   Docker Engine",
        ContainerConnector::RunC => "runc     runC containers",
    }
}

fn metric_color(value: f32) -> Color {
    if value >= 80.0 {
        RED
    } else if value >= 50.0 {
        YELLOW
    } else {
        GREEN
    }
}

fn scale_cpu_pct_to_system(value: f32) -> f32 {
    scale_cpu_pct_by(value, system_cpu_count())
}

fn scale_cpu_pct_for_cpus(value: f32, cpus: Option<u32>) -> f32 {
    cpus.and_then(|cpus| usize::try_from(cpus).ok())
        .map(|cpus| scale_cpu_pct_by(value, cpus))
        .unwrap_or_else(|| scale_cpu_pct_to_system(value))
}

fn scale_cpu_pct_by(value: f32, cpus: usize) -> f32 {
    value / cpus.max(1) as f32
}

fn system_cpu_count() -> usize {
    std::thread::available_parallelism()
        .map(usize::from)
        .unwrap_or(1)
}

fn session_rows(events: &[EventRow]) -> Vec<SessionRow> {
    let mut rows: HashMap<(String, String), SessionRow> = HashMap::new();
    for event in events {
        let Some((source, session)) = session_key_for_event(event) else {
            continue;
        };
        let task = event.task.clone().unwrap_or_else(|| "-".to_string());
        let workspace = event_workspace(event).unwrap_or_else(|| "-".to_string());
        let key = (source.clone(), session.clone());
        let row = rows.entry(key).or_insert_with(|| SessionRow {
            source,
            session,
            task: task.clone(),
            workspace: workspace.clone(),
            events: 0,
            tools: 0,
            security: 0,
            files: 0,
            egress: 0,
            llm: 0,
            prompt_tokens: 0,
            completion_tokens: 0,
            total_tokens: 0,
            model: "-".to_string(),
            provider: "-".to_string(),
            latency_ms: 0,
            latency_samples: 0,
            ttft_ms: 0,
            ttft_samples: 0,
            req_bytes: 0,
            resp_bytes: 0,
            high_risk: 0,
            risk: Risk::Low,
            last_kind: event.kind.clone(),
            last_message: event.message.clone(),
        });
        row.events += 1;
        if row.workspace == "-" && workspace != "-" {
            row.workspace = workspace;
        }
        if event.kind == "ToolExec" {
            row.tools += 1;
        }
        if event.kind == "SecurityAction" {
            row.security += 1;
        }
        if matches!(event.kind.as_str(), "FileAccess" | "FileDelete") {
            row.files += 1;
        }
        if event.kind == "Egress" {
            row.egress += 1;
        }
        if is_llm_event_kind(&event.kind) {
            row.llm += 1;
        }
        if let Some(tokens) = event_token_usage(event) {
            row.prompt_tokens += tokens.prompt;
            row.completion_tokens += tokens.completion;
            row.total_tokens += tokens.total;
        }
        if row.model == "-" {
            if let Some(model) = event_model(event) {
                row.model = model;
            }
        }
        if let Some(network) = event_llm_network(event) {
            if row.provider == "-" {
                if let Some(provider) = network.provider {
                    row.provider = provider;
                }
            }
            if let Some(latency_ms) = network.latency_ms {
                row.latency_ms += latency_ms;
                row.latency_samples += 1;
            }
            if let Some(ttft_ms) = network.ttft_ms {
                row.ttft_ms += ttft_ms;
                row.ttft_samples += 1;
            }
            row.req_bytes += network.req_bytes;
            row.resp_bytes += network.resp_bytes;
        }
        if event.risk == Risk::High {
            row.high_risk += 1;
        }
        if risk_rank(event.risk) > risk_rank(row.risk) {
            row.risk = event.risk;
        }
        if row.task == "-" && task != "-" {
            row.task = task;
        }
    }

    let mut rows = rows.into_values().collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.high_risk
            .cmp(&a.high_risk)
            .then(b.events.cmp(&a.events))
            .then(a.source.cmp(&b.source))
            .then(a.session.cmp(&b.session))
    });
    rows
}

fn session_key_for_event(event: &EventRow) -> Option<(String, String)> {
    let agent = agent_kind_for_source(&event.source)?;
    let session = event
        .session
        .clone()
        .or_else(|| event.task.clone())
        .unwrap_or_else(|| "no-session".to_string());
    Some((agent.label().to_string(), session))
}

fn session_focus_matches_event(focus: &SessionFocus, event: &EventRow) -> bool {
    session_key_for_event(event)
        .is_some_and(|(source, session)| source == focus.source && session == focus.session)
}

fn sort_sessions(rows: &mut [SessionRow], sort_by: SortBy) {
    match sort_by {
        SortBy::Cpu => rows.sort_by(|a, b| {
            b.high_risk
                .cmp(&a.high_risk)
                .then(b.events.cmp(&a.events))
                .then(a.source.cmp(&b.source))
                .then(a.session.cmp(&b.session))
        }),
        SortBy::Mem => rows.sort_by(|a, b| {
            b.tools
                .cmp(&a.tools)
                .then(b.events.cmp(&a.events))
                .then(a.source.cmp(&b.source))
                .then(a.session.cmp(&b.session))
        }),
        SortBy::Net => rows.sort_by(|a, b| {
            b.egress
                .cmp(&a.egress)
                .then(b.events.cmp(&a.events))
                .then(a.source.cmp(&b.source))
                .then(a.session.cmp(&b.session))
        }),
        SortBy::Block | SortBy::Pids | SortBy::State | SortBy::Id | SortBy::Uptime => {
            rows.sort_by(|a, b| {
                b.events
                    .cmp(&a.events)
                    .then(a.source.cmp(&b.source))
                    .then(a.session.cmp(&b.session))
            })
        }
        SortBy::Name => rows.sort_by(|a, b| {
            a.source
                .cmp(&b.source)
                .then(a.session.cmp(&b.session))
                .then(a.task.cmp(&b.task))
        }),
        SortBy::Tokens => rows.sort_by(|a, b| {
            b.total_tokens
                .cmp(&a.total_tokens)
                .then(b.llm.cmp(&a.llm))
                .then(b.events.cmp(&a.events))
                .then(a.source.cmp(&b.source))
                .then(a.session.cmp(&b.session))
        }),
    }
}

fn risk_rank(risk: Risk) -> u8 {
    match risk {
        Risk::Low => 0,
        Risk::Medium => 1,
        Risk::High => 2,
    }
}

fn process_history_key(pid: u32) -> String {
    format!("process:{pid}")
}

fn agent_tree_history_key(pid: u32) -> String {
    format!("agent-tree:{pid}")
}

fn container_history_key(id: &str) -> String {
    format!("container:{id}")
}

fn process_tree_usage(rows: &[ProcessRow], root_pid: u32) -> ProcessTreeUsage {
    let mut usage = ProcessTreeUsage::default();
    let mut stack = vec![root_pid];
    let mut visited = HashSet::new();

    while let Some(pid) = stack.pop() {
        if !visited.insert(pid) {
            continue;
        }
        let Some(process) = rows.iter().find(|process| process.pid == pid) else {
            continue;
        };
        usage.cpu_pct += process.cpu_pct;
        usage.mem_pct += process.mem_pct;
        if pid != root_pid {
            usage.descendants += 1;
        }
        stack.extend(
            rows.iter()
                .filter(|candidate| candidate.ppid == pid)
                .map(|child| child.pid),
        );
    }

    usage
}

fn push_history(history: &mut MetricHistory, cpu: f32, mem: f32) {
    history.cpu.push(cpu.clamp(0.0, 100.0));
    history.mem.push(mem.clamp(0.0, 100.0));
    cap_f32_history(&mut history.cpu);
    cap_f32_history(&mut history.mem);
}

fn add_aligned_f32_history(total: &mut Vec<f32>, values: &[f32]) {
    if values.is_empty() {
        return;
    }
    if total.len() < values.len() {
        let mut padded = vec![0.0; values.len() - total.len()];
        padded.extend(total.iter().copied());
        *total = padded;
    }
    let offset = total.len() - values.len();
    for (idx, value) in values.iter().enumerate() {
        total[offset + idx] += value;
    }
    cap_f32_history(total);
}

fn push_io_history(history: &mut MetricHistory, net_io_bytes: u64, block_io_bytes: u64) {
    history.net_io_bytes.push(net_io_bytes as f64);
    history.block_io_bytes.push(block_io_bytes as f64);
    cap_f64_history(&mut history.net_io_bytes);
    cap_f64_history(&mut history.block_io_bytes);
}

fn cap_f32_history(values: &mut Vec<f32>) {
    if values.len() > HISTORY_LIMIT {
        let excess = values.len() - HISTORY_LIMIT;
        values.drain(0..excess);
    }
}

fn cap_f64_history(values: &mut Vec<f64>) {
    if values.len() > HISTORY_LIMIT {
        let excess = values.len() - HISTORY_LIMIT;
        values.drain(0..excess);
    }
}

fn observe_raw_cpu_pct(
    history: &mut MetricHistory,
    total_ns: Option<u64>,
    now: Instant,
) -> Option<f32> {
    let total_ns = total_ns?;
    let pct = match (history.cpu_usage_total_ns, history.raw_sample_at) {
        (Some(previous), Some(previous_at)) if total_ns >= previous => {
            let elapsed = now.duration_since(previous_at).as_secs_f64();
            (elapsed > 0.0).then(|| {
                ((total_ns - previous) as f64 / (elapsed * 1_000_000_000.0) * 100.0) as f32
            })
        }
        _ => None,
    };
    history.cpu_usage_total_ns = Some(total_ns);
    history.raw_sample_at = Some(now);
    pct.map(|value| value.clamp(0.0, 10_000.0))
}

fn parse_percent(s: &str) -> Option<f32> {
    s.trim().trim_end_matches('%').parse().ok()
}

fn json_u64(value: &serde_json::Value) -> Option<u64> {
    value
        .as_u64()
        .or_else(|| value.as_i64().and_then(|v| (v >= 0).then_some(v as u64)))
        .or_else(|| value.as_f64().and_then(|v| (v >= 0.0).then_some(v as u64)))
}

fn json_field_u64(value: &serde_json::Value, fields: &[&str]) -> u64 {
    fields
        .iter()
        .find_map(|field| value.get(*field).and_then(json_u64))
        .unwrap_or(0)
}

fn format_byte_pair(first: u64, second: u64) -> String {
    format!("{} / {}", format_bytes(first), format_bytes(second))
}

fn container_net_total(container: &ContainerRow) -> u64 {
    parse_byte_pair_total(&container.net_io)
}

fn container_block_total(container: &ContainerRow) -> u64 {
    parse_byte_pair_total(&container.block_io)
}

fn container_pid_count(container: &ContainerRow) -> u64 {
    container.pids.trim().parse().unwrap_or(0)
}

fn container_state_label(status: &str) -> String {
    let lower = status.trim().to_ascii_lowercase();
    if lower.contains("paused") {
        "paused".into()
    } else if lower.starts_with("up") || lower == "running" || lower.contains("running") {
        "running".into()
    } else if lower.contains("restarting") {
        "restart".into()
    } else if lower.contains("exited") || lower.contains("stopped") {
        "exited".into()
    } else if lower.contains("created") {
        "created".into()
    } else if lower.contains("dead") {
        "dead".into()
    } else {
        status
            .split_whitespace()
            .next()
            .filter(|value| !value.is_empty())
            .unwrap_or("-")
            .to_ascii_lowercase()
    }
}

fn container_state_color(status: &str) -> Color {
    match container_state_label(status).as_str() {
        "running" => GREEN,
        "restart" => ORANGE,
        "paused" => YELLOW,
        "exited" | "stopped" => Color::BrightBlack,
        "created" => CYAN,
        "dead" => RED,
        _ => CYAN,
    }
}

fn container_state_summary(rows: &[ContainerRow]) -> ContainerStateSummary {
    let mut summary = ContainerStateSummary {
        total: rows.len(),
        ..ContainerStateSummary::default()
    };

    for row in rows {
        match container_state_label(&row.status).as_str() {
            "running" => summary.running += 1,
            "restart" => summary.restarting += 1,
            "paused" => summary.paused += 1,
            "exited" | "stopped" => summary.exited += 1,
            "created" => summary.created += 1,
            "dead" => summary.dead += 1,
            _ => summary.other += 1,
        }
    }

    summary
}

impl ContainerStateSummary {
    fn header_label(self) -> String {
        if self.total == 0 {
            return "0".into();
        }
        let mut parts = Vec::new();
        push_state_count(&mut parts, "run", self.running);
        push_state_count(&mut parts, "restart", self.restarting);
        push_state_count(&mut parts, "pause", self.paused);
        push_state_count(&mut parts, "exit", self.exited);
        push_state_count(&mut parts, "create", self.created);
        push_state_count(&mut parts, "dead", self.dead);
        push_state_count(&mut parts, "other", self.other);
        if parts.is_empty() {
            self.total.to_string()
        } else {
            format!("{} {}", self.total, parts.join(" "))
        }
    }
}

fn push_state_count(parts: &mut Vec<String>, label: &str, count: usize) {
    if count > 0 {
        parts.push(format!("{label}:{count}"));
    }
}

fn container_state_summary_json(summary: ContainerStateSummary) -> serde_json::Value {
    serde_json::json!({
        "total": summary.total,
        "running": summary.running,
        "restarting": summary.restarting,
        "paused": summary.paused,
        "exited": summary.exited,
        "created": summary.created,
        "dead": summary.dead,
        "other": summary.other,
    })
}

fn container_state_rank(container: &ContainerRow) -> u8 {
    match container_state_label(&container.status).as_str() {
        "running" => 5,
        "restart" => 4,
        "paused" => 3,
        "exited" | "stopped" => 2,
        "created" => 1,
        _ => 0,
    }
}

fn container_uptime_label(container: &ContainerRow) -> String {
    let status = container.status.trim();
    let lower = status.to_ascii_lowercase();
    if lower.starts_with("up ") {
        let value = status
            .trim_start_matches("Up ")
            .split(" (")
            .next()
            .unwrap_or(status)
            .trim();
        if !value.is_empty() {
            return truncate(value, 14);
        }
    }
    if lower == "running" && container.inspect.started != "-" {
        return truncate(&container.inspect.started, 14);
    }
    "-".into()
}

fn container_uptime_seconds(container: &ContainerRow) -> u64 {
    parse_uptime_seconds(&container.status).unwrap_or(0)
}

fn parse_uptime_seconds(status: &str) -> Option<u64> {
    let lower = status.to_ascii_lowercase();
    let rest = lower.strip_prefix("up ")?;
    let rest = rest.split(" (").next().unwrap_or(rest).trim();
    let mut parts = rest.split_whitespace();
    let first = parts.next()?;
    let amount = match first {
        "a" | "an" => 1,
        "about" => match parts.next()? {
            "a" | "an" => 1,
            value => value.parse::<u64>().ok()?,
        },
        value => value.parse::<u64>().ok()?,
    };
    let unit = parts.next().unwrap_or("second");
    let seconds = if unit.starts_with("second") {
        amount
    } else if unit.starts_with("minute") {
        amount.saturating_mul(60)
    } else if unit.starts_with("hour") {
        amount.saturating_mul(60 * 60)
    } else if unit.starts_with("day") {
        amount.saturating_mul(24 * 60 * 60)
    } else if unit.starts_with("week") {
        amount.saturating_mul(7 * 24 * 60 * 60)
    } else if unit.starts_with("month") {
        amount.saturating_mul(30 * 24 * 60 * 60)
    } else if unit.starts_with("year") {
        amount.saturating_mul(365 * 24 * 60 * 60)
    } else {
        return None;
    };
    Some(seconds)
}

fn parse_byte_pair_total(value: &str) -> u64 {
    value.split('/').filter_map(parse_human_bytes).sum()
}

fn parse_human_bytes(value: &str) -> Option<u64> {
    let value = value.trim();
    if value.is_empty() || value == "-" {
        return None;
    }

    let mut number = String::new();
    let mut unit = String::new();
    let mut seen_number = false;
    for ch in value.chars() {
        if ch.is_ascii_digit() || ch == '.' {
            seen_number = true;
            number.push(ch);
        } else if seen_number && ch.is_ascii_alphabetic() {
            unit.push(ch);
        } else if seen_number && ch.is_whitespace() {
            continue;
        } else if seen_number {
            break;
        }
    }
    let amount = number.parse::<f64>().ok()?;
    let multiplier = match unit.to_ascii_lowercase().as_str() {
        "" | "b" => 1.0,
        "k" | "kb" => 1_000.0,
        "m" | "mb" => 1_000_000.0,
        "g" | "gb" => 1_000_000_000.0,
        "t" | "tb" => 1_000_000_000_000.0,
        "kib" => 1024.0,
        "mib" => 1024.0 * 1024.0,
        "gib" => 1024.0 * 1024.0 * 1024.0,
        "tib" => 1024.0 * 1024.0 * 1024.0 * 1024.0,
        _ => return None,
    };
    Some((amount.max(0.0) * multiplier).round() as u64)
}

fn format_bytes(bytes: u64) -> String {
    const KIB: f64 = 1024.0;
    const UNITS: [&str; 5] = ["B", "KiB", "MiB", "GiB", "TiB"];

    let mut value = bytes as f64;
    let mut unit = 0;
    while value >= KIB && unit < UNITS.len() - 1 {
        value /= KIB;
        unit += 1;
    }

    if unit == 0 {
        format!("{bytes}B")
    } else if value >= 10.0 {
        format!("{value:.1}{}", UNITS[unit])
    } else {
        format!("{value:.2}{}", UNITS[unit])
    }
}

fn parse_duration(s: &str) -> anyhow::Result<Duration> {
    if let Some(ms) = s.strip_suffix("ms") {
        return Ok(Duration::from_millis(ms.parse()?));
    }
    if let Some(sec) = s.strip_suffix('s') {
        let seconds: f64 = sec.parse()?;
        return Ok(Duration::from_secs_f64(seconds));
    }
    Ok(Duration::from_millis(s.parse()?))
}

fn compact_json_value(v: &serde_json::Value) -> String {
    match v {
        serde_json::Value::String(s) => truncate(s, 80),
        _ => truncate(&v.to_string(), 80),
    }
}

fn display_workspace(path: Option<&str>) -> String {
    let Some(path) = path.map(str::trim).filter(|path| !path.is_empty()) else {
        return "-".to_string();
    };
    truncate(path, 72)
}

fn display_model(model: &str) -> String {
    let model = model.trim();
    if model.is_empty() || model == "-" {
        "-".to_string()
    } else {
        truncate(model, 40)
    }
}

fn display_provider(provider: &str) -> String {
    let provider = provider.trim();
    if provider.is_empty() || provider == "-" {
        "-".to_string()
    } else {
        truncate(provider, 28)
    }
}

fn normalize_ports(ports: &str) -> String {
    let ports = ports.trim();
    if ports.is_empty() {
        "-".to_string()
    } else {
        ports.to_string()
    }
}

fn display_ports(ports: &str) -> String {
    let ports = normalize_ports(ports);
    if ports == "-" {
        ports
    } else {
        truncate(&ports, 72)
    }
}

fn container_web_url(container: &ContainerRow) -> Option<String> {
    first_published_host_port(&container.ports).map(|port| format!("http://localhost:{port}/"))
}

fn first_published_host_port(ports: &str) -> Option<u16> {
    normalize_ports(ports)
        .split(',')
        .find_map(parse_published_host_port)
}

fn parse_published_host_port(port: &str) -> Option<u16> {
    let port = port.trim();
    if port.is_empty() || port == "-" {
        return None;
    }

    if let Some((host, _guest)) = port.split_once("->") {
        return parse_host_side_port(host);
    }

    let without_proto = port.split('/').next().unwrap_or(port).trim();
    let parts = without_proto
        .split(':')
        .map(str::trim)
        .filter(|part| !part.is_empty())
        .collect::<Vec<_>>();

    match parts.len() {
        0 | 1 => None,
        2 => parse_nonzero_port(parts[0]),
        _ => parse_nonzero_port(parts[parts.len().saturating_sub(2)]),
    }
}

fn parse_host_side_port(host: &str) -> Option<u16> {
    let host = host.split('/').next().unwrap_or(host).trim();
    let candidate = host.rsplit(':').find(|part| !part.trim().is_empty())?;
    parse_nonzero_port(candidate)
}

fn parse_nonzero_port(value: &str) -> Option<u16> {
    let cleaned = value
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']')
        .trim();
    let port = cleaned.parse::<u16>().ok()?;
    (port > 0).then_some(port)
}

fn format_avg_ms(total_ms: u64, samples: u64) -> String {
    match total_ms.checked_div(samples) {
        Some(avg) => format_duration_ms(avg),
        None => "-".to_string(),
    }
}

fn format_duration_ms(ms: u64) -> String {
    if ms >= 1000 {
        format!("{:.1}s", ms as f64 / 1000.0)
    } else {
        format!("{ms}ms")
    }
}

fn format_count(value: u64) -> String {
    if value >= 1_000_000 {
        format!("{:.1}M", value as f64 / 1_000_000.0)
    } else if value >= 1_000 {
        format!("{:.1}K", value as f64 / 1_000.0)
    } else {
        value.to_string()
    }
}

fn push_agent_more_leaf(
    children: &mut Vec<TreeNode>,
    total: usize,
    shown: usize,
    label: &str,
    agent: AgentKind,
) {
    if total > shown {
        children.push(TreeNode::leaf(agent_tree_label(
            agent,
            format!("... {} more {}", total.saturating_sub(shown), label),
        )));
    }
}

fn display_cmd(command: &str) -> String {
    truncate(command, 64)
}

fn short_id(id: &str) -> &str {
    id.get(..12.min(id.len())).unwrap_or(id)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    if max <= 1 {
        return "…".to_string();
    }
    let mut out = s.chars().take(max - 1).collect::<String>();
    out.push('…');
    out
}

fn pad_plain(s: &str, width: usize) -> String {
    let len = s.chars().count();
    if len >= width {
        truncate(s, width)
    } else {
        format!("{s}{}", " ".repeat(width - len))
    }
}

fn pad_line(s: &str, width: usize) -> String {
    let visible = a3s_tui::style::visible_len(s);
    if visible >= width {
        s.to_string()
    } else {
        format!("{s}{}", " ".repeat(width - visible))
    }
}

fn invert_screen(screen: &str) -> String {
    format!(
        "\x1b[7m{}\x1b[0m",
        screen.replace("\x1b[0m", "\x1b[0m\x1b[7m")
    )
}

fn format_event_pid(pid: Option<u32>) -> String {
    pid.map(|pid| pid.to_string())
        .unwrap_or_else(|| "-".to_string())
}

fn format_optional_pct(value: Option<f32>) -> String {
    value
        .map(|value| format!("{value:.1}"))
        .unwrap_or_else(|| "-".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn all_columns_config() -> TopConfig {
        TopConfig {
            hidden_columns: HashSet::new(),
            ..TopConfig::default()
        }
    }

    #[test]
    fn detects_known_agents() {
        assert_eq!(detect_agent("/usr/bin/a3s code"), Some(AgentKind::A3sCode));
        assert_eq!(
            detect_agent("node /bin/claude"),
            Some(AgentKind::ClaudeCode)
        );
        assert_eq!(detect_agent("codex exec task"), Some(AgentKind::Codex));
        assert_eq!(
            detect_agent("node /opt/node_modules/@anthropic-ai/claude-code/cli.js"),
            Some(AgentKind::ClaudeCode)
        );
        assert_eq!(
            detect_agent("node C:\\tools\\@openai\\codex\\bin\\codex.js"),
            Some(AgentKind::Codex)
        );
        assert_eq!(
            detect_agent("C:\\tools\\cursor-agent.exe --print"),
            Some(AgentKind::Cursor)
        );
        assert_eq!(
            detect_agent("gemini --prompt task"),
            Some(AgentKind::Gemini)
        );
        assert_eq!(
            detect_agent("/usr/local/bin/workbuddy --resume"),
            Some(AgentKind::WorkBuddy)
        );
        assert_eq!(
            detect_agent("node /opt/node_modules/workbuddy/bin/cli.js"),
            Some(AgentKind::WorkBuddy)
        );
        assert_eq!(
            detect_agent("C:\\tools\\codebuddy.exe --resume"),
            Some(AgentKind::WorkBuddy)
        );
        assert_eq!(detect_agent("rg -n codex src"), None);
    }

    #[test]
    fn parses_process_rows() {
        let row = parse_process_line(" 123  1  2.5  0.7  01:02:03 codex exec hello").unwrap();
        assert_eq!(row.pid, 123);
        assert_eq!(row.ppid, 1);
        assert_eq!(row.agent, Some(AgentKind::Codex));
        assert_eq!(row.cpu_pct, 2.5);
        assert!(row.cwd.is_none());
    }

    #[test]
    fn parses_lsof_cwd_output() {
        let text = "p123\nn/Users/roylin/code/a3s\n";
        assert_eq!(
            parse_lsof_cwd(text).as_deref(),
            Some("/Users/roylin/code/a3s")
        );
    }

    #[test]
    fn parses_durations() {
        assert_eq!(parse_duration("250ms").unwrap(), Duration::from_millis(250));
        assert_eq!(parse_duration("2s").unwrap(), Duration::from_secs(2));
        assert_eq!(parse_duration("1500").unwrap(), Duration::from_millis(1500));
    }

    #[test]
    fn defaults_to_agents_view_with_a3s_box_connector() {
        let options = TopOptions::default();

        assert_eq!(options.tab, Tab::Agents);
        assert_eq!(options.config.connector, ContainerConnector::A3sBox);
    }

    #[test]
    fn parses_all_container_option() {
        let options = parse_options(vec![
            "--containers".into(),
            "--all".into(),
            "--filter".into(),
            "codex".into(),
            "--sort".into(),
            "mem".into(),
            "--risk".into(),
            "high".into(),
            "--kind".into(),
            "security".into(),
            "--connector".into(),
            "runc".into(),
            "--reverse".into(),
            "--invert".into(),
            "--compact".into(),
            "--watch".into(),
            "2s".into(),
        ])
        .unwrap();

        assert_eq!(options.tab, Tab::Containers);
        assert!(options.force_all_containers);
        assert!(!options.force_active_containers);
        assert_eq!(options.filter.as_deref(), Some("codex"));
        assert_eq!(options.sort_by, Some(SortBy::Mem));
        assert_eq!(options.risk_filter, Some(RiskFilter::High));
        assert_eq!(options.kind_filter, Some(KindFilter::Security));
        assert_eq!(options.connector, Some(ContainerConnector::RunC));
        assert_eq!(options.reverse_sort, Some(true));
        assert!(options.invert_colors);
        assert!(options.compact_columns);
        assert!(options.watch);
        assert_eq!(options.interval, Duration::from_secs(2));
    }

    #[test]
    fn parses_ctop_style_short_options_and_start_help() {
        let options = parse_options(vec![
            "-f".into(),
            "api".into(),
            "-s".into(),
            "pids".into(),
            "-r".into(),
            "-h".into(),
        ])
        .unwrap();

        assert_eq!(options.filter.as_deref(), Some("api"));
        assert_eq!(options.sort_by, Some(SortBy::Pids));
        assert_eq!(options.reverse_sort, Some(true));
        assert!(options.start_help);

        let app = TopApp::new(options);
        assert!(app.help);
    }

    #[test]
    fn parses_ctop_style_single_container_target() {
        let options = parse_options(vec!["web".into()]).unwrap();

        assert_eq!(options.tab, Tab::Containers);
        assert_eq!(options.container_query.as_deref(), Some("web"));

        let options = parse_options(vec!["--container".into(), "abcdef".into()]).unwrap();
        assert_eq!(options.tab, Tab::Containers);
        assert_eq!(options.container_query.as_deref(), Some("abcdef"));

        assert!(parse_options(vec!["web".into(), "api".into()])
            .unwrap_err()
            .to_string()
            .contains("only one container target"));
    }

    #[test]
    fn single_container_target_defaults_to_all_unless_active_is_forced() {
        let mut options = TopOptions {
            container_query: Some("stopped-job".into()),
            config: TopConfig {
                show_all_containers: false,
                ..TopConfig::default()
            },
            ..TopOptions::default()
        };

        apply_cli_overrides(&mut options, false, false);

        assert_eq!(options.tab, Tab::Containers);
        assert!(options.config.show_all_containers);

        let mut options = TopOptions {
            container_query: Some("running-only".into()),
            force_active_containers: true,
            config: TopConfig {
                show_all_containers: true,
                ..TopConfig::default()
            },
            ..TopOptions::default()
        };

        apply_cli_overrides(&mut options, false, true);

        assert_eq!(options.tab, Tab::Containers);
        assert!(!options.config.show_all_containers);
    }

    #[test]
    fn parses_llm_kind_option() {
        let options =
            parse_options(vec!["--events".into(), "--kind".into(), "llm".into()]).unwrap();

        assert_eq!(options.tab, Tab::Events);
        assert_eq!(options.kind_filter, Some(KindFilter::Llm));
        assert!(KindFilter::Llm.matches("LlmApi"));
        assert!(KindFilter::Llm.matches("LlmCall"));
        assert!(!KindFilter::Other.matches("LlmApi"));
    }

    #[test]
    fn parses_tokens_sort_option() {
        let options = parse_options(vec!["--sort".into(), "tokens".into()]).unwrap();

        assert_eq!(options.sort_by, Some(SortBy::Tokens));
        assert_eq!(SortBy::Mem.next(), SortBy::Net);
        assert_eq!(SortBy::Net.next(), SortBy::Block);
        assert_eq!(SortBy::Block.next(), SortBy::Pids);
        assert_eq!(SortBy::Pids.next(), SortBy::State);
        assert_eq!(SortBy::State.next(), SortBy::Id);
        assert_eq!(SortBy::Id.next(), SortBy::Uptime);
        assert_eq!(SortBy::Uptime.next(), SortBy::Name);
        assert_eq!(SortBy::Name.next(), SortBy::Tokens);
        assert_eq!(SortBy::Tokens.next(), SortBy::Cpu);
    }

    #[test]
    fn parses_container_io_sort_options() {
        assert_eq!(
            parse_options(vec!["--sort".into(), "net".into()])
                .unwrap()
                .sort_by,
            Some(SortBy::Net)
        );
        assert_eq!(SortBy::from_label("block"), Some(SortBy::Block));
        assert_eq!(SortBy::from_label("io"), Some(SortBy::Block));
        assert_eq!(SortBy::from_label("pids"), Some(SortBy::Pids));
        assert_eq!(SortBy::from_label("state"), Some(SortBy::State));
        assert_eq!(SortBy::from_label("id"), Some(SortBy::Id));
        assert_eq!(SortBy::from_label("uptime"), Some(SortBy::Uptime));
    }

    #[test]
    fn parses_sessions_option() {
        let options = parse_options(vec!["--sessions".into()]).unwrap();

        assert_eq!(options.tab, Tab::Sessions);
    }

    #[test]
    fn parses_json_option() {
        let options = parse_options(vec!["--json".into(), "--containers".into()]).unwrap();

        assert!(options.json);
        assert_eq!(options.tab, Tab::Containers);
    }

    #[test]
    fn parses_json_watch_count_option() {
        let options = parse_options(vec![
            "--json".into(),
            "--watch".into(),
            "25ms".into(),
            "--count".into(),
            "3".into(),
        ])
        .unwrap();

        assert!(options.json);
        assert!(options.watch);
        assert_eq!(options.interval, Duration::from_millis(25));
        assert_eq!(options.json_count, Some(3));
        assert!(parse_options(vec!["--count".into(), "0".into()]).is_err());
    }

    #[test]
    fn invert_screen_reapplies_reverse_after_resets() {
        let rendered = invert_screen("left\x1b[31mred\x1b[0mright");

        assert!(rendered.starts_with("\x1b[7m"));
        assert!(rendered.contains("\x1b[0m\x1b[7mright"));
        assert!(rendered.ends_with("\x1b[0m"));
    }

    #[test]
    fn parses_top_config() {
        let config = parse_top_config(
            r#"{
                "show_all_containers": true,
                "show_header": false,
                "reverse_sort": true,
                "sort_by": "mem",
                "risk_filter": "high",
                "kind_filter": "tool",
                "connector": "runc",
                "filter": "codex",
                "hidden_columns": ["containers.net", "events.message"]
            }"#,
        )
        .unwrap();

        assert!(config.show_all_containers);
        assert!(!config.show_header);
        assert!(config.reverse_sort);
        assert_eq!(config.sort_by, SortBy::Mem);
        assert_eq!(config.risk_filter, RiskFilter::High);
        assert_eq!(config.kind_filter, KindFilter::Tool);
        assert_eq!(config.connector, ContainerConnector::RunC);
        assert_eq!(config.filter, "codex");
        assert!(config.hidden_columns.contains("containers.net"));
        assert!(config.hidden_columns.contains("events.message"));
    }

    #[test]
    fn cli_overrides_loaded_config() {
        let mut options = TopOptions {
            force_all_containers: true,
            force_active_containers: false,
            filter: Some("claude".into()),
            sort_by: Some(SortBy::Name),
            risk_filter: Some(RiskFilter::High),
            kind_filter: Some(KindFilter::Security),
            connector: Some(ContainerConnector::RunC),
            reverse_sort: Some(true),
            show_header: Some(false),
            compact_columns: true,
            config: TopConfig {
                filter: "codex".into(),
                sort_by: SortBy::Cpu,
                risk_filter: RiskFilter::All,
                kind_filter: KindFilter::All,
                hidden_columns: HashSet::new(),
                ..TopConfig::default()
            },
            ..TopOptions::default()
        };

        apply_cli_overrides(&mut options, true, false);

        assert!(options.config.show_all_containers);
        assert_eq!(options.config.filter, "claude");
        assert_eq!(options.config.sort_by, SortBy::Name);
        assert_eq!(options.config.risk_filter, RiskFilter::High);
        assert_eq!(options.config.kind_filter, KindFilter::Security);
        assert_eq!(options.config.connector, ContainerConnector::RunC);
        assert!(options.config.reverse_sort);
        assert!(!options.config.show_header);
        assert!(options.config.hidden_columns.contains("agents.session"));
        assert!(options.config.hidden_columns.contains("containers.ports"));
        assert!(!options.config.hidden_columns.contains("agents.command"));
    }

    #[test]
    fn ctop_active_flag_overrides_saved_all_container_config() {
        let options = parse_options(vec!["-a".into()]).unwrap();
        assert!(options.force_active_containers);
        assert!(!options.force_all_containers);

        let mut options = TopOptions {
            force_active_containers: true,
            config: TopConfig {
                show_all_containers: true,
                ..TopConfig::default()
            },
            ..TopOptions::default()
        };

        apply_cli_overrides(&mut options, false, true);

        assert!(!options.config.show_all_containers);
    }

    #[test]
    fn container_visibility_flags_use_last_value() {
        let options = parse_options(vec!["-a".into(), "--all".into()]).unwrap();
        assert!(options.force_all_containers);
        assert!(!options.force_active_containers);

        let options = parse_options(vec!["--all".into(), "--active".into()]).unwrap();
        assert!(options.force_active_containers);
        assert!(!options.force_all_containers);
    }

    #[test]
    fn config_json_sorts_hidden_columns() {
        let mut config = TopConfig::default();
        config.hidden_columns.insert("events.source".into());
        config.hidden_columns.insert("agents.cpu".into());

        let text = top_config_json(&config);
        let agents_idx = text.find("agents.cpu").unwrap();
        let events_idx = text.find("events.source").unwrap();

        assert!(agents_idx < events_idx);
        assert!(text.contains("\"risk_filter\": \"all\""));
        assert!(text.contains("\"kind_filter\": \"all\""));
    }

    #[test]
    fn default_columns_are_compact_but_core_agent_fields_remain() {
        let config = TopConfig::default();
        assert!(config.hidden_columns.contains("agents.session"));
        assert!(config.hidden_columns.contains("agents.task"));
        assert!(config.hidden_columns.contains("agents.tools"));
        assert!(config.hidden_columns.contains("containers.ports"));
        assert!(config.hidden_columns.contains("containers.health"));
        assert!(config.hidden_columns.contains("containers.image"));
        assert!(config.hidden_columns.contains("containers.mem_usage"));
        assert!(config.hidden_columns.contains("containers.cpus"));
        assert!(config.hidden_columns.contains("events.pid"));
        assert!(!config.hidden_columns.contains("agents.agent"));
        assert!(!config.hidden_columns.contains("agents.cpu"));
        assert!(!config.hidden_columns.contains("agents.command"));
        assert!(!config.hidden_columns.contains("containers.status"));
        assert!(!config.hidden_columns.contains("containers.name"));
        assert!(!config.hidden_columns.contains("containers.id"));
        assert!(!config.hidden_columns.contains("containers.cpu"));
        assert!(!config.hidden_columns.contains("containers.mem"));
        assert!(!config.hidden_columns.contains("containers.net"));
        assert!(!config.hidden_columns.contains("containers.block"));
        assert!(!config.hidden_columns.contains("containers.pids"));
        assert!(!config.hidden_columns.contains("containers.uptime"));

        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 260;
        app.snapshot.processes = vec![
            process_row(42, 1, "codex exec task"),
            process_row(100, 42, "bash -lc git status"),
        ];
        app.snapshot.events = vec![event_row(
            "codex",
            Some("sess-a"),
            Some("task-a"),
            "ToolExec",
            "git status",
            Risk::Medium,
        )];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("Agents"));
        assert!(plain.contains("codex"));
        assert!(plain.contains("Sessions (1)"));
        assert!(plain.contains("Processes (1 system · 1 agent)"));
        assert!(plain.contains("Events (1)"));
        assert!(plain.contains("sess-a"));
        assert!(plain.contains("task-a"));
        assert!(plain.contains("> pid 100"));
    }

    #[test]
    fn default_container_columns_align_with_ctop_and_show_cid() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 260;
        app.snapshot.containers = vec![container_row(
            "abcdef1234567890",
            "a3s-box-dev",
            "Up 2 minutes",
            Some(12.5),
            Some(30.0),
        )];
        app.record_history(&mut app.snapshot.clone());
        let plain = a3s_tui::style::strip_ansi(&app.table());

        for header in [
            "STATUS", "NAME", "CID", "CPU", "MEM", "NET I/O", "IO", "PIDS", "UPTIME",
        ] {
            assert!(plain.contains(header), "missing {header} in {plain}");
        }
        assert!(plain.contains("abcdef123456"));
        assert!(plain.contains("12.5%"));
        assert!(plain.contains("30.0%"));
        assert!(plain.contains("2 minutes"));
        assert!(!plain.contains("CPUS"));
        assert!(!plain.contains("PORTS"));
        assert!(!plain.contains("HEALTH"));
        assert!(!plain.contains("IMAGE"));
        assert!(!plain.contains("MEM USAGE"));
    }

    #[test]
    fn scaled_cpu_column_is_configurable_like_ctop_cpus() {
        assert_eq!(scale_cpu_pct_by(200.0, 4), 50.0);
        assert_eq!(scale_cpu_pct_by(42.0, 0), 42.0);
        assert_eq!(scale_cpu_pct_for_cpus(200.0, Some(2)), 100.0);

        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 300;
        let mut row = container_row(
            "abcdef1234567890",
            "a3s-box-dev",
            "Up 2 minutes",
            Some(100.0),
            Some(30.0),
        );
        row.cpu_count = Some(2);
        app.snapshot.containers = vec![row];
        app.record_history(&mut app.snapshot.clone());

        let plain = a3s_tui::style::strip_ansi(&app.table());
        let scaled = format!("{:.1}%", scale_cpu_pct_for_cpus(100.0, Some(2)));

        assert!(plain.contains("CPUS"), "{plain}");
        assert!(plain.contains(&scaled), "{plain}");
    }

    #[test]
    fn narrow_container_table_keeps_ctop_identity_and_core_metrics() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 80;
        app.snapshot.containers = vec![container_row(
            "abcdef1234567890",
            "a3s-box-dev",
            "Up 2 minutes",
            Some(12.5),
            Some(30.0),
        )];
        app.record_history(&mut app.snapshot.clone());
        let plain = a3s_tui::style::strip_ansi(&app.table());

        for header in ["STATUS", "NAME", "CID", "CPU", "MEM"] {
            assert!(plain.contains(header), "missing {header} in {plain}");
        }
        assert!(plain.contains("abcdef123456"));
        assert!(plain.contains("12.5%"));
        assert!(plain.contains("30.0%"));
        assert!(!plain.contains("NET I/O"));
        assert!(!plain.contains("UPTIME"));
    }

    #[test]
    fn container_table_marks_current_sort_column() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 260;
        app.snapshot.containers = vec![container_row(
            "abcdef1234567890",
            "a3s-box-dev",
            "Up 2 minutes",
            Some(12.5),
            Some(30.0),
        )];
        app.record_history(&mut app.snapshot.clone());

        let plain = a3s_tui::style::strip_ansi(&app.table());
        let header = plain.lines().next().unwrap();
        assert!(header.contains("CPU↓"), "{header}");
        assert!(!header.contains("MEM↓"), "{header}");

        app.sort_by = SortBy::Mem;
        app.reverse_sort = true;
        let plain = a3s_tui::style::strip_ansi(&app.table());
        let header = plain.lines().next().unwrap();
        assert!(header.contains("MEM↑"), "{header}");
        assert!(!header.contains("CPU↑"), "{header}");
    }

    #[test]
    fn snapshot_json_includes_agents_sessions_containers_and_events() {
        let mut process = process_row(42, 1, "codex exec task");
        process.cwd = Some("/work/a3s".into());
        process.cpu_pct = 12.5;
        let child = process_row(100, 42, "bash -lc cargo test");
        let grandchild = process_row(101, 100, "cargo test");
        let mut container = container_row("abcdef123456", "app", "Up", Some(1.0), Some(2.0));
        container.inspect.health = "healthy".into();
        let mut app = TopApp::new(TopOptions::default());
        let mut snapshot = TopSnapshot {
            processes: vec![process, child, grandchild],
            containers: vec![container],
            events: vec![
                parse_observer_line(
                    r#"{"identity":{"agent":"codex","session":"sess-a","task":"task-a"},"event":{"ToolExec":{"pid":42,"argv":["git","status"],"cwd":"/work/a3s"}}}"#,
                )
                .unwrap(),
                parse_observer_line(
                    r#"{"identity":{"agent":"codex","session":"sess-b","task":"task-b"},"event":{"SecurityAction":{"pid":100,"ppid":42,"action":"ptrace","cwd":"/work/a3s"}}}"#,
                )
                .unwrap(),
            ],
            ..Default::default()
        };
        app.record_history(&mut snapshot);
        app.snapshot = snapshot;

        let value = top_snapshot_json(&app, 123);

        assert_eq!(value["schema"], "a3s.top.snapshot.v1");
        assert_eq!(value["collected_at_unix_ms"], 123);
        assert_eq!(value["summary"]["agents"], 1);
        assert_eq!(value["summary"]["sessions"], 2);
        assert_eq!(value["summary"]["containers"], 1);
        assert_eq!(value["summary"]["container_states"]["running"], 1);
        assert_eq!(value["summary"]["container_states"]["total"], 1);
        assert_eq!(value["summary"]["raw_container_states"]["running"], 1);
        assert_eq!(value["agents"][0]["agent"], "codex");
        assert_eq!(value["agents"][0]["activity"]["tools"], 1);
        assert_eq!(value["agents"][0]["activity"]["sessions"], 2);
        assert_eq!(value["agents"][0]["top_session"], "sess-b");
        assert_eq!(value["agents"][0]["top_task"], "task-b");
        assert_eq!(value["agents"][0]["sessions"][0]["session"], "sess-b");
        assert_eq!(value["agents"][0]["sessions"][0]["security"], 1);
        assert_eq!(value["agents"][0]["sessions"][1]["session"], "sess-a");
        assert_eq!(value["agents"][0]["sessions"][1]["tools"], 1);
        assert_eq!(value["agents"][0]["history"]["cpu_pct"][0], 12.5);
        assert_eq!(value["agents"][0]["recent_events"][0]["kind"], "ToolExec");
        assert_eq!(value["sessions"][0]["workspace"], "/work/a3s");
        assert_eq!(value["containers"][0]["cid"], "abcdef123456");
        assert_eq!(value["containers"][0]["short_id"], "abcdef123456");
        assert_eq!(value["containers"][0]["inspect"]["health"], "healthy");
        assert_eq!(value["containers"][0]["history"]["cpu_pct"][0], 1.0);
        assert_eq!(value["processes"][0]["history"]["cpu_pct"][0], 12.5);
        assert_eq!(value["agents"][0]["subtree"]["descendants"], 2);
        assert_eq!(value["agents"][0]["process_tree"]["pid"], 42);
        assert_eq!(
            value["agents"][0]["process_tree"]["children"][0]["pid"],
            100
        );
        assert_eq!(
            value["agents"][0]["process_tree"]["children"][0]["children"][0]["pid"],
            101
        );
        let details = value["events"][0]["details"].as_array().unwrap();
        assert!(details.iter().any(|detail| detail["key"] == "argv"));
    }

    #[test]
    fn keeps_one_visible_column_per_tab() {
        let mut config = TopConfig::default();
        for id in [
            "containers.status",
            "containers.name",
            "containers.id",
            "containers.cpu",
            "containers.cpus",
            "containers.mem",
            "containers.net",
            "containers.block",
            "containers.pids",
            "containers.uptime",
            "containers.image",
            "containers.mem_usage",
            "containers.ports",
            "containers.health",
        ] {
            config.hidden_columns.insert(id.into());
        }
        let app = TopApp::new(TopOptions {
            config,
            ..TopOptions::default()
        });

        assert!(app.column_visible("containers.status"));
    }

    #[test]
    fn column_panel_can_restore_compact_defaults_for_current_tab() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.hidden_columns.insert("events.source".into());

        assert!(app.column_visible("agents.session"));
        assert!(app.column_visible("agents.tools"));
        assert!(!app.column_visible("events.source"));

        app.open_column_panel();
        app.handle_key(KeyEvent {
            code: KeyCode::Char('d'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.column_panel.is_none());
        assert!(!app.column_visible("agents.session"));
        assert!(!app.column_visible("agents.tools"));
        assert!(app.column_visible("agents.agent"));
        assert!(app.column_visible("agents.command"));
        assert!(!app.column_visible("events.source"));
    }

    #[test]
    fn column_panel_uses_shared_menu_panel_and_accepts_number_shortcuts() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 42;

        assert!(app.column_visible("containers.id"));
        app.open_column_panel();
        let rendered = app.column_panel_view();
        let plain = a3s_tui::style::strip_ansi(&rendered);
        assert!(plain.contains("columns · Containers"), "{plain}");
        assert!(plain.contains("> 1. [✓] Status"), "{plain}");
        assert!(plain.contains("  2. [✓] Name"), "{plain}");
        assert!(plain.contains("  3. [✓] CID"), "{plain}");
        assert!(rendered.contains("\x1b["), "selected row should be styled");
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 42),
            "{plain}"
        );

        app.handle_key(KeyEvent {
            code: KeyCode::Char('3'),
            modifiers: KeyModifiers::empty(),
        });
        app.handle_key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.column_panel.is_none());
        assert!(!app.column_visible("containers.id"));
        assert!(app.column_visible("containers.name"));
    }

    #[test]
    fn help_view_mentions_compact_column_restore() {
        let mut app = TopApp::new(TopOptions::default());
        app.width = 72;

        let rendered = app.help_view();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("Keys"));
        assert!(plain.contains("c then d"));
        assert!(plain.contains("restore compact columns"));
        assert!(plain.contains("s / r"));
        assert!(plain.contains("clear filter"));
        assert!(rendered.contains("\x1b["), "help view should be styled");
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 72),
            "{plain}"
        );
    }

    #[test]
    fn confirm_view_uses_shared_confirm_and_fits_narrow_width() {
        let mut app = TopApp::new(TopOptions::default());
        app.width = 36;
        app.confirm = Some(Action::RemoveContainer(
            ContainerConnector::Docker,
            "container-1".into(),
            "very-long-container-name-that-needs-wrapping".into(),
        ));

        let rendered = app.confirm_view();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("Remove container?"), "{plain}");
        assert!(plain.contains("very-long-container"), "{plain}");
        assert!(plain.contains("[ y / Enter ] confirm"), "{plain}");
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 36),
            "{plain}"
        );
    }

    #[test]
    fn shifted_uppercase_keys_resolve() {
        let keymap = top_keymap();
        let key = |c| KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::SHIFT,
        };

        assert_eq!(keymap.resolve(&key('S')), Some(TopKey::SaveConfig));
        assert_eq!(keymap.resolve(&key('K')), Some(TopKey::Kill));
        assert_eq!(keymap.resolve(&key('H')), Some(TopKey::ToggleHeader));
        assert_eq!(keymap.resolve(&key('R')), Some(TopKey::ToggleReverse));
        assert_eq!(keymap.resolve(&key('!')), Some(TopKey::ToggleRiskFilter));
        assert_eq!(keymap.resolve(&key('C')), Some(TopKey::Connector));
        assert_eq!(keymap.resolve(&key('G')), Some(TopKey::ToggleKindFilter));
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::Char('g'),
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::ToggleKindFilter)
        );
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::Char('f'),
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::Filter)
        );
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::ToggleReverse)
        );
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::Char('w'),
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::OpenBrowser)
        );
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::Char('h'),
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::Help)
        );
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::Char('p'),
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::TogglePause)
        );
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::Home,
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::Home)
        );
        assert_eq!(
            keymap.resolve(&KeyEvent {
                code: KeyCode::End,
                modifiers: KeyModifiers::empty(),
            }),
            Some(TopKey::End)
        );
    }

    #[test]
    fn home_end_jump_selection_and_p_toggles_pause() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.snapshot.containers = vec![
            container_row("aaa111", "alpha", "Up", Some(1.0), Some(1.0)),
            container_row("bbb222", "beta", "Up", Some(1.0), Some(1.0)),
            container_row("ccc333", "gamma", "Up", Some(1.0), Some(1.0)),
        ];

        app.handle_key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::empty(),
        });
        assert_eq!(app.selected, 2);

        app.handle_key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::empty(),
        });
        assert_eq!(app.selected, 0);

        app.handle_key(KeyEvent {
            code: KeyCode::Char('p'),
            modifiers: KeyModifiers::empty(),
        });
        assert!(app.paused);
    }

    #[test]
    fn ctop_arrow_keys_open_container_view_and_logs() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.snapshot.containers = vec![container_row(
            "abcdef",
            "app",
            "Up 2 minutes",
            Some(1.0),
            Some(2.0),
        )];

        let focus_cmd = app.handle_key(KeyEvent {
            code: KeyCode::Right,
            modifiers: KeyModifiers::empty(),
        });

        assert!(focus_cmd.is_some());
        assert_eq!(app.focused_container.as_deref(), Some("abcdef"));
        assert!(app
            .container_processes
            .as_ref()
            .is_some_and(|panel| panel.container_id == "abcdef" && panel.loading));

        app.focused_container = None;
        app.container_processes = None;
        let log_cmd = app.handle_key(KeyEvent {
            code: KeyCode::Left,
            modifiers: KeyModifiers::empty(),
        });

        assert!(log_cmd.is_some());
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.container_id, "abcdef");
        assert_eq!(log.container_name, "app");
        assert!(log.loading);
        assert_eq!(app.tab, Tab::Containers);
    }

    #[test]
    fn primary_tab_order_is_agents_then_containers() {
        assert_eq!(
            Tab::ALL,
            [
                Tab::Agents,
                Tab::Containers,
                Tab::Sessions,
                Tab::Events,
                Tab::Processes,
            ]
        );
        assert_eq!(Tab::PRIMARY, [Tab::Agents, Tab::Containers]);
        assert_eq!(Tab::Agents.next(), Tab::Containers);
        assert_eq!(Tab::Containers.next(), Tab::Agents);
        assert_eq!(Tab::Agents.prev(), Tab::Containers);
    }

    #[test]
    fn arrow_keys_switch_primary_agents_and_containers_tabs() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });

        app.handle_key(KeyEvent {
            code: KeyCode::Tab,
            modifiers: KeyModifiers::empty(),
        });
        assert_eq!(app.tab, Tab::Containers);

        app.handle_key(KeyEvent {
            code: KeyCode::BackTab,
            modifiers: KeyModifiers::empty(),
        });
        assert_eq!(app.tab, Tab::Agents);
    }

    #[test]
    fn tabs_use_shared_component_for_filter_and_focus_suffixes() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 72;
        app.filter = "api".into();
        app.focused_container = Some("abcdef123456".into());
        app.snapshot.containers = vec![container_row(
            "abcdef123456",
            "very-long-api-container-name",
            "Up",
            Some(1.0),
            Some(2.0),
        )];

        let rendered = app.tabs();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("Agents"), "{plain}");
        assert!(plain.contains("Containers"), "{plain}");
        assert!(plain.contains("/api"), "{plain}");
        assert!(plain.contains("focus:very-long-api-cont"), "{plain}");
        assert!(rendered.contains("\x1b["), "active tab should be styled");
        assert_eq!(a3s_tui::style::visible_len(&rendered), 72);
    }

    #[test]
    fn tabs_append_non_primary_active_tab_and_fit_width() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.width = 36;
        app.filter = "security".into();

        let rendered = app.tabs();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("Agents"), "{plain}");
        assert!(plain.contains("Containers"), "{plain}");
        assert!(plain.contains("Events"), "{plain}");
        assert!(a3s_tui::style::visible_len(&rendered) <= 36, "{plain}");
    }

    #[test]
    fn lowercase_r_reverses_sort_without_restarting_container() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.snapshot.containers = vec![container_row(
            "abcdef123456",
            "a3s-box-dev",
            "Up 2 minutes",
            Some(12.5),
            Some(30.0),
        )];

        app.handle_key(KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.reverse_sort);
        assert!(app.confirm.is_none());
        assert!(app.container_menu.is_none());

        app.handle_key(KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(!app.reverse_sort);
        assert!(app.confirm.is_none());
    }

    #[test]
    fn parses_observer_events() {
        let line = r#"{"timestamp":"2026-06-26T08:00:00Z","identity":{"agent":"codex","task":"task-1","session":"sess-1"},"provider":null,"event":{"ToolExec":{"pid":2,"argv":["git","status"],"cwd":"/tmp"}}}"#;
        let row = parse_observer_line(line).unwrap();
        assert_eq!(row.ts, "2026-06-26T08:00:00Z");
        assert_eq!(row.source, "codex");
        assert_eq!(row.session.as_deref(), Some("sess-1"));
        assert_eq!(row.task.as_deref(), Some("task-1"));
        assert_eq!(row.pid, Some(2));
        assert!(row.ppid.is_none());
        assert_eq!(row.kind, "ToolExec");
        assert_eq!(row.risk, Risk::Medium);
        assert!(row.message.contains("argv=[\"git\",\"status\"]"));
        assert!(row
            .details
            .iter()
            .any(|(key, value)| key == "cwd" && value == "/tmp"));
    }

    #[test]
    fn parses_numeric_observer_timestamps() {
        let row = parse_observer_line(
            r#"{"ts":1782460800000,"identity":{"agent":"codex"},"event":{"ToolExec":{"argv":["pwd"]}}}"#,
        )
        .unwrap();

        assert_eq!(row.ts, "1782460800000");
    }

    #[test]
    fn events_table_renders_process_scope_columns() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 360;
        app.snapshot.events = vec![parse_observer_line(
            r#"{"identity":{"agent":"codex","task":"task-a","session":"sess-a"},"event":{"ToolExec":{"pid":77,"ppid":42,"argv":["git","status"],"cwd":"/tmp/a3s"}}}"#,
        )
        .unwrap()];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("PID"));
        assert!(plain.contains("PPID"));
        assert!(plain.contains("77"));
        assert!(plain.contains("42"));
    }

    #[test]
    fn events_process_scope_columns_are_configurable() {
        let app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        let choices = app.column_choices(Tab::Events);

        assert!(choices.iter().any(|choice| choice.id == "events.pid"));
        assert!(choices.iter().any(|choice| choice.id == "events.ppid"));
    }

    #[test]
    fn event_filter_matches_payload_details() {
        let line = r#"{"identity":{"agent":"codex","task":"task-1","session":"sess-1"},"event":{"ToolExec":{"a":1,"b":2,"c":3,"d":4,"z":"needle-value"}}}"#;
        let row = parse_observer_line(line).unwrap();
        assert!(!row.message.contains("needle-value"));

        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.filter = "needle".into();
        app.snapshot.events = vec![row];

        assert_eq!(app.filtered_events().len(), 1);
    }

    #[test]
    fn container_lifecycle_events_track_a3s_box_state_changes() {
        let mut previous =
            container_row("abcdef1234567890", "dev", "running", Some(1.0), Some(2.0));
        previous.connector = ContainerConnector::A3sBox;
        previous.inspect.health = "healthy".into();
        let mut current = previous.clone();
        current.status = "paused".into();

        let events =
            container_lifecycle_events(ContainerConnector::A3sBox, &[previous], &[current]);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].source, "a3s-box");
        assert_eq!(events[0].kind, "Container");
        assert_eq!(events[0].risk, Risk::Medium);
        assert!(events[0].message.contains("pause dev"));
        assert!(events[0]
            .details
            .contains(&("action".into(), "pause".into())));
        assert!(events[0]
            .details
            .contains(&("cid".into(), "abcdef123456".into())));
        assert!(events[0]
            .details
            .contains(&("previous_status".into(), "running".into())));
    }

    #[test]
    fn runtime_container_events_are_merged_into_snapshots() {
        let mut previous =
            container_row("abcdef1234567890", "dev", "running", Some(1.0), Some(2.0));
        previous.connector = ContainerConnector::A3sBox;
        let mut current = previous.clone();
        current.status = "paused".into();
        let snapshot = TopSnapshot {
            containers: vec![current],
            ..Default::default()
        };
        let mut app = TopApp::new(TopOptions::default());
        app.last_refresh = Some(Instant::now());
        app.snapshot.containers = vec![previous];

        app.apply_snapshot(
            snapshot,
            ObserverState::default(),
            ContainerConnector::A3sBox,
        );

        assert_eq!(app.snapshot.events.len(), 1);
        assert_eq!(app.snapshot.events[0].source, "a3s-box");
        assert_eq!(app.snapshot.events[0].kind, "Container");
        let value = top_snapshot_json(&app, 123);
        assert_eq!(value["events"][0]["source"], "a3s-box");
        assert_eq!(value["events"][0]["kind"], "Container");
        assert_eq!(value["events"][0]["details"][1]["key"], "action");
        assert_eq!(value["events"][0]["details"][1]["value"], "pause");
    }

    #[test]
    fn counts_process_descendants() {
        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.processes = vec![
            process_row(1, 0, "codex"),
            process_row(2, 1, "node child"),
            process_row(3, 2, "sh grandchild"),
            process_row(4, 0, "other"),
        ];

        assert_eq!(app.descendant_count(1), 2);
        assert_eq!(app.descendant_count(4), 0);
    }

    #[test]
    fn scopes_agent_activity_to_process_tree() {
        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.processes = vec![
            process_row(10, 1, "codex exec left"),
            process_row(20, 1, "codex exec right"),
            process_row(11, 10, "bash child"),
            process_row(21, 20, "bash child"),
        ];
        app.snapshot.events = vec![
            parse_observer_line(
                r#"{"identity":{"agent":"codex","session":"left"},"event":{"ToolExec":{"pid":11,"ppid":10,"argv":["git","status"],"cwd":"/tmp"}}}"#,
            )
            .unwrap(),
            parse_observer_line(
                r#"{"identity":{"agent":"codex","session":"right"},"event":{"SecurityAction":{"pid":999,"ppid":20,"action":"ptrace","target":"other"}}}"#,
            )
            .unwrap(),
            event_row(
                "codex",
                Some("ambiguous"),
                None,
                "ToolExec",
                "no pid",
                Risk::Medium,
            ),
        ];

        let left = app.agent_activity_for_process(&app.snapshot.processes[0]);
        let right = app.agent_activity_for_process(&app.snapshot.processes[1]);
        let aggregate = app.agent_activity(AgentKind::Codex);

        assert_eq!(left.events, 1);
        assert_eq!(left.tools, 1);
        assert_eq!(left.high_risk, 0);
        assert_eq!(right.events, 1);
        assert_eq!(right.security, 1);
        assert_eq!(right.high_risk, 1);
        assert_eq!(aggregate.events, 3);
    }

    #[test]
    fn scopes_pidless_agent_events_by_workspace() {
        let mut left = process_row(10, 1, "codex exec left");
        left.cwd = Some("/work/left".into());
        let mut right = process_row(20, 1, "codex exec right");
        right.cwd = Some("/work/right".into());
        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.processes = vec![left, right];
        app.snapshot.events = vec![
            parse_observer_line(
                r#"{"identity":{"agent":"codex","session":"left"},"event":{"ToolExec":{"argv":["git","status"],"cwd":"/work/left/crate"}}}"#,
            )
            .unwrap(),
            parse_observer_line(
                r#"{"identity":{"agent":"codex","session":"right"},"event":{"SecurityAction":{"action":"ptrace","target":"other","cwd":"/work/right"}}}"#,
            )
            .unwrap(),
            event_row(
                "codex",
                Some("ambiguous"),
                None,
                "ToolExec",
                "no pid or cwd",
                Risk::Medium,
            ),
        ];

        let left = app.agent_activity_for_process(&app.snapshot.processes[0]);
        let right = app.agent_activity_for_process(&app.snapshot.processes[1]);

        assert_eq!(left.events, 1);
        assert_eq!(left.tools, 1);
        assert_eq!(right.events, 1);
        assert_eq!(right.security, 1);
        assert_eq!(right.high_risk, 1);
    }

    #[test]
    fn pidless_workspace_events_do_not_duplicate_across_matching_agents() {
        let mut first = process_row(10, 1, "codex exec first");
        first.cwd = Some("/work/shared".into());
        let mut second = process_row(20, 1, "codex exec second");
        second.cwd = Some("/work/shared".into());
        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.processes = vec![first, second];
        app.snapshot.events = vec![parse_observer_line(
            r#"{"identity":{"agent":"codex","session":"shared"},"event":{"ToolExec":{"argv":["git","status"],"cwd":"/work/shared"}}}"#,
        )
        .unwrap()];

        assert_eq!(
            app.agent_activity_for_process(&app.snapshot.processes[0])
                .events,
            0
        );
        assert_eq!(
            app.agent_activity_for_process(&app.snapshot.processes[1])
                .events,
            0
        );
    }

    #[test]
    fn workspace_path_matching_uses_path_boundaries() {
        assert!(workspace_paths_overlap("/work/a3s", "/work/a3s/src"));
        assert!(workspace_paths_overlap("/work/a3s/", "\"/work/a3s\""));
        assert!(!workspace_paths_overlap("/work/a3s", "/work/a3s-other"));
        assert!(!workspace_paths_overlap("-", "/work/a3s"));
    }

    #[test]
    fn finds_recent_agent_events() {
        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.events = vec![
            EventRow {
                ts: "recent".into(),
                source: "codex".into(),
                session: Some("sess-a".into()),
                task: Some("task-a".into()),
                pid: None,
                ppid: None,
                kind: "ToolExec".into(),
                message: "git status".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
            EventRow {
                ts: "recent".into(),
                source: "claude".into(),
                session: Some("sess-b".into()),
                task: None,
                pid: None,
                ppid: None,
                kind: "FileAccess".into(),
                message: "README.md".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
        ];

        let events = app.recent_agent_events(AgentKind::Codex);
        let activity = app.agent_activity(AgentKind::Codex);

        assert_eq!(events.len(), 1);
        assert_eq!(events[0].message, "git status");
        assert_eq!(activity.events, 1);
        assert_eq!(activity.sessions, 1);
        assert_eq!(activity.tools, 1);
    }

    #[test]
    fn aggregates_agent_activity_across_source_aliases() {
        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.events = vec![
            EventRow {
                ts: "recent".into(),
                source: "claude-code".into(),
                session: Some("sess-a".into()),
                task: Some("task-a".into()),
                pid: None,
                ppid: None,
                kind: "ToolExec".into(),
                message: "bash".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
            EventRow {
                ts: "recent".into(),
                source: "claude".into(),
                session: Some("sess-a".into()),
                task: Some("task-b".into()),
                pid: None,
                ppid: None,
                kind: "SecurityAction".into(),
                message: "ptrace".into(),
                details: Vec::new(),
                risk: Risk::High,
            },
            EventRow {
                ts: "recent".into(),
                source: "codex".into(),
                session: Some("other".into()),
                task: None,
                pid: None,
                ppid: None,
                kind: "ToolExec".into(),
                message: "git status".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
        ];

        let activity = app.agent_activity(AgentKind::ClaudeCode);

        assert_eq!(activity.events, 2);
        assert_eq!(activity.sessions, 1);
        assert_eq!(activity.tools, 1);
        assert_eq!(activity.security, 1);
        assert_eq!(activity.files, 0);
        assert_eq!(activity.egress, 0);
        assert_eq!(activity.high_risk, 1);
    }

    #[test]
    fn groups_observer_events_into_session_rows() {
        let events = vec![
            EventRow {
                ts: "recent".into(),
                source: "codex".into(),
                session: Some("sess-a".into()),
                task: Some("task-a".into()),
                pid: None,
                ppid: None,
                kind: "ToolExec".into(),
                message: "git status".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
            EventRow {
                ts: "recent".into(),
                source: "codex".into(),
                session: Some("sess-a".into()),
                task: Some("task-b".into()),
                pid: None,
                ppid: None,
                kind: "SecurityAction".into(),
                message: "ptrace".into(),
                details: Vec::new(),
                risk: Risk::High,
            },
            EventRow {
                ts: "recent".into(),
                source: "collector".into(),
                session: None,
                task: None,
                pid: None,
                ppid: None,
                kind: "warning".into(),
                message: "ignored".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
            EventRow {
                ts: "recent".into(),
                source: "codex".into(),
                session: Some("sess-a".into()),
                task: Some("task-c".into()),
                pid: None,
                ppid: None,
                kind: "FileAccess".into(),
                message: "README.md".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
            EventRow {
                ts: "recent".into(),
                source: "codex".into(),
                session: Some("sess-a".into()),
                task: Some("task-d".into()),
                pid: None,
                ppid: None,
                kind: "Egress".into(),
                message: "example.com".into(),
                details: Vec::new(),
                risk: Risk::Medium,
            },
        ];

        let rows = session_rows(&events);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].source, "codex");
        assert_eq!(rows[0].session, "sess-a");
        assert_eq!(rows[0].events, 4);
        assert_eq!(rows[0].tools, 1);
        assert_eq!(rows[0].security, 1);
        assert_eq!(rows[0].files, 1);
        assert_eq!(rows[0].egress, 1);
        assert_eq!(rows[0].high_risk, 1);
        assert_eq!(rows[0].risk, Risk::High);
        assert_eq!(rows[0].workspace, "-");
    }

    #[test]
    fn sessions_extract_workspace_from_event_payload() {
        let event = parse_observer_line(
            r#"{"identity":{"agent":"codex","task":"task-a","session":"sess-a"},"event":{"ToolExec":{"argv":["cargo","test"],"cwd":"/Users/roylin/code/a3s"}}}"#,
        )
        .unwrap();

        let rows = session_rows(&[event]);

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].workspace, "/Users/roylin/code/a3s");
    }

    #[test]
    fn aggregates_llm_model_and_tokens() {
        let event = parse_observer_line(
            r#"{"identity":{"agent":"codex","task":"task-a","session":"sess-a"},"event":{"LlmApi":{"pid":7,"is_request":false,"model":"gpt-4o","prompt_tokens":12,"completion_tokens":34}}}"#,
        )
        .unwrap();
        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.events = vec![event.clone()];

        let activity = app.agent_activity(AgentKind::Codex);
        assert_eq!(activity.llm, 1);
        assert_eq!(activity.prompt_tokens, 12);
        assert_eq!(activity.completion_tokens, 34);
        assert_eq!(activity.total_tokens, 46);
        assert_eq!(activity.model, "gpt-4o");

        let sessions = session_rows(&[event]);
        assert_eq!(sessions[0].llm, 1);
        assert_eq!(sessions[0].total_tokens, 46);
        assert_eq!(sessions[0].model, "gpt-4o");
    }

    #[test]
    fn extracts_llm_provider_latency_and_wire_metrics() {
        let event = parse_observer_line(
            r#"{"identity":{"agent":"codex","task":"task-a","session":"sess-a"},"provider":"OpenAi","event":{"LlmCall":{"pid":7,"sni":"api.openai.com","peer":"1.2.3.4","req_bytes":1024,"resp_bytes":2048,"latency":{"secs":1,"nanos":500000000},"ttft":{"secs":0,"nanos":250000000}}}}"#,
        )
        .unwrap();
        let network = event_llm_network(&event).unwrap();

        assert_eq!(network.provider.as_deref(), Some("OpenAi"));
        assert_eq!(network.latency_ms, Some(1500));
        assert_eq!(network.ttft_ms, Some(250));
        assert_eq!(network.req_bytes, 1024);
        assert_eq!(network.resp_bytes, 2048);

        let mut app = TopApp::new(TopOptions::default());
        app.snapshot.events = vec![event.clone()];
        let activity = app.agent_activity(AgentKind::Codex);
        assert_eq!(activity.provider, "OpenAi");
        assert_eq!(activity.latency_ms, 1500);
        assert_eq!(activity.latency_samples, 1);
        assert_eq!(activity.ttft_ms, 250);
        assert_eq!(activity.req_bytes, 1024);
        assert_eq!(activity.resp_bytes, 2048);

        let sessions = session_rows(&[event]);
        assert_eq!(sessions[0].provider, "OpenAi");
        assert_eq!(sessions[0].latency_ms, 1500);
        assert_eq!(sessions[0].ttft_ms, 250);
        assert_eq!(sessions[0].req_bytes, 1024);
        assert_eq!(sessions[0].resp_bytes, 2048);
    }

    #[test]
    fn header_summarizes_llm_usage() {
        let mut app = TopApp::new(TopOptions::default());
        app.width = 360;
        app.snapshot.events = vec![parse_observer_line(
            r#"{"identity":{"agent":"codex","session":"sess-a"},"event":{"LlmApi":{"pid":7,"prompt_tokens":1200,"completion_tokens":300}}}"#,
        )
        .unwrap()];

        let plain = a3s_tui::style::strip_ansi(&app.header());

        assert!(plain.contains("llm:1"));
        assert!(plain.contains("tok:1.5K"));
    }

    #[test]
    fn header_summarizes_container_states() {
        let mut app = TopApp::new(TopOptions::default());
        app.width = 360;
        app.snapshot.containers = vec![
            container_row("run", "run", "Up 2 minutes", Some(1.0), Some(1.0)),
            container_row("pause", "pause", "Up 4 minutes (Paused)", None, None),
            container_row("exit", "exit", "Exited (0) 1 hour ago", None, None),
            container_row("dead", "dead", "dead", None, None),
        ];

        let plain = a3s_tui::style::strip_ansi(&app.header());

        assert!(plain.contains("boxes:4 run:1 pause:1 exit:1 dead:1"));
    }

    #[test]
    fn header_uses_shared_status_bar_and_preserves_right_status() {
        let mut app = TopApp::new(TopOptions::default());
        app.width = 44;
        app.paused = true;
        app.snapshot.containers = vec![
            container_row("run", "run", "Up 2 minutes", Some(1.0), Some(1.0)),
            container_row("pause", "pause", "Up 4 minutes (Paused)", None, None),
            container_row("exit", "exit", "Exited (0) 1 hour ago", None, None),
        ];
        app.snapshot.processes = vec![process_row(42, 1, "codex")];

        let rendered = app.header();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.starts_with(" a3s top"), "{plain}");
        assert!(plain.ends_with("paused"), "{plain}");
        assert!(plain.contains('…'), "{plain}");
        assert_eq!(a3s_tui::style::visible_len(&rendered), 44);
    }

    #[test]
    fn tokens_sort_prioritizes_busy_agents_and_sessions() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.sort_by = SortBy::Tokens;
        app.snapshot.processes = vec![
            process_row(10, 1, "claude worker"),
            process_row(20, 1, "codex worker"),
        ];
        app.snapshot.events = vec![
            parse_observer_line(
                r#"{"identity":{"agent":"claude","session":"sess-claude"},"event":{"LlmApi":{"pid":10,"model":"claude-3","prompt_tokens":5,"completion_tokens":5}}}"#,
            )
            .unwrap(),
            parse_observer_line(
                r#"{"identity":{"agent":"codex","session":"sess-codex"},"event":{"LlmApi":{"pid":20,"model":"gpt-4o","prompt_tokens":100,"completion_tokens":50}}}"#,
            )
            .unwrap(),
        ];

        let agents = app.filtered_agents();
        assert_eq!(agents[0].agent, Some(AgentKind::Codex));

        app.tab = Tab::Sessions;
        let sessions = app.filtered_sessions();
        assert_eq!(sessions[0].session, "sess-codex");
        assert_eq!(sessions[0].total_tokens, 150);
    }

    #[test]
    fn sessions_table_renders_grouped_activity() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Sessions,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 180;
        app.snapshot.events = vec![EventRow {
            ts: "recent".into(),
            source: "claude-code".into(),
            session: Some("sess-a".into()),
            task: Some("task-a".into()),
            pid: None,
            ppid: None,
            kind: "ToolExec".into(),
            message: "bash".into(),
            details: Vec::new(),
            risk: Risk::Medium,
        }];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("AGENT"));
        assert!(plain.contains("claude"));
        assert!(plain.contains("sess-a"));
        assert!(plain.contains("task-a"));
        assert!(plain.contains("CWD"));
        assert!(plain.contains("Tool"));
        assert!(plain.contains("SEC"));
        assert!(plain.contains("FILE"));
        assert!(plain.contains("NET"));
    }

    #[test]
    fn sessions_table_renders_llm_usage() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Sessions,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 240;
        app.snapshot.events = vec![parse_observer_line(
            r#"{"identity":{"agent":"codex","task":"task-a","session":"sess-a"},"provider":"OpenAi","event":{"LlmApi":{"pid":7,"model":"gpt-4o","prompt_tokens":1200,"completion_tokens":3400,"latency_ms":1500}}}"#,
        )
        .unwrap()];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("LLM"));
        assert!(plain.contains("TOK"));
        assert!(plain.contains("MODEL"));
        assert!(plain.contains("4.6K"));
        assert!(plain.contains("gpt-4o"));
        assert!(plain.contains("PROV"));
        assert!(plain.contains("OpenAi"));
        assert!(plain.contains("1.5s"));
    }

    #[test]
    fn agents_tree_groups_sessions_processes_and_events_by_agent() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 360;
        app.snapshot.processes = vec![
            process_row(42, 1, "codex exec task"),
            process_row(100, 42, "bash -lc cargo test"),
        ];
        app.snapshot.events = vec![
            event_row(
                "codex",
                Some("sess-a"),
                Some("task-a"),
                "SecurityAction",
                "ptrace",
                Risk::High,
            ),
            event_row(
                "codex",
                Some("sess-a"),
                Some("task-b"),
                "FileAccess",
                "README.md",
                Risk::Medium,
            ),
            event_row(
                "codex",
                Some("sess-a"),
                Some("task-c"),
                "Egress",
                "example.com",
                Risk::Medium,
            ),
        ];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("Agents"));
        assert!(plain.contains("codex"));
        assert!(plain.contains("Sessions (1)"));
        assert!(plain.contains("Processes (1 system · 1 agent)"));
        assert!(plain.contains("Events (3)"));
        assert!(plain.contains("sess-a"));
        assert!(plain.contains("task-a"));
        assert!(plain.contains("> pid 100"));
        assert!(plain.contains("SecurityAction"));
        assert!(plain.contains("FileAccess"));
        assert!(plain.contains("Egress"));
    }

    #[test]
    fn agents_tree_uses_agent_theme_colors() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 360;
        app.snapshot.processes = vec![
            process_row(42, 1, "claude code task"),
            process_row(84, 1, "codex exec task"),
        ];

        let rendered = app.table();
        let claude_prefix = agent_tree_label(AgentKind::ClaudeCode, "> claude");
        let codex_prefix = agent_tree_label(AgentKind::Codex, "  codex");

        assert!(rendered.contains(claude_prefix.trim_end_matches("\u{1b}[0m")));
        assert!(rendered.contains(codex_prefix.trim_end_matches("\u{1b}[0m")));
        assert!(a3s_tui::style::strip_ansi(&rendered).contains("codex"));
    }

    #[test]
    fn agents_filter_uses_working_directory() {
        let mut row = process_row(42, 1, "codex exec task");
        row.cwd = Some("/Users/roylin/code/a3s".into());
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 280;
        app.snapshot.processes = vec![row];

        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("Processes (0 system · 1 agent)"));

        app.filter = "code/a3s".into();
        assert_eq!(app.filtered_agents().len(), 1);
    }

    #[test]
    fn agents_filter_matches_related_sessions_and_events() {
        let mut codex = process_row(42, 1, "codex exec task");
        codex.cwd = Some("/work/a3s".into());
        let mut claude = process_row(84, 1, "claude code other");
        claude.cwd = Some("/work/other".into());
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.snapshot.processes = vec![codex, claude];
        app.snapshot.events = vec![
            parse_observer_line(
                r#"{"identity":{"agent":"codex","session":"sess-target","task":"task-target"},"provider":"openai","event":{"LlmCall":{"pid":42,"model":"gpt-5","prompt_tokens":10,"completion_tokens":5,"cwd":"/work/a3s"}}}"#,
            )
            .unwrap(),
            parse_observer_line(
                r#"{"identity":{"agent":"claude","session":"other"},"event":{"ToolExec":{"pid":84,"argv":["pwd"],"cwd":"/work/other"}}}"#,
            )
            .unwrap(),
        ];

        app.filter = "sess-target".into();
        assert_eq!(app.filtered_agents().len(), 1);
        assert_eq!(app.filtered_agents()[0].pid, 42);

        app.filter = "gpt-5".into();
        assert_eq!(app.filtered_agents().len(), 1);
        assert_eq!(app.filtered_agents()[0].pid, 42);

        app.filter = "prompt_tokens".into();
        assert_eq!(app.filtered_agents().len(), 1);
        assert_eq!(app.filtered_agents()[0].pid, 42);

        app.filter = "other".into();
        assert_eq!(app.filtered_agents().len(), 1);
        assert_eq!(app.filtered_agents()[0].pid, 84);
    }

    #[test]
    fn process_tree_usage_sums_descendants() {
        let rows = vec![
            ProcessRow {
                cpu_pct: 1.0,
                mem_pct: 2.0,
                ..process_row(42, 1, "codex exec task")
            },
            ProcessRow {
                cpu_pct: 30.0,
                mem_pct: 4.0,
                ..process_row(100, 42, "cargo test")
            },
            ProcessRow {
                cpu_pct: 5.0,
                mem_pct: 1.0,
                ..process_row(101, 100, "rustc")
            },
            ProcessRow {
                cpu_pct: 99.0,
                mem_pct: 99.0,
                ..process_row(200, 1, "other")
            },
        ];

        let usage = process_tree_usage(&rows, 42);

        assert_eq!(usage.descendants, 2);
        assert_eq!(usage.cpu_pct, 36.0);
        assert_eq!(usage.mem_pct, 7.0);
    }

    #[test]
    fn agents_tree_processes_show_agent_started_system_processes() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 260;
        app.snapshot.processes = vec![
            process_row(42, 1, "codex exec task"),
            ProcessRow {
                cpu_pct: 30.0,
                mem_pct: 4.0,
                ..process_row(100, 42, "bash -lc cargo test")
            },
            ProcessRow {
                cpu_pct: 5.0,
                mem_pct: 1.0,
                ..process_row(101, 100, "rustc")
            },
            process_row(200, 1, "unrelated"),
        ];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("Processes (2 system · 1 agent)"));
        assert!(plain.contains("> pid 100 · ppid 42 · CPU 30.0% · MEM 4.0%"));
        assert!(plain.contains("pid 101 · ppid 100 · CPU 5.0% · MEM 1.0%"));
        assert!(!plain.contains("> pid 42"));
        assert!(!plain.contains("pid 200"));
    }

    #[test]
    fn agents_tree_uses_process_tree_resource_totals() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 220;
        app.snapshot.processes = vec![
            ProcessRow {
                cpu_pct: 1.0,
                mem_pct: 2.0,
                ..process_row(42, 1, "codex exec task")
            },
            ProcessRow {
                cpu_pct: 30.0,
                mem_pct: 4.0,
                ..process_row(100, 42, "cargo test")
            },
        ];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("31.0"));
        assert!(plain.contains("6.0"));
        assert!(plain.contains("children 1"));
    }

    #[test]
    fn agents_tree_selection_can_land_on_session_only_agent() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 220;
        app.filter = "sess-a".into();
        app.snapshot.events = vec![event_row(
            "codex",
            Some("sess-a"),
            Some("task-a"),
            "LlmCall",
            "thinking",
            Risk::Low,
        )];

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert_eq!(app.visible_len(), 1);
        assert!(plain.contains("> codex"));
        assert!(plain.contains("S1 P0 E1"));
        assert!(plain.contains("Sessions (1)"));
        assert!(plain.contains("Processes (0 system · 0 agent)"));
        assert!(plain.contains("Events (1)"));
    }

    #[test]
    fn o_focuses_session_only_agent_from_agents_tree() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.filter = "sess-a".into();
        app.snapshot.events = vec![event_row(
            "codex",
            Some("sess-a"),
            Some("task-a"),
            "ToolExec",
            "bash",
            Risk::Medium,
        )];

        app.handle_key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.tab, Tab::Sessions);
        assert_eq!(
            app.focused_session,
            Some(SessionFocus {
                source: "codex".into(),
                session: "sess-a".into(),
            })
        );
    }

    #[test]
    fn agents_tree_renders_separate_resource_trends() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 220;
        app.snapshot.processes = vec![
            ProcessRow {
                cpu_pct: 1.0,
                mem_pct: 2.0,
                ..process_row(42, 1, "codex exec task")
            },
            ProcessRow {
                cpu_pct: 30.0,
                mem_pct: 4.0,
                ..process_row(100, 42, "cargo test")
            },
        ];
        app.history.insert(
            agent_tree_history_key(42),
            MetricHistory {
                cpu: vec![1.0, 31.0],
                mem: vec![2.0, 6.0],
                ..MetricHistory::default()
            },
        );

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("Resources"));
        assert!(plain.contains("CPU  31.0%"));
        assert!(plain.contains("MEM   6.0%"));
    }

    #[test]
    fn agents_tree_does_not_double_count_nested_agent_processes() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 220;
        app.snapshot.processes = vec![
            ProcessRow {
                cpu_pct: 1.0,
                mem_pct: 2.0,
                ..process_row(42, 1, "codex exec parent")
            },
            ProcessRow {
                cpu_pct: 30.0,
                mem_pct: 4.0,
                ..process_row(100, 42, "codex exec child")
            },
            ProcessRow {
                cpu_pct: 5.0,
                mem_pct: 1.0,
                ..process_row(101, 100, "cargo test")
            },
        ];

        let group = app
            .agent_tree_groups()
            .into_iter()
            .find(|group| group.agent == AgentKind::Codex)
            .unwrap();

        assert_eq!(group.usage.cpu_pct, 36.0);
        assert_eq!(group.usage.mem_pct, 7.0);
        assert_eq!(group.usage.descendants, 2);
    }

    #[test]
    fn agent_cpu_sort_uses_process_tree_totals() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.sort_by = SortBy::Cpu;
        app.snapshot.processes = vec![
            ProcessRow {
                cpu_pct: 1.0,
                ..process_row(10, 1, "codex exec slow-root")
            },
            ProcessRow {
                cpu_pct: 50.0,
                ..process_row(11, 10, "cargo test")
            },
            ProcessRow {
                cpu_pct: 20.0,
                ..process_row(20, 1, "claude")
            },
        ];

        let rows = app.filtered_agents();

        assert_eq!(rows[0].pid, 10);
    }

    #[test]
    fn risk_filter_applies_to_events_and_sessions() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.risk_filter = RiskFilter::High;
        app.snapshot.events = vec![
            event_row(
                "codex",
                Some("safe"),
                Some("task-a"),
                "ToolExec",
                "git status",
                Risk::Medium,
            ),
            event_row(
                "codex",
                Some("danger"),
                Some("task-b"),
                "SecurityAction",
                "ptrace",
                Risk::High,
            ),
        ];

        let events = app.filtered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].session.as_deref(), Some("danger"));

        app.tab = Tab::Sessions;
        let sessions = app.filtered_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session, "danger");
    }

    #[test]
    fn kind_filter_applies_to_events_sessions_and_session_focus() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.kind_filter = KindFilter::Security;
        app.snapshot.events = vec![
            event_row(
                "codex",
                Some("sess-a"),
                Some("task-a"),
                "ToolExec",
                "git status",
                Risk::Medium,
            ),
            event_row(
                "codex",
                Some("sess-a"),
                Some("task-b"),
                "SecurityAction",
                "ptrace",
                Risk::High,
            ),
            event_row(
                "codex",
                Some("sess-b"),
                Some("task-c"),
                "FileAccess",
                "README.md",
                Risk::Medium,
            ),
        ];

        let events = app.filtered_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].kind, "SecurityAction");

        app.tab = Tab::Sessions;
        let sessions = app.filtered_sessions();
        assert_eq!(sessions.len(), 1);
        assert_eq!(sessions[0].session, "sess-a");
        assert_eq!(sessions[0].events, 1);
        assert_eq!(sessions[0].last_kind, "SecurityAction");

        app.focused_session = Some(SessionFocus {
            source: "codex".into(),
            session: "sess-a".into(),
        });
        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("ptrace"));
        assert!(!plain.contains("git status"));
    }

    #[test]
    fn high_risk_filter_keeps_agents_with_high_risk_activity() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.risk_filter = RiskFilter::High;
        app.snapshot.processes = vec![process_row(42, 1, "codex exec task")];
        app.snapshot.events = vec![event_row(
            "codex",
            Some("sess-a"),
            Some("task-a"),
            "SecurityAction",
            "ptrace",
            Risk::High,
        )];

        let agents = app.filtered_agents();
        assert_eq!(agents.len(), 1);
        assert_eq!(agents[0].pid, 42);
    }

    #[test]
    fn filter_keys_cycle_and_reset_selection() {
        let mut app = TopApp::new(TopOptions::default());
        app.selected = 3;
        app.detail = true;

        app.handle_key(KeyEvent {
            code: KeyCode::Char('!'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.risk_filter, RiskFilter::Medium);
        assert_eq!(app.selected, 0);
        assert!(!app.detail);

        app.selected = 2;
        app.detail = true;
        app.handle_key(KeyEvent {
            code: KeyCode::Char('g'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.kind_filter, KindFilter::Tool);
        assert_eq!(app.selected, 0);
        assert!(!app.detail);
    }

    #[test]
    fn filter_editor_esc_clears_and_ctrl_keys_edit() {
        let mut app = TopApp::new(TopOptions::default());
        let key = |code| KeyEvent {
            code,
            modifiers: KeyModifiers::empty(),
        };
        let ctrl = |c| KeyEvent {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::CONTROL,
        };

        app.filter = "codex".into();
        app.selected = 4;
        app.detail = true;
        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(ctrl('u'));
        app.handle_key(key(KeyCode::Char('a')));
        app.handle_key(key(KeyCode::Char('p')));
        app.handle_key(key(KeyCode::Char('i')));
        app.handle_key(key(KeyCode::Esc));

        assert_eq!(app.filter, "");
        assert!(!app.editing_filter);
        assert_eq!(app.selected, 0);
        assert!(!app.detail);
        assert!(app.filter_before_edit.is_none());

        app.handle_key(key(KeyCode::Char('/')));
        app.handle_key(ctrl('u'));
        for c in "agent run".chars() {
            app.handle_key(key(KeyCode::Char(c)));
        }
        app.handle_key(ctrl('w'));
        app.handle_key(key(KeyCode::Enter));

        assert_eq!(app.filter, "agent ");
        assert!(!app.editing_filter);
        assert!(app.filter_before_edit.is_none());
    }

    #[test]
    fn sort_key_opens_select_panel_and_applies_choice() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.sort_by = SortBy::Cpu;

        app.handle_key(KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::empty(),
        });

        let panel = app.sort_panel.as_ref().unwrap();
        assert_eq!(panel.select.selected_index(), 0);
        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("sort by"));
        assert!(plain.contains("state"));
        assert!(plain.contains("uptime"));
        assert!(!plain.contains("tokens"));

        app.handle_key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::empty(),
        });
        app.handle_key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.sort_by, SortBy::Mem);
        assert!(app.sort_panel.is_none());
    }

    #[test]
    fn sort_panel_accepts_number_shortcuts() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.sort_by = SortBy::Cpu;
        app.open_sort_panel();

        let plain = a3s_tui::style::strip_ansi(&app.sort_panel_view());
        assert!(plain.contains("> 1 cpu"));
        assert!(plain.contains("  2 mem"));
        assert!(plain.contains("  3 net"));

        app.handle_key(KeyEvent {
            code: KeyCode::Char('4'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.sort_by, SortBy::Block);
        assert!(app.sort_panel.is_none());
    }

    #[test]
    fn sort_and_connector_panels_use_shared_menu_panel_and_fit_width() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 34;

        app.open_sort_panel();
        let sort_rendered = app.sort_panel_view();
        let sort_plain = a3s_tui::style::strip_ansi(&sort_rendered);
        assert!(sort_plain.contains("sort by"), "{sort_plain}");
        assert!(sort_plain.contains("> 1 cpu"), "{sort_plain}");
        assert!(
            sort_rendered.contains("\x1b["),
            "selected sort row should be styled"
        );
        assert!(
            sort_rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 34),
            "{sort_plain}"
        );

        app.open_connector_panel();
        let connector_rendered = app.connector_panel_view();
        let connector_plain = a3s_tui::style::strip_ansi(&connector_rendered);
        assert!(
            connector_plain.contains("container connector"),
            "{connector_plain}"
        );
        assert!(connector_plain.contains("> 1 a3s-box"), "{connector_plain}");
        assert!(
            connector_rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 34),
            "{connector_plain}"
        );
    }

    #[test]
    fn sort_panel_choices_are_scoped_to_current_tab() {
        let mut agents = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        agents.open_sort_panel();
        assert_eq!(
            agents.sort_panel.as_ref().unwrap().choices,
            vec![
                SortBy::Cpu,
                SortBy::Mem,
                SortBy::Net,
                SortBy::Pids,
                SortBy::Name,
                SortBy::Tokens,
            ]
        );

        let mut processes = TopApp::new(TopOptions {
            tab: Tab::Processes,
            ..TopOptions::default()
        });
        processes.open_sort_panel();
        let choices = &processes.sort_panel.as_ref().unwrap().choices;
        assert!(choices.contains(&SortBy::Id));
        assert!(!choices.contains(&SortBy::Tokens));

        let mut events = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        events.open_sort_panel();
        assert!(events.sort_panel.is_none());
        assert!(events.note.as_deref().unwrap().contains("newest-first"));
    }

    #[test]
    fn esc_closes_sort_panel_without_changing_sort() {
        let mut app = TopApp::new(TopOptions::default());
        app.sort_by = SortBy::Tokens;

        app.handle_key(KeyEvent {
            code: KeyCode::Char('s'),
            modifiers: KeyModifiers::empty(),
        });
        app.handle_key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.sort_by, SortBy::Tokens);
        assert!(app.sort_panel.is_none());
    }

    #[test]
    fn connector_key_opens_select_panel_and_switches_runtime() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.focused_container = Some("abcdef".into());
        app.container_processes = Some(ContainerProcessPanel {
            container_id: "abcdef".into(),
            container_name: "app".into(),
            rows: Vec::new(),
            scroll: 0,
            error: None,
            loading: false,
        });

        app.handle_key(KeyEvent {
            code: KeyCode::Char('C'),
            modifiers: KeyModifiers::SHIFT,
        });

        let panel = app.connector_panel.as_ref().unwrap();
        assert_eq!(panel.select.selected_index(), 0);
        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("container connector"));
        assert!(plain.contains("a3s-box"));
        assert!(plain.contains("docker"));
        assert!(plain.contains("runc"));

        app.handle_key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::empty(),
        });
        let cmd = app.handle_key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
        });

        assert!(cmd.is_some());
        assert_eq!(app.connector, ContainerConnector::Docker);
        assert!(app.connector_panel.is_none());
        assert!(app.focused_container.is_none());
        assert!(app.container_processes.is_none());
    }

    #[test]
    fn connector_panel_accepts_number_shortcuts() {
        let mut app = TopApp::new(TopOptions::default());
        app.open_connector_panel();

        let plain = a3s_tui::style::strip_ansi(&app.connector_panel_view());
        assert!(plain.contains("> 1 a3s-box"));
        assert!(plain.contains("  2 docker"));
        assert!(plain.contains("  3 runc"));

        let cmd = app.handle_key(KeyEvent {
            code: KeyCode::Char('3'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(cmd.is_some());
        assert_eq!(app.connector, ContainerConnector::RunC);
        assert!(app.connector_panel.is_none());
    }

    #[test]
    fn esc_closes_connector_panel_without_changing_runtime() {
        let mut app = TopApp::new(TopOptions::default());
        app.connector = ContainerConnector::RunC;

        app.handle_key(KeyEvent {
            code: KeyCode::Char('C'),
            modifiers: KeyModifiers::SHIFT,
        });
        app.handle_key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.connector, ContainerConnector::RunC);
        assert!(app.connector_panel.is_none());
    }

    #[test]
    fn focused_session_renders_event_stream_without_unrelated_events() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Sessions,
            ..TopOptions::default()
        });
        app.width = 72;
        app.snapshot.events = vec![
            event_row(
                "codex",
                Some("sess-a"),
                Some("task-a"),
                "ToolExec",
                "git status",
                Risk::Medium,
            ),
            event_row(
                "codex",
                Some("sess-b"),
                Some("task-b"),
                "ToolExec",
                "npm test",
                Risk::Medium,
            ),
            event_row(
                "claude-code",
                Some("sess-a"),
                Some("task-c"),
                "FileAccess",
                "README.md",
                Risk::Medium,
            ),
        ];
        app.focused_session = Some(SessionFocus {
            source: "codex".into(),
            session: "sess-a".into(),
        });

        let rendered = app.table();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("session view codex"));
        assert!(plain.contains("sess-a"));
        assert!(plain.contains("task task-a"), "{plain}");
        assert!(plain.contains("git status"));
        assert!(!plain.contains("npm test"));
        assert!(!plain.contains("README.md"));
        assert!(rendered.contains("\x1b["), "session focus should be styled");
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 72),
            "{plain}"
        );
    }

    #[test]
    fn focused_session_renders_event_process_scope() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Sessions,
            ..TopOptions::default()
        });
        app.width = 180;
        app.snapshot.events = vec![parse_observer_line(
            r#"{"identity":{"agent":"codex","task":"task-a","session":"sess-a"},"event":{"ToolExec":{"pid":77,"ppid":42,"argv":["git","status"],"cwd":"/tmp/a3s"}}}"#,
        )
        .unwrap()];
        app.focused_session = Some(SessionFocus {
            source: "codex".into(),
            session: "sess-a".into(),
        });

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("PID"));
        assert!(plain.contains("PPID"));
        assert!(plain.contains("77"));
        assert!(plain.contains("42"));
    }

    #[test]
    fn o_focuses_selected_session() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Sessions,
            ..TopOptions::default()
        });
        app.snapshot.events = vec![event_row(
            "codex",
            Some("sess-a"),
            Some("task-a"),
            "ToolExec",
            "git status",
            Risk::Medium,
        )];

        app.handle_key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(
            app.focused_session,
            Some(SessionFocus {
                source: "codex".into(),
                session: "sess-a".into(),
            })
        );
        assert_eq!(app.visible_len(), 1);

        app.handle_key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.focused_session.is_none());
    }

    #[test]
    fn o_on_event_focuses_event_session() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.snapshot.events = vec![
            event_row(
                "codex",
                Some("sess-a"),
                Some("task-a"),
                "ToolExec",
                "git status",
                Risk::Medium,
            ),
            event_row(
                "codex",
                Some("sess-b"),
                Some("task-b"),
                "ToolExec",
                "npm test",
                Risk::Medium,
            ),
        ];

        app.handle_key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.tab, Tab::Sessions);
        assert_eq!(
            app.focused_session,
            Some(SessionFocus {
                source: "codex".into(),
                session: "sess-a".into(),
            })
        );

        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("session view codex"));
        assert!(plain.contains("git status"));
        assert!(!plain.contains("npm test"));
    }

    #[test]
    fn o_on_non_agent_event_sets_note() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.snapshot.events = vec![event_row(
            "collector",
            None,
            None,
            "warning",
            "docker unavailable",
            Risk::Medium,
        )];

        app.handle_key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.tab, Tab::Events);
        assert!(app.focused_session.is_none());
        assert_eq!(
            app.note.as_deref(),
            Some("event focus is available for coding-agent events")
        );
    }

    #[test]
    fn event_detail_shows_identity_scope_and_focus_hint() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.detail = true;
        app.snapshot.events = vec![event_row(
            "codex",
            Some("sess-a"),
            Some("task-a"),
            "SecurityAction",
            "action=ptrace pid=3 target=other",
            Risk::High,
        )];

        let plain = a3s_tui::style::strip_ansi(&app.details());

        assert!(plain.contains("event codex"));
        assert!(plain.contains("SecurityAction"));
        assert!(plain.contains("session sess-a task task-a"));
        assert!(plain.contains("source codex · session sess-a · task task-a"));
        assert!(plain.contains("actions o session focus"));
    }

    #[test]
    fn event_detail_shows_payload_fields() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Events,
            ..TopOptions::default()
        });
        app.detail = true;
        app.snapshot.events = vec![parse_observer_line(
            r#"{"identity":{"agent":"codex","task":"task-a","session":"sess-a"},"event":{"ToolExec":{"pid":7,"argv":["git","status"],"cwd":"/tmp/a3s"}}}"#,
        )
        .unwrap()];

        let plain = a3s_tui::style::strip_ansi(&app.details());

        assert!(plain.contains("detail argv [\"git\",\"status\"]"));
        assert!(plain.contains("detail cwd /tmp/a3s"));
        assert!(plain.contains("detail pid 7"));
    }

    #[test]
    fn session_focus_detail_shows_selected_event() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Sessions,
            ..TopOptions::default()
        });
        app.detail = true;
        app.focused_session = Some(SessionFocus {
            source: "codex".into(),
            session: "sess-a".into(),
        });
        app.snapshot.events = vec![event_row(
            "codex",
            Some("sess-a"),
            Some("task-a"),
            "ToolExec",
            "argv=[\"git\",\"status\"]",
            Risk::Medium,
        )];

        let plain = a3s_tui::style::strip_ansi(&app.details());

        assert!(plain.contains("event codex"));
        assert!(plain.contains("ToolExec"));
        assert!(plain.contains("argv=[\"git\",\"status\"]"));
        assert!(plain.contains("actions o session focus"));
    }

    #[test]
    fn container_menu_reflects_running_state() {
        let mut running = container_row("abcdef", "app", "Up 2 minutes", Some(1.0), Some(2.0));
        running.ports = "8080:80".into();
        let stopped = container_row("123456", "job", "Exited (0) 1 hour ago", None, None);
        let paused = container_row(
            "999999",
            "db",
            "Up 4 minutes (Paused)",
            Some(0.0),
            Some(1.0),
        );

        let running_actions = container_menu_items(&running)
            .into_iter()
            .map(|item| item.action)
            .collect::<Vec<_>>();
        let stopped_actions = container_menu_items(&stopped)
            .into_iter()
            .map(|item| item.action)
            .collect::<Vec<_>>();
        let paused_actions = container_menu_items(&paused)
            .into_iter()
            .map(|item| item.action)
            .collect::<Vec<_>>();

        assert!(running_actions.contains(&ContainerMenuAction::ExecShell));
        assert!(running_actions.contains(&ContainerMenuAction::OpenBrowser));
        assert!(running_actions.contains(&ContainerMenuAction::Pause));
        assert!(running_actions.contains(&ContainerMenuAction::Stop));
        assert!(!running_actions.contains(&ContainerMenuAction::Remove));
        assert!(stopped_actions.contains(&ContainerMenuAction::Start));
        assert!(stopped_actions.contains(&ContainerMenuAction::Remove));
        assert!(paused_actions.contains(&ContainerMenuAction::Unpause));
        assert!(!paused_actions.contains(&ContainerMenuAction::ExecShell));
    }

    #[test]
    fn runc_menu_hides_docker_only_actions() {
        let mut row = container_row("abcdef", "app", "running", Some(1.0), Some(2.0));
        row.connector = ContainerConnector::RunC;

        let actions = container_menu_items(&row)
            .into_iter()
            .map(|item| item.action)
            .collect::<Vec<_>>();

        assert!(actions.contains(&ContainerMenuAction::Focus));
        assert!(actions.contains(&ContainerMenuAction::Pause));
        assert!(actions.contains(&ContainerMenuAction::Stop));
        assert!(!actions.contains(&ContainerMenuAction::Logs));
        assert!(!actions.contains(&ContainerMenuAction::ExecShell));
        assert!(!actions.contains(&ContainerMenuAction::Restart));
    }

    #[test]
    fn w_opens_first_published_container_web_port() {
        let external_action = Arc::new(Mutex::new(None));
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            external_action: external_action.clone(),
            ..TopOptions::default()
        });
        let mut row = container_row("abcdef", "app", "Up 2 minutes", Some(1.0), Some(2.0));
        row.ports = "8080:80".into();
        app.snapshot.containers = vec![row];

        let cmd = app.handle_key(KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(cmd.is_some());
        let action = external_action.lock().unwrap().clone();
        assert!(matches!(
            action,
            Some(ExternalAction::OpenBrowser { url, name })
                if url == "http://localhost:8080/" && name == "app"
        ));
    }

    #[test]
    fn w_without_published_port_sets_note() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.snapshot.containers = vec![container_row(
            "abcdef",
            "app",
            "Up 2 minutes",
            Some(1.0),
            Some(2.0),
        )];

        let cmd = app.handle_key(KeyEvent {
            code: KeyCode::Char('w'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(cmd.is_none());
        assert_eq!(
            app.note.as_deref(),
            Some("app has no published web port to open")
        );
    }

    #[test]
    fn a3s_box_menu_exposes_box_runtime_actions() {
        let mut row = container_row("abcdef", "app", "running", Some(1.0), Some(2.0));
        row.connector = ContainerConnector::A3sBox;
        row.ports = "8080:80".into();

        let actions = container_menu_items(&row)
            .into_iter()
            .map(|item| item.action)
            .collect::<Vec<_>>();

        assert!(actions.contains(&ContainerMenuAction::Logs));
        assert!(actions.contains(&ContainerMenuAction::ExecShell));
        assert!(actions.contains(&ContainerMenuAction::OpenBrowser));
        assert!(actions.contains(&ContainerMenuAction::Restart));
        assert!(actions.contains(&ContainerMenuAction::Pause));
        assert!(actions.contains(&ContainerMenuAction::Stop));
    }

    #[test]
    fn enter_opens_container_menu_and_x_toggles_detail() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.snapshot.containers = vec![container_row(
            "abcdef",
            "app",
            "Up 2 minutes",
            Some(1.0),
            Some(2.0),
        )];

        app.handle_key(KeyEvent {
            code: KeyCode::Enter,
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.container_menu.is_some());

        app.container_menu = None;
        app.handle_key(KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.detail);
    }

    #[test]
    fn container_detail_uses_shared_detail_panel_and_fits_width() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 64;
        app.detail = true;
        let mut row = container_row("abcdef123456", "app", "Up 2 minutes", Some(1.0), Some(2.0));
        row.ports = "0.0.0.0:3000->3000/tcp".into();
        row.net_io = "1.00KiB / 2.00KiB".into();
        row.block_io = "4.00KiB / 8.00KiB".into();
        app.snapshot.containers = vec![row];

        let rendered = app.details();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("container app (abcdef123456)"), "{plain}");
        assert!(plain.contains("image img"), "{plain}");
        assert!(plain.contains("status Up 2 minutes"), "{plain}");
        assert!(plain.contains("cpu 1.0%"), "{plain}");
        assert!(plain.contains("actions"), "{plain}");
        assert!(plain.contains("Enter menu"), "{plain}");
        assert!(
            rendered.contains("\x1b["),
            "container detail should be styled"
        );
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 64),
            "{plain}"
        );
    }

    #[test]
    fn container_menu_r_shortcut_restarts_only_inside_menu() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.snapshot.containers = vec![container_row(
            "abcdef123456",
            "app",
            "Up 2 minutes",
            Some(1.0),
            Some(2.0),
        )];

        app.open_container_menu();
        let plain = a3s_tui::style::strip_ansi(&app.container_menu_view());
        assert!(plain.contains("r  Restart container"));

        app.handle_key(KeyEvent {
            code: KeyCode::Char('r'),
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.container_menu.is_none());
        assert!(!app.reverse_sort);
        assert!(matches!(
            app.confirm,
            Some(Action::RestartContainer(_, _, ref name)) if name.contains("app")
        ));
    }

    #[test]
    fn container_menu_view_uses_shared_menu_panel_and_fits_width() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 42;
        app.height = 12;
        app.snapshot.containers = vec![container_row(
            "abcdef1234567890",
            "very-long-container-name",
            "Up 2 minutes",
            Some(1.0),
            Some(2.0),
        )];

        app.open_container_menu();
        let rendered = app.container_menu_view();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("container menu"), "{plain}");
        assert!(plain.contains("image"), "{plain}");
        assert!(plain.contains("status"), "{plain}");
        assert!(plain.contains("r  Restart container"), "{plain}");
        assert!(rendered.contains("\x1b["), "selected row should be styled");
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 42),
            "{plain}"
        );
    }

    #[test]
    fn container_menu_shortcuts_follow_available_actions() {
        let running = container_row("abcdef", "app", "Up 2 minutes", Some(1.0), Some(2.0));
        let mut web = running.clone();
        web.ports = "8080:80".into();
        let stopped = container_row("123456", "job", "Exited (0) 1 hour ago", None, None);
        let paused = container_row(
            "999999",
            "db",
            "Up 4 minutes (Paused)",
            Some(0.0),
            Some(1.0),
        );

        let running_keys = container_menu_items(&running)
            .into_iter()
            .map(|item| (item.key, item.action))
            .collect::<Vec<_>>();
        let web_keys = container_menu_items(&web)
            .into_iter()
            .map(|item| (item.key, item.action))
            .collect::<Vec<_>>();
        let stopped_keys = container_menu_items(&stopped)
            .into_iter()
            .map(|item| (item.key, item.action))
            .collect::<Vec<_>>();
        let paused_keys = container_menu_items(&paused)
            .into_iter()
            .map(|item| (item.key, item.action))
            .collect::<Vec<_>>();

        assert!(running_keys.contains(&('o', ContainerMenuAction::Focus)));
        assert!(running_keys.contains(&('l', ContainerMenuAction::Logs)));
        assert!(running_keys.contains(&('e', ContainerMenuAction::ExecShell)));
        assert!(!running_keys.contains(&('w', ContainerMenuAction::OpenBrowser)));
        assert!(web_keys.contains(&('w', ContainerMenuAction::OpenBrowser)));
        assert!(running_keys.contains(&('p', ContainerMenuAction::Pause)));
        assert!(running_keys.contains(&('s', ContainerMenuAction::Stop)));
        assert!(running_keys.contains(&('r', ContainerMenuAction::Restart)));
        assert!(stopped_keys.contains(&('s', ContainerMenuAction::Start)));
        assert!(stopped_keys.contains(&('d', ContainerMenuAction::Remove)));
        assert!(paused_keys.contains(&('u', ContainerMenuAction::Unpause)));
        assert!(paused_keys.contains(&('s', ContainerMenuAction::Stop)));
    }

    #[test]
    fn focused_container_renders_single_view() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.height = 42;
        app.width = 96;
        let mut row = container_row("abcdef", "app", "Up 2 minutes", Some(42.0), Some(30.0));
        row.inspect = ContainerInspect {
            health: "healthy".into(),
            restarts: "2".into(),
            restart_policy: "unless-stopped".into(),
            created: "2026-06-26 08:00:00".into(),
            started: "2026-06-26 08:01:00".into(),
            exit: "-".into(),
            mounts: "1 mount: /data(rw)".into(),
            env: "4 vars".into(),
            labels: "1 label: com.example.role".into(),
            networks: "1 net: bridge 172.17.0.2".into(),
        };
        row.cpu_count = Some(2);
        row.net_io = "1.00KiB / 2.00KiB".into();
        row.block_io = "4.00KiB / 8.00KiB".into();
        row.pids = "7".into();
        row.ports = "0.0.0.0:3000->3000/tcp".into();
        app.snapshot.containers = vec![row];
        app.focused_container = Some("abcdef".into());

        let rendered = app.table();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("single view app"));
        assert!(plain.contains("status Up 2 minutes"));
        assert!(plain.contains("health healthy"));
        assert!(plain.contains("image"));
        assert!(plain.contains("CPU"));
        assert!(plain.contains("MEM"));
        assert!(plain.contains("NET trend"));
        assert!(plain.contains("IO trend"));
        assert!(plain.contains("RESOURCE"));
        assert!(plain.contains("NET I/O"));
        assert!(plain.contains("CPUS"));
        assert!(plain.contains("PIDS"));
        assert!(plain.contains("0.0.0.0:3000->3000/tcp"));
        assert!(plain.contains("HEALTH"));
        assert!(plain.contains("healthy"));
        assert!(plain.contains("RESTART POLICY"));
        assert!(plain.contains("unless-stopped"));
        assert!(!plain.contains("CONTAINER"));
        assert!(
            rendered.contains("\x1b["),
            "container focus should be styled"
        );
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 96),
            "{plain}"
        );
    }

    #[test]
    fn focused_container_footer_surfaces_ctop_actions() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.focused_container = Some("abcdef".into());

        assert_eq!(
            app.footer_help_text(),
            "Esc list · ↑/↓ proc · Enter actions(start/stop/restart/pause) · l logs · e shell · w browser · K stop"
        );

        app.container_menu = Some(ContainerMenu {
            container: container_row("abcdef", "app", "Up 2 minutes", Some(1.0), Some(2.0)),
            items: vec![ContainerMenuItem {
                action: ContainerMenuAction::Restart,
                key: 'r',
                label: "Restart container".into(),
            }],
            select: Select::new(vec!["Restart container"]),
        });
        assert_eq!(
            app.footer_help_text(),
            "Enter run action · ↑/↓ select · Esc close menu"
        );
    }

    #[test]
    fn single_container_target_matches_name_cid_short_cid_and_prefix() {
        let row = container_row(
            "abcdef1234567890",
            "web",
            "Up 2 minutes",
            Some(1.0),
            Some(2.0),
        );

        assert!(container_matches_query(&row, "web"));
        assert!(container_matches_query(&row, "abcdef1234567890"));
        assert!(container_matches_query(&row, "abcdef123456"));
        assert!(container_matches_query(&row, "abcdef"));
        assert!(!container_matches_query(&row, "api"));

        let mut app = TopApp::new(TopOptions {
            container_query: Some("web".into()),
            ..TopOptions::default()
        });
        app.snapshot.containers = vec![
            row.clone(),
            container_row(
                "1111111234567890",
                "api",
                "Up 1 minute",
                Some(1.0),
                Some(2.0),
            ),
        ];

        let rows = app.filtered_containers();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].name, "web");
        assert_eq!(top_snapshot_json(&app, 0)["config"]["container"], "web");
    }

    #[test]
    fn single_container_target_collects_name_and_id_candidates() {
        assert_eq!(container_target_filters(None), vec![None]);
        assert_eq!(container_target_filters(Some("  ")), vec![None]);
        assert_eq!(
            container_target_filters(Some("web")),
            vec![Some("name=web".into()), Some("id=web".into())]
        );
    }

    #[test]
    fn dedupes_container_candidates_from_runtime_filters() {
        let mut rows = vec![
            container_row(
                "abcdef1234567890",
                "web",
                "Up 2 minutes",
                Some(1.0),
                Some(2.0),
            ),
            container_row(
                "abcdef1234567890",
                "web",
                "Up 2 minutes",
                Some(1.0),
                Some(2.0),
            ),
            container_row(
                "1111111234567890",
                "api",
                "Up 1 minute",
                Some(1.0),
                Some(2.0),
            ),
        ];

        dedupe_container_rows(&mut rows);

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].name, "web");
        assert_eq!(rows[1].name, "api");
    }

    #[test]
    fn containers_table_renders_and_filters_ports() {
        let mut row = container_row("abcdef", "app", "Up 2 minutes", Some(1.0), Some(2.0));
        row.ports = "0.0.0.0:3000->3000/tcp".into();
        row.inspect.health = "healthy".into();
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            config: all_columns_config(),
            ..TopOptions::default()
        });
        app.width = 260;
        app.snapshot.containers = vec![row];

        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("PORTS"));
        assert!(plain.contains("HEALTH"));
        assert!(plain.contains("CID"));
        assert!(plain.contains("0.0.0.0:3000->3000/tcp"));
        assert!(plain.contains("healthy"));

        app.filter = "3000".into();
        assert_eq!(app.filtered_containers().len(), 1);
        app.filter = "healthy".into();
        assert_eq!(app.filtered_containers().len(), 1);
        app.filter = "abcdef".into();
        assert_eq!(app.filtered_containers().len(), 1);
    }

    #[test]
    fn focused_container_renders_process_table() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 160;
        app.height = 32;
        app.snapshot.containers = vec![container_row(
            "abcdef",
            "app",
            "Up 2 minutes",
            Some(42.0),
            Some(30.0),
        )];
        app.focused_container = Some("abcdef".into());
        app.container_processes = Some(ContainerProcessPanel {
            container_id: "abcdef".into(),
            container_name: "app".into(),
            rows: vec![ContainerProcessRow {
                pid: "123".into(),
                ppid: "1".into(),
                cpu_pct: Some(2.5),
                mem_pct: Some(0.7),
                elapsed: "00:01".into(),
                command: "node server.js".into(),
            }],
            scroll: 0,
            error: None,
            loading: false,
        });

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("PID"));
        assert!(plain.contains("PPID"));
        assert!(plain.contains("node server.js"));
        assert!(plain.contains("2.5"));
    }

    #[test]
    fn focused_container_scrolls_process_table() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.width = 160;
        app.height = 32;
        app.snapshot.containers = vec![container_row(
            "abcdef",
            "app",
            "Up 2 minutes",
            Some(42.0),
            Some(30.0),
        )];
        app.focused_container = Some("abcdef".into());
        let rows = (0..30)
            .map(|idx| ContainerProcessRow {
                pid: format!("{}", 100 + idx),
                ppid: "1".into(),
                cpu_pct: Some(idx as f32),
                mem_pct: Some(0.5),
                elapsed: "00:01".into(),
                command: format!("worker-{idx:02}"),
            })
            .collect::<Vec<_>>();
        app.container_processes = Some(ContainerProcessPanel {
            container_id: "abcdef".into(),
            container_name: "app".into(),
            rows,
            scroll: 0,
            error: None,
            loading: false,
        });

        app.handle_key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::empty(),
        });
        assert_eq!(app.container_processes.as_ref().unwrap().scroll, 1);

        app.handle_key(KeyEvent {
            code: KeyCode::PageDown,
            modifiers: KeyModifiers::empty(),
        });
        assert!(app.container_processes.as_ref().unwrap().scroll > 1);

        app.handle_key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::empty(),
        });
        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("worker-29"));
        assert!(!plain.contains("worker-00"));

        app.handle_key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::empty(),
        });
        let plain = a3s_tui::style::strip_ansi(&app.table());
        assert!(plain.contains("worker-00"));
    }

    #[test]
    fn log_scroll_controls_follow_mode() {
        let mut app = TopApp::new(TopOptions::default());
        app.height = 10;
        app.log = Some(LogPanel {
            connector: ContainerConnector::Docker,
            container_id: "abcdef".into(),
            container_name: "app".into(),
            text: numbered_lines(10),
            scroll: 7,
            timestamps: false,
            loading: false,
            refreshing: false,
            follow: true,
        });

        app.handle_key(KeyEvent {
            code: KeyCode::Up,
            modifiers: KeyModifiers::empty(),
        });
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.scroll, 6);
        assert!(!log.follow);

        app.handle_key(KeyEvent {
            code: KeyCode::Down,
            modifiers: KeyModifiers::empty(),
        });
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.scroll, 7);
        assert!(log.follow);

        app.handle_key(KeyEvent {
            code: KeyCode::Home,
            modifiers: KeyModifiers::empty(),
        });
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.scroll, 0);
        assert!(!log.follow);

        app.handle_key(KeyEvent {
            code: KeyCode::End,
            modifiers: KeyModifiers::empty(),
        });
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.scroll, 7);
        assert!(log.follow);
    }

    #[test]
    fn log_follow_key_toggles_tail_mode() {
        let mut app = TopApp::new(TopOptions::default());
        app.height = 10;
        app.log = Some(LogPanel {
            connector: ContainerConnector::Docker,
            container_id: "abcdef".into(),
            container_name: "app".into(),
            text: numbered_lines(10),
            scroll: 2,
            timestamps: false,
            loading: false,
            refreshing: false,
            follow: false,
        });

        app.handle_key(KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::empty(),
        });
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.scroll, 7);
        assert!(log.follow);

        app.handle_key(KeyEvent {
            code: KeyCode::Char('f'),
            modifiers: KeyModifiers::empty(),
        });
        assert!(!app.log.as_ref().unwrap().follow);
    }

    #[test]
    fn log_refresh_command_marks_panel_refreshing() {
        let mut app = TopApp::new(TopOptions::default());
        app.log = Some(LogPanel {
            connector: ContainerConnector::Docker,
            container_id: "abcdef".into(),
            container_name: "app".into(),
            text: "line".into(),
            scroll: 0,
            timestamps: false,
            loading: false,
            refreshing: false,
            follow: true,
        });

        assert!(app.open_log_refresh_cmd().is_some());
        assert!(app.log.as_ref().unwrap().refreshing);
        assert!(app.open_log_refresh_cmd().is_none());
    }

    #[test]
    fn log_refresh_key_marks_panel_refreshing() {
        let mut app = TopApp::new(TopOptions::default());
        app.log = Some(LogPanel {
            connector: ContainerConnector::Docker,
            container_id: "abcdef".into(),
            container_name: "app".into(),
            text: "line".into(),
            scroll: 0,
            timestamps: false,
            loading: false,
            refreshing: false,
            follow: true,
        });

        assert!(app
            .handle_key(KeyEvent {
                code: KeyCode::Char('r'),
                modifiers: KeyModifiers::empty(),
            })
            .is_some());
        assert!(app.log.as_ref().unwrap().refreshing);
    }

    #[test]
    fn logs_view_uses_shared_log_view_and_fits_width() {
        let mut app = TopApp::new(TopOptions::default());
        app.width = 64;
        app.height = 10;
        app.log = Some(LogPanel {
            connector: ContainerConnector::Docker,
            container_id: "abcdef".into(),
            container_name: "app".into(),
            text: numbered_lines(6),
            scroll: 2,
            timestamps: true,
            loading: false,
            refreshing: false,
            follow: false,
        });

        let rendered = app.logs_view();
        let plain = a3s_tui::style::strip_ansi(&rendered);
        assert!(plain.contains("logs app (abcdef)"), "{plain}");
        assert!(plain.contains("tail 200"), "{plain}");
        assert!(plain.contains("timestamps:on"), "{plain}");
        assert!(plain.contains("follow:off"), "{plain}");
        assert!(plain.contains("line-2"), "{plain}");
        assert!(!plain.contains("line-0"), "{plain}");
        assert!(rendered.contains("\x1b["), "log view should be styled");
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 64),
            "{plain}"
        );

        app.log.as_mut().unwrap().loading = true;
        let loading = a3s_tui::style::strip_ansi(&app.logs_view());
        assert!(loading.contains("loading container logs"), "{loading}");
        assert!(!loading.contains("line-2"), "{loading}");
    }

    #[test]
    fn log_results_follow_or_preserve_scroll() {
        let mut app = TopApp::new(TopOptions::default());
        app.height = 10;
        app.log = Some(LogPanel {
            connector: ContainerConnector::Docker,
            container_id: "abcdef".into(),
            container_name: "app".into(),
            text: numbered_lines(10),
            scroll: 7,
            timestamps: false,
            loading: false,
            refreshing: true,
            follow: true,
        });

        app.update(Msg::ContainerLogs {
            connector: ContainerConnector::Docker,
            id: "abcdef".into(),
            name: "app".into(),
            timestamps: false,
            result: Ok(numbered_lines(12)),
        });
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.scroll, 9);
        assert!(log.follow);
        assert!(!log.refreshing);

        app.log.as_mut().unwrap().follow = false;
        app.log.as_mut().unwrap().scroll = 4;
        app.update(Msg::ContainerLogs {
            connector: ContainerConnector::Docker,
            id: "abcdef".into(),
            name: "app".into(),
            timestamps: false,
            result: Ok(numbered_lines(12)),
        });
        let log = app.log.as_ref().unwrap();
        assert_eq!(log.scroll, 4);
        assert!(!log.follow);
    }

    #[test]
    fn focused_agent_renders_agent_view() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 96;
        app.snapshot.processes = vec![ProcessRow {
            cpu_pct: 33.0,
            mem_pct: 12.0,
            ..process_row(42, 1, "codex exec task")
        }];
        app.snapshot.events = vec![EventRow {
            ts: "recent".into(),
            source: "codex".into(),
            session: Some("sess-a".into()),
            task: Some("task-a".into()),
            pid: Some(42),
            ppid: None,
            kind: "ToolExec".into(),
            message: "git status".into(),
            details: Vec::new(),
            risk: Risk::Medium,
        }];
        app.focused_agent_pid = Some(42);

        let rendered = app.table();
        let plain = a3s_tui::style::strip_ansi(&rendered);

        assert!(plain.contains("agent view codex"));
        assert!(plain.contains("pid 42"));
        assert!(plain.contains("ppid 1"));
        assert!(plain.contains("activity events 1"));
        assert!(plain.contains("TIME"));
        assert!(plain.contains("KIND"));
        assert!(plain.contains("PID"));
        assert!(plain.contains("ToolExec"));
        assert!(plain.contains("SESSION"));
        assert!(plain.contains("sess-a"));
        assert!(plain.contains("task-a"));
        assert!(plain.contains("git status"));
        assert!(plain.contains("command codex exec task"));
        assert!(!plain.contains("AGENT"));
        assert!(rendered.contains("\x1b["), "agent focus should be styled");
        assert!(
            rendered
                .lines()
                .all(|line| a3s_tui::style::visible_len(line) <= 96),
            "{plain}"
        );
    }

    #[test]
    fn focused_agent_renders_child_process_tree() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.width = 120;
        app.height = 32;
        app.snapshot.processes = vec![
            ProcessRow {
                cpu_pct: 33.0,
                mem_pct: 12.0,
                ..process_row(42, 1, "codex exec task")
            },
            ProcessRow {
                cpu_pct: 4.0,
                mem_pct: 1.5,
                ..process_row(100, 42, "bash -lc cargo test")
            },
            ProcessRow {
                cpu_pct: 2.0,
                mem_pct: 0.5,
                ..process_row(101, 100, "git status --short")
            },
        ];
        app.focused_agent_pid = Some(42);

        let plain = a3s_tui::style::strip_ansi(&app.table());

        assert!(plain.contains("subtree cpu 39.0% mem 14.0%"));
        assert!(plain.contains("42  cpu 33.0% mem 12.0%  codex exec task"));
        assert!(plain.contains("100  cpu 4.0% mem 1.5%  bash -lc cargo test"));
        assert!(plain.contains("101  cpu 2.0% mem 0.5%  git status --short"));
        assert!(plain.contains("└── 100"));
    }

    #[test]
    fn o_focuses_selected_agent() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.snapshot.processes = vec![process_row(42, 1, "codex exec task")];

        app.handle_key(KeyEvent {
            code: KeyCode::Char('o'),
            modifiers: KeyModifiers::empty(),
        });

        assert_eq!(app.focused_agent_pid, Some(42));
        assert_eq!(app.tab, Tab::Agents);
    }

    #[test]
    fn esc_closes_container_focus_before_detail() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.focused_container = Some("abcdef".into());
        app.detail = true;

        app.handle_key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.focused_container.is_none());
    }

    #[test]
    fn esc_closes_agent_focus_before_detail() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Agents,
            ..TopOptions::default()
        });
        app.focused_agent_pid = Some(42);
        app.detail = true;

        app.handle_key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.focused_agent_pid.is_none());
    }

    #[test]
    fn esc_closes_session_focus_before_detail() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Sessions,
            ..TopOptions::default()
        });
        app.focused_session = Some(SessionFocus {
            source: "codex".into(),
            session: "sess-a".into(),
        });
        app.detail = true;

        app.handle_key(KeyEvent {
            code: KeyCode::Esc,
            modifiers: KeyModifiers::empty(),
        });

        assert!(app.focused_session.is_none());
    }

    #[test]
    fn sorts_containers_by_memory_percent() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        app.sort_by = SortBy::Mem;
        app.snapshot.containers = vec![
            container_row("low", "low", "Up", Some(1.0), Some(5.0)),
            container_row("high", "high", "Up", Some(1.0), Some(42.0)),
        ];

        let rows = app.filtered_containers();

        assert_eq!(rows[0].name, "high");
    }

    #[test]
    fn sorts_containers_by_ctop_state_id_and_uptime_fields() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        let exited = container_row("ccc333", "exited", "Exited (0) 1 hour ago", None, None);
        let running_short = container_row("aaa111", "short", "Up 2 minutes", Some(1.0), Some(1.0));
        let running_long = container_row("bbb222", "long", "Up 3 hours", Some(1.0), Some(1.0));

        app.snapshot.containers = vec![exited.clone(), running_short.clone(), running_long.clone()];
        app.sort_by = SortBy::State;
        assert_eq!(app.filtered_containers()[0].name, "long");

        app.snapshot.containers = vec![running_long.clone(), exited.clone(), running_short.clone()];
        app.sort_by = SortBy::Id;
        assert_eq!(app.filtered_containers()[0].id, "aaa111");

        app.snapshot.containers = vec![running_short, exited, running_long];
        app.sort_by = SortBy::Uptime;
        assert_eq!(app.filtered_containers()[0].name, "long");
        assert_eq!(parse_uptime_seconds("Up 2 minutes"), Some(120));
        assert_eq!(container_state_label("Up 4 minutes (Paused)"), "paused");
    }

    #[test]
    fn container_state_colors_match_runtime_state() {
        assert_eq!(container_state_color("running"), GREEN);
        assert_eq!(container_state_color("Up 4 minutes"), GREEN);
        assert_eq!(container_state_color("restarting"), ORANGE);
        assert_eq!(container_state_color("paused"), YELLOW);
        assert_eq!(
            container_state_color("Exited (0) 1 hour ago"),
            Color::BrightBlack
        );
        assert_eq!(container_state_color("created"), CYAN);
        assert_eq!(container_state_color("dead"), RED);
    }

    #[test]
    fn summarizes_container_states() {
        let rows = vec![
            container_row("run", "run", "Up 2 minutes", Some(1.0), Some(1.0)),
            container_row("restart", "restart", "Restarting (1)", None, None),
            container_row("pause", "pause", "paused", None, None),
            container_row("exit", "exit", "stopped", None, None),
            container_row("create", "create", "created", None, None),
            container_row("dead", "dead", "dead", None, None),
            container_row("other", "other", "unknown", None, None),
        ];

        let summary = container_state_summary(&rows);

        assert_eq!(summary.total, 7);
        assert_eq!(summary.running, 1);
        assert_eq!(summary.restarting, 1);
        assert_eq!(summary.paused, 1);
        assert_eq!(summary.exited, 1);
        assert_eq!(summary.created, 1);
        assert_eq!(summary.dead, 1);
        assert_eq!(summary.other, 1);
        assert_eq!(
            summary.header_label(),
            "7 run:1 restart:1 pause:1 exit:1 create:1 dead:1 other:1"
        );
    }

    #[test]
    fn parses_human_byte_pairs_for_container_sorting() {
        assert_eq!(parse_human_bytes("1.5kB"), Some(1500));
        assert_eq!(parse_human_bytes("1.00KiB"), Some(1024));
        assert_eq!(parse_byte_pair_total("1.00KiB / 2.00KiB"), 3072);
        assert_eq!(parse_byte_pair_total("-"), 0);
    }

    #[test]
    fn parses_container_process_tables() {
        let rows = parse_container_process_table(
            "PID PPID %CPU %MEM ELAPSED COMMAND\n123 1 2.5 0.7 00:01 node server.js\n",
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pid, "123");
        assert_eq!(rows[0].ppid, "1");
        assert_eq!(rows[0].cpu_pct, Some(2.5));
        assert_eq!(rows[0].mem_pct, Some(0.7));
        assert_eq!(rows[0].elapsed, "00:01");
        assert_eq!(rows[0].command, "node server.js");

        let rows = parse_container_process_table(
            "UID PID PPID C STIME TTY TIME CMD\nroot 321 1 0 10:00 ? 00:00:01 sleep infinity\n",
        );
        assert_eq!(rows[0].pid, "321");
        assert_eq!(rows[0].command, "sleep infinity");
    }

    #[test]
    fn parses_runc_process_ids() {
        let rows = parse_runc_processes("PID\n123\n456\n");

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pid, "123");
        assert_eq!(rows[1].command, "runc process");
    }

    #[test]
    fn sorts_containers_by_network_block_and_pids() {
        let mut app = TopApp::new(TopOptions {
            tab: Tab::Containers,
            ..TopOptions::default()
        });
        let mut low = container_row("low", "low", "Up", Some(1.0), Some(1.0));
        low.net_io = "1kB / 1kB".into();
        low.block_io = "5kB / 1kB".into();
        low.pids = "2".into();
        let mut high = container_row("high", "high", "Up", Some(1.0), Some(1.0));
        high.net_io = "10kB / 2kB".into();
        high.block_io = "1kB / 1kB".into();
        high.pids = "9".into();
        app.snapshot.containers = vec![low.clone(), high.clone()];

        app.sort_by = SortBy::Net;
        assert_eq!(app.filtered_containers()[0].name, "high");

        app.snapshot.containers = vec![low.clone(), high.clone()];
        app.sort_by = SortBy::Block;
        assert_eq!(app.filtered_containers()[0].name, "low");

        app.snapshot.containers = vec![low, high];
        app.sort_by = SortBy::Pids;
        assert_eq!(app.filtered_containers()[0].name, "high");
    }

    #[test]
    fn maps_container_menu_actions_to_confirmable_actions() {
        let container = container_row("abcdef", "app", "Up", Some(1.0), Some(2.0));

        assert!(matches!(
            container_action(container.clone(), ContainerMenuAction::Pause),
            Action::PauseContainer(ContainerConnector::Docker, _, name) if name.contains("app")
        ));
        assert!(matches!(
            container_action(container, ContainerMenuAction::Remove),
            Action::RemoveContainer(ContainerConnector::Docker, _, name) if name.contains("app")
        ));
    }

    #[test]
    fn parses_runc_list_json() {
        let rows = parse_runc_list(
            r#"[
                {
                    "id": "example",
                    "pid": 1234,
                    "status": "running",
                    "bundle": "/containers/example",
                    "rootfs": "/containers/example/rootfs",
                    "created": "2026-06-26T00:00:00Z"
                }
            ]"#,
        )
        .unwrap();

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].connector, ContainerConnector::RunC);
        assert_eq!(rows[0].name, "example");
        assert_eq!(rows[0].status, "running");
        assert_eq!(rows[0].pids, "1234");
        assert!(rows[0].image.contains("rootfs:"));
    }

    #[test]
    fn runc_global_args_follow_ctop_env_vars() {
        let old_root = std::env::var_os("RUNC_ROOT");
        let old_systemd = std::env::var_os("RUNC_SYSTEMD_CGROUP");

        std::env::remove_var("RUNC_ROOT");
        std::env::remove_var("RUNC_SYSTEMD_CGROUP");
        assert_eq!(runc_global_args(), vec!["--root", "/run/runc"]);

        std::env::set_var("RUNC_ROOT", "/tmp/custom-runc");
        assert_eq!(runc_global_args(), vec!["--root", "/tmp/custom-runc"]);

        std::env::set_var("RUNC_SYSTEMD_CGROUP", "true");
        assert_eq!(
            runc_global_args(),
            vec!["--root", "/tmp/custom-runc", "--systemd-cgroup"]
        );

        std::env::set_var("RUNC_SYSTEMD_CGROUP", "0");
        assert_eq!(runc_global_args(), vec!["--root", "/tmp/custom-runc"]);

        restore_var("RUNC_ROOT", old_root);
        restore_var("RUNC_SYSTEMD_CGROUP", old_systemd);
    }

    #[test]
    fn runc_container_filters_match_active_all_and_single_target() {
        let mut active_rows = vec![
            container_row("abcdef1234567890", "api", "running", None, None),
            container_row("1111111234567890", "paused-box", "paused", None, None),
            container_row("2222221234567890", "stopped-box", "stopped", None, None),
            container_row("3333331234567890", "created-box", "created", None, None),
        ];
        filter_runc_container_rows(&mut active_rows, false, None);
        let names = active_rows
            .iter()
            .map(|row| row.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["api", "paused-box"]);

        let mut all_target_rows = vec![
            container_row("abcdef1234567890", "api", "running", None, None),
            container_row("2222221234567890", "stopped-box", "stopped", None, None),
        ];
        filter_runc_container_rows(&mut all_target_rows, true, Some("222222"));
        assert_eq!(all_target_rows.len(), 1);
        assert_eq!(all_target_rows[0].name, "stopped-box");

        let mut active_target_rows = vec![
            container_row("abcdef1234567890", "api", "running", None, None),
            container_row("2222221234567890", "stopped-box", "stopped", None, None),
        ];
        filter_runc_container_rows(&mut active_target_rows, false, Some("stopped-box"));
        assert!(active_target_rows.is_empty());

        let mut name_target_rows = vec![
            container_row("abcdef1234567890", "api", "running", None, None),
            container_row("1111111234567890", "worker", "running", None, None),
        ];
        filter_runc_container_rows(&mut name_target_rows, false, Some("api"));
        assert_eq!(name_target_rows.len(), 1);
        assert_eq!(name_target_rows[0].id, "abcdef1234567890");
    }

    #[test]
    fn parses_docker_container_ports() {
        let rows = parse_docker_container_list(
            "abcdef\tapp\tnginx:latest\tUp 2 minutes\t0.0.0.0:8080->80/tcp, :::8080->80/tcp\n123456\tworker\talpine\tUp 1 second\t\n",
        );

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].ports, "0.0.0.0:8080->80/tcp, :::8080->80/tcp");
        assert_eq!(rows[1].ports, "-");
    }

    #[test]
    fn derives_browser_url_from_a3s_box_and_docker_ports() {
        let mut row = container_row("abcdef", "app", "running", Some(1.0), Some(2.0));

        row.ports = "8080:80".into();
        assert_eq!(
            container_web_url(&row).as_deref(),
            Some("http://localhost:8080/")
        );

        row.ports = "127.0.0.1:3000:80".into();
        assert_eq!(
            container_web_url(&row).as_deref(),
            Some("http://localhost:3000/")
        );

        row.ports = "80/tcp, 0.0.0.0:9090->90/tcp".into();
        assert_eq!(
            container_web_url(&row).as_deref(),
            Some("http://localhost:9090/")
        );

        row.ports = ":::8443->443/tcp".into();
        assert_eq!(
            container_web_url(&row).as_deref(),
            Some("http://localhost:8443/")
        );

        row.ports = "0:80".into();
        assert_eq!(container_web_url(&row), None);
    }

    #[test]
    fn parses_a3s_box_ps_and_stats_output() {
        let rows = parse_a3s_box_ps(
            "abc123\tdev\talpine:latest\trunning\t8080:80\t2 minutes ago\tsleep 3600\n",
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].connector, ContainerConnector::A3sBox);
        assert_eq!(rows[0].id, "abc123");
        assert_eq!(rows[0].name, "dev");
        assert_eq!(rows[0].status, "running");
        assert_eq!(rows[0].ports, "8080:80");
        assert_eq!(rows[0].inspect.created, "2 minutes ago");
        assert_eq!(rows[0].inspect.started, "2 minutes ago");
        assert!(rows[0].inspect.labels.contains("sleep 3600"));

        let json_rows = parse_a3s_box_ps_json(
            r#"[
                {
                    "id": "abcdef1234567890",
                    "short_id": "abcdef123456",
                    "name": "dev",
                    "image": "alpine:latest",
                    "status": "running (healthy)",
                    "raw_status": "running",
                    "created": "2 minutes ago",
                    "created_at": "2026-06-26T08:00:00Z",
                    "started_at": "2026-06-26T08:01:00Z",
                    "ports": ["8080:80"],
                    "command": "sleep 3600",
                    "labels": {"role": "api"},
                    "health": "healthy",
                    "pid": 4242
                }
            ]"#,
        );
        assert_eq!(json_rows.len(), 1);
        assert_eq!(json_rows[0].id, "abcdef1234567890");
        assert_eq!(json_rows[0].name, "dev");
        assert_eq!(json_rows[0].status, "running");
        assert_eq!(json_rows[0].ports, "8080:80");
        assert_eq!(json_rows[0].pids, "-");
        assert_eq!(json_rows[0].inspect.health, "healthy");
        assert_eq!(json_rows[0].inspect.created, "2 minutes ago");
        assert_eq!(json_rows[0].inspect.started, "2026-06-26 08:01:00");
        assert!(json_rows[0].inspect.labels.contains("sleep 3600"));
        assert!(json_rows[0].inspect.labels.contains("1 label"));

        let stats = parse_a3s_box_stats(
            "BOX ID NAME STATUS CPU % MEM USAGE / LIMIT MEM % PID NET I/O IO\nabc123 dev running 12.50% 64.0 MB / 512.0 MB 12.5% 4242 1.0 KB / 2.0 KB 4.0 KB / 8.0 KB\n",
        );
        let by_id = stats.get("abc123").unwrap();
        assert_eq!(by_id.cpu_pct, Some(12.5));
        assert_eq!(by_id.mem_pct, Some(12.5));
        assert_eq!(by_id.mem_usage, "64.0 MB / 512.0 MB");
        assert_eq!(by_id.net_io, "1.0 KB / 2.0 KB");
        assert_eq!(by_id.block_io, "4.0 KB / 8.0 KB");
        assert_eq!(by_id.pid.as_deref(), Some("4242"));
        assert!(stats.contains_key("dev"));

        let legacy = parse_a3s_box_stats(
            "BOX ID NAME STATUS CPU % MEM USAGE / LIMIT MEM % PID\nabc123 dev running 12.50% 64.0 MB / 512.0 MB 12.5% 4242\n",
        );
        assert_eq!(legacy.get("abc123").unwrap().net_io, "-");
        assert_eq!(legacy.get("abc123").unwrap().block_io, "-");

        let legacy_io = parse_a3s_box_stats(
            "BOX ID NAME STATUS CPU % MEM USAGE / LIMIT MEM % PID IO\nabc123 dev running 12.50% 64.0 MB / 512.0 MB 12.5% 4242 4.0 KB / 8.0 KB\n",
        );
        assert_eq!(legacy_io.get("abc123").unwrap().net_io, "-");
        assert_eq!(legacy_io.get("abc123").unwrap().block_io, "4.0 KB / 8.0 KB");

        let json_stats = parse_a3s_box_stats_json(
            r#"[
                {
                    "id": "abc123",
                    "short_id": "abc123",
                    "name": "dev",
                    "status": "running",
                    "pid": 4242,
                    "cpus": 2,
                    "cpu_percent": 12.5,
                    "memory_bytes": 67108864,
                    "memory_limit_bytes": 536870912,
                    "memory_percent": 12.5,
                    "network_rx_bytes": 1024,
                    "network_tx_bytes": 2048,
                    "block_read_bytes": 4096,
                    "block_write_bytes": 8192,
                    "pids_current": 7
                }
            ]"#,
        );
        let by_id = json_stats.get("abc123").unwrap();
        assert_eq!(by_id.cpu_pct, Some(12.5));
        assert_eq!(by_id.cpu_count, Some(2));
        assert_eq!(by_id.mem_pct, Some(12.5));
        assert_eq!(by_id.mem_usage, "64.0 MB / 512.0 MB");
        assert_eq!(by_id.net_io, "1.0 KB / 2.0 KB");
        assert_eq!(by_id.block_io, "4.0 KB / 8.0 KB");
        assert_eq!(by_id.pid.as_deref(), Some("4242"));
        assert_eq!(by_id.pids_current, Some(7));
        assert!(json_stats.contains_key("dev"));
    }

    #[cfg(unix)]
    #[test]
    fn command_output_error_prefers_stderr_for_visible_collector_errors() {
        use std::os::unix::process::ExitStatusExt;

        let output = std::process::Output {
            status: std::process::ExitStatus::from_raw(1 << 8),
            stdout: b"stdout fallback".to_vec(),
            stderr: b"box runtime unavailable\n".to_vec(),
        };

        let message = command_output_error("a3s-box ps", &output);

        assert!(message.contains("a3s-box ps exited with status"));
        assert!(message.contains("box runtime unavailable"));
        assert!(!message.contains("stdout fallback"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn a3s_box_ps_rows_surfaces_command_failures() {
        use std::os::unix::fs::PermissionsExt;

        let root =
            std::env::temp_dir().join(format!("a3s-top-box-ps-fail-test-{}", std::process::id()));
        let binary = root.join("a3s-box");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            &binary,
            b"#!/bin/sh\necho 'box ps failed visibly' >&2\nexit 42\n",
        )
        .unwrap();
        std::fs::set_permissions(&binary, std::fs::Permissions::from_mode(0o755)).unwrap();

        let err = a3s_box_ps_rows(&binary, false, None)
            .await
            .unwrap_err()
            .to_string();

        let _ = std::fs::remove_dir_all(root);
        assert!(err.contains("a3s-box ps --format json exited with status"));
        assert!(err.contains("box ps failed visibly"));
        assert!(err.contains("fallback table failed"));
    }

    #[test]
    fn counts_a3s_box_processes_without_probe_process() {
        let rows = parse_container_process_table(
            "PID PPID %CPU %MEM ELAPSED COMMAND\n1 0 0.0 0.1 02:00 /sbin/init\n42 1 1.5 0.3 00:01 node server.js\n99 1 0.0 0.0 00:00 ps -eo pid,ppid,pcpu,pmem,etime,args\n",
        );

        assert_eq!(a3s_box_process_count_from_rows(&rows), 2);

        let filtered = filter_a3s_box_probe_processes(rows);
        assert_eq!(filtered.len(), 2);
        assert!(filtered
            .iter()
            .all(|row| row.command != "ps -eo pid,ppid,pcpu,pmem,etime,args"));
    }

    #[test]
    fn parses_a3s_box_top_json_output() {
        let rows = parse_a3s_box_top_json(
            r#"[
                {
                    "pid": "1",
                    "ppid": "0",
                    "cpu_percent": 0.0,
                    "memory_percent": 0.1,
                    "elapsed": "02:00",
                    "command": "/sbin/init"
                },
                {
                    "pid": 42,
                    "ppid": 1,
                    "cpu_percent": "1.5%",
                    "memory_percent": "0.3%",
                    "elapsed": "00:01",
                    "command": "node server.js"
                }
            ]"#,
        )
        .unwrap();

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].pid, "1");
        assert_eq!(rows[0].ppid, "0");
        assert_eq!(rows[0].cpu_pct, Some(0.0));
        assert_eq!(rows[0].mem_pct, Some(0.1));
        assert_eq!(rows[1].pid, "42");
        assert_eq!(rows[1].ppid, "1");
        assert_eq!(rows[1].cpu_pct, Some(1.5));
        assert_eq!(rows[1].mem_pct, Some(0.3));
        assert_eq!(rows[1].command, "node server.js");
    }

    #[test]
    fn parses_a3s_box_inspect_metadata() {
        let rows = parse_a3s_box_inspect(
            r#"[
                {
                    "id": "abcdef1234567890",
                    "name": "dev",
                    "image": "alpine:latest",
                    "status": "running",
                    "created_at": "2026-06-26T08:00:00Z",
                    "started_at": "2026-06-26T08:01:00Z",
                    "restart_policy": "on-failure",
                    "restart_count": 3,
                    "max_restart_count": 5,
                    "exit_code": 137,
                    "health_status": "healthy",
                    "volumes": ["/host:/guest"],
                    "volume_names": ["data"],
                    "env": {
                        "TOKEN": "secret",
                        "RUST_LOG": "debug"
                    },
                    "labels": {
                        "com.example.role": "api"
                    },
                    "network_name": "bridge",
                    "add_host": ["db:10.0.0.2"],
                    "status_detail": {
                        "health": "healthy",
                        "restart_count": 3
                    },
                    "State": {
                        "Status": "running",
                        "Running": true,
                        "Paused": false,
                        "ExitCode": 0
                    }
                }
            ]"#,
        );
        let inspect = rows.get("abcdef1234567890").unwrap();

        assert_eq!(rows.get("abcdef123456").unwrap(), inspect);
        assert_eq!(inspect.health, "healthy");
        assert_eq!(inspect.restarts, "3");
        assert_eq!(inspect.restart_policy, "on-failure:5");
        assert_eq!(inspect.created, "2026-06-26 08:00:00");
        assert_eq!(inspect.started, "2026-06-26 08:01:00");
        assert_eq!(inspect.exit, "-");
        assert_eq!(inspect.env, "2 vars");
        assert!(inspect.mounts.contains("/host:/guest"));
        assert!(inspect.mounts.contains("data"));
        assert!(inspect.labels.contains("com.example.role"));
        assert!(inspect.networks.contains("bridge"));
        assert!(inspect.networks.contains("db:10.0.0.2"));
        assert!(!inspect.env.contains("secret"));
    }

    #[test]
    fn parses_docker_inspect_metadata() {
        let rows = parse_docker_inspect(
            r#"[
                {
                    "Id": "abcdef1234567890",
                    "Created": "2026-06-26T08:00:00.123456789Z",
                    "RestartCount": 3,
                    "State": {
                        "Status": "running",
                        "StartedAt": "2026-06-26T08:01:00Z",
                        "FinishedAt": "0001-01-01T00:00:00Z",
                        "ExitCode": 0,
                        "OOMKilled": false,
                        "Dead": false,
                        "Health": {"Status": "healthy", "FailingStreak": 0}
                    },
                    "HostConfig": {
                        "RestartPolicy": {"Name": "on-failure", "MaximumRetryCount": 5}
                    },
                    "Config": {
                        "Env": ["TOKEN=secret", "RUST_LOG=debug"],
                        "Labels": {
                            "com.example.role": "api",
                            "maintainer": "a3s"
                        }
                    },
                    "Mounts": [
                        {"Type": "bind", "Source": "/host/data", "Destination": "/data", "RW": true},
                        {"Type": "volume", "Name": "cfg", "Destination": "/cfg", "RW": false}
                    ],
                    "NetworkSettings": {
                        "Networks": {
                            "bridge": {"IPAddress": "172.17.0.2"}
                        }
                    }
                }
            ]"#,
        );
        let inspect = rows.get("abcdef1234567890").unwrap();

        assert_eq!(rows.get("abcdef123456").unwrap(), inspect);
        assert_eq!(inspect.health, "healthy");
        assert_eq!(inspect.restarts, "3");
        assert_eq!(inspect.restart_policy, "on-failure:5");
        assert_eq!(inspect.created, "2026-06-26 08:00:00");
        assert_eq!(inspect.started, "2026-06-26 08:01:00");
        assert_eq!(inspect.exit, "-");
        assert_eq!(inspect.env, "2 vars");
        assert!(inspect.mounts.contains("/data(rw)"));
        assert!(inspect.mounts.contains("/cfg(ro)"));
        assert!(inspect.labels.contains("com.example.role"));
        assert!(inspect.networks.contains("bridge 172.17.0.2"));
        assert!(!inspect.env.contains("secret"));
    }

    #[test]
    fn parses_runc_stats_json() {
        let stats = parse_runc_stats_event(
            r#"{
                "type": "stats",
                "id": "example",
                "data": {
                    "cpu": {"usage": {"total": 2000000000}},
                    "memory": {"usage": {"usage": 1048576, "limit": 4194304}},
                    "pids": {"current": 7},
                    "network_interfaces": [
                        {"Name": "eth0", "RxBytes": 1024, "TxBytes": 2048},
                        {"Name": "lo", "RxBytes": 512, "TxBytes": 256}
                    ],
                    "blkio": {
                        "ioServiceBytesRecursive": [
                            {"op": "Read", "value": 4096},
                            {"op": "Write", "value": 8192}
                        ]
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(stats.cpu_usage_total_ns, Some(2_000_000_000));
        assert_eq!(stats.memory_usage, Some(1_048_576));
        assert_eq!(stats.memory_limit, Some(4_194_304));
        assert_eq!(stats.pids_current, Some(7));
        assert_eq!(stats.net_rx, 1536);
        assert_eq!(stats.net_tx, 2304);
        assert_eq!(stats.block_read, 4096);
        assert_eq!(stats.block_write, 8192);
    }

    #[test]
    fn applies_runc_stats_to_container_row() {
        let mut row = container_row("example", "example", "running", None, None);
        row.connector = ContainerConnector::RunC;

        apply_runc_stats(
            &mut row,
            &RuncStats {
                memory_usage: Some(1_048_576),
                memory_limit: Some(4_194_304),
                cpu_usage_total_ns: Some(2_000_000_000),
                pids_current: Some(7),
                net_rx: 1024,
                net_tx: 2048,
                block_read: 4096,
                block_write: 8192,
            },
        );

        assert_eq!(row.mem_pct, Some(25.0));
        assert_eq!(row.cpu_usage_total_ns, Some(2_000_000_000));
        assert_eq!(row.mem_usage, "1.00MiB / 4.00MiB");
        assert_eq!(row.net_io, "1.00KiB / 2.00KiB");
        assert_eq!(row.block_io, "4.00KiB / 8.00KiB");
        assert_eq!(row.pids, "7");
    }

    #[test]
    fn records_metric_history_and_prunes_stale_keys() {
        let mut app = TopApp::new(TopOptions::default());
        let mut snapshot = TopSnapshot::default();
        snapshot.processes.push(ProcessRow {
            cpu_pct: 12.0,
            mem_pct: 3.0,
            ..process_row(42, 0, "codex")
        });
        snapshot.processes.push(ProcessRow {
            cpu_pct: 4.0,
            mem_pct: 2.0,
            ..process_row(100, 42, "cargo test")
        });
        snapshot.containers.push(ContainerRow {
            connector: ContainerConnector::Docker,
            id: "abcdef".into(),
            name: "app".into(),
            image: "img".into(),
            status: "Up".into(),
            inspect: ContainerInspect::default(),
            cpu_pct: Some(8.0),
            cpu_count: None,
            cpu_usage_total_ns: None,
            mem_pct: Some(9.0),
            mem_usage: "1MiB / 10MiB".into(),
            net_io: "1.00KiB / 2.00KiB".into(),
            block_io: "4.00KiB / 8.00KiB".into(),
            pids: "1".into(),
            ports: "-".into(),
        });

        app.record_history(&mut snapshot);

        assert_eq!(app.metric_history("process:42").cpu, vec![12.0]);
        assert_eq!(app.metric_history("agent-tree:42").cpu, vec![16.0]);
        assert_eq!(app.metric_history("agent-tree:42").mem, vec![5.0]);
        assert_eq!(app.metric_history("container:abcdef").mem, vec![9.0]);
        assert_eq!(
            app.metric_history("container:abcdef").net_io_bytes,
            vec![3072.0]
        );
        assert_eq!(
            app.metric_history("container:abcdef").block_io_bytes,
            vec![12288.0]
        );

        app.record_history(&mut TopSnapshot::default());
        assert!(app.history.is_empty());
    }

    #[test]
    fn derives_cpu_percent_from_runc_raw_totals() {
        let now = Instant::now();
        let mut history = MetricHistory::default();

        assert_eq!(
            observe_raw_cpu_pct(&mut history, Some(1_000_000_000), now),
            None
        );

        let pct = observe_raw_cpu_pct(
            &mut history,
            Some(1_500_000_000),
            now + Duration::from_secs(1),
        )
        .unwrap();

        assert!((pct - 50.0).abs() < 0.01);
    }

    #[test]
    fn caps_metric_history() {
        let mut history = MetricHistory::default();
        for value in 0..(HISTORY_LIMIT + 5) {
            push_history(&mut history, value as f32, value as f32);
        }

        assert_eq!(history.cpu.len(), HISTORY_LIMIT);
        assert_eq!(history.cpu[0], 5.0);
    }

    #[test]
    fn observer_chunk_is_incremental_and_newest_first() {
        let mut file = ObserverFileState::default();
        let first =
            r#"{"identity":{"agent":"codex"},"event":{"ToolExec":{"argv":["git","status"]}}}"#;
        let second =
            r#"{"identity":{"agent":"claude"},"event":{"FileAccess":{"path":"README.md"}}}"#;

        let rows = append_observer_chunk(&mut file, first);
        assert!(rows.is_empty());
        assert_eq!(file.pending, first);

        let rows = append_observer_chunk(&mut file, &format!("\n{second}\n"));

        assert!(file.pending.is_empty());
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].source, "claude");
        assert_eq!(rows[1].source, "codex");
    }

    #[test]
    fn observer_rows_merge_newest_first_across_files() {
        let mut state = ObserverState {
            paths: vec![PathBuf::from("left.ndjson"), PathBuf::from("right.ndjson")],
            ..ObserverState::default()
        };
        let mut left = ObserverFileState::default();
        let mut right = ObserverFileState::default();
        push_observer_rows(
            &mut state,
            append_observer_chunk(
                &mut left,
                r#"{"identity":{"agent":"codex"},"event":{"ToolExec":{"argv":["git","status"]}}}
"#,
            ),
        );
        push_observer_rows(
            &mut state,
            append_observer_chunk(
                &mut right,
                r#"{"identity":{"agent":"claude"},"event":{"FileAccess":{"path":"README.md"}}}
"#,
            ),
        );

        assert_eq!(state.events.len(), 2);
        assert_eq!(state.events[0].source, "claude");
        assert_eq!(state.events[1].source, "codex");
        assert_eq!(observer_status_label(&state), "obs:2 files");
    }

    #[test]
    fn observer_auto_discovers_known_agent_logs_and_dedupes_explicit_paths() {
        let root =
            std::env::temp_dir().join(format!("a3s-top-observer-auto-test-{}", std::process::id()));
        let explicit = root.join(".a3s").join("observer").join("events.ndjson");
        let claude = root
            .join(".claude")
            .join("projects")
            .join("-tmp-work")
            .join("session.jsonl");
        let codex = root
            .join(".codex")
            .join("sessions")
            .join("2026")
            .join("06")
            .join("26")
            .join("rollout.jsonl");
        let a3s = root
            .join(".a3s")
            .join("tui")
            .join("sessions")
            .join("runs")
            .join("tui-default.json");
        let a3s_workspace = root
            .join(".a3s")
            .join("workspace")
            .join("users")
            .join("u")
            .join("sessions")
            .join("s")
            .join(".sessions")
            .join("runs")
            .join("s.json");
        write_file(&explicit, "{}\n");
        write_file(&claude, "{}\n");
        write_file(&codex, "{}\n");
        write_file(&a3s, "[]\n");
        write_file(&a3s_workspace, "[]\n");

        let old_home = std::env::var_os("HOME");
        let old_log = std::env::var_os("A3S_TOP_OBSERVER_LOG");
        let old_logs = std::env::var_os("A3S_TOP_OBSERVER_LOGS");
        let old_auto = std::env::var_os("A3S_TOP_OBSERVER_AUTO");
        std::env::set_var("HOME", &root);
        std::env::set_var("A3S_TOP_OBSERVER_AUTO", "1");
        std::env::set_var(
            "A3S_TOP_OBSERVER_LOGS",
            explicit.to_string_lossy().to_string(),
        );
        std::env::remove_var("A3S_TOP_OBSERVER_LOG");

        let paths = observer_paths();

        restore_var("HOME", old_home);
        restore_var("A3S_TOP_OBSERVER_LOG", old_log);
        restore_var("A3S_TOP_OBSERVER_LOGS", old_logs);
        restore_var("A3S_TOP_OBSERVER_AUTO", old_auto);
        let _ = std::fs::remove_dir_all(root);

        assert_eq!(
            paths.paths.iter().filter(|path| *path == &explicit).count(),
            1
        );
        assert!(paths.paths.contains(&claude));
        assert!(paths.paths.contains(&codex));
        assert!(paths.paths.contains(&a3s));
        assert!(paths.paths.contains(&a3s_workspace));
        assert!(!paths.auto_paths.contains(&explicit));
        assert!(paths.auto_paths.contains(&claude));
    }

    #[test]
    fn observer_status_labels_auto_sources() {
        let auto = PathBuf::from("/tmp/claude.jsonl");
        let state = ObserverState {
            paths: vec![auto.clone()],
            auto_paths: HashSet::from([auto]),
            ..ObserverState::default()
        };

        assert_eq!(observer_status_label(&state), "obs:auto:claude.jsonl");
    }

    #[test]
    fn observer_timestamp_sort_key_normalizes_iso_and_numeric_millis() {
        assert_eq!(
            observer_timestamp_sort_key("2026-06-26T08:00:00Z"),
            Some(1_782_460_800_000)
        );
        assert_eq!(
            observer_timestamp_sort_key("1782460800000"),
            Some(1_782_460_800_000)
        );
        assert!(observer_timestamp_sort_key("now").unwrap() > 1_782_460_800_000);
    }

    #[test]
    fn observer_trimming_keeps_newest_events_across_sources() {
        let mut events = (0..220)
            .map(|idx| EventRow {
                ts: format!("{}", 1_000_000 + idx),
                source: "a3s-code".into(),
                session: Some("old".into()),
                task: None,
                pid: None,
                ppid: None,
                kind: "AgentEvent".into(),
                message: format!("old-{idx}"),
                details: Vec::new(),
                risk: Risk::Low,
            })
            .collect::<Vec<_>>();
        events.push(EventRow {
            ts: "2026-06-26T08:00:00Z".into(),
            source: "codex".into(),
            session: Some("new".into()),
            task: None,
            pid: None,
            ppid: None,
            kind: "LlmCall".into(),
            message: "new codex event".into(),
            details: Vec::new(),
            risk: Risk::Low,
        });

        trim_observer_events(&mut events);

        assert_eq!(events.len(), OBSERVER_EVENT_LIMIT);
        assert_eq!(events[0].source, "codex");
        assert!(events
            .iter()
            .any(|event| event.message == "new codex event"));
    }

    #[test]
    fn parses_claude_jsonl_tool_use() {
        let row = parse_observer_line(
            r#"{
                "type":"assistant",
                "timestamp":"2026-06-26T08:00:00Z",
                "sessionId":"claude-session",
                "cwd":"/work/a3s",
                "message":{
                    "model":"claude-opus-4",
                    "usage":{"input_tokens":10,"output_tokens":5,"total_tokens":15},
                    "content":[
                        {"type":"tool_use","name":"Bash","input":{"command":"cargo test","description":"Run tests"}}
                    ]
                }
            }"#,
        )
        .unwrap();

        assert_eq!(row.source, "claude");
        assert_eq!(row.session.as_deref(), Some("claude-session"));
        assert_eq!(row.kind, "ToolExec");
        assert_eq!(row.risk, Risk::Medium);
        assert!(row.message.contains("Bash"));
        assert_eq!(
            event_detail_value(&row, &["model"]).as_deref(),
            Some("claude-opus-4")
        );
        assert_eq!(event_token_usage(&row).unwrap().total, 15);
        assert_eq!(event_workspace(&row).as_deref(), Some("/work/a3s"));
    }

    #[test]
    fn parses_codex_jsonl_token_usage() {
        let row = parse_observer_line(
            r#"{
                "timestamp":"2026-06-26T08:00:00Z",
                "type":"event_msg",
                "payload":{
                    "type":"token_count",
                    "info":{
                        "last_token_usage":{"input_tokens":7,"output_tokens":3,"total_tokens":10},
                        "model_context_window":258400
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(row.source, "codex");
        assert_eq!(row.kind, "LlmCall");
        assert_eq!(event_token_usage(&row).unwrap().total, 10);
        assert_eq!(
            event_detail_value(&row, &["context_window"]).as_deref(),
            Some("258400")
        );
    }

    #[test]
    fn codex_token_usage_prefers_last_turn_over_lifetime_total() {
        let row = parse_observer_line(
            r#"{
                "timestamp":"2026-06-26T08:00:00Z",
                "type":"event_msg",
                "payload":{
                    "type":"token_count",
                    "info":{
                        "total_token_usage":{"input_tokens":1000000,"output_tokens":500000,"total_tokens":1500000},
                        "last_token_usage":{"input_tokens":70,"output_tokens":30,"total_tokens":100}
                    }
                }
            }"#,
        )
        .unwrap();

        assert_eq!(event_token_usage(&row).unwrap().total, 100);
        assert_eq!(
            event_detail_value(&row, &["lifetime_total_tokens"]).as_deref(),
            Some("1500000")
        );
    }

    #[test]
    fn parses_a3s_run_record_document() {
        let rows = parse_observer_json_document(
            r#"[
                {
                    "snapshot": {
                        "id": "run-1",
                        "session_id": "session-1",
                        "workspace": "/work/a3s"
                    },
                    "events": [
                        {
                            "sequence": 1,
                            "timestamp_ms": 1782470400000,
                            "event": {"type":"tool_start","name":"shell","command":"cargo test"}
                        },
                        {
                            "sequence": 2,
                            "timestamp_ms": 1782470400100,
                            "event": {"type":"turn_end","total_tokens":123}
                        }
                    ]
                }
            ]"#,
        );

        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].source, "a3s-code");
        assert_eq!(rows[0].session.as_deref(), Some("session-1"));
        assert_eq!(rows[0].task.as_deref(), Some("run-1"));
        assert_eq!(rows[0].kind, "ToolExec");
        assert_eq!(event_workspace(&rows[0]).as_deref(), Some("/work/a3s"));
        assert_eq!(rows[1].kind, "LlmCall");
        assert_eq!(event_token_usage(&rows[1]).unwrap().total, 123);
    }

    fn event_row(
        source: &str,
        session: Option<&str>,
        task: Option<&str>,
        kind: &str,
        message: &str,
        risk: Risk,
    ) -> EventRow {
        EventRow {
            ts: "recent".into(),
            source: source.into(),
            session: session.map(Into::into),
            task: task.map(Into::into),
            pid: None,
            ppid: None,
            kind: kind.into(),
            message: message.into(),
            details: Vec::new(),
            risk,
        }
    }

    fn process_row(pid: u32, ppid: u32, command: &str) -> ProcessRow {
        let agent = detect_agent(command);
        ProcessRow {
            pid,
            ppid,
            cpu_pct: 0.0,
            mem_pct: 0.0,
            elapsed: "00:01".into(),
            cwd: None,
            command: command.into(),
            agent,
            risk: process_risk(command, agent),
        }
    }

    fn container_row(
        id: &str,
        name: &str,
        status: &str,
        cpu_pct: Option<f32>,
        mem_pct: Option<f32>,
    ) -> ContainerRow {
        ContainerRow {
            connector: ContainerConnector::Docker,
            id: id.into(),
            name: name.into(),
            image: "img".into(),
            status: status.into(),
            inspect: ContainerInspect::default(),
            cpu_pct,
            cpu_count: None,
            cpu_usage_total_ns: None,
            mem_pct,
            mem_usage: "1MiB / 10MiB".into(),
            net_io: "-".into(),
            block_io: "-".into(),
            pids: "1".into(),
            ports: "-".into(),
        }
    }

    fn numbered_lines(count: usize) -> String {
        (0..count)
            .map(|idx| format!("line-{idx}"))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn write_file(path: &Path, content: &str) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, content).unwrap();
    }

    fn restore_var(key: &str, value: Option<std::ffi::OsString>) {
        match value {
            Some(value) => std::env::set_var(key, value),
            None => std::env::remove_var(key),
        }
    }
}
