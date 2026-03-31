use crate::lexer::token::TokenKind;

/// Compact bitset over `TokenKind` for O(1) membership testing.
///
/// Used for recovery sets and lookahead predicates in the parser.
/// Requires `TokenKind` to be `#[repr(u8)]` with fewer than 64 variants.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct TokenSet(u64);

impl TokenSet {
    pub const EMPTY: TokenSet = TokenSet(0);

    pub const fn new(kinds: &[TokenKind]) -> TokenSet {
        let mut bits = 0u64;
        let mut i = 0;
        while i < kinds.len() {
            bits |= 1 << (kinds[i] as u64);
            i += 1;
        }
        TokenSet(bits)
    }

    pub const fn contains(&self, kind: TokenKind) -> bool {
        self.0 & (1 << (kind as u64)) != 0
    }

    pub const fn union(self, other: TokenSet) -> TokenSet {
        TokenSet(self.0 | other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_contains_nothing() {
        assert!(!TokenSet::EMPTY.contains(TokenKind::BareWord));
        assert!(!TokenSet::EMPTY.contains(TokenKind::Number));
    }

    #[test]
    fn set_contains_added_kinds() {
        let set = TokenSet::new(&[TokenKind::BareWord, TokenKind::Number]);
        assert!(set.contains(TokenKind::BareWord));
        assert!(set.contains(TokenKind::Number));
        assert!(!set.contains(TokenKind::StringLiteral));
    }

    #[test]
    fn union_combines_sets() {
        let a = TokenSet::new(&[TokenKind::BareWord]);
        let b = TokenSet::new(&[TokenKind::Number]);
        let combined = a.union(b);
        assert!(combined.contains(TokenKind::BareWord));
        assert!(combined.contains(TokenKind::Number));
        assert!(!combined.contains(TokenKind::Comma));
    }
}
