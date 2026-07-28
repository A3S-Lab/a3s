//! `/memory` panel data: read the agent's long-term memory store
//! (`~/.a3s/memory`, an `a3s-memory` FileMemoryStore) and derive a lightweight
//! knowledge graph over it. The persisted store stays backwards-compatible:
//! `index.json` remains the timeline source and `items/{id}.json` remains the
//! durable item payload. The graph is rebuilt from those files on open/refresh
//! so old flat memories immediately gain event/entity/relation structure.

use chrono::{DateTime, Utc};
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

/// A memory as listed in the timeline (from the store's `index.json`).
#[derive(Debug, Clone, Deserialize)]
pub(crate) struct MemEntry {
    pub id: String,
    /// Lowercased content — the index only stores this; used for the preview.
    #[serde(default)]
    pub content_lower: String,
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub importance: f32,
    pub timestamp: DateTime<Utc>,
    /// `episodic` | `semantic` | `procedural` | `working`.
    #[serde(default)]
    pub memory_type: String,
}

/// A memory's full content + metadata (lazily read for the detail pane).
#[derive(Debug, Clone, Default, Deserialize)]
pub(crate) struct MemDetail {
    #[serde(default)]
    pub content: String,
    /// BTreeMap so the detail pane shows metadata in a stable order.
    #[serde(default)]
    pub metadata: std::collections::BTreeMap<String, String>,
    #[serde(default)]
    pub access_count: u32,
    #[serde(default)]
    pub last_accessed: Option<DateTime<Utc>>,
}

/// Lifecycle tier derived from age, memory type, importance, and access count.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum MemoryTier {
    #[default]
    Short,
    Mid,
    Long,
}

impl MemoryTier {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Short => "short-term",
            Self::Mid => "mid-term",
            Self::Long => "long-term",
        }
    }

    pub(crate) fn badge(self) -> &'static str {
        match self {
            Self::Short => "S",
            Self::Mid => "M",
            Self::Long => "L",
        }
    }
}

/// Forgetting state for a memory. `Candidate` is the only state the TUI lets
/// the user remove directly; high-importance, repeatedly-accessed, or curated
/// memories are marked `Protected` to mirror store pruning decisions.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ForgetSignal {
    #[default]
    Keep,
    Cooling,
    Candidate,
    Protected,
}

impl ForgetSignal {
    pub(crate) fn label(self) -> &'static str {
        match self {
            Self::Keep => "keep",
            Self::Cooling => "cooling",
            Self::Candidate => "forget candidate",
            Self::Protected => "protected",
        }
    }

    pub(crate) fn is_candidate(self) -> bool {
        matches!(self, Self::Candidate)
    }
}

/// One entity node in the derived graph. Entities are merged by
/// `(kind, normalized-name)` and retain observed spellings as aliases.
#[derive(Debug, Clone, Default)]
pub(crate) struct EntityNode {
    pub kind: String,
    pub name: String,
    pub aliases: Vec<String>,
    pub mentions: usize,
    pub importance: f32,
    pub first_seen: Option<DateTime<Utc>>,
    pub last_seen: Option<DateTime<Utc>>,
    pub memory_ids: Vec<String>,
}

/// One memory item as an event node in the graph.
#[derive(Debug, Clone)]
pub(crate) struct MemoryEvent {
    pub id: String,
    pub memory_id: String,
    pub label: String,
    pub source: String,
    pub tier: MemoryTier,
    pub forget: ForgetSignal,
    pub retention_score: f32,
    pub timestamp: DateTime<Utc>,
    pub entity_ids: Vec<String>,
}

/// Directed relation between an event/entity and another entity.
#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryRelation {
    pub from: String,
    pub to: String,
    pub kind: String,
    pub memory_id: String,
    pub weight: f32,
}

/// Fast lookup data for the selected memory's graph neighborhood.
#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryGraphFacet {
    pub event_id: String,
    pub tier: MemoryTier,
    pub forget: ForgetSignal,
    pub retention_score: f32,
    pub llm_extracted: bool,
    pub consolidated: bool,
    pub conflicts: bool,
    pub entity_ids: Vec<String>,
    pub relation_ids: Vec<usize>,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryGraphStats {
    pub events: usize,
    pub entities: usize,
    pub relations: usize,
    pub aliases: usize,
    pub short: usize,
    pub mid: usize,
    pub long: usize,
    pub forget_candidates: usize,
    pub llm_extracted: usize,
    pub consolidated: usize,
    pub conflicts: usize,
}

/// Derived knowledge graph for the `/memory` panel.
#[derive(Debug, Clone, Default)]
pub(crate) struct MemoryGraph {
    pub events: Vec<MemoryEvent>,
    pub entities: BTreeMap<String, EntityNode>,
    pub relations: Vec<MemoryRelation>,
    pub by_memory: BTreeMap<String, MemoryGraphFacet>,
    pub stats: MemoryGraphStats,
}

impl MemoryGraph {
    pub(crate) fn event_for_memory(&self, memory_id: &str) -> Option<&MemoryEvent> {
        let facet = self.by_memory.get(memory_id)?;
        self.events
            .iter()
            .find(|event| event.id == facet.event_id && event.memory_id == memory_id)
    }

    pub(crate) fn entity_label(&self, id: &str) -> Option<String> {
        self.entities
            .get(id)
            .map(|e| format!("{}:{}", e.kind, e.name))
    }

    pub(crate) fn entity_labels(&self, ids: &[String], limit: usize) -> Vec<String> {
        ids.iter()
            .filter_map(|id| self.entity_label(id))
            .take(limit)
            .collect()
    }

    pub(crate) fn alias_labels(&self, ids: &[String], limit: usize) -> Vec<String> {
        let mut out = Vec::new();
        for id in ids {
            let Some(entity) = self.entities.get(id) else {
                continue;
            };
            for alias in &entity.aliases {
                out.push(format!("{}:{}", entity.kind, alias));
                if out.len() >= limit {
                    return out;
                }
            }
        }
        out
    }

    pub(crate) fn relation_labels(&self, ids: &[usize], limit: usize) -> Vec<String> {
        ids.iter()
            .filter_map(|idx| self.relations.get(*idx))
            .filter(|rel| !rel.memory_id.is_empty())
            .filter_map(|rel| {
                let to = self.entity_label(&rel.to)?;
                let from = if rel.from.starts_with("event:") {
                    "event".to_string()
                } else {
                    self.entity_label(&rel.from)?
                };
                Some(format!("{from} -{} {:.1}→ {to}", rel.kind, rel.weight))
            })
            .take(limit)
            .collect()
    }
}

/// The complete `/memory` load result: timeline, detail cache, and graph.
#[derive(Debug, Clone, Default)]
pub(crate) struct MemPanelData {
    pub entries: Vec<MemEntry>,
    pub details: BTreeMap<String, MemDetail>,
    pub graph: MemoryGraph,
    pub loaded_from_session: bool,
}

/// Read the store index, newest first. Empty if the store is absent/unreadable.
pub(crate) fn load_timeline(dir: &Path) -> Vec<MemEntry> {
    let mut v: Vec<MemEntry> = std::fs::read_to_string(dir.join("index.json"))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    v.sort_by_key(|e| std::cmp::Reverse(e.timestamp));
    v
}

/// Lazily read one memory's full item file for the detail pane.
pub(crate) fn load_detail(dir: &Path, id: &str) -> Option<MemDetail> {
    // ids are UUIDs; reject anything that could escape the items dir.
    if id.contains('/') || id.contains('\\') || id.contains("..") {
        return None;
    }
    let path = dir.join("items").join(format!("{id}.json"));
    serde_json::from_str(&std::fs::read_to_string(path).ok()?).ok()
}

/// Load the timeline plus all available details, then build the graph. Missing
/// detail files are tolerated: the graph falls back to index previews/tags.
pub(crate) fn load_panel_data(dir: &Path) -> MemPanelData {
    let entries = load_timeline(dir);
    let details = load_details_for_entries(dir, &entries);
    let graph = build_memory_graph(&entries, &details, Utc::now());
    MemPanelData {
        entries,
        details,
        graph,
        loaded_from_session: false,
    }
}

/// Build panel data from a live session memory store. This is used as a fallback
/// when the file-backed snapshot is unavailable (for example after the session
/// falls back to an in-memory store).
pub(crate) fn panel_data_from_memory_items(mut items: Vec<a3s_memory::MemoryItem>) -> MemPanelData {
    items.sort_by_key(|item| std::cmp::Reverse(item.timestamp));

    let entries: Vec<MemEntry> = items.iter().map(mem_entry_from_item).collect();
    let details: BTreeMap<String, MemDetail> = items
        .iter()
        .map(|item| (item.id.clone(), mem_detail_from_item(item)))
        .collect();
    let graph = build_memory_graph(&entries, &details, Utc::now());

    MemPanelData {
        entries,
        details,
        graph,
        loaded_from_session: true,
    }
}

fn mem_entry_from_item(item: &a3s_memory::MemoryItem) -> MemEntry {
    MemEntry {
        id: item.id.clone(),
        content_lower: item.content.to_lowercase(),
        tags: item.tags.clone(),
        importance: item.importance,
        timestamp: item.timestamp,
        memory_type: memory_type_label(item.memory_type).to_string(),
    }
}

fn mem_detail_from_item(item: &a3s_memory::MemoryItem) -> MemDetail {
    MemDetail {
        content: item.content.clone(),
        metadata: item.metadata.clone().into_iter().collect(),
        access_count: item.access_count,
        last_accessed: item.last_accessed,
    }
}

fn memory_type_label(memory_type: a3s_memory::MemoryType) -> &'static str {
    match memory_type {
        a3s_memory::MemoryType::Episodic => "episodic",
        a3s_memory::MemoryType::Semantic => "semantic",
        a3s_memory::MemoryType::Procedural => "procedural",
        a3s_memory::MemoryType::Working => "working",
    }
}

fn load_details_for_entries(dir: &Path, entries: &[MemEntry]) -> BTreeMap<String, MemDetail> {
    entries
        .iter()
        .filter_map(|entry| load_detail(dir, &entry.id).map(|detail| (entry.id.clone(), detail)))
        .collect()
}

/// Compact "time since" for a node (`now`, `5m`, `3h`, `2d`, `4w`, `5mo`, `1y`).
pub(crate) fn rel_time(ts: DateTime<Utc>, now: DateTime<Utc>) -> String {
    let secs = (now - ts).num_seconds().max(0);
    match secs {
        0..=59 => "now".to_string(),
        60..=3_599 => format!("{}m", secs / 60),
        3_600..=86_399 => format!("{}h", secs / 3_600),
        86_400..=604_799 => format!("{}d", secs / 86_400),
        604_800..=2_591_999 => format!("{}w", secs / 604_800),
        2_592_000..=31_535_999 => format!("{}mo", secs / 2_592_000),
        _ => format!("{}y", secs / 31_536_000),
    }
}

/// Day-bucket header for the timeline (`Today` / `Yesterday` / `YYYY-MM-DD`).
/// Buckets by UTC date — a memory near midnight may land a day off in the
/// viewer's local zone, which is fine for a coarse timeline.
pub(crate) fn day_label(ts: DateTime<Utc>, now: DateTime<Utc>) -> String {
    match (now.date_naive() - ts.date_naive()).num_days() {
        d if d <= 0 => "Today".to_string(),
        1 => "Yesterday".to_string(),
        _ => ts.format("%Y-%m-%d").to_string(),
    }
}

/// One rendered timeline row: a day-bucket header, or a memory node (the index
/// of the entry in `MemPanel::entries`).
pub(crate) enum TlRow {
    Day(String),
    Node(usize),
}

/// Build the timeline rows — day headers interleaved with nodes, newest first.
pub(crate) fn timeline_rows(entries: &[MemEntry], now: DateTime<Utc>) -> Vec<TlRow> {
    let mut rows = Vec::with_capacity(entries.len() + 8);
    let mut last = String::new();
    for (i, e) in entries.iter().enumerate() {
        let d = day_label(e.timestamp, now);
        if d != last {
            rows.push(TlRow::Day(d.clone()));
            last = d;
        }
        rows.push(TlRow::Node(i));
    }
    rows
}

/// Panel state for `/memory`.
pub(crate) struct MemPanel {
    pub entries: Vec<MemEntry>,
    pub sel: usize,
    /// Full item cache for graph/detail rendering. Loaded on open; lazily
    /// backfilled if a file appears after the panel opened.
    pub details: BTreeMap<String, MemDetail>,
    pub graph: MemoryGraph,
    pub loaded_from_session: bool,
    /// Full content + metadata of `entries[sel]`, lazily loaded on selection.
    pub detail: MemDetail,
    pub detail_scroll: usize,
    pub dir: std::path::PathBuf,
    pub note: String,
}

impl MemPanel {
    pub fn apply_data(&mut self, data: MemPanelData) {
        self.entries = data.entries;
        self.details = data.details;
        self.graph = data.graph;
        self.loaded_from_session = data.loaded_from_session;
        self.sel = self.sel.min(self.entries.len().saturating_sub(1));
        self.refresh_detail();
    }

    /// Reload the selected entry's full detail (called on open + selection move).
    pub fn refresh_detail(&mut self) {
        self.detail_scroll = 0;
        let Some(entry) = self.entries.get(self.sel) else {
            self.detail = MemDetail::default();
            return;
        };
        if let Some(detail) = self.details.get(&entry.id).cloned() {
            self.detail = detail;
            return;
        }
        let detail = load_detail(&self.dir, &entry.id).unwrap_or_default();
        if !detail.content.is_empty() || !detail.metadata.is_empty() {
            self.details.insert(entry.id.clone(), detail.clone());
        }
        self.detail = detail;
    }
}

pub(crate) fn build_memory_graph(
    entries: &[MemEntry],
    details: &BTreeMap<String, MemDetail>,
    now: DateTime<Utc>,
) -> MemoryGraph {
    let mut builder = GraphBuilder::default();
    for entry in entries {
        builder.add_memory(entry, details.get(&entry.id), now);
    }
    builder.finish()
}

#[derive(Default)]
struct GraphBuilder {
    events: Vec<MemoryEvent>,
    entities: BTreeMap<String, EntityBuild>,
    relations: Vec<MemoryRelation>,
    by_memory: BTreeMap<String, MemoryGraphFacet>,
}

#[derive(Default)]
struct EntityBuild {
    node: EntityNode,
    aliases: BTreeSet<String>,
    memory_ids: BTreeSet<String>,
}

impl GraphBuilder {
    fn add_memory(&mut self, entry: &MemEntry, detail: Option<&MemDetail>, now: DateTime<Utc>) {
        let empty = MemDetail::default();
        let detail = detail.unwrap_or(&empty);
        let event_id = format!("event:{}", entry.id);
        let content = if detail.content.trim().is_empty() {
            entry.content_lower.as_str()
        } else {
            detail.content.as_str()
        };
        let retention = retention_for(entry, detail, now);
        let lifecycle = lifecycle_for(entry, detail);
        let mut entity_ids = BTreeSet::new();
        let mut relation_ids = Vec::new();

        let source = detail
            .metadata
            .get("source")
            .cloned()
            .or_else(|| entry.tags.first().cloned())
            .unwrap_or_else(|| "memory".to_string());

        let source_id = self.add_entity("source", &source, None, entry);
        entity_ids.insert(source_id.clone());
        relation_ids.push(self.add_relation(&event_id, &source_id, "from", &entry.id, 1.0));

        for tag in &entry.tags {
            let id = self.add_entity("tag", tag, None, entry);
            entity_ids.insert(id.clone());
            relation_ids.push(self.add_relation(&event_id, &id, "tagged", &entry.id, 0.8));
        }

        for (key, value) in &detail.metadata {
            self.add_metadata_entities(
                &event_id,
                entry,
                key,
                value,
                &mut entity_ids,
                &mut relation_ids,
            );
        }

        for extracted in extract_content_entities(content) {
            let id = self.add_entity(
                extracted.kind,
                &extracted.name,
                extracted.alias.as_deref(),
                entry,
            );
            entity_ids.insert(id.clone());
            relation_ids.push(self.add_relation(
                &event_id,
                &id,
                extracted.relation,
                &entry.id,
                extracted.weight,
            ));
        }

        let ids: Vec<String> = entity_ids.into_iter().collect();
        let co_entities = ids.iter().take(10).collect::<Vec<_>>();
        for (i, from) in co_entities.iter().enumerate() {
            for to in co_entities.iter().skip(i + 1) {
                relation_ids.push(self.add_relation(
                    from.as_str(),
                    to.as_str(),
                    "co-occurs",
                    &entry.id,
                    0.35,
                ));
            }
        }

        let event = MemoryEvent {
            id: event_id.clone(),
            memory_id: entry.id.clone(),
            label: event_label(content, &entry.id),
            source,
            tier: retention.tier,
            forget: retention.forget,
            retention_score: retention.score,
            timestamp: entry.timestamp,
            entity_ids: ids.clone(),
        };
        self.events.push(event);
        self.by_memory.insert(
            entry.id.clone(),
            MemoryGraphFacet {
                event_id,
                tier: retention.tier,
                forget: retention.forget,
                retention_score: retention.score,
                llm_extracted: lifecycle.llm_extracted,
                consolidated: lifecycle.consolidated,
                conflicts: lifecycle.conflicts,
                entity_ids: ids,
                relation_ids,
            },
        );
    }

    fn add_metadata_entities(
        &mut self,
        event_id: &str,
        entry: &MemEntry,
        key: &str,
        value: &str,
        entity_ids: &mut BTreeSet<String>,
        relation_ids: &mut Vec<usize>,
    ) {
        let value = value.trim();
        if value.is_empty() {
            return;
        }
        let (kind, relation) = match key {
            "provider" => ("provider", "via"),
            "ctx_session_id" => ("session", "in-session"),
            "ctx_event_id" => ("ctx-event", "source-event"),
            "sleep_date" | "ctx_time" => ("date", "at"),
            "prompt" => ("prompt", "about"),
            "error" => ("error", "failed-with"),
            "tools" => {
                for tool in split_list(value) {
                    let id = self.add_entity("tool", tool, None, entry);
                    entity_ids.insert(id.clone());
                    relation_ids.push(self.add_relation(event_id, &id, "used", &entry.id, 0.9));
                }
                return;
            }
            "aliases" | "entity_aliases" => {
                let canonical = event_label(
                    entry
                        .content_lower
                        .lines()
                        .next()
                        .unwrap_or(entry.id.as_str()),
                    &entry.id,
                );
                let id = self.add_entity("topic", &canonical, None, entry);
                for alias in split_list(value) {
                    let _ = self.add_entity("topic", &canonical, Some(alias), entry);
                }
                entity_ids.insert(id.clone());
                relation_ids.push(self.add_relation(event_id, &id, "aliases", &entry.id, 0.7));
                return;
            }
            "source" => ("source", "from"),
            _ => return,
        };
        let id = self.add_entity(kind, value, None, entry);
        entity_ids.insert(id.clone());
        relation_ids.push(self.add_relation(event_id, &id, relation, &entry.id, 0.8));
    }

    fn add_entity(
        &mut self,
        kind: &str,
        name: &str,
        alias: Option<&str>,
        entry: &MemEntry,
    ) -> String {
        let name = name.trim();
        let canonical = canonical_key(name);
        if canonical.is_empty() {
            return format!("{kind}:unknown");
        }
        let id = format!("{kind}:{canonical}");
        let build = self
            .entities
            .entry(id.clone())
            .or_insert_with(|| EntityBuild {
                node: EntityNode {
                    kind: kind.to_string(),
                    name: display_entity_name(name),
                    aliases: Vec::new(),
                    mentions: 0,
                    importance: 0.0,
                    first_seen: Some(entry.timestamp),
                    last_seen: Some(entry.timestamp),
                    memory_ids: Vec::new(),
                },
                aliases: BTreeSet::new(),
                memory_ids: BTreeSet::new(),
            });
        build.node.mentions += 1;
        build.node.importance = build.node.importance.max(entry.importance);
        build.node.first_seen = Some(match build.node.first_seen {
            Some(ts) => ts.min(entry.timestamp),
            None => entry.timestamp,
        });
        build.node.last_seen = Some(match build.node.last_seen {
            Some(ts) => ts.max(entry.timestamp),
            None => entry.timestamp,
        });
        build.memory_ids.insert(entry.id.clone());
        let display = display_entity_name(name);
        if display != build.node.name {
            build.aliases.insert(display);
        }
        if let Some(alias) = alias {
            let alias = display_entity_name(alias);
            if !alias.is_empty() && alias != build.node.name {
                build.aliases.insert(alias);
            }
        }
        id
    }

    fn add_relation(
        &mut self,
        from: &str,
        to: &str,
        kind: &str,
        memory_id: &str,
        weight: f32,
    ) -> usize {
        let idx = self.relations.len();
        self.relations.push(MemoryRelation {
            from: from.to_string(),
            to: to.to_string(),
            kind: kind.to_string(),
            memory_id: memory_id.to_string(),
            weight,
        });
        idx
    }

    fn finish(self) -> MemoryGraph {
        let mut entities = BTreeMap::new();
        for (id, mut build) in self.entities {
            build.node.aliases = build.aliases.into_iter().collect();
            build.node.memory_ids = build.memory_ids.into_iter().collect();
            entities.insert(id, build.node);
        }
        let mut stats = MemoryGraphStats {
            events: self.events.len(),
            entities: entities.len(),
            relations: self.relations.len(),
            aliases: entities.values().map(|e| e.aliases.len()).sum(),
            ..MemoryGraphStats::default()
        };
        for facet in self.by_memory.values() {
            match facet.tier {
                MemoryTier::Short => stats.short += 1,
                MemoryTier::Mid => stats.mid += 1,
                MemoryTier::Long => stats.long += 1,
            }
            if facet.forget.is_candidate() {
                stats.forget_candidates += 1;
            }
            if facet.llm_extracted {
                stats.llm_extracted += 1;
            }
            if facet.consolidated {
                stats.consolidated += 1;
            }
            if facet.conflicts {
                stats.conflicts += 1;
            }
        }
        MemoryGraph {
            events: self.events,
            entities,
            relations: self.relations,
            by_memory: self.by_memory,
            stats,
        }
    }
}

#[derive(Debug, Clone, Copy)]
struct Retention {
    tier: MemoryTier,
    forget: ForgetSignal,
    score: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct MemoryLifecycle {
    llm_extracted: bool,
    consolidated: bool,
    conflicts: bool,
}

fn lifecycle_for(entry: &MemEntry, detail: &MemDetail) -> MemoryLifecycle {
    let has_tag = |needle: &str| entry.tags.iter().any(|tag| tag == needle);
    let metadata_nonempty = |key: &str| {
        detail
            .metadata
            .get(key)
            .is_some_and(|value| !value.trim().is_empty())
    };
    let source = detail
        .metadata
        .get("source")
        .map(|value| value.trim())
        .unwrap_or_default();
    let llm_source = matches!(
        source,
        "llm_extractor" | "project_fact" | "workflow" | "failure" | "preference" | "decision"
    );

    MemoryLifecycle {
        llm_extracted: has_tag("llm") || has_tag("extracted") || llm_source,
        consolidated: has_tag("consolidated") || metadata_nonempty("supersedes"),
        conflicts: has_tag("conflict") || metadata_nonempty("conflicts_with"),
    }
}

fn retention_for(entry: &MemEntry, detail: &MemDetail, now: DateTime<Utc>) -> Retention {
    let age_days = ((now - entry.timestamp).num_seconds().max(0) as f32) / 86_400.0;
    let recency = (-age_days / 30.0).exp();
    let access = ((detail.access_count as f32) + 1.0).ln() / 3.0_f32.ln();
    let access = access.clamp(0.0, 1.0);
    let type_bonus = match entry.memory_type.as_str() {
        "semantic" | "procedural" => 0.08,
        "working" => 0.10,
        _ => 0.0,
    };
    let score =
        (entry.importance.clamp(0.0, 1.0) * 0.55 + recency * 0.30 + access * 0.15 + type_bonus)
            .clamp(0.0, 1.0);

    let protected = memory_is_prune_protected(entry, detail);
    let forget = if protected {
        ForgetSignal::Protected
    } else if age_days > 90.0 && entry.importance < 0.45 && detail.access_count == 0 && score < 0.35
    {
        ForgetSignal::Candidate
    } else if age_days > 30.0 && entry.importance < 0.60 && score < 0.45 {
        ForgetSignal::Cooling
    } else {
        ForgetSignal::Keep
    };

    let tier = if entry.memory_type == "working" || age_days <= 7.0 {
        MemoryTier::Short
    } else if protected
        || (matches!(entry.memory_type.as_str(), "semantic" | "procedural")
            && entry.importance >= 0.65)
        || age_days > 45.0
    {
        MemoryTier::Long
    } else {
        MemoryTier::Mid
    };

    Retention {
        tier,
        forget,
        score,
    }
}

fn memory_is_prune_protected(entry: &MemEntry, detail: &MemDetail) -> bool {
    entry.importance >= 0.80
        || detail.access_count >= 3
        || entry.tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "keep" | "pinned" | "protected" | "consolidated" | "conflict"
            )
        })
        || metadata_truthy(&detail.metadata, "keep")
        || metadata_truthy(&detail.metadata, "pinned")
        || metadata_truthy(&detail.metadata, "protected")
        || metadata_nonempty(&detail.metadata, "supersedes")
        || metadata_nonempty(&detail.metadata, "conflicts_with")
}

fn metadata_truthy(metadata: &BTreeMap<String, String>, key: &str) -> bool {
    metadata
        .get(key)
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "keep" | "pinned" | "protected"
            )
        })
        .unwrap_or(false)
}

fn metadata_nonempty(metadata: &BTreeMap<String, String>, key: &str) -> bool {
    metadata
        .get(key)
        .is_some_and(|value| !value.trim().is_empty())
}

#[derive(Debug, Clone)]
struct ExtractedEntity {
    kind: &'static str,
    name: String,
    alias: Option<String>,
    relation: &'static str,
    weight: f32,
}

fn extract_content_entities(content: &str) -> Vec<ExtractedEntity> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Tools:") {
            for tool in split_list(rest) {
                out.push(ExtractedEntity {
                    kind: "tool",
                    name: tool.to_string(),
                    alias: None,
                    relation: "used",
                    weight: 0.9,
                });
            }
        }
        if trimmed.starts_with("Success:") {
            out.push(simple_entity("outcome", "success", "resulted-in", 0.8));
        } else if trimmed.starts_with("Failure:") {
            out.push(simple_entity("outcome", "failure", "resulted-in", 0.9));
        }
        for raw in trimmed.split_whitespace() {
            let token = raw.trim_matches(|c: char| {
                matches!(c, ',' | ';' | ':' | '"' | '\'' | ')' | '(' | '[' | ']')
            });
            if token.starts_with("https://") || token.starts_with("http://") {
                out.push(simple_entity("url", token, "references", 0.7));
            } else if is_slash_command(token) {
                out.push(simple_entity("command", token, "mentions", 0.65));
            } else if looks_like_path(token) {
                out.push(simple_entity("file", token, "touches", 0.7));
            }
        }
    }
    dedupe_extracted(out)
}

fn simple_entity(
    kind: &'static str,
    name: impl Into<String>,
    relation: &'static str,
    weight: f32,
) -> ExtractedEntity {
    ExtractedEntity {
        kind,
        name: name.into(),
        alias: None,
        relation,
        weight,
    }
}

fn dedupe_extracted(items: Vec<ExtractedEntity>) -> Vec<ExtractedEntity> {
    let mut seen = BTreeSet::new();
    let mut out = Vec::new();
    for item in items {
        let key = format!("{}:{}", item.kind, canonical_key(&item.name));
        if seen.insert(key) {
            out.push(item);
        }
    }
    out
}

fn split_list(value: &str) -> impl Iterator<Item = &str> {
    value
        .split([',', ';'])
        .map(str::trim)
        .filter(|part| !part.is_empty())
}

fn event_label(content: &str, fallback: &str) -> String {
    let line = content
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .unwrap_or(fallback);
    line.chars().take(96).collect()
}

fn canonical_key(value: &str) -> String {
    let mut out = String::new();
    let mut last_space = false;
    for ch in value.trim().chars().flat_map(char::to_lowercase) {
        if ch.is_ascii_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.' | ':' | '@') {
            out.push(ch);
            last_space = false;
        } else if ch.is_whitespace() && !last_space && !out.is_empty() {
            out.push(' ');
            last_space = true;
        }
    }
    out.trim().to_string()
}

fn display_entity_name(value: &str) -> String {
    value
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
        .chars()
        .take(80)
        .collect()
}

fn is_slash_command(token: &str) -> bool {
    let rest = token.strip_prefix('/').unwrap_or_default();
    !rest.is_empty()
        && rest
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
}

fn looks_like_path(token: &str) -> bool {
    if !(token.contains('/') || token.contains('\\')) {
        return false;
    }
    let lower = token.to_ascii_lowercase();
    [
        ".rs", ".md", ".toml", ".json", ".yaml", ".yml", ".ts", ".tsx", ".js", ".py", ".go",
        ".java", ".sh", ".sql",
    ]
    .iter()
    .any(|ext| lower.ends_with(ext))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ts(s: &str) -> DateTime<Utc> {
        DateTime::parse_from_rfc3339(s).unwrap().with_timezone(&Utc)
    }

    #[test]
    fn rel_time_buckets() {
        let now = ts("2026-06-30T12:00:00Z");
        assert_eq!(rel_time(ts("2026-06-30T11:59:30Z"), now), "now");
        assert_eq!(rel_time(ts("2026-06-30T11:30:00Z"), now), "30m");
        assert_eq!(rel_time(ts("2026-06-30T09:00:00Z"), now), "3h");
        assert_eq!(rel_time(ts("2026-06-27T12:00:00Z"), now), "3d");
        assert_eq!(rel_time(ts("2026-06-10T12:00:00Z"), now), "2w");
    }

    #[test]
    fn day_labels() {
        let now = ts("2026-06-30T12:00:00Z");
        assert_eq!(day_label(ts("2026-06-30T01:00:00Z"), now), "Today");
        assert_eq!(day_label(ts("2026-06-29T23:00:00Z"), now), "Yesterday");
        assert_eq!(day_label(ts("2026-06-20T10:00:00Z"), now), "2026-06-20");
    }

    #[test]
    fn load_timeline_reads_index_newest_first() {
        let dir = std::env::temp_dir().join(format!("a3s-mem-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let index = r#"[
            {"id":"a","content_lower":"older","tags":[],"importance":0.5,"timestamp":"2026-06-20T10:00:00Z","memory_type":"episodic"},
            {"id":"b","content_lower":"newer","tags":["x"],"importance":0.9,"timestamp":"2026-06-29T10:00:00Z","memory_type":"semantic"}
        ]"#;
        std::fs::write(dir.join("index.json"), index).unwrap();
        let tl = load_timeline(&dir);
        assert_eq!(tl.len(), 2);
        assert_eq!(tl[0].id, "b"); // newest first
        assert_eq!(tl[1].id, "a");
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn load_detail_reads_item_and_rejects_traversal() {
        let dir = std::env::temp_dir().join(format!("a3s-memd-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("items")).unwrap();
        std::fs::write(
            dir.join("items/abc.json"),
            r#"{"content":"Hello World","metadata":{"k":"v"},"access_count":3,"last_accessed":null}"#,
        )
        .unwrap();
        let d = load_detail(&dir, "abc").unwrap();
        assert_eq!(d.content, "Hello World"); // original case preserved
        assert_eq!(d.access_count, 3);
        assert_eq!(d.metadata.get("k").unwrap(), "v");
        assert!(load_detail(&dir, "../secret").is_none()); // traversal rejected
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn timeline_rows_group_by_day() {
        let now = ts("2026-06-30T12:00:00Z");
        let entries = vec![
            mk("a", "2026-06-30T11:00:00Z"),
            mk("b", "2026-06-30T09:00:00Z"),
            mk("c", "2026-06-29T09:00:00Z"),
        ];
        let rows = timeline_rows(&entries, now);
        // Day(Today), Node(0), Node(1), Day(Yesterday), Node(2)
        assert!(matches!(&rows[0], TlRow::Day(d) if d == "Today"));
        assert!(matches!(rows[1], TlRow::Node(0)));
        assert!(matches!(rows[2], TlRow::Node(1)));
        assert!(matches!(&rows[3], TlRow::Day(d) if d == "Yesterday"));
        assert!(matches!(rows[4], TlRow::Node(2)));
    }

    #[test]
    fn graph_builds_events_entities_relations_and_aliases() {
        let now = ts("2026-06-30T12:00:00Z");
        let entries = vec![
            MemEntry {
                id: "recent".into(),
                content_lower:
                    "success: build graph memory\nTools: Read, Bash\nUse /memory and src/tui/context/memutil.rs"
                        .to_lowercase(),
                tags: vec!["ctx".into(), "rust".into()],
                importance: 0.7,
                timestamp: ts("2026-06-30T10:00:00Z"),
                memory_type: "episodic".into(),
            },
            MemEntry {
                id: "stale".into(),
                content_lower: "old low-value note".into(),
                tags: vec!["ctx".into()],
                importance: 0.2,
                timestamp: ts("2026-02-01T10:00:00Z"),
                memory_type: "episodic".into(),
            },
        ];
        let mut details = BTreeMap::new();
        details.insert(
            "recent".into(),
            MemDetail {
                content:
                    "Success: Build graph memory\nTools: Read, Bash\nUse /memory and src/tui/context/memutil.rs"
                        .into(),
                metadata: BTreeMap::from([
                    ("source".into(), "ctx".into()),
                    ("provider".into(), "Claude".into()),
                    ("ctx_session_id".into(), "session-1".into()),
                ]),
                access_count: 1,
                last_accessed: None,
            },
        );
        details.insert(
            "stale".into(),
            MemDetail {
                content: "old low-value note".into(),
                metadata: BTreeMap::from([
                    ("source".into(), "ctx".into()),
                    ("provider".into(), "claude".into()),
                ]),
                access_count: 0,
                last_accessed: None,
            },
        );

        let graph = build_memory_graph(&entries, &details, now);
        assert_eq!(graph.stats.events, 2);
        assert_eq!(
            graph
                .entities
                .keys()
                .filter(|id| id.starts_with("provider:claude"))
                .count(),
            1,
            "provider entities should dedupe by canonical name"
        );
        let provider = graph.entities.get("provider:claude").unwrap();
        assert_eq!(provider.mentions, 2);
        assert!(
            provider.aliases.iter().any(|alias| alias == "claude"),
            "{provider:?}"
        );
        assert!(graph.entities.contains_key("command:/memory"));
        assert!(graph
            .entities
            .contains_key("file:src/tui/context/memutil.rs"));
        assert!(
            graph
                .relations
                .iter()
                .any(|rel| rel.kind == "used" && rel.to == "tool:read"),
            "tool relation should be extracted from Tools:"
        );

        let recent = graph.by_memory.get("recent").unwrap();
        assert_eq!(recent.tier, MemoryTier::Short);
        assert_eq!(recent.forget, ForgetSignal::Keep);
        let stale = graph.by_memory.get("stale").unwrap();
        assert_eq!(stale.tier, MemoryTier::Long);
        assert_eq!(stale.forget, ForgetSignal::Candidate);
    }

    #[test]
    fn graph_assigns_short_mid_long_and_protected_retention() {
        let now = ts("2026-06-30T12:00:00Z");
        let entries = vec![
            MemEntry {
                id: "short".into(),
                content_lower: "fresh working item".into(),
                tags: vec![],
                importance: 0.4,
                timestamp: ts("2026-06-29T12:00:00Z"),
                memory_type: "working".into(),
            },
            MemEntry {
                id: "mid".into(),
                content_lower: "recent but not fresh".into(),
                tags: vec![],
                importance: 0.5,
                timestamp: ts("2026-06-10T12:00:00Z"),
                memory_type: "episodic".into(),
            },
            MemEntry {
                id: "long".into(),
                content_lower: "stable user preference".into(),
                tags: vec!["keep".into()],
                importance: 0.9,
                timestamp: ts("2026-03-01T12:00:00Z"),
                memory_type: "semantic".into(),
            },
        ];
        let graph = build_memory_graph(&entries, &BTreeMap::new(), now);

        assert_eq!(graph.by_memory["short"].tier, MemoryTier::Short);
        assert_eq!(graph.by_memory["mid"].tier, MemoryTier::Mid);
        assert_eq!(graph.by_memory["long"].tier, MemoryTier::Long);
        assert_eq!(graph.by_memory["long"].forget, ForgetSignal::Protected);
        assert_eq!(graph.stats.short, 1);
        assert_eq!(graph.stats.mid, 1);
        assert_eq!(graph.stats.long, 1);
    }

    #[test]
    fn graph_protects_curated_relation_memories_from_forget_candidates() {
        let now = ts("2026-06-30T12:00:00Z");
        let entries = vec![MemEntry {
            id: "curated".into(),
            content_lower: "old consolidated conflict memory".into(),
            tags: vec!["consolidated".into()],
            importance: 0.1,
            timestamp: ts("2026-01-01T12:00:00Z"),
            memory_type: "semantic".into(),
        }];
        let details = BTreeMap::from([(
            "curated".into(),
            MemDetail {
                content: "Old consolidated conflict memory.".into(),
                metadata: BTreeMap::from([
                    ("supersedes".into(), "old-memory".into()),
                    ("conflicts_with".into(), "legacy-memory".into()),
                ]),
                access_count: 0,
                last_accessed: None,
            },
        )]);

        let graph = build_memory_graph(&entries, &details, now);
        let facet = graph.by_memory.get("curated").unwrap();

        assert_eq!(facet.forget, ForgetSignal::Protected);
        assert_eq!(graph.stats.forget_candidates, 0);
    }

    #[test]
    fn graph_counts_llm_lifecycle_signals() {
        let now = ts("2026-06-30T12:00:00Z");
        let entries = vec![MemEntry {
            id: "llm-1".into(),
            content_lower: "run focused memory tests after extraction changes".into(),
            tags: vec![
                "llm".into(),
                "extracted".into(),
                "consolidated".into(),
                "conflict".into(),
            ],
            importance: 0.85,
            timestamp: ts("2026-06-30T10:00:00Z"),
            memory_type: "procedural".into(),
        }];
        let details = BTreeMap::from([(
            "llm-1".into(),
            MemDetail {
                content: "Run focused memory tests after extraction changes.".into(),
                metadata: BTreeMap::from([
                    ("source".into(), "workflow".into()),
                    ("supersedes".into(), "old-memory".into()),
                    ("conflicts_with".into(), "legacy-memory".into()),
                ]),
                access_count: 0,
                last_accessed: None,
            },
        )]);

        let graph = build_memory_graph(&entries, &details, now);
        let facet = graph.by_memory.get("llm-1").unwrap();

        assert!(facet.llm_extracted);
        assert!(facet.consolidated);
        assert!(facet.conflicts);
        assert_eq!(graph.stats.llm_extracted, 1);
        assert_eq!(graph.stats.consolidated, 1);
        assert_eq!(graph.stats.conflicts, 1);
    }

    #[test]
    fn load_panel_data_reads_details_and_derives_graph() {
        let dir = std::env::temp_dir().join(format!(
            "a3s-memgraph-{}",
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(dir.join("items")).unwrap();
        std::fs::write(
            dir.join("index.json"),
            r#"[
                {"id":"g1","content_lower":"use /memory","tags":["ctx"],"importance":0.7,"timestamp":"2026-06-20T10:00:00Z","memory_type":"episodic"}
            ]"#,
        )
        .unwrap();
        std::fs::write(
            dir.join("items/g1.json"),
            r#"{
                "content":"Use /memory after /ctx save",
                "metadata":{"source":"ctx","provider":"Claude","ctx_event_id":"evt-1"},
                "access_count":2,
                "last_accessed":null
            }"#,
        )
        .unwrap();

        let data = load_panel_data(&dir);
        assert_eq!(data.entries.len(), 1);
        assert!(!data.loaded_from_session);
        assert_eq!(data.details["g1"].metadata["ctx_event_id"], "evt-1");
        assert_eq!(data.graph.stats.events, 1);
        assert!(data.graph.entities.contains_key("source:ctx"));
        assert!(data.graph.entities.contains_key("provider:claude"));
        assert!(data.graph.entities.contains_key("command:/memory"));

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn panel_data_from_memory_items_preserves_live_session_details() {
        let mut item = a3s_memory::MemoryItem::new("Prefer LLM memory value judgment.")
            .with_type(a3s_memory::MemoryType::Semantic)
            .with_importance(0.82)
            .with_tag("llm")
            .with_metadata("source", "session_fallback");
        item.id = "live-1".to_string();
        item.timestamp = ts("2026-06-30T10:00:00Z");
        item.access_count = 2;

        let data = panel_data_from_memory_items(vec![item]);

        assert_eq!(data.entries.len(), 1);
        assert!(data.loaded_from_session);
        assert_eq!(data.entries[0].id, "live-1");
        assert_eq!(data.entries[0].memory_type, "semantic");
        assert_eq!(
            data.details["live-1"].content,
            "Prefer LLM memory value judgment."
        );
        assert_eq!(
            data.details["live-1"].metadata["source"],
            "session_fallback"
        );
        assert_eq!(data.graph.stats.events, 1);
        assert!(data.graph.entities.contains_key("source:session_fallback"));
    }

    fn mk(id: &str, ts_s: &str) -> MemEntry {
        MemEntry {
            id: id.into(),
            content_lower: String::new(),
            tags: vec![],
            importance: 0.5,
            timestamp: ts(ts_s),
            memory_type: "episodic".into(),
        }
    }
}
