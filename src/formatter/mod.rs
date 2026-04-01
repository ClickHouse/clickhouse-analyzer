mod context;
mod format_node;

use crate::parser::syntax_tree::SyntaxTree;
use context::FormatterContext;

pub struct FormatConfig {
    pub indent_width: usize,
    pub uppercase_keywords: bool,
}

impl Default for FormatConfig {
    fn default() -> Self {
        Self {
            indent_width: 4,
            uppercase_keywords: true,
        }
    }
}

pub fn format(tree: &SyntaxTree, config: &FormatConfig) -> String {
    let mut ctx = FormatterContext::new(config);
    format_node::format_node(tree, &mut ctx);
    ctx.finish()
}
