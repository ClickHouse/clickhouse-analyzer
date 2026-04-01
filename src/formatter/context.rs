use super::FormatConfig;

pub struct FormatterContext<'a> {
    config: &'a FormatConfig,
    buf: String,
    indent_level: usize,
    at_line_start: bool,
    needs_space: bool,
}

impl<'a> FormatterContext<'a> {
    pub fn new(config: &'a FormatConfig) -> Self {
        Self {
            config,
            buf: String::new(),
            indent_level: 0,
            at_line_start: true,
            needs_space: false,
        }
    }

    pub fn write_newline(&mut self) {
        self.buf.push('\n');
        self.at_line_start = true;
        self.needs_space = false;
    }

    pub fn write_token(&mut self, text: &str) {
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

    pub fn write_space(&mut self) {
        if !self.at_line_start {
            self.needs_space = true;
        }
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
