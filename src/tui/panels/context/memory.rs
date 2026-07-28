//! `/memory` panel: a graph view of the agent's durable memory.
//!
//! Left column is the event timeline, annotated with memory type, lifecycle
//! tier, and forgetting state. Right column shows the selected memory's
//! metadata, graph neighborhood, aliases, relations, and full content.

use super::super::*;
use a3s_tui::components::{
    divider_line_with, Badge, DetailPanel, DetailRow, Paragraph, Progress, Timeline, TimelineItem,
    TimelineRow,
};

const MEMORY_PANEL_SESSION_LIMIT: usize = 1_000;

/// Short badge + accent colour for a memory type.
fn mem_type_style(t: &str) -> (&'static str, Color) {
    match t {
        "semantic" => ("sem", TN_CYAN),
        "procedural" => ("proc", TN_GREEN),
        "working" => ("work", TN_GRAY),
        _ => ("epis", TN_YELLOW), // episodic / unknown
    }
}

fn tier_style(tier: MemoryTier) -> Color {
    match tier {
        MemoryTier::Short => TN_GREEN,
        MemoryTier::Mid => TN_CYAN,
        MemoryTier::Long => TN_YELLOW,
    }
}

fn forget_mark(signal: ForgetSignal) -> &'static str {
    match signal {
        ForgetSignal::Keep => " ",
        ForgetSignal::Cooling => "~",
        ForgetSignal::Candidate => "!",
        ForgetSignal::Protected => "*",
    }
}

fn importance_bar(importance: f32, color: Color) -> String {
    // Keep 5-cell rounding aligned with the former f32 bar math.
    Progress::new()
        .value(f64::from(importance.clamp(0.0, 1.0)) + 1e-7)
        .width(5)
        .filled_char('▰')
        .empty_char('▱')
        .filled_color(color)
        .empty_color(TN_GRAY)
        .show_percentage(false)
        .view()
}

fn memory_line(rendered: &str, width: usize) -> String {
    pad_to(&truncate(rendered, width), width)
}

fn memory_detail_metadata_lines(
    e: &MemEntry,
    detail: &MemDetail,
    graph: &MemoryGraph,
    now: chrono::DateTime<chrono::Utc>,
    width: usize,
) -> Vec<String> {
    if width == 0 {
        return Vec::new();
    }

    let (_, color) = mem_type_style(&e.memory_type);
    let ty = if e.memory_type.is_empty() {
        "memory"
    } else {
        &e.memory_type
    };
    let header = format!(
        "{} {} {}",
        Badge::new(ty).color(color).view(),
        importance_bar(e.importance, color),
        Style::new().fg(TN_GRAY).render(&format!(
            "importance {:.2} · {}",
            e.importance,
            rel_time(e.timestamp, now)
        ))
    );
    let mut lines = vec![a3s_tui::style::fit_visible(&header, width)];
    let mut panel = DetailPanel::without_title()
        .show_separator(false)
        .indent(0)
        .label_width(15)
        .label_color(TN_GRAY)
        .value_color(TN_GRAY)
        .muted_color(TN_GRAY)
        .unlimited_rows();

    if !e.tags.is_empty() {
        panel = panel.row(DetailRow::pair("tags", e.tags.join(", ")).color(TN_CYAN));
    }

    if let Some(facet) = graph.by_memory.get(&e.id) {
        let lifecycle = lifecycle_labels(facet);
        if !lifecycle.is_empty() {
            let color = if facet.conflicts { TN_RED } else { TN_GREEN };
            panel = panel.row(DetailRow::pair("lifecycle", lifecycle.join(" · ")).color(color));
        }
    }

    if let Some(event) = graph.event_for_memory(&e.id) {
        panel = panel
            .pair(
                "event",
                format!(
                    "{} · {} · {} · {} entities",
                    event.label,
                    event.source,
                    event.timestamp.format("%Y-%m-%d %H:%M"),
                    event.entity_ids.len()
                ),
            )
            .pair(
                "event retention",
                format!(
                    "{} · {:.2} · {}",
                    event.tier.label(),
                    event.retention_score,
                    event.forget.label()
                ),
            );
    }

    if let Some(facet) = graph.by_memory.get(&e.id) {
        panel = panel.row(
            DetailRow::pair(
                "tier",
                format!(
                    "{} · retention {:.2} · {}",
                    facet.tier.label(),
                    facet.retention_score,
                    facet.forget.label()
                ),
            )
            .color(tier_style(facet.tier)),
        );
        let entities = graph.entity_labels(&facet.entity_ids, 8);
        if !entities.is_empty() {
            panel = panel.row(DetailRow::pair("entities", entities.join(", ")).color(TN_CYAN));
        }
        let aliases = graph.alias_labels(&facet.entity_ids, 6);
        if !aliases.is_empty() {
            panel = panel.pair("aliases", aliases.join(", "));
        }
        let relations = graph.relation_labels(&facet.relation_ids, 6);
        if !relations.is_empty() {
            panel = panel.pair("relations", relations.join(" · "));
        }
    }

    let created = e.timestamp.format("%Y-%m-%d %H:%M").to_string();
    let accessed = match detail.last_accessed {
        Some(la) => format!(" · last {}", la.format("%Y-%m-%d %H:%M")),
        None => String::new(),
    };
    panel = panel.pair(
        "created",
        format!("{created} · {}× accessed{accessed}", detail.access_count),
    );
    for (k, v) in &detail.metadata {
        panel = panel.pair(k.clone(), v.clone());
    }

    lines.extend(
        panel
            .view(width.min(u16::MAX as usize) as u16, panel.rows().len())
            .lines()
            .map(str::to_string),
    );
    lines
}

impl App {
    pub(crate) fn load_memory_panel(&self, dir: std::path::PathBuf) -> Cmd<Msg> {
        let memory = self.session.memory().cloned();
        cmd::cmd(
            move || async move { Msg::MemoryLoaded(load_memory_panel_data(dir, memory).await) },
        )
    }

    /// Handle a key while the `/memory` panel is open. Returns a `Cmd` only for
    /// the `c` back-jump (memory → its ctx source session).
    pub(crate) fn memory_key(&mut self, key: &KeyEvent) -> Option<Cmd<Msg>> {
        if key.code == KeyCode::Esc {
            self.memory = None;
            return None;
        }
        // `c` — jump to the originating ctx session for a memory promoted from
        // history (`source=ctx`): pull `ctx show event <id>` into a read-only
        // viewer. The back-link that closes the ctx↔memory loop.
        if matches!(key.code, KeyCode::Char('c')) {
            if let Some(cmd) = self.memory_open_ctx_source() {
                return Some(cmd);
            }
            return None;
        }
        if matches!(key.code, KeyCode::Char('f')) {
            return self.memory_forget_candidate();
        }
        let session_memory = self.session.memory().cloned();
        let body = (self.height as usize).saturating_sub(3);
        let m = self.memory.as_mut()?;
        let last = m.entries.len().saturating_sub(1);
        match key.code {
            KeyCode::Up | KeyCode::Char('k') => {
                m.sel = m.sel.saturating_sub(1);
                m.refresh_detail();
            }
            KeyCode::Down | KeyCode::Char('j') => {
                m.sel = (m.sel + 1).min(last);
                m.refresh_detail();
            }
            KeyCode::Char('g') => {
                m.sel = 0;
                m.refresh_detail();
            }
            KeyCode::Char('G') => {
                m.sel = last;
                m.refresh_detail();
            }
            // Page keys scroll the detail pane (long memories).
            KeyCode::PageUp => {
                m.detail_scroll = m
                    .detail_scroll
                    .saturating_sub(body.saturating_sub(1).max(1));
            }
            KeyCode::PageDown => m.detail_scroll += body.saturating_sub(1).max(1),
            // Reload the durable snapshot; fall back to the live session store
            // when the file-backed view is unavailable.
            KeyCode::Char('r') => {
                let dir = m.dir.clone();
                m.note = "refreshing graph…".to_string();
                return Some(memory_panel_load_cmd(dir, session_memory));
            }
            _ => {}
        }
        None
    }

    /// Spawn `ctx show event <id>` for the selected memory's `ctx_event_id`
    /// provenance (nothing if it has none / ctx isn't installed).
    fn memory_open_ctx_source(&mut self) -> Option<Cmd<Msg>> {
        let m = self.memory.as_mut()?;
        let Some(event_id) = m.detail.metadata.get("ctx_event_id").cloned() else {
            m.note = "this memory has no ctx source (press c only on source=ctx)".to_string();
            return None;
        };
        if !self.ctx_ready {
            if let Some(m) = self.memory.as_mut() {
                m.note = "ctx is not installed — can't open the source session".to_string();
            }
            return None;
        }
        Some(cmd::cmd(move || async move {
            let out = tokio::process::Command::new("ctx")
                .args(["show", "event", &event_id, "--window", "8"])
                .output()
                .await;
            Msg::CtxMemorySource(match out {
                Ok(o) if o.status.success() => {
                    Ok((event_id, String::from_utf8_lossy(&o.stdout).into_owned()))
                }
                Ok(o) => Err(String::from_utf8_lossy(&o.stderr).into_owned()),
                Err(e) => Err(e.to_string()),
            })
        }))
    }

    /// Forget the selected memory, but only when the graph retention pass marks
    /// it as a low-value candidate. This keeps destructive cleanup explicit and
    /// auditable.
    fn memory_forget_candidate(&mut self) -> Option<Cmd<Msg>> {
        let session_memory = self.session.memory().cloned();
        let m = self.memory.as_mut()?;
        let entry = m.entries.get(m.sel).cloned()?;
        let Some(facet) = m.graph.by_memory.get(&entry.id) else {
            m.note = "graph data is still loading".to_string();
            return None;
        };
        if !facet.forget.is_candidate() {
            m.note = format!(
                "{} memory is {} · only forget candidates can be removed here",
                facet.tier.label(),
                facet.forget.label()
            );
            return None;
        }
        let id = entry.id.clone();
        let dir = m.dir.clone();
        let loaded_from_session = m.loaded_from_session;
        m.note = format!("forgetting candidate {id}…");
        Some(cmd::cmd(move || async move {
            let id_for_msg = id.clone();
            let result = async {
                delete_memory_item(&dir, session_memory.clone(), loaded_from_session, &id).await?;
                let data = load_memory_panel_data(dir, session_memory).await;
                Ok((id_for_msg, data))
            }
            .await;
            Msg::MemoryForgotten(result)
        }))
    }

    /// Full-screen `/memory` panel: timeline (left) + selected detail (right).
    pub(crate) fn render_memory(&self, m: &MemPanel) -> String {
        let width = self.width as usize;
        let h = self.height as usize;
        let now = chrono::Utc::now();

        let header = format!(
            "  memory graph · {} events · {} entities · {} aliases · {} relations · LLM {} · merged {} · conflicts {} · S/M/L {}/{}/{} · {} forget candidates   {}",
            m.graph.stats.events,
            m.graph.stats.entities,
            m.graph.stats.aliases,
            m.graph.stats.relations,
            m.graph.stats.llm_extracted,
            m.graph.stats.consolidated,
            m.graph.stats.conflicts,
            m.graph.stats.short,
            m.graph.stats.mid,
            m.graph.stats.long,
            m.graph.stats.forget_candidates,
            Style::new().fg(TN_GRAY).render(&m.note),
        );
        let mut out = vec![
            memory_line(&header, width),
            memory_line(
                &divider_line_with(width.min(u16::MAX as usize) as u16, "─", TN_GRAY),
                width,
            ),
        ];
        let body = h.saturating_sub(3);

        if m.entries.is_empty() {
            out.push(memory_line(
                &Style::new().fg(TN_GRAY).render(
                    "  no memories yet — the agent records them as you work (success/failure patterns, facts)",
                ),
                width,
            ));
            while out.len() + 1 < h {
                out.push(String::new());
            }
            out.push(memory_line(
                &Style::new().fg(TN_GRAY).render("  Esc close"),
                width,
            ));
            out.truncate(h);
            return out.join("\n");
        }

        let tw = (width / 3).clamp(26, 52);
        let sep = Style::new().fg(TN_GRAY).render(" │ ");
        let left_lines = memory_timeline_lines(&m.entries, &m.graph, m.sel, now, tw, body);

        // Right: the selected memory's detail, scrollable.
        let right_lines = self.memory_detail_lines(m, now, width.saturating_sub(tw + 4));

        for i in 0..body {
            let left = left_lines.get(i).cloned().unwrap_or_else(|| " ".repeat(tw));
            let right = right_lines
                .get(m.detail_scroll + i)
                .cloned()
                .unwrap_or_default();
            out.push(format!("{left}{sep}{right}"));
        }

        let hint = "  ↑↓/jk select · g/G top/bottom · PgUp/PgDn scroll · c ctx source · f forget candidate · r refresh · Esc close";
        while out.len() + 1 < h {
            out.push(String::new());
        }
        out.push(memory_line(&Style::new().fg(TN_GRAY).render(hint), width));
        out.truncate(h);
        out.join("\n")
    }

    /// Build the right-pane lines for the selected memory: a metadata block, a
    /// rule, then the full original-case content (word-wrapped to `w`).
    fn memory_detail_lines(
        &self,
        m: &MemPanel,
        now: chrono::DateTime<chrono::Utc>,
        w: usize,
    ) -> Vec<String> {
        let mut lines = Vec::new();
        let Some(e) = m.entries.get(m.sel) else {
            return lines;
        };
        lines.extend(memory_detail_metadata_lines(e, &m.detail, &m.graph, now, w));
        lines.push(divider_line_with(w.min(48) as u16, "─", TN_GRAY));
        // Prefer the full original-case content; fall back to the index preview.
        let content = if m.detail.content.is_empty() {
            e.content_lower.as_str()
        } else {
            m.detail.content.as_str()
        };
        lines.extend(memory_detail_content_rows(content, w));
        lines
            .into_iter()
            .map(|line| memory_line(&line, w))
            .collect()
    }
}

fn memory_detail_content_rows(content: &str, width: usize) -> Vec<String> {
    let mut lines = Vec::new();
    for raw in content.lines() {
        if raw.is_empty() {
            lines.push(String::new());
        } else {
            lines.extend(Paragraph::new(raw).width(width.max(8)).color(TN_FG).lines());
        }
    }
    lines
}

fn memory_timeline_lines(
    entries: &[MemEntry],
    graph: &MemoryGraph,
    selected: usize,
    now: chrono::DateTime<chrono::Utc>,
    width: usize,
    height: usize,
) -> Vec<String> {
    if width == 0 || height == 0 || entries.is_empty() {
        return Vec::new();
    }

    let rows = timeline_rows(entries, now)
        .into_iter()
        .filter_map(|row| match row {
            TlRow::Day(label) => Some(TimelineRow::section(label)),
            TlRow::Node(idx) => {
                let entry = entries.get(idx)?;
                let (badge, color) = mem_type_style(&entry.memory_type);
                let facet = graph.by_memory.get(&entry.id);
                let tier = facet.map(|f| f.tier).unwrap_or(MemoryTier::Short);
                let forget = facet.map(|f| f.forget).unwrap_or(ForgetSignal::Keep);
                let preview = entry.content_lower.lines().next().unwrap_or("");
                Some(TimelineRow::item(
                    TimelineItem::new(
                        rel_time(entry.timestamp, now),
                        format!("{}{} {badge}", tier.badge(), forget_mark(forget)),
                        preview,
                    )
                    .color(color),
                ))
            }
        })
        .collect::<Vec<_>>();

    Timeline::new()
        .rows(rows)
        .selected_item(selected)
        .margin(1)
        .marker("●")
        .time_width(4)
        .badge_width(7)
        .fill_height(true)
        .selected_fg(TN_FG)
        .selected_bg(SURFACE_SELECTED)
        .section_color(TN_GRAY)
        .time_color(TN_GRAY)
        .preview_color(TN_FG)
        .view(width.min(u16::MAX as usize) as u16, height)
        .lines()
        .map(str::to_string)
        .collect()
}

fn lifecycle_labels(facet: &MemoryGraphFacet) -> Vec<&'static str> {
    let mut labels = Vec::new();
    if facet.llm_extracted {
        labels.push("llm extracted");
    }
    if facet.consolidated {
        labels.push("consolidated");
    }
    if facet.conflicts {
        labels.push("conflict");
    }
    labels
}

fn memory_panel_load_cmd(
    dir: std::path::PathBuf,
    memory: Option<std::sync::Arc<a3s_code_core::memory::AgentMemory>>,
) -> Cmd<Msg> {
    cmd::cmd(move || async move { Msg::MemoryLoaded(load_memory_panel_data(dir, memory).await) })
}

async fn load_memory_panel_data(
    dir: std::path::PathBuf,
    memory: Option<std::sync::Arc<a3s_code_core::memory::AgentMemory>>,
) -> MemPanelData {
    let file_dir = dir.clone();
    let file_data = tokio::task::spawn_blocking(move || memutil::load_panel_data(&file_dir))
        .await
        .unwrap_or_default();
    if !file_data.entries.is_empty() {
        return file_data;
    }

    let Some(memory) = memory else {
        return file_data;
    };
    match memory.get_recent(MEMORY_PANEL_SESSION_LIMIT).await {
        Ok(items) if !items.is_empty() => memutil::panel_data_from_memory_items(items),
        Ok(_) => file_data,
        Err(_) => file_data,
    }
}

async fn delete_memory_item(
    dir: &std::path::Path,
    memory: Option<std::sync::Arc<a3s_code_core::memory::AgentMemory>>,
    prefer_session: bool,
    id: &str,
) -> Result<(), String> {
    if prefer_session {
        let Some(memory) = memory else {
            return Err("session memory is unavailable".to_string());
        };
        let store = memory.store().clone();
        return a3s_memory::MemoryStore::delete(store.as_ref(), id)
            .await
            .map_err(|e| e.to_string());
    }

    let store = a3s_memory::FileMemoryStore::new(dir)
        .await
        .map_err(|e| e.to_string())?;
    a3s_memory::MemoryStore::delete(&store, id)
        .await
        .map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_ts(raw: &str) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::parse_from_rfc3339(raw)
            .unwrap()
            .with_timezone(&chrono::Utc)
    }

    fn test_entry(id: &str, content: &str, ty: &str, raw_time: &str) -> MemEntry {
        MemEntry {
            id: id.to_string(),
            content_lower: content.to_string(),
            tags: Vec::new(),
            importance: 0.7,
            timestamp: test_ts(raw_time),
            memory_type: ty.to_string(),
        }
    }

    #[test]
    fn memory_timeline_lines_use_shared_component_and_fit_width() {
        let now = test_ts("2026-06-30T12:00:00Z");
        let entries = vec![
            test_entry(
                "a",
                "remember terminal layout",
                "semantic",
                "2026-06-30T11:58:00Z",
            ),
            test_entry(
                "b",
                "fix narrow tui overflow",
                "procedural",
                "2026-06-30T11:45:00Z",
            ),
        ];
        let mut graph = MemoryGraph::default();
        graph.by_memory.insert(
            "b".to_string(),
            MemoryGraphFacet {
                tier: MemoryTier::Long,
                forget: ForgetSignal::Candidate,
                ..MemoryGraphFacet::default()
            },
        );

        let lines = memory_timeline_lines(&entries, &graph, 1, now, 40, 4);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert_eq!(lines.len(), 4);
        assert!(plain.contains("Today"), "{plain}");
        assert!(plain.contains("sem"), "{plain}");
        assert!(plain.contains("fix narrow tui"), "{plain}");
        assert!(plain.contains("L! proc"), "{plain}");
        assert!(
            lines.iter().any(|line| line.contains("48;2;42;46;52")),
            "selected memory row should use the neutral surface: {lines:?}"
        );
        let selected = lines
            .iter()
            .find(|line| a3s_tui::style::strip_ansi(line).contains("fix narrow tui"))
            .expect("selected procedural memory row");
        assert!(
            selected.contains(&TN_GREEN.fg_ansi()),
            "selected row should retain its procedural semantic color: {selected:?}"
        );
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 40),
            "{plain}"
        );
    }

    #[test]
    fn memory_detail_metadata_lines_use_shared_badge_and_detail_panel() {
        let now = test_ts("2026-06-30T12:00:00Z");
        let mut entry = test_entry(
            "m1",
            "remember terminal layout",
            "semantic",
            "2026-06-30T11:58:00Z",
        );
        entry.tags = vec!["tui".into(), "layout".into()];
        let mut detail = MemDetail {
            access_count: 3,
            last_accessed: Some(test_ts("2026-06-30T11:59:00Z")),
            ..MemDetail::default()
        };
        detail
            .metadata
            .insert("ctx_event_id".into(), "event-1234567890".into());
        let mut graph = MemoryGraph::default();
        graph.by_memory.insert(
            "m1".to_string(),
            MemoryGraphFacet {
                tier: MemoryTier::Long,
                forget: ForgetSignal::Protected,
                retention_score: 0.92,
                llm_extracted: true,
                consolidated: true,
                ..MemoryGraphFacet::default()
            },
        );

        let lines = memory_detail_metadata_lines(&entry, &detail, &graph, now, 42);
        let plain = lines
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>()
            .join("\n");

        assert!(plain.contains("[semantic]"), "{plain}");
        assert!(plain.contains("▰▰▰▰▱ importance"), "{plain}");
        assert!(plain.contains("tags"), "{plain}");
        assert!(plain.contains("lifecycle"), "{plain}");
        assert!(plain.contains("long-term"), "{plain}");
        assert!(plain.contains("ctx_event_id"), "{plain}");
        assert!(
            lines
                .iter()
                .any(|line| line.contains(&format!("\x1b[1;{}m[semantic]", TN_CYAN.fg_ansi()))),
            "{lines:?}"
        );
        assert!(lines.iter().any(|line| line.contains("\x1b[")));
        assert!(
            lines
                .iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 42),
            "{plain}"
        );
    }

    #[test]
    fn importance_bar_uses_shared_progress() {
        let rendered = importance_bar(0.7, TN_CYAN);
        let expected = Progress::new()
            .value(0.7)
            .width(5)
            .filled_char('▰')
            .empty_char('▱')
            .filled_color(TN_CYAN)
            .empty_color(TN_GRAY)
            .show_percentage(false)
            .view();

        assert_eq!(rendered, expected);
        assert_eq!(a3s_tui::style::strip_ansi(&rendered), "▰▰▰▰▱");
        assert_eq!(a3s_tui::style::visible_len(&rendered), 5);
    }

    #[test]
    fn memory_detail_content_rows_use_shared_paragraph() {
        let rows = memory_detail_content_rows("alpha beta gamma\n\n中文测试内容 with suffix", 12);
        let plain = rows
            .iter()
            .map(|line| a3s_tui::style::strip_ansi(line))
            .collect::<Vec<_>>();

        assert!(plain.contains(&String::new()), "{plain:?}");
        assert!(plain.iter().any(|line| line.contains("alpha")), "{plain:?}");
        assert!(plain.iter().any(|line| line.contains("中文")), "{plain:?}");
        assert!(
            rows.iter()
                .filter(|line| !line.is_empty())
                .all(|line| line.contains(&TN_FG.fg_ansi())),
            "{rows:?}"
        );
        assert!(
            rows.iter()
                .all(|line| a3s_tui::style::visible_len(line) <= 12),
            "{plain:?}"
        );
    }

    #[test]
    fn memory_lines_are_width_bounded_with_styles() {
        let line = memory_line(
            &Style::new().fg(TN_GRAY).render(
                "  ↑↓/jk select · g/G top/bottom · PgUp/PgDn scroll · c open ctx source · r refresh · Esc close",
            ),
            40,
        );

        assert!(
            a3s_tui::style::visible_len(&line) <= 40,
            "{}",
            a3s_tui::style::strip_ansi(&line)
        );
    }
}
