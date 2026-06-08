use std::collections::VecDeque;

use serde::{Deserialize, Serialize};
use specta::Type;
use vte::{Params, Parser, Perform};

pub const TRIM_MARKER: &str = "… earlier output trimmed";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct LogLine {
    pub html: String,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct LogBatch {
    pub lines: Vec<LogLine>,
}

pub fn sanitize_chunk(bytes: &[u8]) -> String {
    let mut parser = Parser::new();
    let mut performer = HtmlPerformer::default();
    parser.advance(&mut performer, bytes);
    performer.finish()
}

pub struct LogBatcher {
    max_lines: usize,
    max_bytes: usize,
    lines: VecDeque<LogLine>,
    current: Vec<u8>,
    trimmed: bool,
    pending_cr: bool,
}

impl LogBatcher {
    pub fn new(max_lines: usize, max_bytes: usize) -> Self {
        Self {
            max_lines,
            max_bytes,
            lines: VecDeque::with_capacity(max_lines),
            current: Vec::new(),
            trimmed: false,
            pending_cr: false,
        }
    }

    pub fn push_bytes(&mut self, bytes: &[u8]) -> LogBatch {
        let start_len = self.lines.len();
        for &byte in bytes {
            if self.pending_cr {
                self.pending_cr = false;
                if byte == b'\n' {
                    self.finish_current();
                    continue;
                }
                self.current.clear();
            }
            match byte {
                b'\n' => self.finish_current(),
                b'\r' => self.pending_cr = true,
                _ => self.current.push(byte),
            }
        }
        self.enforce_byte_limit();
        let lines = self
            .lines
            .iter()
            .skip(start_len.min(self.lines.len()))
            .cloned()
            .collect();
        LogBatch { lines }
    }

    pub fn snapshot(&self) -> Vec<LogLine> {
        let mut lines: Vec<LogLine> = self.lines.iter().cloned().collect();
        if !self.current.is_empty() {
            lines.push(LogLine {
                html: sanitize_chunk(&self.current),
            });
        }
        lines
    }

    fn finish_current(&mut self) {
        let html = sanitize_chunk(&self.current);
        self.current.clear();
        self.push_line(LogLine { html });
    }

    fn push_line(&mut self, line: LogLine) {
        while self.lines.len() >= self.max_lines {
            self.lines.pop_front();
            self.trimmed = true;
        }
        self.lines.push_back(line);
        if self.trimmed {
            self.ensure_trim_marker();
        }
    }

    fn ensure_trim_marker(&mut self) {
        if self.max_lines == 0 {
            return;
        }
        if self.lines.front().map(|line| line.html.as_str()) == Some(TRIM_MARKER) {
            return;
        }
        if self.lines.len() >= self.max_lines {
            self.lines.pop_front();
        }
        self.lines.push_front(LogLine {
            html: TRIM_MARKER.to_string(),
        });
    }

    fn enforce_byte_limit(&mut self) {
        while self.total_bytes() > self.max_bytes && self.lines.len() > 1 {
            self.lines.pop_front();
            self.trimmed = true;
        }
        if self.trimmed {
            self.ensure_trim_marker();
        }
    }

    fn total_bytes(&self) -> usize {
        self.lines.iter().map(|line| line.html.len()).sum::<usize>() + self.current.len()
    }
}

#[derive(Default)]
struct HtmlPerformer {
    output: String,
    active_class: Option<&'static str>,
}

impl HtmlPerformer {
    fn finish(mut self) -> String {
        self.close_span();
        self.output
    }

    fn push_escaped_char(&mut self, c: char) {
        match c {
            '&' => self.output.push_str("&amp;"),
            '<' => self.output.push_str("&lt;"),
            '>' => self.output.push_str("&gt;"),
            '"' => self.output.push_str("&quot;"),
            '\'' => self.output.push_str("&#39;"),
            _ => self.output.push(c),
        }
    }

    fn close_span(&mut self) {
        if self.active_class.take().is_some() {
            self.output.push_str("</span>");
        }
    }

    fn set_class(&mut self, class: Option<&'static str>) {
        if self.active_class == class {
            return;
        }
        self.close_span();
        if let Some(class) = class {
            self.output.push_str("<span class=\"");
            self.output.push_str(class);
            self.output.push_str("\">");
            self.active_class = Some(class);
        }
    }

    fn apply_sgr(&mut self, params: &Params) {
        if params.is_empty() {
            self.set_class(None);
            return;
        }

        for param in params {
            let code = param.first().copied().unwrap_or(0);
            match code {
                0 => self.set_class(None),
                1 => self.set_class(Some("ansi-bold")),
                2 => self.set_class(Some("ansi-dim")),
                3 => self.set_class(Some("ansi-italic")),
                4 => self.set_class(Some("ansi-underline")),
                30 => self.set_class(Some("ansi-fg-black")),
                31 => self.set_class(Some("ansi-fg-red")),
                32 => self.set_class(Some("ansi-fg-green")),
                33 => self.set_class(Some("ansi-fg-yellow")),
                34 => self.set_class(Some("ansi-fg-blue")),
                35 => self.set_class(Some("ansi-fg-magenta")),
                36 => self.set_class(Some("ansi-fg-cyan")),
                37 => self.set_class(Some("ansi-fg-white")),
                90 => self.set_class(Some("ansi-fg-bright-black")),
                91 => self.set_class(Some("ansi-fg-bright-red")),
                92 => self.set_class(Some("ansi-fg-bright-green")),
                93 => self.set_class(Some("ansi-fg-bright-yellow")),
                94 => self.set_class(Some("ansi-fg-bright-blue")),
                95 => self.set_class(Some("ansi-fg-bright-magenta")),
                96 => self.set_class(Some("ansi-fg-bright-cyan")),
                97 => self.set_class(Some("ansi-fg-bright-white")),
                _ => {}
            }
        }
    }
}

impl Perform for HtmlPerformer {
    fn print(&mut self, c: char) {
        self.push_escaped_char(c);
    }

    fn execute(&mut self, byte: u8) {
        if byte == b'\t' {
            self.output.push_str("    ");
        }
    }

    fn csi_dispatch(&mut self, params: &Params, _intermediates: &[u8], ignore: bool, action: char) {
        if !ignore && action == 'm' {
            self.apply_sgr(params);
        }
    }
}
