#[derive(Debug, Copy, Clone)]
pub enum Keyword {
    With,
    Select,
    From,
    Order,
    By,
    As,
    Where,
    And,
    Or,
    Limit,
}

impl Keyword {
    pub fn as_str(&self) -> &'static str {
        match self {
            Keyword::With => "WITH",
            Keyword::Select => "SELECT",
            Keyword::From => "FROM",
            Keyword::Order => "ORDER",
            Keyword::By => "BY",
            Keyword::As => "AS",
            Keyword::Where => "WHERE",
            Keyword::And => "AND",
            Keyword::Or => "OR",
            Keyword::Limit => "LIMIT",
        }
    }
}
