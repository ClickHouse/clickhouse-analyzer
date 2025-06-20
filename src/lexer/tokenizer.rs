use std::collections::HashMap;
use crate::lexer::token::{Token, TokenKind};

/// ClickHouse Keywords
struct Keywords;

impl Keywords {
    pub fn get_map() -> HashMap<String, bool> {
        let mut keywords = HashMap::new();

        // Add ClickHouse keywords (case-insensitive)
        // These will be recognized as BareWord but can be checked
        // by the parser for keyword status
        let keyword_list = [
            "SELECT", "FROM", "WHERE", "GROUP", "BY", "HAVING", "ORDER",
            "LIMIT", "OFFSET", "UNION", "ALL", "EXCEPT", "INTERSECT",
            "JOIN", "ON", "USING", "PREWHERE", "INNER", "LEFT", "RIGHT",
            "FULL", "OUTER", "CROSS", "GLOBAL", "ANY", "AS", "DISTINCT",
            "INTO", "FORMAT", "SETTINGS", "INSERT", "VALUES", "DELETE",
            "WITH", "CREATE", "ALTER", "DROP", "DETACH", "ATTACH", "USE",
            "BETWEEN", "LIKE", "NOT", "AND", "OR", "IN", "ARRAY", "TUPLE",
            "MAP", "IS", "NULL", "CAST", "CASE", "WHEN", "THEN", "ELSE", "END",
            "TRUE", "FALSE", "FUNCTION", "TABLE", "VIEW", "DICTIONARY", "DATABASE"
        ];

        for keyword in keyword_list.iter() {
            keywords.insert(keyword.to_lowercase(), true);
        }

        keywords
    }
}

/// Maximum query size (can be configured)
const MAX_QUERY_SIZE: usize = 1_000_000; // 1MB

/// Tokenizer for ClickHouse SQL
pub struct Tokenizer<'a> {
    input: &'a str,
    chars: std::str::Chars<'a>,
    position: usize,
    start: usize,
    line: usize,
    column: usize,
    keywords: HashMap<String, bool>,
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
            line: 1,
            column: 1,
            keywords: Keywords::get_map(),
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
            tokens.push(self.error_token(TokenKind::ErrorMaxQuerySizeExceeded));
            tokens.push(self.eof_token());
            return tokens;
        }

        loop {
            let token = self.next_token();

            // Skip whitespace if not included
            if !self.include_whitespace && (token.kind == TokenKind::Whitespace || token.kind == TokenKind::Comment) {
                continue;
            }

            if token.kind == TokenKind::EndOfStream {
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
            '\'' => self.read_string('\'', TokenKind::StringLiteral, TokenKind::ErrorSingleQuoteIsNotClosed),
            '"' => self.read_string('"', TokenKind::QuotedIdentifier, TokenKind::ErrorDoubleQuoteIsNotClosed),
            '`' => self.read_string('`', TokenKind::QuotedIdentifier, TokenKind::ErrorBackQuoteIsNotClosed),

            // Brackets
            '(' => self.create_token(TokenKind::OpeningRoundBracket),
            ')' => self.create_token(TokenKind::ClosingRoundBracket),
            '[' => self.create_token(TokenKind::OpeningSquareBracket),
            ']' => self.create_token(TokenKind::ClosingSquareBracket),
            '{' => self.create_token(TokenKind::OpeningCurlyBrace),
            '}' => self.create_token(TokenKind::ClosingCurlyBrace),

            // Punctuation
            ',' => self.create_token(TokenKind::Comma),
            ';' => self.create_token(TokenKind::Semicolon),
            '.' => self.create_token(TokenKind::Dot),

            // Operators and symbols
            '*' => self.create_token(TokenKind::Asterisk),
            '$' => self.create_token(TokenKind::DollarSign),
            '+' => self.create_token(TokenKind::Plus),
            '-' => {
                if self.match_char('>') {
                    self.create_token(TokenKind::Arrow)
                } else {
                    self.create_token(TokenKind::Minus)
                }
            },
            '/' => self.create_token(TokenKind::Slash),
            '%' => self.create_token(TokenKind::Percent),
            '?' => self.create_token(TokenKind::QuestionMark),
            ':' => {
                if self.match_char(':') {
                    self.create_token(TokenKind::DoubleColon)
                } else {
                    self.create_token(TokenKind::Colon)
                }
            },
            '^' => self.create_token(TokenKind::Caret),
            '=' => {
                if self.match_char('>') {
                    if self.match_char('<') {
                        self.create_token(TokenKind::Spaceship)
                    } else {
                        // Invalid, but treat as equals for now
                        self.create_token(TokenKind::Equals)
                    }
                } else {
                    self.create_token(TokenKind::Equals)
                }
            },
            '!' => {
                if self.match_char('=') {
                    self.create_token(TokenKind::NotEquals)
                } else {
                    self.create_token(TokenKind::ErrorSingleExclamationMark)
                }
            },
            '<' => {
                if self.match_char('=') {
                    if self.match_char('>') {
                        self.create_token(TokenKind::Spaceship)
                    } else {
                        self.create_token(TokenKind::LessOrEquals)
                    }
                } else if self.match_char('>') {
                    self.create_token(TokenKind::NotEquals)
                } else {
                    self.create_token(TokenKind::Less)
                }
            },
            '>' => {
                if self.match_char('=') {
                    self.create_token(TokenKind::GreaterOrEquals)
                } else {
                    self.create_token(TokenKind::Greater)
                }
            },
            '|' => {
                if self.match_char('|') {
                    self.create_token(TokenKind::Concatenation)
                } else {
                    self.create_token(TokenKind::ErrorSinglePipeMark)
                }
            },
            '@' => {
                if self.match_char('@') {
                    self.create_token(TokenKind::DoubleAt)
                } else {
                    self.create_token(TokenKind::At)
                }
            },

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => self.read_bare_word(),

            // Catch vertical delimiter - ClickHouse specific
            '\\' => {
                if self.match_char('G') || self.match_char('g') {
                    self.create_token(TokenKind::VerticalDelimiter)
                } else {
                    self.create_token(TokenKind::Error)
                }
            },

            // Anything else is an error
            _ => self.create_token(TokenKind::Error),
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

        self.create_token(TokenKind::Whitespace)
    }

    /// Read a single-line comment
    fn read_single_line_comment(&mut self) -> Token {
        while let Some(c) = self.peek() {
            if c == '\n' {
                break;
            }
            self.advance();
        }

        self.create_token(TokenKind::Comment)
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
                return self.create_token(TokenKind::ErrorMultilineCommentIsNotClosed);
            }
        }

        self.create_token(TokenKind::Comment)
    }

    /// Read a number (integer, float, hex, etc.)
    fn read_number(&mut self) -> Token {
        // Check if previous token was a dot - for chained tuple access operators (x.1.1)
        let prev_was_dot = self.position > 0 &&
            self.start > 0 &&
            &self.input[self.start-1..self.start] == ".";

        if prev_was_dot {
            // Simple integer parsing for tuple access
            self.read_digits();
        } else {
            // Check for hex/binary prefix
            let mut hex = false;

            if self.position - self.start == 1 &&
                &self.input[self.start..self.position] == "0" {

                if let Some(next) = self.peek() {
                    match next {
                        'x' | 'X' => {
                            if let Some(next_next) = self.peek_next() {
                                if next_next.is_ascii_hexdigit() {
                                    self.advance(); // Consume 'x' or 'X'
                                    hex = true;
                                }
                            }
                        },
                        'b' | 'B' => {
                            if let Some(next_next) = self.peek_next() {
                                if next_next == '0' || next_next == '1' {
                                    self.advance(); // Consume 'b' or 'B'
                                }
                            }
                        },
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
                        return self.create_token(TokenKind::ErrorWrongNumber);
                    }

                    self.read_digits();
                }
            }
        }

        // Check if followed by identifier characters
        if !self.is_at_end() && self.peek().map_or(false, |c| c.is_alphabetic() || c == '_') {
            return self.read_identifier_starting_with_number();
        }

        self.create_token(TokenKind::Number)
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
            self.create_token(TokenKind::BareWord)
        } else {
            self.create_token(TokenKind::ErrorWrongNumber)
        }
    }

    /// Read a string or quoted identifier
    fn read_string(&mut self, quote: char, success_type: TokenKind, error_type: TokenKind) -> Token {
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
                },
                Some('\\') if !escaped => {
                    self.advance(); // Skip the backslash
                    escaped = true;
                },
                Some(_) => {
                    self.advance();
                    escaped = false;
                },
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
        self.create_token(TokenKind::BareWord)
    }

    /// Create a token with the current lexeme
    fn create_token(&self, kind: TokenKind) -> Token {
        let lexeme = &self.input[self.start..self.position];

        // For multi-line tokens, we need special handling for column calculation
        let is_multiline = lexeme.contains('\n');

        let token_column = if is_multiline {
            // For multi-line tokens, set column to start of the token
            // Calculate the column at the start of the token
            let mut col = 1;
            let mut current_pos = 0;

            // Find the last line break before start
            for (i, c) in self.input[..self.start].char_indices() {
                if c == '\n' {
                    current_pos = i + 1; // Position after line break
                    col = 1; // Reset column count
                } else {
                    col += 1;
                }
            }

            col
        } else {
            // For single-line tokens, use the current column minus lexeme length
            // This might need adjustment if you have UTF-8 characters
            self.column - lexeme.len()
        };

        Token::new(
            kind,
            lexeme.to_string(),
            self.start,
            self.position,
            self.line - lexeme.chars().filter(|&c| c == '\n').count(), // Adjust line for token's start
            token_column,
        )
    }

    /// Create an error token
    fn error_token(&self, kind: TokenKind) -> Token {
        Token::new(
            kind,
            "".to_string(),
            self.position,
            self.position,
            self.line,
            self.column,
        )
    }

    /// Create an EOF token
    fn eof_token(&self) -> Token {
        Token::new(
            TokenKind::EndOfStream,
            "".to_string(),
            self.position,
            self.position,
            self.line,
            self.column,
        )
    }

    /// Advance to the next character
    fn advance(&mut self) -> Option<char> {
        if let Some(c) = self.chars.next() {
            self.position += c.len_utf8();

            if c == '\n' {
                self.line += 1;
                self.column = 1;
            } else {
                self.column += 1;
            }

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
    pub fn tokenize_up_to_position(&mut self, position: usize) -> Vec<Token> {
        let mut tokens = Vec::new();

        while !self.is_at_end() && self.position < position {
            let token = self.next_token();

            if !self.include_whitespace && token.kind == TokenKind::Whitespace {
                continue;
            }

            tokens.push(token.clone());

            if token.kind == TokenKind::EndOfStream {
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
pub fn tokenize(sql: &str) -> Vec<Token> {
    let mut tokenizer = Tokenizer::new(sql);
    tokenizer.set_include_whitespace(false);
    tokenizer.tokenize()
}

/// Helper function to tokenize up to a position, excluding whitespace
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

        assert_eq!(tokens[0].kind, TokenKind::BareWord);
        assert_eq!(tokens[0].text, "SELECT");

        assert_eq!(tokens[1].kind, TokenKind::Asterisk);
        assert_eq!(tokens[1].text, "*");

        assert_eq!(tokens[2].kind, TokenKind::BareWord);
        assert_eq!(tokens[2].text, "FROM");

        assert_eq!(tokens[3].kind, TokenKind::BareWord);
        assert_eq!(tokens[3].text, "system");

        assert_eq!(tokens[4].kind, TokenKind::Dot);
        assert_eq!(tokens[4].text, ".");

        assert_eq!(tokens[5].kind, TokenKind::BareWord);
        assert_eq!(tokens[5].text, "numbers");

        assert_eq!(tokens[6].kind, TokenKind::BareWord);
        assert_eq!(tokens[6].text, "WHERE");

        assert_eq!(tokens[7].kind, TokenKind::BareWord);
        assert_eq!(tokens[7].text, "number");

        assert_eq!(tokens[8].kind, TokenKind::Greater);
        assert_eq!(tokens[8].text, ">");

        assert_eq!(tokens[9].kind, TokenKind::Number);
        assert_eq!(tokens[9].text, "1");

        assert_eq!(tokens[10].kind, TokenKind::BareWord);
        assert_eq!(tokens[10].text, "LIMIT");

        assert_eq!(tokens[11].kind, TokenKind::Number);
        assert_eq!(tokens[11].text, "5");
    }

    #[test]
    fn test_tokenize_with_whitespace() {
        let sql = "SELECT * FROM";
        let tokens = tokenize_with_whitespace(sql);

        assert_eq!(tokens.len(), 5); // SELECT, WS, *, WS, FROM, EndOfStream
        assert_eq!(tokens[0].kind, TokenKind::BareWord);
        assert_eq!(tokens[1].kind, TokenKind::Whitespace);
        assert_eq!(tokens[2].kind, TokenKind::Asterisk);
        assert_eq!(tokens[3].kind, TokenKind::Whitespace);
        assert_eq!(tokens[4].kind, TokenKind::BareWord);
    }

    #[test]
    fn test_tokenize_string_literals() {
        let sql = "SELECT 'string literal', \"quoted identifier\", `backtick identifier`";

        let tokens = tokenize(sql);

        // Find string literal
        let string_token = tokens.iter().find(|t| t.kind == TokenKind::StringLiteral).unwrap();
        assert_eq!(string_token.text, "'string literal'");

        // Find quoted identifiers
        let quoted_tokens: Vec<&Token> = tokens.iter()
            .filter(|t| t.kind == TokenKind::QuotedIdentifier)
            .collect();

        assert_eq!(quoted_tokens.len(), 2);
        assert_eq!(quoted_tokens[0].text, "\"quoted identifier\"");
        assert_eq!(quoted_tokens[1].text, "`backtick identifier`");
    }

    #[test]
    fn test_tokenize_numbers() {
        let sql = "SELECT 123, 123.456, 1.23e4, 1.23E-4, 0xFF, 0b101";

        let tokens = tokenize(sql);

        let number_tokens: Vec<&Token> = tokens.iter()
            .filter(|t| t.kind == TokenKind::Number)
            .collect();

        assert_eq!(number_tokens.len(), 6);
        assert_eq!(number_tokens[0].text, "123");
        assert_eq!(number_tokens[1].text, "123.456");
        assert_eq!(number_tokens[2].text, "1.23e4");
        assert_eq!(number_tokens[3].text, "1.23E-4");
        assert_eq!(number_tokens[4].text, "0xFF");
        assert_eq!(number_tokens[5].text, "0b101");
    }

    #[test]
    fn test_tokenize_comments() {
        let sql = "SELECT * -- This is a comment\nFROM table /* Multi\nline\ncomment */ WHERE id = 1";

        let tokens = tokenize(sql);

        // Comments should be excluded from the output
        assert!(!tokens.iter().any(|t| t.kind == TokenKind::Comment));

        // Test with whitespace and comments included
        let tokens_with_comments = tokenize_with_whitespace(sql);

        let comment_tokens: Vec<&Token> = tokens_with_comments.iter()
            .filter(|t| t.kind == TokenKind::Comment)
            .collect();

        assert_eq!(comment_tokens.len(), 2);
        assert_eq!(comment_tokens[0].text, "-- This is a comment");
        assert_eq!(comment_tokens[1].text, "/* Multi\nline\ncomment */");
    }

    #[test]
    fn test_tokenize_operators() {
        let sql = "SELECT a + b, c - d, e * f, g / h, i % j, k || l, m = n, o <=> p, q != r, s < t, u > v, w <= x, y >= z";

        let tokens = tokenize(sql);

        // Spot check a few operators
        let plus_token = tokens.iter().find(|t| t.kind == TokenKind::Plus).unwrap();
        assert_eq!(plus_token.text, "+");

        let minus_token = tokens.iter().find(|t| t.kind == TokenKind::Minus).unwrap();
        assert_eq!(minus_token.text, "-");

        let concat_token = tokens.iter().find(|t| t.kind == TokenKind::Concatenation).unwrap();
        assert_eq!(concat_token.text, "||");

        let spaceship_token = tokens.iter().find(|t| t.kind == TokenKind::Spaceship).unwrap();
        assert_eq!(spaceship_token.text, "<=>");

        let not_equals_token = tokens.iter().find(|t| t.kind == TokenKind::NotEquals).unwrap();
        assert_eq!(not_equals_token.text, "!=");
    }

    #[test]
    fn test_tokenize_errors() {
        // Unterminated string
        let sql = "SELECT 'unterminated string";
        let tokens = tokenize(sql);

        let error_token = tokens.iter().find(|t| t.kind == TokenKind::ErrorSingleQuoteIsNotClosed).unwrap();
        assert_eq!(error_token.text, "'unterminated string");

        // Unterminated multi-line comment
        let sql = "SELECT /* unterminated comment";
        let tokens = tokenize(sql);

        let error_token = tokens.iter().find(|t| t.kind == TokenKind::ErrorMultilineCommentIsNotClosed).unwrap();
        assert_eq!(error_token.text, "/* unterminated comment");

        // Invalid use of pipe
        let sql = "SELECT column | other";
        let tokens = tokenize(sql);

        let error_token = tokens.iter().find(|t| t.kind == TokenKind::ErrorSinglePipeMark).unwrap();
        assert_eq!(error_token.text, "|");
    }

    #[test]
    fn test_tokenize_up_to_position() {
        let sql = "SELECT * FROM system.numbers WHERE number > 1 LIMIT 5";

        // Position after "SELECT * FROM "
        let tokens = tokenize_up_to(sql, 14);

        // Should contain just SELECT, *, FROM (no whitespace)
        assert_eq!(tokens.len(), 3);
        assert_eq!(tokens[0].kind, TokenKind::BareWord);
        assert_eq!(tokens[0].text, "SELECT");
        assert_eq!(tokens[1].kind, TokenKind::Asterisk);
        assert_eq!(tokens[2].kind, TokenKind::BareWord);
        assert_eq!(tokens[2].text, "FROM");
    }

    #[test]
    fn test_clickhouse_specific_tokens() {
        // Vertical delimiter
        let sql = "SELECT * FROM table\\G";
        let tokens = tokenize(sql);

        let vdelim_token = tokens.iter().find(|t| t.kind == TokenKind::VerticalDelimiter).unwrap();
        assert_eq!(vdelim_token.text, "\\G");

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

        let quoted_identifiers: Vec<&Token> = tokens.iter()
            .filter(|t| t.kind == TokenKind::QuotedIdentifier)
            .collect();

        assert_eq!(quoted_identifiers.len(), 3);
        assert_eq!(quoted_identifiers[0].text, "`column.with.dots`");
        assert_eq!(quoted_identifiers[1].text, "\"another.column\"");
        assert_eq!(quoted_identifiers[2].text, "`table.name`");
    }

    #[test]
    fn test_escaped_quotes() {
        // Single quotes with escaping
        let sql = "SELECT 'it\\'s a string', 'it''s another string'";
        let tokens = tokenize(sql);

        let string_literals: Vec<&Token> = tokens.iter()
            .filter(|t| t.kind == TokenKind::StringLiteral)
            .collect();

        assert_eq!(string_literals.len(), 2);
        assert_eq!(string_literals[0].text, "'it\\'s a string'");
        assert_eq!(string_literals[1].text, "'it''s another string'");
    }
}
