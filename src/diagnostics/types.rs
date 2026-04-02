#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub enum Severity {
    Error,
    Warning,
    Hint,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Suggestion {
    pub message: String,
    pub replacement: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct RelatedSpan {
    pub range: (usize, usize),
    pub message: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize))]
pub struct Diagnostic {
    pub message: String,
    pub range: (usize, usize),
    pub severity: Severity,
    pub code: Option<&'static str>,
    pub suggestion: Option<Suggestion>,
    pub related: Vec<RelatedSpan>,
}
