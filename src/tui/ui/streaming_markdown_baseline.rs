use super::{Markdown, StreamingMarkdown};
use a3s_tui::style::visible_len;

const BASELINE_DOCUMENTS: &[&str] = &[
    "# Heading\n\nA paragraph with **bold**, `code`, and [a link](https://example.com).\n\n- first\n- second\n\n",
    "> Quoted text with 中文 and an emoji 🦀.\n>\n> ```rust\n> fn main() {}\n> ```\n\n",
    "Before.\n\n| Name | Description |\n| --- | --- |\n| alpha | a deliberately long value that must wrap |\n| beta | short |\n\nAfter.\n",
    "[forward reference][target]\n\n```json\n{\"enabled\":true,\"items\":[1,2,3]}\n```\n\n[target]: https://example.com/reference\n",
];

fn utf8_chunks(source: &str, target_bytes: usize) -> Vec<&str> {
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < source.len() {
        let mut end = (start + target_bytes).min(source.len());
        while end > start && !source.is_char_boundary(end) {
            end -= 1;
        }
        if end == start {
            end = source[start..]
                .char_indices()
                .nth(1)
                .map_or(source.len(), |(offset, _)| start + offset);
        }
        chunks.push(&source[start..end]);
        start = end;
    }
    chunks
}

fn assert_width_bound(rendered: &str, width: usize) {
    for line in rendered.lines() {
        assert!(
            visible_len(line) <= width,
            "rendered row exceeds width {width}: {line:?}"
        );
    }
}

#[test]
fn final_streaming_render_matches_the_non_streaming_baseline() {
    for &width in &[24, 48, 80] {
        for source in BASELINE_DOCUMENTS {
            let expected = Markdown::new().with_width(width).render(source);
            for chunk_bytes in [1, 2, 5, 13, source.len().max(1)] {
                let mut streaming = StreamingMarkdown::new(width);
                for chunk in utf8_chunks(source, chunk_bytes) {
                    streaming.push(chunk);
                }

                assert_eq!(
                    streaming.raw_content(),
                    *source,
                    "stream source changed at width {width}, chunk size {chunk_bytes}"
                );
                assert_eq!(
                    streaming.final_view(),
                    expected,
                    "final render diverged at width {width}, chunk size {chunk_bytes}"
                );
                assert_width_bound(&streaming.final_view(), width);
            }
        }
    }
}

#[test]
fn complete_newline_terminated_documents_reconcile_live_and_final_views() {
    for &width in &[24, 32, 64] {
        for source in BASELINE_DOCUMENTS {
            let mut streaming = StreamingMarkdown::new(width);
            for line in source.split_inclusive('\n') {
                streaming.push(line);
            }

            assert_eq!(
                streaming.view(),
                streaming.final_view(),
                "completed live render did not reconcile at width {width}: {source:?}"
            );
            assert_width_bound(&streaming.view(), width);
        }
    }
}
