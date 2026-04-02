use crate::lexer::token::Token;
use crate::parser::syntax_kind::SyntaxKind;

/// Maximum query size (can be configured)
const MAX_QUERY_SIZE: usize = 1_000_000; // 1MB

/// Tokenizer for ClickHouse SQL
pub struct Tokenizer<'a> {
    input: &'a str,
    chars: std::str::Chars<'a>,
    position: usize,
    start: usize,
    include_whitespace: bool,
}

impl<'a> Tokenizer<'a> {
    /// Create a new tokenizer for the given input
    pub fn new(input: &'a str) -> Self {
        // Check query size limit
        if input.len() > MAX_QUERY_SIZE {
            // We still create the tokenizer but will return an error token
            // when tokenizing starts
        }

        Self {
            input,
            chars: input.chars(),
            position: 0,
            start: 0,
            include_whitespace: true, // Default to including whitespace
        }
    }

    /// Set whether to include whitespace tokens in the output
    pub fn set_include_whitespace(&mut self, include: bool) -> &mut Self {
        self.include_whitespace = include;
        self
    }

    /// Tokenize the entire input
    pub fn tokenize(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();

        // Check for max query size
        if self.input.len() > MAX_QUERY_SIZE {
            tokens.push(self.error_token(SyntaxKind::ErrorMaxQuerySizeExceeded));
            tokens.push(self.eof_token());
            return tokens;
        }

        loop {
            let token = self.next_token();

            // Skip whitespace if not included
            if !self.include_whitespace
                && (token.kind == SyntaxKind::Whitespace || token.kind == SyntaxKind::Comment)
            {
                continue;
            }

            if token.kind == SyntaxKind::EndOfStream {
                break;
            }

            tokens.push(token.clone());
        }

        tokens
    }

    /// Get the next token
    pub fn next_token(&mut self) -> Token {
        self.start = self.position;

        // Check for end of input
        if self.is_at_end() {
            return self.eof_token();
        }

        let c = match self.advance() {
            Some(c) => c,
            None => return self.eof_token(),
        };

        // Handle whitespace
        if c.is_whitespace() {
            return self.read_whitespace();
        }

        // Handle comments
        if c == '-' && self.match_char('-') {
            return self.read_single_line_comment();
        }

        if c == '/' && self.match_char('*') {
            return self.read_multi_line_comment();
        }

        // Handle various token types
        match c {
            // Numbers
            '0'..='9' => self.read_number(),

            // String literals and quoted identifiers
            '\'' => self.read_string(
                '\'',
                SyntaxKind::StringToken,
                SyntaxKind::ErrorSingleQuoteIsNotClosed,
            ),
            '"' => self.read_string(
                '"',
                SyntaxKind::QuotedIdentifier,
                SyntaxKind::ErrorDoubleQuoteIsNotClosed,
            ),
            '`' => self.read_string(
                '`',
                SyntaxKind::QuotedIdentifier,
                SyntaxKind::ErrorBackQuoteIsNotClosed,
            ),

            // Brackets
            '(' => self.create_token(SyntaxKind::OpeningRoundBracket),
            ')' => self.create_token(SyntaxKind::ClosingRoundBracket),
            '[' => self.create_token(SyntaxKind::OpeningSquareBracket),
            ']' => self.create_token(SyntaxKind::ClosingSquareBracket),
            '{' => self.create_token(SyntaxKind::OpeningCurlyBrace),
            '}' => self.create_token(SyntaxKind::ClosingCurlyBrace),

            // Punctuation
            ',' => self.create_token(SyntaxKind::Comma),
            ';' => self.create_token(SyntaxKind::Semicolon),
            '.' => self.create_token(SyntaxKind::Dot),

            // Operators and symbols
            '*' => self.create_token(SyntaxKind::Star),
            '$' => self.create_token(SyntaxKind::DollarSign),
            '+' => self.create_token(SyntaxKind::Plus),
            '-' => {
                if self.match_char('>') {
                    self.create_token(SyntaxKind::Arrow)
                } else {
                    self.create_token(SyntaxKind::Minus)
                }
            }
            '/' => self.create_token(SyntaxKind::Slash),
            '%' => self.create_token(SyntaxKind::Percent),
            '?' => self.create_token(SyntaxKind::QuestionMark),
            ':' => {
                if self.match_char(':') {
                    self.create_token(SyntaxKind::DoubleColon)
                } else {
                    self.create_token(SyntaxKind::Colon)
                }
            }
            '^' => self.create_token(SyntaxKind::Caret),
            '=' => {
                if self.match_char('>') {
                    if self.match_char('<') {
                        self.create_token(SyntaxKind::Spaceship)
                    } else {
                        // Invalid, but treat as equals for now
                        self.create_token(SyntaxKind::Equals)
                    }
                } else if self.match_char('=') {
                    // `==` is treated as `=` in ClickHouse
                    self.create_token(SyntaxKind::Equals)
                } else {
                    self.create_token(SyntaxKind::Equals)
                }
            }
            '!' => {
                if self.match_char('=') {
                    self.create_token(SyntaxKind::NotEquals)
                } else {
                    self.create_token(SyntaxKind::ErrorSingleExclamationMark)
                }
            }
            '<' => {
                if self.match_char('=') {
                    if self.match_char('>') {
                        self.create_token(SyntaxKind::Spaceship)
                    } else {
                        self.create_token(SyntaxKind::LessOrEquals)
                    }
                } else if self.match_char('>') {
                    self.create_token(SyntaxKind::NotEquals)
                } else {
                    self.create_token(SyntaxKind::Less)
                }
            }
            '>' => {
                if self.match_char('=') {
                    self.create_token(SyntaxKind::GreaterOrEquals)
                } else {
                    self.create_token(SyntaxKind::Greater)
                }
            }
            '|' => {
                if self.match_char('|') {
                    self.create_token(SyntaxKind::Concatenation)
                } else {
                    self.create_token(SyntaxKind::ErrorSinglePipeMark)
                }
            }
            '@' => {
                if self.match_char('@') {
                    self.create_token(SyntaxKind::DoubleAt)
                } else {
                    self.create_token(SyntaxKind::At)
                }
            }

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => self.read_bare_word(),

            // Catch vertical delimiter - ClickHouse specific
            '\\' => {
                if self.match_char('G') || self.match_char('g') {
                    self.create_token(SyntaxKind::VerticalDelimiter)
                } else {
                    self.create_token(SyntaxKind::ErrorToken)
                }
            }

            // Anything else is an error
            _ => self.create_token(SyntaxKind::ErrorToken),
        }
    }

    /// Read whitespace characters
    fn read_whitespace(&mut self) -> Token {
        while let Some(c) = self.peek() {
            if c.is_whitespace() {
                self.advance();
            } else {
                break;
            }
        }

        self.create_token(SyntaxKind::Whitespace)
    }

    /// Read a single-line comment
    fn read_single_line_comment(&mut self) -> Token {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }

        self.create_token(SyntaxKind::Comment)
    }

    /// Read a multi-line comment
    fn read_multi_line_comment(&mut self) -> Token {
        let mut depth = 1;

        while depth > 0 {
            if let Some(c) = self.advance() {
                match c {
                    '/' if self.match_char('*') => depth += 1,
                    '*' if self.match_char('/') => depth -= 1,
                    _ => {}
                }
            } else {
                // Unclosed comment
                return self.create_token(SyntaxKind::ErrorMultilineCommentIsNotClosed);
            }
        }

        self.create_token(SyntaxKind::Comment)
    }

    /// Read a number (integer, float, hex, etc.)
    fn read_number(&mut self) -> Token {
        // Check if previous token was a dot - for chained tuple access operators (x.1.1)
        let prev_was_dot =
            self.start > 0 && self.input.as_bytes()[self.start - 1] == b'.';

        if prev_was_dot {
            // Simple integer parsing for tuple access
            self.read_digits();
        } else {
            // Check for hex/binary prefix
            let mut hex = false;

            if self.position - self.start == 1 && &self.input[self.start..self.position] == "0" {
                if let Some(next) = self.peek() {
                    match next {
                        'x' | 'X' => {
                            if let Some(next_next) = self.peek_next() {
                                if next_next.is_ascii_hexdigit() {
                                    self.advance(); // Consume 'x' or 'X'
                                    hex = true;
                                }
                            }
                        }
                        'b' | 'B' => {
                            if let Some(next_next) = self.peek_next() {
                                if next_next == '0' || next_next == '1' {
                                    self.advance(); // Consume 'b' or 'B'
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }

            // Read the main part of the number
            if hex {
                self.read_hex_digits();
            } else {
                self.read_digits();
            }

            // Decimal point
            if self.peek_is('.') {
                self.advance(); // Consume the decimal point

                if hex {
                    self.read_hex_digits();
                } else {
                    self.read_digits();
                }
            }

            // Exponentiation
            if let Some(c) = self.peek() {
                // Hex numbers use 'p'/'P', decimal numbers use 'e'/'E'
                if (hex && (c == 'p' || c == 'P')) || (!hex && (c == 'e' || c == 'E')) {
                    self.advance(); // Consume e/E/p/P

                    // Optional sign
                    if self.peek_is('+') || self.peek_is('-') {
                        self.advance();
                    }

                    // Exponent is always decimal
                    if !self.current_char_is_digit() {
                        return self.create_token(SyntaxKind::ErrorWrongNumber);
                    }

                    self.read_digits();
                }
            }
        }

        // Check if followed by identifier characters
        if !self.is_at_end() && self.peek().is_some_and(|c| c.is_alphabetic() || c == '_') {
            return self.read_identifier_starting_with_number();
        }

        self.create_token(SyntaxKind::Number)
    }

    /// Read hex digits, including underscore separators
    fn read_hex_digits(&mut self) {
        let mut start_of_block = true;

        while let Some(c) = self.peek() {
            if c.is_ascii_hexdigit() {
                self.advance();
                start_of_block = false;
            } else if c == '_' && !start_of_block {
                // Underscore separator is valid only between digits
                if let Some(next) = self.peek_next() {
                    if next.is_ascii_hexdigit() {
                        self.advance();
                        start_of_block = true;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Read decimal digits, including underscore separators
    fn read_digits(&mut self) {
        let mut start_of_block = true;

        while let Some(c) = self.peek() {
            if c.is_ascii_digit() {
                self.advance();
                start_of_block = false;
            } else if c == '_' && !start_of_block {
                // Underscore separator is valid only between digits
                if let Some(next) = self.peek_next() {
                    if next.is_ascii_digit() {
                        self.advance();
                        start_of_block = true;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Read binary digits (0 and 1), including underscore separators
    #[allow(dead_code)]
    fn read_binary_digits(&mut self) {
        let mut start_of_block = true;

        while let Some(c) = self.peek() {
            if c == '0' || c == '1' {
                self.advance();
                start_of_block = false;
            } else if c == '_' && !start_of_block {
                // Underscore separator is valid only between digits
                if let Some(next) = self.peek_next() {
                    if next == '0' || next == '1' {
                        self.advance();
                        start_of_block = true;
                    } else {
                        break;
                    }
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }

    /// Read an identifier that starts with a number (like 1name)
    fn read_identifier_starting_with_number(&mut self) -> Token {
        // Continue reading identifier characters
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' || c == '$' {
                self.advance();
            } else {
                break;
            }
        }

        // Validate the entire token as an identifier
        let lexeme = &self.input[self.start..self.position];
        let mut is_valid_identifier = true;

        for c in lexeme.chars() {
            if !c.is_alphanumeric() && c != '_' && c != '$' {
                is_valid_identifier = false;
                break;
            }
        }

        if is_valid_identifier {
            self.create_token(SyntaxKind::BareWord)
        } else {
            self.create_token(SyntaxKind::ErrorWrongNumber)
        }
    }

    /// Read a string or quoted identifier
    fn read_string(
        &mut self,
        quote: char,
        success_type: SyntaxKind,
        error_type: SyntaxKind,
    ) -> Token {
        let mut escaped = false;

        loop {
            match self.peek() {
                Some(c) if c == quote && !escaped => {
                    // Handle double quotes as escape sequences
                    if self.peek_next() == Some(quote) {
                        self.advance(); // Skip the first quote
                        self.advance(); // Skip the second quote
                        escaped = false;
                        continue;
                    }

                    // End of string
                    self.advance(); // Skip the closing quote
                    return self.create_token(success_type);
                }
                Some('\\') if !escaped => {
                    self.advance(); // Skip the backslash
                    escaped = true;
                }
                Some(_) => {
                    self.advance();
                    escaped = false;
                }
                None => {
                    // Unterminated string
                    return self.create_token(error_type);
                }
            }
        }
    }

    /// Read a bareword (identifier or keyword)
    fn read_bare_word(&mut self) -> Token {
        while let Some(c) = self.peek() {
            if c.is_alphanumeric() || c == '_' {
                self.advance();
            } else {
                break;
            }
        }

        // Check if it's a keyword (for information only, still returns BareWord)
        self.create_token(SyntaxKind::BareWord)
    }

    /// Create a token with the current lexeme
    fn create_token(&self, kind: SyntaxKind) -> Token {
        Token::new(kind, self.start as u32, self.position as u32)
    }

    /// Create an error token
    fn error_token(&self, kind: SyntaxKind) -> Token {
        Token::new(kind, self.position as u32, self.position as u32)
    }

    /// Create an EOF token
    fn eof_token(&self) -> Token {
        Token::new(SyntaxKind::EndOfStream, self.position as u32, self.position as u32)
    }

    /// Advance to the next character
    fn advance(&mut self) -> Option<char> {
        if let Some(c) = self.chars.next() {
            self.position += c.len_utf8();
            Some(c)
        } else {
            None
        }
    }

    /// Peek at the next character without advancing
    fn peek(&self) -> Option<char> {
        self.chars.clone().next()
    }

    /// Peek at the character after the next one
    fn peek_next(&self) -> Option<char> {
        let mut chars = self.chars.clone();
        chars.next(); // Skip the next character
        chars.next() // Get the one after
    }

    /// Check if the next character matches the expected one
    fn peek_is(&self, expected: char) -> bool {
        if let Some(c) = self.peek() {
            c == expected
        } else {
            false
        }
    }

    /// Check if the current character is a digit
    fn current_char_is_digit(&self) -> bool {
        if let Some(c) = self.peek() {
            c.is_ascii_digit()
        } else {
            false
        }
    }

    /// Check if the next character matches and consume it if it does
    fn match_char(&mut self, expected: char) -> bool {
        if self.peek_is(expected) {
            self.advance();
            true
        } else {
            false
        }
    }

    /// Check if the tokenizer has reached the end of input
    pub fn is_at_end(&self) -> bool {
        self.position >= self.input.len()
    }

    /// Tokenize up to a specific position
    #[allow(dead_code)]
    pub fn tokenize_up_to_position(&mut self, position: usize) -> Vec<Token> {
        let mut tokens = Vec::new();

        while !self.is_at_end() && self.position < position {
            let token = self.next_token();

            if !self.include_whitespace && token.kind == SyntaxKind::Whitespace {
                continue;
            }

            tokens.push(token.clone());

            if token.kind == SyntaxKind::EndOfStream {
                break;
            }
        }

        tokens
    }
}

/// Helper function to tokenize a SQL string, including whitespace
pub fn tokenize_with_whitespace(sql: &str) -> Vec<Token> {
    let mut tokenizer = Tokenizer::new(sql);
    tokenizer.set_include_whitespace(true);
    tokenizer.tokenize()
}

/// Helper function to tokenize a SQL string, excluding whitespace
#[allow(dead_code)]
pub fn tokenize(sql: &str) -> Vec<Token> {
    let mut tokenizer = Tokenizer::new(sql);
    tokenizer.set_include_whitespace(false);
    tokenizer.tokenize()
}

/// Helper function to tokenize up to a position, excluding whitespace
#[allow(dead_code)]
pub fn tokenize_up_to(sql: &str, position: usize) -> Vec<Token> {
    let mut tokenizer = Tokenizer::new(sql);
    tokenizer.set_include_whitespace(false);
    tokenizer.tokenize_up_to_position(position)
}

// Test module
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tokenize_select_query() {
        let sql = "SELECT * FROM system.numbers WHERE number > 1 LIMIT 5";
        let tokens = tokenize(sql);

        assert_eq!(tokens.len(), 12);

        assert_eq!(tokens[0].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[0].text(sql), "SELECT");

        assert_eq!(tokens[1].kind, SyntaxKind::Star);
        assert_eq!(tokens[1].text(sql), "*");

        assert_eq!(tokens[2].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[2].text(sql), "FROM");

        assert_eq!(tokens[3].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[3].text(sql), "system");

        assert_eq!(tokens[4].kind, SyntaxKind::Dot);
        assert_eq!(tokens[4].text(sql), ".");

        assert_eq!(tokens[5].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[5].text(sql), "numbers");

        assert_eq!(tokens[6].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[6].text(sql), "WHERE");

        assert_eq!(tokens[7].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[7].text(sql), "number");

        assert_eq!(tokens[8].kind, SyntaxKind::Greater);
        assert_eq!(tokens[8].text(sql), ">");

        assert_eq!(tokens[9].kind, SyntaxKind::Number);
        assert_eq!(tokens[9].text(sql), "1");

        assert_eq!(tokens[10].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[10].text(sql), "LIMIT");

        assert_eq!(tokens[11].kind, SyntaxKind::Number);
        assert_eq!(tokens[11].text(sql), "5");
    }

    #[test]
    fn test_tokenize_with_whitespace() {
        let sql = "SELECT * FROM";
        let tokens = tokenize_with_whitespace(sql);

        assert_eq!(tokens.len(), 5); // SELECT, WS, *, WS, FROM, EndOfStream
        assert_eq!(tokens[0].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[1].kind, SyntaxKind::Whitespace);
        assert_eq!(tokens[2].kind, SyntaxKind::Star);
        assert_eq!(tokens[3].kind, SyntaxKind::Whitespace);
        assert_eq!(tokens[4].kind, SyntaxKind::BareWord);
    }

    #[test]
    fn test_tokenize_string_literals() {
        let sql = "SELECT 'string literal', \"quoted identifier\", `backtick identifier`";

        let tokens = tokenize(sql);

        // Find string literal
        let string_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::StringToken)
            .unwrap();
        assert_eq!(string_token.text(sql), "'string literal'");

        // Find quoted identifiers
        let quoted_tokens: Vec<&Token> = tokens
            .iter()
            .filter(|t| t.kind == SyntaxKind::QuotedIdentifier)
            .collect();

        assert_eq!(quoted_tokens.len(), 2);
        assert_eq!(quoted_tokens[0].text(sql), "\"quoted identifier\"");
        assert_eq!(quoted_tokens[1].text(sql), "`backtick identifier`");
    }

    #[test]
    fn test_tokenize_numbers() {
        let sql = "SELECT 123, 123.456, 1.23e4, 1.23E-4, 0xFF, 0b101";

        let tokens = tokenize(sql);

        let number_tokens: Vec<&Token> = tokens
            .iter()
            .filter(|t| t.kind == SyntaxKind::Number)
            .collect();

        assert_eq!(number_tokens.len(), 6);
        assert_eq!(number_tokens[0].text(sql), "123");
        assert_eq!(number_tokens[1].text(sql), "123.456");
        assert_eq!(number_tokens[2].text(sql), "1.23e4");
        assert_eq!(number_tokens[3].text(sql), "1.23E-4");
        assert_eq!(number_tokens[4].text(sql), "0xFF");
        assert_eq!(number_tokens[5].text(sql), "0b101");
    }

    #[test]
    fn test_tokenize_comments() {
        let sql =
            "SELECT * -- This is a comment\nFROM table /* Multi\nline\ncomment */ WHERE id = 1";

        let tokens = tokenize(sql);

        // Comments should be excluded from the output
        assert!(!tokens.iter().any(|t| t.kind == SyntaxKind::Comment));

        // Test with whitespace and comments included
        let tokens_with_comments = tokenize_with_whitespace(sql);

        let comment_tokens: Vec<&Token> = tokens_with_comments
            .iter()
            .filter(|t| t.kind == SyntaxKind::Comment)
            .collect();

        assert_eq!(comment_tokens.len(), 2);
        assert_eq!(comment_tokens[0].text(sql), "-- This is a comment");
        assert_eq!(comment_tokens[1].text(sql), "/* Multi\nline\ncomment */");
    }

    #[test]
    fn test_tokenize_operators() {
        let sql = "SELECT a + b, c - d, e * f, g / h, i % j, k || l, m = n, o <=> p, q != r, s < t, u > v, w <= x, y >= z";

        let tokens = tokenize(sql);

        // Spot check a few operators
        let plus_token = tokens.iter().find(|t| t.kind == SyntaxKind::Plus).unwrap();
        assert_eq!(plus_token.text(sql), "+");

        let minus_token = tokens.iter().find(|t| t.kind == SyntaxKind::Minus).unwrap();
        assert_eq!(minus_token.text(sql), "-");

        let concat_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::Concatenation)
            .unwrap();
        assert_eq!(concat_token.text(sql), "||");

        let spaceship_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::Spaceship)
            .unwrap();
        assert_eq!(spaceship_token.text(sql), "<=>");

        let not_equals_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::NotEquals)
            .unwrap();
        assert_eq!(not_equals_token.text(sql), "!=");
    }

    #[test]
    fn test_tokenize_errors() {
        // Unterminated string
        let sql = "SELECT 'unterminated string";
        let tokens = tokenize(sql);

        let error_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::ErrorSingleQuoteIsNotClosed)
            .unwrap();
        assert_eq!(error_token.text(sql), "'unterminated string");

        // Unterminated multi-line comment
        let sql = "SELECT /* unterminated comment";
        let tokens = tokenize(sql);

        let error_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::ErrorMultilineCommentIsNotClosed)
            .unwrap();
        assert_eq!(error_token.text(sql), "/* unterminated comment");

        // Invalid use of pipe
        let sql = "SELECT column | other";
        let tokens = tokenize(sql);

        let error_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::ErrorSinglePipeMark)
            .unwrap();
        assert_eq!(error_token.text(sql), "|");
    }

    #[test]
    fn test_tokenize_up_to_position() {
        let sql = "SELECT * FROM system.numbers WHERE number > 1 LIMIT 5";

        // Position after "SELECT * FROM "
        let tokens = tokenize_up_to(sql, 14);

        // Should contain just SELECT, *, FROM (no whitespace)
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[0].text(sql), "SELECT");
        assert_eq!(tokens[1].kind, SyntaxKind::Star);
        assert_eq!(tokens[2].kind, SyntaxKind::BareWord);
        assert_eq!(tokens[2].text(sql), "FROM");
    }

    #[test]
    fn test_clickhouse_specific_tokens() {
        // Vertical delimiter
        let sql = "SELECT * FROM table\\G";
        let tokens = tokenize(sql);

        let vdelim_token = tokens
            .iter()
            .find(|t| t.kind == SyntaxKind::VerticalDelimiter)
            .unwrap();
        assert_eq!(vdelim_token.text(sql), "\\G");

        // Here-doc (if your implementation supports it)
        // let sql = "SELECT <<<EOF\nsome text\nEOF";
        // let tokens = tokenize(sql);
        //
        // let heredoc_token = tokens.iter().find(|t| t.kind == TokenType::HereDoc).unwrap();
        // assert!(heredoc_token.value.starts_with("<<<EOF"));
    }

    #[test]
    fn test_quoted_identifiers() {
        let sql = "SELECT `column.with.dots`, \"another.column\", * FROM `table.name`";
        let tokens = tokenize(sql);

        let quoted_identifiers: Vec<&Token> = tokens
            .iter()
            .filter(|t| t.kind == SyntaxKind::QuotedIdentifier)
            .collect();

        assert_eq!(quoted_identifiers.len(), 3);
        assert_eq!(quoted_identifiers[0].text(sql), "`column.with.dots`");
        assert_eq!(quoted_identifiers[1].text(sql), "\"another.column\"");
        assert_eq!(quoted_identifiers[2].text(sql), "`table.name`");
    }

    #[test]
    fn test_escaped_quotes() {
        // Single quotes with escaping
        let sql = "SELECT 'it\\'s a string', 'it''s another string'";
        let tokens = tokenize(sql);

        let string_literals: Vec<&Token> = tokens
            .iter()
            .filter(|t| t.kind == SyntaxKind::StringToken)
            .collect();

        assert_eq!(string_literals.len(), 2);
        assert_eq!(string_literals[0].text(sql), "'it\\'s a string'");
        assert_eq!(string_literals[1].text(sql), "'it''s another string'");
    }
}
