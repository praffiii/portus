pub mod ansi;

#[cfg(test)]
mod tests {
    use super::ansi::{sanitize_chunk, LogBatcher, TRIM_MARKER};

    #[test]
    fn sgr_red_renders_as_allowed_span_without_raw_escape() {
        let html = sanitize_chunk(b"\x1b[31mred\x1b[0m");

        assert!(html.contains("<span class=\"ansi-fg-red\">red</span>"));
        assert!(!html.contains("\x1b[31m"));
    }

    #[test]
    fn script_and_osc_hyperlink_render_as_inert_text() {
        let html = sanitize_chunk(
            b"<script>alert(1)</script>\x1b]8;;https://example.com\x07link\x1b]8;;\x07",
        );

        assert!(html.contains("&lt;script&gt;alert(1)&lt;/script&gt;"));
        assert!(html.contains("link"));
        assert!(!html.contains("<script>"));
        assert!(!html.contains("<a"));
        assert!(!html.contains("https://example.com"));
    }

    #[test]
    fn carriage_return_rewrites_the_current_line() {
        let mut batcher = LogBatcher::new(20, 4096);

        batcher.push_bytes(b"progress 10%\rprogress 20%\n");

        let lines = batcher.snapshot();
        assert_eq!(lines.len(), 1);
        assert!(lines[0].html.contains("progress 20%"));
        assert!(!lines[0].html.contains("progress 10%"));
    }

    #[test]
    fn flood_is_trimmed_to_bounded_lines_with_marker() {
        let mut batcher = LogBatcher::new(3, 4096);

        for i in 0..10 {
            batcher.push_bytes(format!("line {i}\n").as_bytes());
        }

        let lines = batcher.snapshot();
        assert_eq!(lines.len(), 3);
        assert_eq!(lines[0].html, TRIM_MARKER);
        assert!(lines[2].html.contains("line 9"));
    }
}
