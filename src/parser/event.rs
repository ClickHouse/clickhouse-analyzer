use crate::parser::syntax_kind::SyntaxKind;

#[derive(Debug)]
pub enum Event {
    Open { kind: SyntaxKind },
    Close,
    Advance,
}
