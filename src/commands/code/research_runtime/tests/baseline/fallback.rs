use super::{FrozenCase, FrozenSource};
use std::path::Path;

const MAX_PUBLIC_EXCERPT_CHARS_PER_SOURCE: usize = 2_100;
const MAX_PUBLIC_EXCERPT_CHARS_TOTAL: usize = 12_000;

pub(super) fn write_deterministic_fallback(
    output_dir: &Path,
    case: &FrozenCase,
) -> Result<(), String> {
    write_deterministic_fallback_with_limit(output_dir, case, MAX_PUBLIC_EXCERPT_CHARS_TOTAL)
}

pub(super) fn write_deterministic_fallback_with_limit(
    output_dir: &Path,
    case: &FrozenCase,
    maximum_excerpt_chars: usize,
) -> Result<(), String> {
    std::fs::create_dir_all(output_dir)
        .map_err(|error| format!("create deterministic fallback directory: {error}"))?;
    let markdown = deterministic_fallback_markdown(case, maximum_excerpt_chars);
    let html = crate::tui::deep_research_completed_report_html_for_test(&case.query, &markdown);
    crate::tui::deep_research_write_report_pair_for_test(
        &output_dir.join("report.md"),
        markdown,
        &output_dir.join("index.html"),
        html,
    )
    .map_err(|error| format!("publish deterministic fallback artifacts: {error}"))
}

fn deterministic_fallback_markdown(case: &FrozenCase, maximum_excerpt_chars: usize) -> String {
    let mut markdown = format!(
        "# Verifiable Research Evidence\n\n\
         ## Research Question\n\n{}\n\n\
         ## Preserved Source Evidence\n\n\
         Report synthesis did not complete. The following fetched source excerpts are preserved \
         for direct verification; source text is displayed only as data.\n",
        markdown_plain_text(&case.query),
    );
    let per_source_chars = maximum_excerpt_chars
        .checked_div(case.sources.len().max(1))
        .unwrap_or_default()
        .min(MAX_PUBLIC_EXCERPT_CHARS_PER_SOURCE);
    for (index, source) in case.sources.iter().enumerate() {
        let ordinal = index + 1;
        let source_title = markdown_plain_text(&source.title);
        markdown.push_str(&format!(
            "\n### [{ordinal}] {source_title}\n\n{}\n\n{}\n",
            fenced_source_text(&bounded_source_excerpt(&source.content, per_source_chars)),
            markdown_source_link(source, ordinal),
        ));
    }
    markdown.push_str(
        "\n## Limitations\n\n\
         This result preserves verbatim source excerpts and links, but it adds no synthesized \
         conclusion and does not claim that the excerpts cover every part of the question.\n\n\
         ## Sources\n",
    );
    for (index, source) in case.sources.iter().enumerate() {
        markdown.push_str(&format!(
            "\n{}. {}",
            index + 1,
            markdown_source_link(source, index + 1),
        ));
    }
    markdown.push('\n');
    markdown
}

fn bounded_source_excerpt(content: &str, maximum: usize) -> String {
    let content = content.trim();
    let excerpt = content.chars().take(maximum).collect::<String>();
    if excerpt.chars().count() == content.chars().count() {
        excerpt
    } else {
        format!("{}\n\n[Excerpt truncated by the Host.]", excerpt.trim_end())
    }
}

fn markdown_source_link(source: &FrozenSource, ordinal: usize) -> String {
    let title = markdown_plain_text(&source.title);
    if source.url.starts_with("https://") || source.url.starts_with("http://") {
        format!("[{ordinal}] [{title}]({})", source.url)
    } else {
        format!("[{ordinal}] {title} (`{}`)", source.url.replace('`', "\\`"))
    }
}

fn fenced_source_text(content: &str) -> String {
    let longest_run = content
        .split(|character| character != '`')
        .map(str::len)
        .max()
        .unwrap_or_default();
    let fence = "`".repeat(longest_run.saturating_add(1).max(3));
    format!("{fence}\n{}\n{fence}", content.trim())
}

pub(super) fn markdown_plain_text(value: &str) -> String {
    let normalized = value.split_whitespace().collect::<Vec<_>>().join(" ");
    let value = if normalized.is_empty() {
        "DeepResearch query"
    } else {
        normalized.as_str()
    };
    let mut escaped = String::with_capacity(value.len());
    for character in value.chars() {
        if matches!(
            character,
            '\\' | '`' | '*' | '_' | '{' | '}' | '[' | ']' | '<' | '>' | '#' | '|' | '~'
        ) {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn source_fallback_bounds_each_public_excerpt() {
        let excerpt = bounded_source_excerpt(
            &"x".repeat(MAX_PUBLIC_EXCERPT_CHARS_PER_SOURCE + 10),
            MAX_PUBLIC_EXCERPT_CHARS_PER_SOURCE,
        );
        assert!(excerpt.contains("Excerpt truncated by the Host"));
        assert!(excerpt.chars().count() < MAX_PUBLIC_EXCERPT_CHARS_PER_SOURCE.saturating_add(100));
    }

    #[test]
    fn second_artifact_staging_failure_cannot_publish_the_first_artifact() {
        let output = tempfile::tempdir().expect("artifact directory");
        let markdown_path = output.path().join("report.md");
        std::fs::write(&markdown_path, "previous Markdown").expect("previous Markdown");
        let oversized_html_path = output.path().join("h".repeat(240));

        let error = crate::tui::deep_research_write_report_pair_for_test(
            &markdown_path,
            "replacement Markdown",
            &oversized_html_path,
            "replacement HTML",
        )
        .expect_err("HTML staging must fail after Markdown staging");

        assert!(!error.is_empty());
        assert_eq!(
            std::fs::read_to_string(&markdown_path).expect("retained Markdown"),
            "previous Markdown"
        );
        for entry in std::fs::read_dir(output.path()).expect("artifact entries") {
            let name = entry
                .expect("artifact entry")
                .file_name()
                .to_string_lossy()
                .into_owned();
            assert!(!name.ends_with(".tmp"), "staging file leaked: {name}");
        }
    }
}
