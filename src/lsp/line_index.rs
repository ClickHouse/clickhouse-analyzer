/// Maps between byte offsets and LSP `Position` (line, UTF-16 character offset).
///
/// The LSP protocol defaults to UTF-16 code-unit offsets for the character field.
/// Our parser stores byte offsets, so this struct bridges the two worlds.
pub struct LineIndex {
    /// Byte offset of the start of each line (including line 0 at offset 0).
    line_starts: Vec<u32>,
    source: String,
}

impl LineIndex {
    pub fn new(source: &str) -> Self {
        let mut line_starts = vec![0u32];
        for (i, b) in source.bytes().enumerate() {
            if b == b'\n' {
                line_starts.push((i + 1) as u32);
            }
        }
        Self {
            line_starts,
            source: source.to_owned(),
        }
    }

    /// Convert a byte offset to an LSP `Position` (0-based line, UTF-16 character offset).
    pub fn position(&self, offset: u32) -> tower_lsp::lsp_types::Position {
        let offset = offset.min(self.source.len() as u32);
        let line = self.line_starts.partition_point(|&start| start <= offset) - 1;
        let line_start = self.line_starts[line] as usize;
        let col_bytes = (offset as usize) - line_start;
        // Count UTF-16 code units for the slice from line_start to offset.
        let col_utf16 = self.source[line_start..line_start + col_bytes]
            .chars()
            .map(|c| c.len_utf16())
            .sum::<usize>();
        tower_lsp::lsp_types::Position {
            line: line as u32,
            character: col_utf16 as u32,
        }
    }

    /// Convert an LSP `Position` back to a byte offset.
    pub fn offset(&self, position: tower_lsp::lsp_types::Position) -> u32 {
        let line = position.line as usize;
        if line >= self.line_starts.len() {
            return self.source.len() as u32;
        }
        let line_start = self.line_starts[line] as usize;
        let line_end = self
            .line_starts
            .get(line + 1)
            .map(|&s| s as usize)
            .unwrap_or(self.source.len());
        let line_text = &self.source[line_start..line_end];
        let mut utf16_count = 0u32;
        let mut byte_offset = 0usize;
        for c in line_text.chars() {
            if utf16_count >= position.character {
                break;
            }
            utf16_count += c.len_utf16() as u32;
            byte_offset += c.len_utf8();
        }
        // Clamp to document bounds
        (line_start + byte_offset).min(self.source.len()) as u32
    }

    /// Convert a byte range `(start, end)` to an LSP `Range`.
    pub fn range(&self, start: u32, end: u32) -> tower_lsp::lsp_types::Range {
        tower_lsp::lsp_types::Range {
            start: self.position(start),
            end: self.position(end),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_line() {
        let idx = LineIndex::new("SELECT 1");
        assert_eq!(idx.position(0), tower_lsp::lsp_types::Position { line: 0, character: 0 });
        assert_eq!(idx.position(7), tower_lsp::lsp_types::Position { line: 0, character: 7 });
    }

    #[test]
    fn multi_line() {
        let idx = LineIndex::new("SELECT\n  1\nFROM t");
        assert_eq!(idx.position(0), tower_lsp::lsp_types::Position { line: 0, character: 0 });
        assert_eq!(idx.position(7), tower_lsp::lsp_types::Position { line: 1, character: 0 });
        assert_eq!(idx.position(11), tower_lsp::lsp_types::Position { line: 2, character: 0 });
        assert_eq!(idx.position(17), tower_lsp::lsp_types::Position { line: 2, character: 6 });
    }

    #[test]
    fn utf16_multibyte() {
        // '€' is 3 bytes in UTF-8, 1 code unit in UTF-16.
        // '𐍈' is 4 bytes in UTF-8, 2 code units in UTF-16 (surrogate pair).
        let idx = LineIndex::new("a€𐍈b");
        assert_eq!(idx.position(0), tower_lsp::lsp_types::Position { line: 0, character: 0 }); // 'a'
        assert_eq!(idx.position(1), tower_lsp::lsp_types::Position { line: 0, character: 1 }); // start of '€'
        assert_eq!(idx.position(4), tower_lsp::lsp_types::Position { line: 0, character: 2 }); // start of '𐍈'
        assert_eq!(idx.position(8), tower_lsp::lsp_types::Position { line: 0, character: 4 }); // 'b' (2 UTF-16 units for 𐍈)
    }

    #[test]
    fn roundtrip() {
        let text = "SELECT\n  1 + 2\nFROM t";
        let idx = LineIndex::new(text);
        for offset in 0..text.len() as u32 {
            // Only test at char boundaries
            if text.is_char_boundary(offset as usize) {
                let pos = idx.position(offset);
                assert_eq!(idx.offset(pos), offset, "roundtrip failed for offset {offset}");
            }
        }
    }
}
