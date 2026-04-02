use crate::parser::syntax_kind::SyntaxKind;

/// Compact bitset over `SyntaxKind` for O(1) membership testing.
///
/// Used for recovery sets and lookahead predicates in the parser.
/// Uses a pair of u128 to support up to 256 variants.
#[derive(Clone, Copy)]
#[allow(dead_code)]
pub struct TokenSet(u128, u128);

impl TokenSet {
    pub const EMPTY: TokenSet = TokenSet(0, 0);

    pub const fn new(kinds: &[SyntaxKind]) -> TokenSet {
        let mut lo = 0u128;
        let mut hi = 0u128;
        let mut i = 0;
        while i < kinds.len() {
            let bit = kinds[i] as u16;
            if bit < 128 {
                lo |= 1u128 << bit;
            } else {
                hi |= 1u128 << (bit - 128);
            }
            i += 1;
        }
        TokenSet(lo, hi)
    }

    pub const fn contains(&self, kind: SyntaxKind) -> bool {
        let bit = kind as u16;
        if bit < 128 {
            self.0 & (1u128 << bit) != 0
        } else {
            self.1 & (1u128 << (bit - 128)) != 0
        }
    }

    pub const fn union(self, other: TokenSet) -> TokenSet {
        TokenSet(self.0 | other.0, self.1 | other.1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_set_contains_nothing() {
        assert!(!TokenSet::EMPTY.contains(SyntaxKind::BareWord));
        assert!(!TokenSet::EMPTY.contains(SyntaxKind::Number));
    }

    #[test]
    fn set_contains_added_kinds() {
        let set = TokenSet::new(&[SyntaxKind::BareWord, SyntaxKind::Number]);
        assert!(set.contains(SyntaxKind::BareWord));
        assert!(set.contains(SyntaxKind::Number));
        assert!(!set.contains(SyntaxKind::StringToken));
    }

    #[test]
    fn union_combines_sets() {
        let a = TokenSet::new(&[SyntaxKind::BareWord]);
        let b = TokenSet::new(&[SyntaxKind::Number]);
        let combined = a.union(b);
        assert!(combined.contains(SyntaxKind::BareWord));
        assert!(combined.contains(SyntaxKind::Number));
        assert!(!combined.contains(SyntaxKind::Comma));
    }
}
