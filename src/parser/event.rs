use crate::parser::syntax_kind::SyntaxKind;

#[derive(Debug)]
pub enum Event {
    Open {
        kind: SyntaxKind,
        /// If set, points to another Open event that wraps this one.
        /// Used by `precede()` to avoid O(n) Vec::insert.
        forward_parent: Option<u32>,
    },
    Close,
    Advance,
}
