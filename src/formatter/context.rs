use super::FormatConfig;

pub struct FormatterContext<'a> {
    config: &'a FormatConfig,
    buf: String,
    indent_level: usize,
    at_line_start: bool,
    needs_space: bool,
    /// Set when a skipped whitespace token contained a newline.
    /// Used to decide whether a following comment should start on a new line.
    pending_newline: bool,
    /// Set when skipped whitespace contained a blank line (2+ newlines).
    pending_blank_line: bool,
}

impl<'a> FormatterContext<'a> {
    pub fn new(config: &'a FormatConfig) -> Self {
        Self {
            config,
            buf: String::new(),
            indent_level: 0,
            at_line_start: true,
            needs_space: false,
            pending_newline: false,
            pending_blank_line: false,
        }
    }

    pub fn write_newline(&mut self) {
        self.buf.push('\n');
        self.at_line_start = true;
        self.needs_space = false;
        self.pending_newline = false;
        self.pending_blank_line = false;
    }

    pub fn write_token(&mut self, text: &str) {
        self.pending_newline = false;
        self.pending_blank_line = false;
        if self.at_line_start {
            self.write_indent();
            self.at_line_start = false;
            self.needs_space = false;
        } else if self.needs_space {
            self.buf.push(' ');
            self.needs_space = false;
        }
        self.buf.push_str(text);
    }

    pub fn write_keyword(&mut self, text: &str) {
        if self.config.uppercase_keywords {
            self.write_token(&text.to_uppercase());
        } else {
            self.write_token(&text.to_lowercase());
        }
    }

    pub fn is_at_line_start(&self) -> bool {
        self.at_line_start
    }

    pub fn write_space(&mut self) {
        if !self.at_line_start {
            self.needs_space = true;
        }
    }

    /// Record skipped whitespace; if it contains a newline, note it
    /// so a following comment can preserve line placement.
    pub fn note_skipped_whitespace(&mut self, text: &str) {
        let newline_count = text.chars().filter(|&c| c == '\n').count();
        if newline_count >= 1 {
            self.pending_newline = true;
        }
        if newline_count >= 2 {
            self.pending_blank_line = true;
        }
    }

    /// Consume and return the pending-newline flag.
    pub fn take_pending_newline(&mut self) -> bool {
        let v = self.pending_newline;
        self.pending_newline = false;
        v
    }

    /// Consume and return the pending-blank-line flag.
    pub fn take_pending_blank_line(&mut self) -> bool {
        let v = self.pending_blank_line;
        self.pending_blank_line = false;
        v
    }

    /// Write `n` spaces directly (for comment alignment padding).
    pub fn write_padding(&mut self, n: usize) {
        for _ in 0..n {
            self.buf.push(' ');
        }
    }

    /// Create a child context that inherits config and indent but writes to
    /// its own buffer. Used for measuring formatted width.
    pub fn child(&self) -> FormatterContext<'a> {
        FormatterContext {
            config: self.config,
            buf: String::new(),
            indent_level: self.indent_level,
            at_line_start: true,
            needs_space: false,
            pending_newline: false,
            pending_blank_line: false,
        }
    }

    /// Return the current buffer contents (for measuring).
    pub fn output(&self) -> &str {
        &self.buf
    }

    pub fn indent(&mut self) {
        self.indent_level += 1;
    }

    pub fn dedent(&mut self) {
        self.indent_level = self.indent_level.saturating_sub(1);
    }

    pub fn finish(self) -> String {
        let trimmed = self.buf.trim_end();
        let mut result = trimmed.to_string();
        if !result.is_empty() {
            result.push('\n');
        }
        result
    }

    fn write_indent(&mut self) {
        let width = self.indent_level * self.config.indent_width;
        for _ in 0..width {
            self.buf.push(' ');
        }
    }
}
