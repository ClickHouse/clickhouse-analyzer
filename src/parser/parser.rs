use crate::lexer::token::{Token, TokenKind};
use crate::parser::event::Event;
use crate::parser::keyword::Keyword;
use crate::parser::marker::{CompletedMarker, Marker};
use crate::parser::syntax_kind::SyntaxKind;
use crate::parser::syntax_tree::{SyntaxChild, SyntaxTree};
use crate::parser::token_set::TokenSet;
use std::cell::Cell;

pub struct Parser {
    tokens: Vec<Token>,
    pos: usize,
    fuel: Cell<u32>,
    events: Vec<Event>,
}

impl Parser {
    pub fn new(tokens: Vec<Token>) -> Parser {
        Parser {
            tokens,
            pos: 0,
            fuel: Cell::new(256),
            events: Vec::new(),
        }
    }

    pub fn build_tree(self) -> SyntaxTree {
        let mut tokens = self.tokens.into_iter();
        let mut events = self.events;

        assert!(matches!(events.pop(), Some(Event::Close)));
        let mut stack = Vec::new();
        for event in events {
            match event {
                Event::Open { kind } => stack.push(SyntaxTree {
                    kind,
                    children: Vec::new(),
                }),
                Event::Close => {
                    let tree = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(SyntaxChild::Tree(tree));
                }
                Event::Advance => {
                    let token = tokens.next().unwrap();
                    stack.last_mut().unwrap().children.push(SyntaxChild::Token(token));
                }
            }
        }

        let tree = stack.pop().unwrap();
        assert!(stack.is_empty());
        assert!(tokens.next().is_none());
        tree
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
        assert!(!self.eof());
        self.fuel.set(256);
        self.events.push(Event::Advance);
        self.pos += 1;
    }

    pub fn recover_with_error(&mut self, error: &str) {
        let m = self.start();
        // TODO: Error reporting.
        eprintln!("{error}");
        self.complete(m, SyntaxKind::Error);
    }

    pub fn advance_with_error(&mut self, error: &str) {
        let m = self.start();
        // TODO: Error reporting.
        eprintln!("{error}");
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
            panic!("parser is stuck")
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or(TokenKind::EndOfStream, |it| it.kind)
    }

    pub fn nth_with_trivia(&self, lookahead: usize) -> TokenKind {
        if self.fuel.get() == 0 {
            panic!("parser is stuck")
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
        // TODO: Error reporting.
        eprintln!("expected {kind:?}");
    }

    pub fn nth_text(&mut self, lookahead: usize) -> &str {
        self.skip_trivia();
        if self.fuel.get() == 0 {
            panic!("parser is stuck")
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or("", |it| it.text.as_str())
    }

    pub fn nth_text_with_trivia(&mut self, lookahead: usize) -> &str {
        if self.fuel.get() == 0 {
            panic!("parser is stuck")
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
        // TODO: Error reporting.
        eprintln!("expected {keyword:?}");
    }
}
