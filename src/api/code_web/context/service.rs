use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;

use a3s_boot::{BootError, Result as BootResult};
use a3s_memory::{FileMemoryStore, MemoryItem, MemoryStore, MemoryType};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::process::Command;
use tokio::time::timeout;

use crate::api::code_web::state::CodeWebState;
use crate::config;

const CTX_TIMEOUT: Duration = Duration::from_secs(15);
const DEFAULT_MEMORY_LIMIT: usize = 100;
const MAX_MEMORY_LIMIT: usize = 500;
const DEFAULT_CTX_LIMIT: usize = 8;
const MAX_CTX_LIMIT: usize = 30;

pub(in crate::api::code_web) struct ContextService {
    state: Arc<CodeWebState>,
}

impl ContextService {
    pub(in crate::api::code_web) fn new(state: Arc<CodeWebState>) -> Self {
        Self { state }
    }

    pub(in crate::api::code_web) async fn memory(
        &self,
        query: Option<String>,
        offset: Option<usize>,
        limit: Option<usize>,
        include_graph: Option<bool>,
    ) -> BootResult<serde_json::Value> {
        let root = config::memory_dir();
        let data = load_memory_store(&root).await?;
        let query = query.unwrap_or_default().trim().to_ascii_lowercase();
        let limit = limit
            .unwrap_or(DEFAULT_MEMORY_LIMIT)
            .clamp(1, MAX_MEMORY_LIMIT);
        let matching_entries = data
            .entries
            .iter()
            .filter(|entry| {
                query.is_empty()
                    || memory_search_text(entry, data.details.get(&entry.id))
                        .to_ascii_lowercase()
                        .contains(&query)
            })
            .collect::<Vec<_>>();
        let total = matching_entries.len();
        let (offset, end, has_more) = memory_page_bounds(total, offset.unwrap_or(0), limit);
        let mut entries = matching_entries[offset..end]
            .iter()
            .map(|entry| memory_entry_json(entry, data.details.get(&entry.id)))
            .collect::<Vec<_>>();
        entries
            .sort_by(|left, right| value_str(right, "timestamp").cmp(value_str(left, "timestamp")));
        let returned = entries.len();
        let mut response = json!({
            "root": root.display().to_string(),
            "entries": entries,
            "stats": memory_stats(&data),
            "pagination": {
                "offset": offset,
                "limit": limit,
                "returned": returned,
                "total": total,
                "hasMore": has_more,
            },
        });
        if include_graph.unwrap_or(true) {
            response["graph"] = memory_graph_json(&data);
        }

        Ok(response)
    }

    pub(in crate::api::code_web) async fn memory_detail(
        &self,
        id: String,
    ) -> BootResult<serde_json::Value> {
        validate_memory_id(&id)?;
        let root = config::memory_dir();
        let data = load_memory_store(&root).await?;
        let entry = data
            .entries
            .iter()
            .find(|entry| entry.id == id)
            .ok_or_else(|| BootError::NotFound(format!("memory `{id}` was not found")))?;
        let detail = data.details.get(&entry.id);
        Ok(json!({
            "root": root.display().to_string(),
            "entry": memory_entry_json(entry, detail),
            "detail": detail.map(memory_detail_json).unwrap_or_else(|| json!({})),
        }))
    }

    pub(in crate::api::code_web) fn ctx_status(&self) -> serde_json::Value {
        json!({
            "available": ctx_available(),
        })
    }

    pub(in crate::api::code_web) async fn ctx_search(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let query = required_text(&request, "query")?;
        let limit = optional_usize(&request, "limit")
            .unwrap_or(DEFAULT_CTX_LIMIT)
            .clamp(1, MAX_CTX_LIMIT)
            .to_string();
        let output = run_ctx(&[
            "search",
            "--refresh",
            "off",
            "--limit",
            &limit,
            "--json",
            "--",
            query,
        ])
        .await?;
        let hits = parse_ctx_search_hits(&output)?;
        Ok(json!({
            "query": query,
            "available": true,
            "hits": hits,
        }))
    }

    pub(in crate::api::code_web) async fn ctx_show_event(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let event_id = required_text(&request, "eventId")?;
        let window = optional_usize(&request, "window").unwrap_or(5).clamp(1, 50);
        let output = run_ctx(&["show", "event", event_id, "--window", &window.to_string()]).await?;
        Ok(json!({
            "eventId": event_id,
            "window": window,
            "content": output,
        }))
    }

    pub(in crate::api::code_web) async fn ctx_show_session(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let session_id = required_text(&request, "sessionId")?;
        let output = run_ctx(&["show", "session", session_id]).await?;
        Ok(json!({
            "sessionId": session_id,
            "content": output,
        }))
    }

    pub(in crate::api::code_web) async fn ctx_save_memory(
        &self,
        request: Value,
    ) -> BootResult<serde_json::Value> {
        let request = serde_json::from_value::<CtxSaveMemoryRequest>(request).map_err(|error| {
            BootError::BadRequest(format!("invalid ctx memory request: {error}"))
        })?;
        let item = ctx_memory_item(&request)?;
        let storage = self
            .store_ctx_memory(request.web_session_id.as_deref(), item.clone())
            .await?;

        Ok(json!({
            "saved": true,
            "storage": storage,
            "memory": memory_item_json(&item),
        }))
    }

    pub(in crate::api::code_web) async fn top(&self) -> BootResult<serde_json::Value> {
        let rows = crate::top::collect_processes().await.map_err(|error| {
            BootError::Internal(format!("failed to collect processes: {error}"))
        })?;
        let agents = rows.iter().filter(|row| row.agent.is_some()).count();
        let high_risk = rows
            .iter()
            .filter(|row| row.risk == crate::top::Risk::High)
            .count();
        let cwd = self.state.default_workspace.display().to_string();

        Ok(json!({
            "workspaceRoot": cwd,
            "processes": rows.len(),
            "agents": agents,
            "highRisk": high_risk,
            "rows": rows
                .into_iter()
                .map(process_row_json)
                .collect::<Vec<_>>(),
        }))
    }

    async fn store_ctx_memory(
        &self,
        web_session_id: Option<&str>,
        item: MemoryItem,
    ) -> BootResult<&'static str> {
        if let Some(session_id) = web_session_id
            .map(str::trim)
            .filter(|session_id| !session_id.is_empty())
        {
            let session = self.state.sessions.lock().await.get(session_id).cloned();
            if let Some(memory) = session.and_then(|session| session.memory().cloned()) {
                memory
                    .remember(item)
                    .await
                    .map_err(|error| BootError::Internal(error.to_string()))?;
                return Ok("session");
            }
        }

        let store = FileMemoryStore::new(config::memory_dir())
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        MemoryStore::store(&store, item)
            .await
            .map_err(|error| BootError::Internal(error.to_string()))?;
        Ok("file")
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MemoryEntry {
    id: String,
    #[serde(default)]
    #[serde(alias = "contentLower")]
    content_lower: String,
    #[serde(default)]
    tags: Vec<String>,
    #[serde(default)]
    importance: f32,
    #[serde(default)]
    timestamp: String,
    #[serde(default)]
    #[serde(alias = "memoryType")]
    memory_type: String,
}

#[derive(Debug, Clone, Default, Deserialize)]
struct MemoryDetail {
    #[serde(default)]
    content: String,
    #[serde(default)]
    metadata: BTreeMap<String, String>,
    #[serde(default)]
    #[serde(alias = "accessCount")]
    access_count: u32,
    #[serde(default)]
    #[serde(alias = "lastAccessed")]
    last_accessed: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct CtxSaveMemoryRequest {
    event_id: Option<String>,
    session_id: Option<String>,
    provider: Option<String>,
    time: Option<String>,
    timestamp: Option<String>,
    title: Option<String>,
    snippet: Option<String>,
    web_session_id: Option<String>,
}

#[derive(Default)]
struct MemoryStoreData {
    entries: Vec<MemoryEntry>,
    details: BTreeMap<String, MemoryDetail>,
}

async fn load_memory_store(root: &Path) -> BootResult<MemoryStoreData> {
    let mut entries = match tokio::fs::read_to_string(root.join("index.json")).await {
        Ok(content) => serde_json::from_str::<Vec<MemoryEntry>>(&content).map_err(|error| {
            BootError::Internal(format!("failed to parse memory index.json: {error}"))
        })?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(fs_error(error)),
    };
    entries.sort_by(|left, right| right.timestamp.cmp(&left.timestamp));

    let mut details = BTreeMap::new();
    for entry in &entries {
        if let Some(detail) = load_memory_detail(root, &entry.id).await? {
            details.insert(entry.id.clone(), detail);
        }
    }

    Ok(MemoryStoreData { entries, details })
}

async fn load_memory_detail(root: &Path, id: &str) -> BootResult<Option<MemoryDetail>> {
    validate_memory_id(id)?;
    let path = root.join("items").join(format!("{id}.json"));
    match tokio::fs::read_to_string(path).await {
        Ok(content) => serde_json::from_str::<MemoryDetail>(&content)
            .map(Some)
            .map_err(|error| {
                BootError::Internal(format!("failed to parse memory item {id}: {error}"))
            }),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(fs_error(error)),
    }
}

fn memory_page_bounds(total: usize, offset: usize, limit: usize) -> (usize, usize, bool) {
    let start = offset.min(total);
    let end = start.saturating_add(limit).min(total);
    (start, end, end < total)
}

fn memory_stats(data: &MemoryStoreData) -> serde_json::Value {
    let mut types = BTreeMap::<String, usize>::new();
    let mut tags = BTreeSet::<String>::new();
    let mut important = 0usize;
    let mut ctx_sources = 0usize;

    for entry in &data.entries {
        *types
            .entry(if entry.memory_type.is_empty() {
                "memory".to_string()
            } else {
                entry.memory_type.clone()
            })
            .or_default() += 1;
        if entry.importance >= 0.7 {
            important += 1;
        }
        for tag in &entry.tags {
            tags.insert(tag.clone());
        }
        if data
            .details
            .get(&entry.id)
            .and_then(|detail| detail.metadata.get("source"))
            .is_some_and(|source| source == "ctx")
        {
            ctx_sources += 1;
        }
    }

    json!({
        "entries": data.entries.len(),
        "types": types,
        "tags": tags.len(),
        "important": important,
        "ctxSources": ctx_sources,
    })
}

fn memory_graph_json(data: &MemoryStoreData) -> serde_json::Value {
    let now = chrono::Utc::now();
    let mut entities = BTreeMap::<String, GraphEntity>::new();
    let mut relations = Vec::<GraphRelation>::new();
    let mut events = Vec::<serde_json::Value>::new();
    let mut facets = serde_json::Map::new();
    let mut tier_counts =
        BTreeMap::<&'static str, usize>::from([("short", 0), ("mid", 0), ("long", 0)]);
    let mut forget_candidates = 0usize;
    let mut llm_extracted = 0usize;
    let mut consolidated = 0usize;
    let mut conflicts = 0usize;

    for entry in &data.entries {
        let detail = data.details.get(&entry.id);
        let timestamp = parse_memory_timestamp(&entry.timestamp).unwrap_or(now);
        let content = detail
            .map(|detail| detail.content.trim())
            .filter(|content| !content.is_empty())
            .unwrap_or(entry.content_lower.trim());
        let source = detail
            .and_then(|detail| detail.metadata.get("source").cloned())
            .or_else(|| entry.tags.first().cloned())
            .unwrap_or_else(|| "memory".to_string());
        let lifecycle = memory_lifecycle(entry, detail);
        let retention = memory_retention(entry, detail, timestamp, now);
        *tier_counts.entry(retention.tier).or_default() += 1;
        if retention.forget == "candidate" {
            forget_candidates += 1;
        }
        if lifecycle.llm_extracted {
            llm_extracted += 1;
        }
        if lifecycle.consolidated {
            consolidated += 1;
        }
        if lifecycle.conflicts {
            conflicts += 1;
        }

        let event_id = format!("event:{}", entry.id);
        let mut entity_ids = BTreeSet::<String>::new();
        let mut relation_ids = Vec::<usize>::new();
        if let Some(source_id) = add_graph_entity(
            &mut entities,
            "source",
            &source,
            None,
            entry,
            timestamp,
            &mut entity_ids,
        ) {
            push_graph_relation(
                &mut relations,
                &mut relation_ids,
                &event_id,
                &source_id,
                "from",
                &entry.id,
                1.0,
            );
        }

        for tag in &entry.tags {
            if let Some(tag_id) = add_graph_entity(
                &mut entities,
                "tag",
                tag,
                None,
                entry,
                timestamp,
                &mut entity_ids,
            ) {
                push_graph_relation(
                    &mut relations,
                    &mut relation_ids,
                    &event_id,
                    &tag_id,
                    "tagged",
                    &entry.id,
                    0.8,
                );
            }
        }

        if let Some(detail) = detail {
            for (key, value) in &detail.metadata {
                add_metadata_graph_entities(
                    &mut entities,
                    &mut relations,
                    &mut entity_ids,
                    &mut relation_ids,
                    &event_id,
                    entry,
                    timestamp,
                    key,
                    value,
                );
            }
        }

        for extracted in extract_graph_entities(content) {
            if let Some(entity_id) = add_graph_entity(
                &mut entities,
                extracted.kind,
                &extracted.name,
                None,
                entry,
                timestamp,
                &mut entity_ids,
            ) {
                push_graph_relation(
                    &mut relations,
                    &mut relation_ids,
                    &event_id,
                    &entity_id,
                    extracted.relation,
                    &entry.id,
                    extracted.weight,
                );
            }
        }

        let ids = entity_ids.into_iter().collect::<Vec<_>>();
        for (index, from) in ids.iter().take(10).enumerate() {
            for to in ids.iter().take(10).skip(index + 1) {
                push_graph_relation(
                    &mut relations,
                    &mut relation_ids,
                    from,
                    to,
                    "co-occurs",
                    &entry.id,
                    0.35,
                );
            }
        }

        events.push(json!({
            "id": event_id,
            "memoryId": entry.id,
            "label": event_label(content, &entry.id),
            "source": source,
            "tier": retention.tier,
            "forget": retention.forget,
            "retentionScore": retention.score,
            "timestamp": entry.timestamp,
            "entityIds": ids,
        }));
        facets.insert(
            entry.id.clone(),
            json!({
                "eventId": format!("event:{}", entry.id),
                "tier": retention.tier,
                "forget": retention.forget,
                "retentionScore": retention.score,
                "llmExtracted": lifecycle.llm_extracted,
                "consolidated": lifecycle.consolidated,
                "conflicts": lifecycle.conflicts,
                "entityIds": ids,
                "relationIds": relation_ids,
            }),
        );
    }

    let entity_items = entities
        .into_iter()
        .map(|(id, entity)| {
            json!({
                "id": id,
                "kind": entity.kind,
                "name": entity.name,
                "aliases": entity.aliases.into_iter().collect::<Vec<_>>(),
                "mentions": entity.mentions,
                "importance": entity.importance,
                "firstSeen": entity.first_seen,
                "lastSeen": entity.last_seen,
                "memoryIds": entity.memory_ids.into_iter().collect::<Vec<_>>(),
            })
        })
        .collect::<Vec<_>>();
    let alias_count = entity_items
        .iter()
        .filter_map(|entity| entity.get("aliases").and_then(Value::as_array))
        .map(Vec::len)
        .sum::<usize>();
    let relation_items = relations
        .into_iter()
        .enumerate()
        .map(|(id, relation)| {
            json!({
                "id": id,
                "from": relation.from,
                "to": relation.to,
                "kind": relation.kind,
                "memoryId": relation.memory_id,
                "weight": relation.weight,
            })
        })
        .collect::<Vec<_>>();

    json!({
        "stats": {
            "events": events.len(),
            "entities": entity_items.len(),
            "relations": relation_items.len(),
            "aliases": alias_count,
            "short": tier_counts.get("short").copied().unwrap_or(0),
            "mid": tier_counts.get("mid").copied().unwrap_or(0),
            "long": tier_counts.get("long").copied().unwrap_or(0),
            "forgetCandidates": forget_candidates,
            "llmExtracted": llm_extracted,
            "consolidated": consolidated,
            "conflicts": conflicts,
        },
        "events": events,
        "entities": entity_items,
        "relations": relation_items,
        "facets": Value::Object(facets),
    })
}

#[derive(Debug, Default)]
struct GraphEntity {
    kind: String,
    name: String,
    aliases: BTreeSet<String>,
    mentions: usize,
    importance: f32,
    first_seen: Option<String>,
    last_seen: Option<String>,
    memory_ids: BTreeSet<String>,
}

#[derive(Debug)]
struct GraphRelation {
    from: String,
    to: String,
    kind: String,
    memory_id: String,
    weight: f32,
}

#[derive(Debug, Clone, Copy)]
struct MemoryRetention {
    tier: &'static str,
    forget: &'static str,
    score: f32,
}

#[derive(Debug, Clone, Copy, Default)]
struct MemoryLifecycle {
    llm_extracted: bool,
    consolidated: bool,
    conflicts: bool,
}

#[derive(Debug)]
struct ExtractedGraphEntity {
    kind: &'static str,
    name: String,
    relation: &'static str,
    weight: f32,
}

fn add_graph_entity(
    entities: &mut BTreeMap<String, GraphEntity>,
    kind: &str,
    name: &str,
    alias: Option<&str>,
    entry: &MemoryEntry,
    timestamp: chrono::DateTime<chrono::Utc>,
    entity_ids: &mut BTreeSet<String>,
) -> Option<String> {
    let canonical = canonical_key(name);
    if canonical.is_empty() {
        return None;
    }
    let id = format!("{kind}:{canonical}");
    let display = display_entity_name(name);
    let timestamp = timestamp.to_rfc3339();
    let entity = entities.entry(id.clone()).or_insert_with(|| GraphEntity {
        kind: kind.to_string(),
        name: display.clone(),
        first_seen: Some(timestamp.clone()),
        last_seen: Some(timestamp.clone()),
        ..GraphEntity::default()
    });
    entity.mentions += 1;
    entity.importance = entity.importance.max(entry.importance);
    entity.first_seen = min_timestamp_string(entity.first_seen.take(), &timestamp);
    entity.last_seen = max_timestamp_string(entity.last_seen.take(), &timestamp);
    entity.memory_ids.insert(entry.id.clone());
    if display != entity.name {
        entity.aliases.insert(display);
    }
    if let Some(alias) = alias {
        let alias = display_entity_name(alias);
        if !alias.is_empty() && alias != entity.name {
            entity.aliases.insert(alias);
        }
    }
    entity_ids.insert(id.clone());
    Some(id)
}

#[allow(clippy::too_many_arguments)]
fn add_metadata_graph_entities(
    entities: &mut BTreeMap<String, GraphEntity>,
    relations: &mut Vec<GraphRelation>,
    entity_ids: &mut BTreeSet<String>,
    relation_ids: &mut Vec<usize>,
    event_id: &str,
    entry: &MemoryEntry,
    timestamp: chrono::DateTime<chrono::Utc>,
    key: &str,
    value: &str,
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
        "source" => ("source", "from"),
        "tools" => {
            for tool in split_graph_list(value) {
                if let Some(tool_id) =
                    add_graph_entity(entities, "tool", tool, None, entry, timestamp, entity_ids)
                {
                    push_graph_relation(
                        relations,
                        relation_ids,
                        event_id,
                        &tool_id,
                        "used",
                        &entry.id,
                        0.9,
                    );
                }
            }
            return;
        }
        "aliases" | "entity_aliases" => {
            let canonical = event_label(&entry.content_lower, &entry.id);
            let topic_id = add_graph_entity(
                entities, "topic", &canonical, None, entry, timestamp, entity_ids,
            );
            for alias in split_graph_list(value) {
                let _ = add_graph_entity(
                    entities,
                    "topic",
                    &canonical,
                    Some(alias),
                    entry,
                    timestamp,
                    entity_ids,
                );
            }
            if let Some(topic_id) = topic_id {
                push_graph_relation(
                    relations,
                    relation_ids,
                    event_id,
                    &topic_id,
                    "aliases",
                    &entry.id,
                    0.7,
                );
            }
            return;
        }
        _ => return,
    };
    if let Some(entity_id) =
        add_graph_entity(entities, kind, value, None, entry, timestamp, entity_ids)
    {
        push_graph_relation(
            relations,
            relation_ids,
            event_id,
            &entity_id,
            relation,
            &entry.id,
            0.8,
        );
    }
}

fn push_graph_relation(
    relations: &mut Vec<GraphRelation>,
    relation_ids: &mut Vec<usize>,
    from: &str,
    to: &str,
    kind: &str,
    memory_id: &str,
    weight: f32,
) {
    let id = relations.len();
    relations.push(GraphRelation {
        from: from.to_string(),
        to: to.to_string(),
        kind: kind.to_string(),
        memory_id: memory_id.to_string(),
        weight,
    });
    relation_ids.push(id);
}

fn memory_lifecycle(entry: &MemoryEntry, detail: Option<&MemoryDetail>) -> MemoryLifecycle {
    let has_tag = |needle: &str| entry.tags.iter().any(|tag| tag == needle);
    let source = detail
        .and_then(|detail| detail.metadata.get("source"))
        .map(|value| value.trim())
        .unwrap_or_default();
    let metadata_nonempty = |key: &str| {
        detail
            .and_then(|detail| detail.metadata.get(key))
            .is_some_and(|value| !value.trim().is_empty())
    };
    MemoryLifecycle {
        llm_extracted: has_tag("llm")
            || has_tag("extracted")
            || matches!(
                source,
                "llm_extractor"
                    | "project_fact"
                    | "workflow"
                    | "failure"
                    | "preference"
                    | "decision"
            ),
        consolidated: has_tag("consolidated") || metadata_nonempty("supersedes"),
        conflicts: has_tag("conflict") || metadata_nonempty("conflicts_with"),
    }
}

fn memory_retention(
    entry: &MemoryEntry,
    detail: Option<&MemoryDetail>,
    timestamp: chrono::DateTime<chrono::Utc>,
    now: chrono::DateTime<chrono::Utc>,
) -> MemoryRetention {
    let age_days = ((now - timestamp).num_seconds().max(0) as f32) / 86_400.0;
    let recency = (-age_days / 30.0).exp();
    let access_count = detail.map(|detail| detail.access_count).unwrap_or(0);
    let access = (((access_count as f32) + 1.0).ln() / 3.0_f32.ln()).clamp(0.0, 1.0);
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
        "protected"
    } else if age_days > 90.0 && entry.importance < 0.45 && access_count == 0 && score < 0.35 {
        "candidate"
    } else if age_days > 30.0 && entry.importance < 0.60 && score < 0.45 {
        "cooling"
    } else {
        "keep"
    };
    let tier = if entry.memory_type == "working" || age_days <= 7.0 {
        "short"
    } else if protected
        || (matches!(entry.memory_type.as_str(), "semantic" | "procedural")
            && entry.importance >= 0.65)
        || age_days > 45.0
    {
        "long"
    } else {
        "mid"
    };
    MemoryRetention {
        tier,
        forget,
        score,
    }
}

fn memory_is_prune_protected(entry: &MemoryEntry, detail: Option<&MemoryDetail>) -> bool {
    entry.importance >= 0.80
        || detail
            .map(|detail| detail.access_count >= 3)
            .unwrap_or(false)
        || entry.tags.iter().any(|tag| {
            matches!(
                tag.as_str(),
                "keep" | "pinned" | "protected" | "consolidated" | "conflict"
            )
        })
        || metadata_truthy(detail, "keep")
        || metadata_truthy(detail, "pinned")
        || metadata_truthy(detail, "protected")
        || metadata_nonempty(detail, "supersedes")
        || metadata_nonempty(detail, "conflicts_with")
}

fn metadata_truthy(detail: Option<&MemoryDetail>, key: &str) -> bool {
    detail
        .and_then(|detail| detail.metadata.get(key))
        .map(|value| {
            matches!(
                value.trim().to_ascii_lowercase().as_str(),
                "1" | "true" | "yes" | "keep" | "pinned" | "protected"
            )
        })
        .unwrap_or(false)
}

fn metadata_nonempty(detail: Option<&MemoryDetail>, key: &str) -> bool {
    detail
        .and_then(|detail| detail.metadata.get(key))
        .is_some_and(|value| !value.trim().is_empty())
}

fn extract_graph_entities(content: &str) -> Vec<ExtractedGraphEntity> {
    let mut out = Vec::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("Tools:") {
            for tool in split_graph_list(rest) {
                out.push(ExtractedGraphEntity {
                    kind: "tool",
                    name: tool.to_string(),
                    relation: "used",
                    weight: 0.9,
                });
            }
        }
        if trimmed.starts_with("Success:") {
            out.push(extracted_graph_entity(
                "outcome",
                "success",
                "resulted-in",
                0.8,
            ));
        } else if trimmed.starts_with("Failure:") {
            out.push(extracted_graph_entity(
                "outcome",
                "failure",
                "resulted-in",
                0.9,
            ));
        }
        for raw in trimmed.split_whitespace() {
            let token = raw.trim_matches(|ch: char| {
                matches!(ch, ',' | ';' | ':' | '"' | '\'' | ')' | '(' | '[' | ']')
            });
            if token.starts_with("https://") || token.starts_with("http://") {
                out.push(extracted_graph_entity("url", token, "references", 0.7));
            } else if is_slash_command(token) {
                out.push(extracted_graph_entity("command", token, "mentions", 0.65));
            } else if looks_like_path(token) {
                out.push(extracted_graph_entity("file", token, "touches", 0.7));
            }
        }
    }
    dedupe_graph_entities(out)
}

fn extracted_graph_entity(
    kind: &'static str,
    name: impl Into<String>,
    relation: &'static str,
    weight: f32,
) -> ExtractedGraphEntity {
    ExtractedGraphEntity {
        kind,
        name: name.into(),
        relation,
        weight,
    }
}

fn dedupe_graph_entities(items: Vec<ExtractedGraphEntity>) -> Vec<ExtractedGraphEntity> {
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

fn split_graph_list(value: &str) -> impl Iterator<Item = &str> {
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
        if ch.is_alphanumeric() || matches!(ch, '/' | '_' | '-' | '.' | ':' | '@') {
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

fn min_timestamp_string(current: Option<String>, candidate: &str) -> Option<String> {
    Some(match current {
        Some(current) if current.as_str() <= candidate => current,
        _ => candidate.to_string(),
    })
}

fn max_timestamp_string(current: Option<String>, candidate: &str) -> Option<String> {
    Some(match current {
        Some(current) if current.as_str() >= candidate => current,
        _ => candidate.to_string(),
    })
}

fn parse_memory_timestamp(value: &str) -> Option<chrono::DateTime<chrono::Utc>> {
    chrono::DateTime::parse_from_rfc3339(value)
        .map(|timestamp| timestamp.with_timezone(&chrono::Utc))
        .ok()
}

fn is_slash_command(token: &str) -> bool {
    let rest = token.strip_prefix('/').unwrap_or_default();
    !rest.is_empty()
        && rest
            .chars()
            .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_')
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

fn memory_entry_json(entry: &MemoryEntry, detail: Option<&MemoryDetail>) -> serde_json::Value {
    let content = detail
        .map(|detail| detail.content.trim())
        .filter(|content| !content.is_empty())
        .unwrap_or(entry.content_lower.trim());
    json!({
        "id": entry.id,
        "content": content,
        "preview": content.chars().take(240).collect::<String>(),
        "tags": entry.tags,
        "importance": entry.importance,
        "timestamp": entry.timestamp,
        "memoryType": if entry.memory_type.is_empty() { "memory" } else { entry.memory_type.as_str() },
        "metadata": detail.map(|detail| &detail.metadata),
        "accessCount": detail.map(|detail| detail.access_count).unwrap_or(0),
        "lastAccessed": detail.and_then(|detail| detail.last_accessed.clone()),
    })
}

fn memory_detail_json(detail: &MemoryDetail) -> serde_json::Value {
    json!({
        "content": detail.content,
        "metadata": detail.metadata,
        "accessCount": detail.access_count,
        "lastAccessed": detail.last_accessed,
    })
}

fn memory_item_json(item: &MemoryItem) -> serde_json::Value {
    json!({
        "id": item.id.clone(),
        "content": item.content.clone(),
        "preview": item.content.chars().take(240).collect::<String>(),
        "tags": item.tags.clone(),
        "importance": item.importance,
        "timestamp": item.timestamp.to_rfc3339(),
        "memoryType": memory_type_label(&item.memory_type),
        "metadata": item.metadata.clone(),
        "accessCount": item.access_count,
        "lastAccessed": item.last_accessed.as_ref().map(|timestamp| timestamp.to_rfc3339()),
    })
}

fn memory_type_label(memory_type: &MemoryType) -> &'static str {
    match memory_type {
        MemoryType::Episodic => "episodic",
        MemoryType::Semantic => "semantic",
        MemoryType::Procedural => "procedural",
        MemoryType::Working => "working",
    }
}

fn memory_search_text(entry: &MemoryEntry, detail: Option<&MemoryDetail>) -> String {
    let content = detail
        .map(|detail| detail.content.as_str())
        .filter(|content| !content.trim().is_empty())
        .unwrap_or(entry.content_lower.as_str());
    let metadata = detail
        .map(|detail| {
            detail
                .metadata
                .iter()
                .map(|(key, value)| format!("{key} {value}"))
                .collect::<Vec<_>>()
                .join(" ")
        })
        .unwrap_or_default();
    format!(
        "{} {} {} {metadata}",
        entry.id,
        entry.tags.join(" "),
        content
    )
}

fn process_row_json(row: crate::top::ProcessRow) -> serde_json::Value {
    json!({
        "pid": row.pid,
        "ppid": row.ppid,
        "cpuPct": row.cpu_pct,
        "memPct": row.mem_pct,
        "elapsed": row.elapsed,
        "cwd": row.cwd,
        "command": row.command,
        "agent": row.agent.map(|agent| agent.label()),
        "risk": row.risk.label(),
    })
}

async fn run_ctx(args: &[&str]) -> BootResult<String> {
    let output = timeout(
        CTX_TIMEOUT,
        Command::new("ctx")
            .args(args)
            .stdin(std::process::Stdio::null())
            .output(),
    )
    .await
    .map_err(|_| {
        BootError::Internal(format!(
            "ctx timed out after {} seconds",
            CTX_TIMEOUT.as_secs()
        ))
    })?
    .map_err(|error| BootError::Internal(format!("failed to run ctx: {error}")))?;

    if !output.status.success() {
        return Err(BootError::BadRequest(
            String::from_utf8_lossy(&output.stderr).trim().to_string(),
        ));
    }

    Ok(String::from_utf8_lossy(&output.stdout).to_string())
}

fn ctx_available() -> bool {
    std::process::Command::new("ctx")
        .arg("--version")
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .is_ok_and(|status| status.success())
}

fn parse_ctx_search_hits(output: &str) -> BootResult<Vec<serde_json::Value>> {
    let value = serde_json::from_str::<Value>(output).map_err(|error| {
        BootError::Internal(format!("failed to parse ctx search JSON: {error}"))
    })?;
    let results = value
        .get("results")
        .and_then(Value::as_array)
        .ok_or_else(|| BootError::Internal("ctx search JSON has no results field".to_string()))?;
    Ok(results.iter().filter_map(ctx_hit_json).collect::<Vec<_>>())
}

fn ctx_hit_json(value: &Value) -> Option<serde_json::Value> {
    let event_id = json_text(value, "ctx_event_id");
    if event_id.is_empty() {
        return None;
    }
    Some(json!({
        "eventId": event_id,
        "sessionId": json_text(value, "ctx_session_id"),
        "provider": flatten_text(&json_text(value, "provider")),
        "timestamp": json_text(value, "timestamp"),
        "time": json_text(value, "timestamp").chars().take(10).collect::<String>(),
        "title": flatten_text(&json_text(value, "title")),
        "snippet": flatten_text(&json_text(value, "snippet")),
    }))
}

fn ctx_memory_item(request: &CtxSaveMemoryRequest) -> BootResult<MemoryItem> {
    let event_id = request
        .event_id
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BootError::BadRequest("eventId is required".to_string()))?
        .to_string();
    let provider = request_text(request.provider.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "ctx".to_string());
    let title = request_text(request.title.as_deref())
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| event_id.clone());
    let snippet = request_text(request.snippet.as_deref()).unwrap_or_default();
    let session_id = request_text(request.session_id.as_deref()).unwrap_or_default();
    let time =
        request_text(request.time.as_deref().or(request.timestamp.as_deref())).unwrap_or_default();
    let content = if snippet.is_empty() {
        format!("[from past session] {title}")
    } else {
        format!("[from past session] {title} - {snippet}")
    };

    let mut item = MemoryItem::new(content)
        .with_type(MemoryType::Episodic)
        .with_importance(0.7)
        .with_tags(vec!["ctx".to_string(), provider.clone()])
        .with_metadata("source", "ctx")
        .with_metadata("ctx_event_id", event_id)
        .with_metadata("provider", provider);
    if !session_id.is_empty() {
        item = item.with_metadata("ctx_session_id", session_id);
    }
    if !time.is_empty() {
        item = item.with_metadata("ctx_time", time);
    }
    Ok(item)
}

fn request_text(value: Option<&str>) -> Option<String> {
    value.map(flatten_text).filter(|value| !value.is_empty())
}

fn json_text(value: &Value, key: &str) -> String {
    value
        .get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn flatten_text(value: &str) -> String {
    strip_controls(value)
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn strip_controls(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' {
            while chars.peek().is_some_and(|next| !next.is_alphabetic()) {
                chars.next();
            }
            chars.next();
        } else if ch == '\n' || ch == '\t' || !ch.is_control() {
            out.push(ch);
        }
    }
    out
}

fn required_text<'a>(request: &'a Value, name: &str) -> BootResult<&'a str> {
    request
        .get(name)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| BootError::BadRequest(format!("{name} is required")))
}

fn optional_usize(request: &Value, name: &str) -> Option<usize> {
    match request.get(name) {
        Some(Value::Number(number)) => number.as_u64().map(|value| value as usize),
        Some(Value::String(value)) => value.trim().parse().ok(),
        _ => None,
    }
}

fn validate_memory_id(id: &str) -> BootResult<()> {
    if id.contains('/') || id.contains('\\') || id.contains("..") || id.trim().is_empty() {
        return Err(BootError::BadRequest("invalid memory id".to_string()));
    }
    Ok(())
}

fn value_str<'a>(value: &'a Value, key: &str) -> &'a str {
    value.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn fs_error(error: std::io::Error) -> BootError {
    match error.kind() {
        std::io::ErrorKind::PermissionDenied => BootError::Forbidden(error.to_string()),
        std::io::ErrorKind::InvalidInput | std::io::ErrorKind::InvalidData => {
            BootError::BadRequest(error.to_string())
        }
        _ => BootError::Io(error),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ctx_memory_item_matches_tui_promotion_shape() {
        let item = ctx_memory_item(&CtxSaveMemoryRequest {
            event_id: Some("event-1".to_string()),
            session_id: Some("session-1".to_string()),
            provider: Some("codex".to_string()),
            time: Some("2026-07-07".to_string()),
            timestamp: None,
            title: Some("Prior plan".to_string()),
            snippet: Some("Use the code_web API split".to_string()),
            web_session_id: None,
        })
        .expect("ctx memory");

        assert_eq!(item.memory_type, MemoryType::Episodic);
        assert_eq!(item.importance, 0.7);
        assert_eq!(item.tags, vec!["ctx".to_string(), "codex".to_string()]);
        assert!(item.content.contains("[from past session] Prior plan"));
        assert!(item.content.contains("Use the code_web API split"));
        assert_eq!(item.metadata.get("source").map(String::as_str), Some("ctx"));
        assert_eq!(
            item.metadata.get("ctx_event_id").map(String::as_str),
            Some("event-1")
        );
        assert_eq!(
            item.metadata.get("ctx_session_id").map(String::as_str),
            Some("session-1")
        );
    }

    #[test]
    fn memory_graph_json_builds_tui_like_graph_facets() {
        let data = MemoryStoreData {
            entries: vec![
                MemoryEntry {
                    id: "recent".to_string(),
                    content_lower: "success: build web graph\nTools: Read, Bash\nUse /memory and apps/web/src/desktop/pages/context/ContextPage.tsx".to_lowercase(),
                    tags: vec!["ctx".to_string(), "rust".to_string()],
                    importance: 0.7,
                    timestamp: chrono::Utc::now().to_rfc3339(),
                    memory_type: "episodic".to_string(),
                },
                MemoryEntry {
                    id: "stale".to_string(),
                    content_lower: "old low-value note".to_string(),
                    tags: vec!["ctx".to_string()],
                    importance: 0.2,
                    timestamp: "2025-01-01T00:00:00Z".to_string(),
                    memory_type: "episodic".to_string(),
                },
            ],
            details: BTreeMap::from([
                (
                    "recent".to_string(),
                    MemoryDetail {
                        content: "Success: Build web graph\nTools: Read, Bash\nUse /memory and apps/web/src/desktop/pages/context/ContextPage.tsx".to_string(),
                        metadata: BTreeMap::from([
                            ("source".to_string(), "ctx".to_string()),
                            ("provider".to_string(), "Codex".to_string()),
                            ("ctx_session_id".to_string(), "session-1".to_string()),
                            ("prompt".to_string(), "分析记忆图谱".to_string()),
                            ("error".to_string(), "！！！".to_string()),
                        ]),
                        access_count: 1,
                        last_accessed: None,
                    },
                ),
                (
                    "stale".to_string(),
                    MemoryDetail {
                        content: "old low-value note".to_string(),
                        metadata: BTreeMap::from([
                            ("source".to_string(), "ctx".to_string()),
                            ("provider".to_string(), "codex".to_string()),
                        ]),
                        access_count: 0,
                        last_accessed: None,
                    },
                ),
            ]),
        };

        let graph = memory_graph_json(&data);
        assert_eq!(graph["stats"]["events"], 2);
        assert!(graph["stats"]["entities"].as_u64().unwrap() >= 5);
        assert!(graph["stats"]["relations"].as_u64().unwrap() >= 5);
        assert_eq!(graph["facets"]["recent"]["tier"], "short");
        assert_eq!(graph["facets"]["stale"]["forget"], "candidate");
        let entities = graph["entities"].as_array().unwrap();
        assert!(entities
            .iter()
            .any(|entity| entity["id"] == "provider:codex" && entity["mentions"] == 2));
        assert!(entities
            .iter()
            .any(|entity| entity["id"] == "command:/memory"));
        assert!(entities
            .iter()
            .any(|entity| entity["id"] == "prompt:分析记忆图谱"));
        assert!(entities.iter().any(|entity| {
            entity["id"] == "file:apps/web/src/desktop/pages/context/contextpage.tsx"
        }));
        let node_ids = graph["events"]
            .as_array()
            .unwrap()
            .iter()
            .chain(entities.iter())
            .map(|node| node["id"].as_str().unwrap())
            .collect::<BTreeSet<_>>();
        for relation in graph["relations"].as_array().unwrap() {
            assert!(node_ids.contains(relation["from"].as_str().unwrap()));
            assert!(node_ids.contains(relation["to"].as_str().unwrap()));
        }
    }

    #[test]
    fn flatten_text_strips_terminal_controls() {
        assert_eq!(flatten_text("\u{1b}[31mred\u{1b}[0m\ntext"), "red text");
    }

    #[test]
    fn memory_page_bounds_handles_final_and_out_of_range_pages() {
        assert_eq!(memory_page_bounds(501, 0, 500), (0, 500, true));
        assert_eq!(memory_page_bounds(501, 500, 500), (500, 501, false));
        assert_eq!(memory_page_bounds(501, 900, 500), (501, 501, false));
    }
}
