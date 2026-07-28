use std::cmp::Ordering;

use a3s_boot::{BootError, Result as BootResult};
use a3s_code_core::{LocalWorkspaceFile, LocalWorkspaceManifestSnapshot};
use serde_json::{json, Value};

use crate::api::code_web::state::CodeWebState;

use super::service::required_path;

const MAX_FILE_RESULTS: usize = 500;

pub(super) async fn workspace_files(
    state: &CodeWebState,
    root_path: String,
    query: String,
    max_results: usize,
) -> BootResult<Value> {
    let root = required_path(root_path)?;
    let snapshot = state
        .workspace_manifest_snapshot_for(&root)
        .await
        .map_err(|error| BootError::BadRequest(error.to_string()))?;
    Ok(catalog_from_snapshot(&snapshot, &query, max_results))
}

fn catalog_from_snapshot(
    snapshot: &LocalWorkspaceManifestSnapshot,
    query: &str,
    max_results: usize,
) -> Value {
    let query = normalize_query(query);
    let limit = max_results.clamp(1, MAX_FILE_RESULTS);
    let mut matches = snapshot
        .files
        .iter()
        .filter_map(|file| match_rank(file, &query).map(|rank| (file, rank)))
        .collect::<Vec<_>>();
    matches.sort_by(|(left_file, left_rank), (right_file, right_rank)| {
        left_rank
            .cmp(right_rank)
            .then_with(|| left_file.generated.cmp(&right_file.generated))
            .then_with(|| compare_paths(&left_file.path, &right_file.path))
    });

    let total = matches.len();
    let items = matches
        .into_iter()
        .take(limit)
        .map(|(file, _)| {
            json!({
                "path": snapshot.root.join(&file.path).display().to_string(),
                "relativePath": file.path,
                "name": file_name(&file.path),
                "isBinary": file.binary,
            })
        })
        .collect::<Vec<_>>();
    json!({
        "workspaceRoot": snapshot.root.display().to_string(),
        "truncated": total > items.len(),
        "total": total,
        "items": items,
    })
}

fn normalize_query(query: &str) -> String {
    query.trim().replace('\\', "/").to_lowercase()
}

fn compare_paths(left: &str, right: &str) -> Ordering {
    left.to_lowercase()
        .cmp(&right.to_lowercase())
        .then_with(|| left.cmp(right))
}

fn file_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
struct MatchRank {
    class: u8,
    start: usize,
    gaps: usize,
    span: usize,
}

fn match_rank(file: &LocalWorkspaceFile, query: &str) -> Option<MatchRank> {
    if query.is_empty() {
        return Some(MatchRank {
            class: 0,
            start: 0,
            gaps: 0,
            span: 0,
        });
    }

    let path = file.path.to_lowercase();
    let name = file_name(&path);
    if name == query || path == query {
        return Some(exact_rank(0));
    }
    if name.starts_with(query) {
        return Some(exact_rank(1));
    }
    if let Some(start) = name.find(query) {
        return Some(contiguous_rank(2, start, query.chars().count()));
    }
    if let Some(start) = path.find(query) {
        return Some(contiguous_rank(3, start, query.chars().count()));
    }
    fuzzy_subsequence_rank(name, query, 4).or_else(|| fuzzy_subsequence_rank(&path, query, 5))
}

fn exact_rank(class: u8) -> MatchRank {
    MatchRank {
        class,
        start: 0,
        gaps: 0,
        span: 0,
    }
}

fn contiguous_rank(class: u8, start: usize, span: usize) -> MatchRank {
    MatchRank {
        class,
        start,
        gaps: 0,
        span,
    }
}

fn fuzzy_subsequence_rank(candidate: &str, query: &str, class: u8) -> Option<MatchRank> {
    let candidate = candidate.chars().collect::<Vec<_>>();
    let mut cursor = 0usize;
    let mut first = None;
    let mut previous = None;
    let mut gaps = 0usize;
    for needle in query.chars() {
        let offset = candidate[cursor..]
            .iter()
            .position(|value| *value == needle)?;
        let index = cursor + offset;
        first.get_or_insert(index);
        if let Some(previous) = previous {
            gaps += index.saturating_sub(previous + 1);
        }
        previous = Some(index);
        cursor = index + 1;
    }
    let first = first?;
    Some(MatchRank {
        class,
        start: first,
        gaps,
        span: previous.unwrap_or(first).saturating_sub(first) + 1,
    })
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use a3s_code_core::{LocalWorkspaceFileStatus, LocalWorkspaceManifestSnapshot};

    use super::*;

    #[test]
    fn exact_basename_precedes_contiguous_path_and_fuzzy_matches() {
        let snapshot = snapshot(vec![
            file("src/application.ts", false),
            file("examples/app.ts.demo", false),
            file("src/app.ts", false),
            file("src/archive-prompt.ts", false),
        ]);

        let catalog = catalog_from_snapshot(&snapshot, "app.ts", 25);
        let paths = catalog["items"]
            .as_array()
            .expect("catalog items")
            .iter()
            .map(|item| item["relativePath"].as_str().expect("relative path"))
            .collect::<Vec<_>>();

        assert_eq!(paths.first(), Some(&"src/app.ts"));
        assert_eq!(catalog["total"], paths.len());
        assert_eq!(catalog["truncated"], false);
    }

    #[test]
    fn catalog_preserves_binary_metadata_and_caps_server_results() {
        let mut files = (0..MAX_FILE_RESULTS + 1)
            .map(|index| file(&format!("src/file-{index:03}.ts"), false))
            .collect::<Vec<_>>();
        files.push(file("public/logo.png", true));
        let snapshot = snapshot(files);

        let all = catalog_from_snapshot(&snapshot, "", usize::MAX);
        assert_eq!(all["items"].as_array().expect("items").len(), 500);
        assert_eq!(all["total"], 502);
        assert_eq!(all["truncated"], true);

        let binary = catalog_from_snapshot(&snapshot, "logo", 10);
        assert_eq!(binary["items"][0]["path"], "/repo/public/logo.png");
        assert_eq!(binary["items"][0]["isBinary"], true);
    }

    fn snapshot(files: Vec<LocalWorkspaceFile>) -> LocalWorkspaceManifestSnapshot {
        LocalWorkspaceManifestSnapshot {
            version: 1,
            root: PathBuf::from("/repo"),
            files,
            scanned_at_ms: 1,
        }
    }

    fn file(path: &str, binary: bool) -> LocalWorkspaceFile {
        LocalWorkspaceFile {
            path: path.to_string(),
            size: 1,
            modified_ms: Some(1),
            language: None,
            status: LocalWorkspaceFileStatus::Tracked,
            binary,
            generated: false,
        }
    }
}
