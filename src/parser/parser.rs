use crate::lexer::token::{Token, TokenKind};
use crate::lexer::tokenizer::tokenize_with_whitespace;
use crate::parser::keyword::Keyword;
use crate::parser::parsers::select::{at_select_statement, parse_select_statement};
use crate::parser::tree::{Child, Tree, TreeKind};
use std::cell::Cell;

#[derive(Debug)]
pub enum Event {
    Open { kind: TreeKind },
    Close,
    Advance,
}

pub struct MarkOpened {
    index: usize,
}

pub struct MarkClosed {
    index: usize,
}

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

    pub fn build_tree(self) -> Tree {
        let mut tokens = self.tokens.into_iter();
        let mut events = self.events;

        assert!(matches!(events.pop(), Some(Event::Close)));
        let mut stack = Vec::new();
        for event in events {
            match event {
                Event::Open { kind } => stack.push(Tree {
                    kind,
                    children: Vec::new(),
                }),
                Event::Close => {
                    let tree = stack.pop().unwrap();
                    stack.last_mut().unwrap().children.push(Child::Tree(tree));
                }
                Event::Advance => {
                    let token = tokens.next().unwrap();
                    stack.last_mut().unwrap().children.push(Child::Token(token));
                }
            }
        }

        let tree = stack.pop().unwrap();
        assert!(stack.is_empty());
        assert!(tokens.next().is_none());
        tree
    }

    pub fn open(&mut self) -> MarkOpened {
        let mark = MarkOpened {
            index: self.events.len(),
        };
        self.events.push(Event::Open {
            kind: TreeKind::ErrorTree,
        });
        mark
    }

    pub fn open_before(&mut self, m: MarkClosed) -> MarkOpened {
        let mark = MarkOpened { index: m.index };
        self.events.insert(
            m.index,
            Event::Open {
                kind: TreeKind::ErrorTree,
            },
        );
        mark
    }

    pub fn close(&mut self, m: MarkOpened, kind: TreeKind) -> MarkClosed {
        self.events[m.index] = Event::Open { kind };
        self.events.push(Event::Close);
        MarkClosed { index: m.index }
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
        println!("{}", self.nth_text_with_trivia(0));
        self.pos += 1;
    }

    pub fn recover_with_error(&mut self, error: &str) {
        let m = self.open();
        // TODO: Error reporting.
        eprintln!("{error}");
        self.close(m, TreeKind::ErrorTree);
    }

    pub fn advance_with_error(&mut self, error: &str) {
        let m = self.open();
        // TODO: Error reporting.
        eprintln!("{error}");
        if !self.eof() {
            self.advance();
        }
        self.close(m, TreeKind::ErrorTree);
    }

    pub fn eof(&self) -> bool {
        self.pos == self.tokens.len()
    }

    // Handles semicolon and eof
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

pub fn parse(text: &str) -> Tree {
    let tokens = tokenize_with_whitespace(text);
    let mut p = Parser::new(tokens);
    parse_sql(&mut p);
    p.build_tree()
}

// Parse a SQL file (entry point)
fn parse_sql(p: &mut Parser) {
    let m = p.open();

    while !p.eof() {
        if at_select_statement(p) {
            parse_select_statement(p);
        }

        if p.at(TokenKind::Semicolon) {
            p.expect(TokenKind::Semicolon);
        }
    }

    p.skip_trivia();

    p.close(m, TreeKind::File);
}

#[cfg(test)]
mod tests {
    use crate::parser::parser::parse;
    use rstest::rstest;

    #[rstest]
    fn test_parse(#[files("test/inputs/**/*.sql")] path: std::path::PathBuf) {
        let inputs_dir = std::path::Path::new("test/inputs").canonicalize().unwrap();
        let snapshots_dir = std::path::Path::new("test/snapshots")
            .canonicalize()
            .unwrap();

        let path_str = path
            .strip_prefix(inputs_dir)
            .unwrap()
            .to_str()
            .expect("Failed to convert path to string");
        let file_content =
            std::fs::read_to_string(&path).expect(&format!("Failed to read file: {}", path_str));

        // Try to parse the file content, catching any panics so that the panic message can be snapshotted
        let parse_result = std::panic::catch_unwind(|| parse(&file_content));

        // Compare the result with the snapshot
        insta::with_settings!({
            description => path_str,
            snapshot_suffix => "parse",
            snapshot_path => &snapshots_dir,
            prepend_module_to_snapshot => false,
            omit_expression => true,
        }, {
            match &parse_result {
                Ok(tree) => insta::assert_yaml_snapshot!(path_str, tree),
                Err(err) => insta::assert_yaml_snapshot!(path_str, err.downcast_ref::<&str>().unwrap().to_string()),
            }
        });
    }
}
