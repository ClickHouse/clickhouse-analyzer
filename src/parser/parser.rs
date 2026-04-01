use crate::lexer::token::{Token, TokenKind};
use crate::parser::diagnostic::{Parse, SyntaxError};
use crate::parser::event::Event;
use crate::parser::keyword::Keyword;
use crate::parser::marker::{CompletedMarker, Marker};
use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};
use crate::parser::token_set::TokenSet;
use std::cell::Cell;

const FUEL_LIMIT: u32 = 2048;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    fuel: Cell<u32>,
    events: Vec<Event>,
    errors: Vec<SyntaxError>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Parser {
        Parser {
            tokens,
            pos: 0,
            fuel: Cell::new(FUEL_LIMIT),
            events: Vec::new(),
            errors: Vec::new(),
        }
    }

    /// Returns the byte offset range of the current token,
    /// or the end-of-input position if at EOF.
    fn current_range(&self) -> (usize, usize) {
        if let Some(token) = self.tokens.get(self.pos) {
            (token.start, token.end)
        } else if let Some(last) = self.tokens.last() {
            (last.end, last.end)
        } else {
            (0, 0)
        }
    }

    fn push_error(&mut self, message: impl Into<String>) {
        let range = self.current_range();
        self.errors.push(SyntaxError {
            message: message.into(),
            range,
        });
    }

    pub fn build_tree(self) -> Parse {
        let errors = self.errors;
        let mut tokens = self.tokens.into_iter();
        let mut events = self.events;

        // Pop trailing Close for root node
        if matches!(events.last(), Some(Event::Close)) {
            events.pop();
        }

        let mut stack: Vec<SyntaxTree> = Vec::new();
        for event in events {
            match event {
                Event::Open { kind } => {
                    stack.push(SyntaxTree {
                        kind,
                        children: Vec::new(),
                    });
                }
                Event::Close => {
                    let Some(tree) = stack.pop() else {
                        continue;
                    };
                    let Some(parent) = stack.last_mut() else {
                        stack.push(tree);
                        continue;
                    };
                    parent.children.push(SyntaxChild::Tree(tree));
                }
                Event::Advance => {
                    let Some(token) = tokens.next() else {
                        continue;
                    };
                    let Some(parent) = stack.last_mut() else {
                        continue;
                    };
                    parent.children.push(SyntaxChild::Token(token));
                }
            }
        }

        let mut tree = stack.pop().unwrap_or(SyntaxTree {
            kind: SyntaxKind::File,
            children: Vec::new(),
        });

        // Fold any orphaned stack entries into root
        while let Some(orphan) = stack.pop() {
            tree.children.insert(0, SyntaxChild::Tree(orphan));
        }

        // Attach any unconsumed tokens
        for token in tokens {
            tree.children.push(SyntaxChild::Token(token));
        }

        Parse { tree, errors }
    }

    pub fn start(&mut self) -> Marker {
        let mark = Marker {
            index: self.events.len(),
        };
        self.events.push(Event::Open {
            kind: SyntaxKind::Error,
        });
        mark
    }

    pub fn precede(&mut self, m: CompletedMarker) -> Marker {
        let mark = Marker { index: m.index };
        self.events.insert(
            m.index,
            Event::Open {
                kind: SyntaxKind::Error,
            },
        );
        mark
    }

    /// Retroactively change the SyntaxKind of an already-completed node.
    pub fn change_kind(&mut self, m: CompletedMarker, kind: SyntaxKind) {
        self.events[m.index] = Event::Open { kind };
    }

    /// Returns the SyntaxKind of an already-completed node.
    pub fn kind_of(&self, m: CompletedMarker) -> SyntaxKind {
        match &self.events[m.index] {
            Event::Open { kind } => *kind,
            _ => SyntaxKind::Error,
        }
    }

    pub fn complete(&mut self, m: Marker, kind: SyntaxKind) -> CompletedMarker {
        self.events[m.index] = Event::Open { kind };
        self.events.push(Event::Close);
        CompletedMarker { index: m.index }
    }

    pub fn skip_trivia(&mut self) {
        while self.at_any_with_trivia(&[TokenKind::Whitespace, TokenKind::Comment]) && !self.eof() {
            self.advance();
        }
    }

    pub fn advance(&mut self) {
        if self.eof() {
            return;
        }
        self.fuel.set(FUEL_LIMIT);
        self.events.push(Event::Advance);
        self.pos += 1;
    }

    pub fn recover_with_error(&mut self, error: &str) {
        let m = self.start();
        self.push_error(error);
        self.complete(m, SyntaxKind::Error);
    }

    pub fn advance_with_error(&mut self, error: &str) {
        let m = self.start();
        self.push_error(error);
        if !self.eof() {
            self.advance();
        }
        self.complete(m, SyntaxKind::Error);
    }

    pub fn eof(&self) -> bool {
        self.pos == self.tokens.len()
    }

    pub fn end_of_statement(&mut self) -> bool {
        self.at(TokenKind::ClosingRoundBracket) || self.at(TokenKind::Semicolon) || self.eof()
    }

    pub fn nth(&mut self, lookahead: usize) -> TokenKind {
        self.skip_trivia();
        if self.fuel.get() == 0 {
            return TokenKind::EndOfStream;
        }
        self.fuel.set(self.fuel.get() - 1);
        if lookahead == 0 {
            self.tokens
                .get(self.pos)
                .map_or(TokenKind::EndOfStream, |it| it.kind)
        } else {
            // Skip trivia tokens for lookahead > 0
            let mut count = 0;
            let mut i = self.pos + 1;
            while i < self.tokens.len() {
                let kind = self.tokens[i].kind;
                if kind != TokenKind::Whitespace && kind != TokenKind::Comment {
                    count += 1;
                    if count == lookahead {
                        return kind;
                    }
                }
                i += 1;
            }
            TokenKind::EndOfStream
        }
    }

    pub fn nth_with_trivia(&self, lookahead: usize) -> TokenKind {
        if self.fuel.get() == 0 {
            return TokenKind::EndOfStream;
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or(TokenKind::EndOfStream, |it| it.kind)
    }

    pub fn at(&mut self, kind: TokenKind) -> bool {
        self.nth(0) == kind
    }

    pub fn at_with_trivia(&mut self, kind: TokenKind) -> bool {
        self.nth_with_trivia(0) == kind
    }

    pub fn at_set(&mut self, set: &TokenSet) -> bool {
        set.contains(self.nth(0))
    }

    pub fn at_any(&mut self, kinds: &[TokenKind]) -> bool {
        kinds.contains(&self.nth(0))
    }

    pub fn at_identifier(&mut self) -> bool {
        self.at_any(&[TokenKind::BareWord, TokenKind::QuotedIdentifier])
    }

    pub fn at_any_with_trivia(&mut self, kinds: &[TokenKind]) -> bool {
        kinds.contains(&self.nth_with_trivia(0))
    }

    pub fn eat(&mut self, kind: TokenKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn expect(&mut self, kind: TokenKind) {
        if self.eat(kind) {
            return;
        }
        self.push_error(format!("expected {kind}"));
    }

    pub fn nth_text(&mut self, lookahead: usize) -> &str {
        self.skip_trivia();
        if self.fuel.get() == 0 {
            return "";
        }
        self.fuel.set(self.fuel.get() - 1);
        if lookahead == 0 {
            self.tokens
                .get(self.pos)
                .map_or("", |it| it.text.as_str())
        } else {
            let mut count = 0;
            let mut i = self.pos + 1;
            while i < self.tokens.len() {
                let kind = self.tokens[i].kind;
                if kind != TokenKind::Whitespace && kind != TokenKind::Comment {
                    count += 1;
                    if count == lookahead {
                        return self.tokens[i].text.as_str();
                    }
                }
                i += 1;
            }
            ""
        }
    }

    pub fn nth_text_with_trivia(&mut self, lookahead: usize) -> &str {
        if self.fuel.get() == 0 {
            return "";
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or("", |it| it.text.as_str())
    }

    pub fn at_keyword(&mut self, keyword: Keyword) -> bool {
        self.nth(0) == TokenKind::BareWord
            && self.nth_text(0).eq_ignore_ascii_case(keyword.as_str())
    }

    /// True if the current (non-trivia) token is followed by '('.
    /// Useful to disambiguate keywords that can also be function names.
    pub fn at_followed_by_paren(&mut self) -> bool {
        self.nth(1) == TokenKind::OpeningRoundBracket
    }

    pub fn eat_keyword(&mut self, keyword: Keyword) -> bool {
        if self.at_keyword(keyword) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn expect_keyword(&mut self, keyword: Keyword) {
        if self.eat_keyword(keyword) {
            return;
        }
        self.push_error(format!("expected {}", keyword.as_str()));
    }
}
