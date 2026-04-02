use crate::lexer::token::Token;
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
    source: String,
}

impl Parser {
    pub fn new(tokens: Vec<Token>, source: String) -> Parser {
        Parser {
            tokens,
            pos: 0,
            fuel: Cell::new(FUEL_LIMIT),
            events: Vec::new(),
            errors: Vec::new(),
            source,
        }
    }

    /// Returns the byte offset range of the current token,
    /// or the end-of-input position if at EOF.
    fn current_range(&self) -> (usize, usize) {
        if let Some(token) = self.tokens.get(self.pos) {
            (token.start as usize, token.end as usize)
        } else if let Some(last) = self.tokens.last() {
            (last.end as usize, last.end as usize)
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
        let source = self.source;
        let mut tokens = self.tokens.into_iter();
        let events = self.events;
        let event_count = events.len();

        // Track which event indices were already opened via forward_parent chains
        let mut opened_via_fp: Vec<bool> = vec![false; event_count];

        // Pre-scan: mark all forward_parent targets so we skip them during linear walk
        for i in 0..event_count {
            if let Event::Open { forward_parent: Some(_), .. } = &events[i] {
                let mut cur = i;
                loop {
                    match &events[cur] {
                        Event::Open { forward_parent: Some(next), .. } => {
                            opened_via_fp[*next as usize] = true;
                            cur = *next as usize;
                        }
                        _ => break,
                    }
                }
            }
        }

        let mut stack: Vec<SyntaxTree> = Vec::new();
        for i in 0..event_count {
            match &events[i] {
                Event::Open { kind, forward_parent } => {
                    // Skip events that were already opened as part of a forward_parent chain
                    if opened_via_fp[i] {
                        continue;
                    }

                    if forward_parent.is_some() {
                        // Collect the chain: self -> fp1 -> fp2 -> ... -> last
                        let mut chain = vec![i];
                        let mut cur = i;
                        loop {
                            match &events[cur] {
                                Event::Open { forward_parent: Some(next), .. } => {
                                    chain.push(*next as usize);
                                    cur = *next as usize;
                                }
                                _ => break,
                            }
                        }
                        // Open wrapper nodes from outermost (end of chain) to innermost,
                        // then the original node last
                        for &idx in chain.iter().rev() {
                            let k = match &events[idx] {
                                Event::Open { kind, .. } => *kind,
                                _ => SyntaxKind::Error,
                            };
                            stack.push(SyntaxTree {
                                kind: k,
                                children: Vec::new(),
                                start: u32::MAX,
                                end: 0,
                            });
                        }
                    } else {
                        stack.push(SyntaxTree {
                            kind: *kind,
                            children: Vec::new(),
                            start: u32::MAX,
                            end: 0,
                        });
                    }
                }
                Event::Close => {
                    // Skip the final Close for the root node
                    if i == event_count - 1 {
                        continue;
                    }
                    let Some(tree) = stack.pop() else {
                        continue;
                    };
                    let Some(parent) = stack.last_mut() else {
                        stack.push(tree);
                        continue;
                    };
                    // Propagate child range to parent
                    if tree.start < parent.start {
                        parent.start = tree.start;
                    }
                    if tree.end > parent.end {
                        parent.end = tree.end;
                    }
                    parent.children.push(SyntaxChild::Tree(tree));
                }
                Event::Advance => {
                    let Some(token) = tokens.next() else {
                        continue;
                    };
                    let Some(parent) = stack.last_mut() else {
                        continue;
                    };
                    // Update parent range from token
                    if token.start < parent.start {
                        parent.start = token.start;
                    }
                    if token.end > parent.end {
                        parent.end = token.end;
                    }
                    parent.children.push(SyntaxChild::Token(token));
                }
            }
        }

        let mut tree = stack.pop().unwrap_or(SyntaxTree {
            kind: SyntaxKind::File,
            children: Vec::new(),
            start: u32::MAX,
            end: 0,
        });

        // Fold any orphaned stack entries into root
        while let Some(orphan) = stack.pop() {
            if orphan.start < tree.start {
                tree.start = orphan.start;
            }
            if orphan.end > tree.end {
                tree.end = orphan.end;
            }
            tree.children.insert(0, SyntaxChild::Tree(orphan));
        }

        // Attach any unconsumed tokens
        for token in tokens {
            if token.start < tree.start {
                tree.start = token.start;
            }
            if token.end > tree.end {
                tree.end = token.end;
            }
            tree.children.push(SyntaxChild::Token(token));
        }

        Parse { tree, errors, source }
    }

    pub fn start(&mut self) -> Marker {
        let mark = Marker {
            index: self.events.len(),
        };
        self.events.push(Event::Open {
            kind: SyntaxKind::Error,
            forward_parent: None,
        });
        mark
    }

    /// Wrap an already-completed node in a new parent node.
    /// Instead of inserting into the middle of the events vec (which would
    /// invalidate all subsequent marker indices), we append the new Open event
    /// at the end and set a forward_parent pointer on the original node.
    pub fn precede(&mut self, m: CompletedMarker) -> Marker {
        let new_index = self.events.len();
        self.events.push(Event::Open {
            kind: SyntaxKind::Error,
            forward_parent: None,
        });
        // Point the original node's Open event to the new wrapper
        match &mut self.events[m.index] {
            Event::Open { forward_parent: _, .. } => {
                // Follow any existing chain to the end
                let mut target = m.index;
                loop {
                    match &self.events[target] {
                        Event::Open { forward_parent: Some(next), .. } => {
                            target = *next as usize;
                        }
                        _ => break,
                    }
                }
                match &mut self.events[target] {
                    Event::Open { forward_parent, .. } => {
                        *forward_parent = Some(new_index as u32);
                    }
                    _ => {}
                }
            }
            _ => {}
        }
        Marker { index: new_index }
    }

    /// Retroactively change the SyntaxKind of an already-completed node.
    pub fn change_kind(&mut self, m: CompletedMarker, kind: SyntaxKind) {
        match &mut self.events[m.index] {
            Event::Open { kind: k, .. } => *k = kind,
            _ => {}
        }
    }

    /// Returns the SyntaxKind of an already-completed node.
    pub fn kind_of(&self, m: CompletedMarker) -> SyntaxKind {
        match &self.events[m.index] {
            Event::Open { kind, .. } => *kind,
            _ => SyntaxKind::Error,
        }
    }

    pub fn complete(&mut self, m: Marker, kind: SyntaxKind) -> CompletedMarker {
        match &mut self.events[m.index] {
            Event::Open { kind: k, .. } => *k = kind,
            _ => {}
        }
        self.events.push(Event::Close);
        CompletedMarker { index: m.index }
    }

    pub fn skip_trivia(&mut self) {
        while self.at_any_with_trivia(&[SyntaxKind::Whitespace, SyntaxKind::Comment]) && !self.eof() {
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
        self.at(SyntaxKind::ClosingRoundBracket) || self.at(SyntaxKind::Semicolon) || self.eof()
    }

    pub fn nth(&mut self, lookahead: usize) -> SyntaxKind {
        self.skip_trivia();
        if self.fuel.get() == 0 {
            return SyntaxKind::EndOfStream;
        }
        self.fuel.set(self.fuel.get() - 1);
        if lookahead == 0 {
            self.tokens
                .get(self.pos)
                .map_or(SyntaxKind::EndOfStream, |it| it.kind)
        } else {
            // Skip trivia tokens for lookahead > 0
            let mut count = 0;
            let mut i = self.pos + 1;
            while i < self.tokens.len() {
                let kind = self.tokens[i].kind;
                if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Comment {
                    count += 1;
                    if count == lookahead {
                        return kind;
                    }
                }
                i += 1;
            }
            SyntaxKind::EndOfStream
        }
    }

    pub fn nth_with_trivia(&self, lookahead: usize) -> SyntaxKind {
        if self.fuel.get() == 0 {
            return SyntaxKind::EndOfStream;
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or(SyntaxKind::EndOfStream, |it| it.kind)
    }

    pub fn at(&mut self, kind: SyntaxKind) -> bool {
        self.nth(0) == kind
    }

    #[allow(dead_code)]
    pub fn at_with_trivia(&mut self, kind: SyntaxKind) -> bool {
        self.nth_with_trivia(0) == kind
    }

    #[allow(dead_code)]
    pub fn at_set(&mut self, set: &TokenSet) -> bool {
        set.contains(self.nth(0))
    }

    pub fn at_any(&mut self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.nth(0))
    }

    pub fn at_identifier(&mut self) -> bool {
        self.at_any(&[SyntaxKind::BareWord, SyntaxKind::QuotedIdentifier])
    }

    pub fn at_any_with_trivia(&mut self, kinds: &[SyntaxKind]) -> bool {
        kinds.contains(&self.nth_with_trivia(0))
    }

    pub fn eat(&mut self, kind: SyntaxKind) -> bool {
        if self.at(kind) {
            self.advance();
            true
        } else {
            false
        }
    }

    pub fn expect(&mut self, kind: SyntaxKind) {
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
                .map_or("", |it| it.text(&self.source))
        } else {
            let mut count = 0;
            let mut i = self.pos + 1;
            while i < self.tokens.len() {
                let kind = self.tokens[i].kind;
                if kind != SyntaxKind::Whitespace && kind != SyntaxKind::Comment {
                    count += 1;
                    if count == lookahead {
                        return self.tokens[i].text(&self.source);
                    }
                }
                i += 1;
            }
            ""
        }
    }

    #[allow(dead_code)]
    pub fn nth_text_with_trivia(&mut self, lookahead: usize) -> &str {
        if self.fuel.get() == 0 {
            return "";
        }
        self.fuel.set(self.fuel.get() - 1);
        self.tokens
            .get(self.pos + lookahead)
            .map_or("", |it| it.text(&self.source))
    }

    pub fn at_keyword(&mut self, keyword: Keyword) -> bool {
        self.nth(0) == SyntaxKind::BareWord
            && self.nth_text(0).eq_ignore_ascii_case(keyword.as_str())
    }

    /// True if the token at the given lookahead offset matches the given keyword.
    pub fn nth_keyword(&mut self, n: usize, keyword: Keyword) -> bool {
        self.nth(n) == SyntaxKind::BareWord
            && self.nth_text(n).eq_ignore_ascii_case(keyword.as_str())
    }

    /// True if the current (non-trivia) token is followed by '('.
    /// Useful to disambiguate keywords that can also be function names.
    pub fn at_followed_by_paren(&mut self) -> bool {
        self.nth(1) == SyntaxKind::OpeningRoundBracket
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
