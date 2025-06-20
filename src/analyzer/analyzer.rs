use crate::lexer::token::TokenKind;
use crate::parser::tree::{Child, ChildOptionExt, Tree, TreeKind};

struct QueryState {
    columns: Vec<Child>,
    tables: Vec<String>,
}

pub fn analyze(cst: Tree) -> Result<(), ()> {
    let mut q = QueryState {
        columns: vec![],
        tables: vec![],
    };

    analyze_tree_rec(&mut q, &cst);

    Ok(())
}

fn get_first_tree_with_kind(children: &Vec<Child>, target_kind: TreeKind) -> Option<&Tree> {
    children
        .iter()
        .find_map(|child| child.get_tree_with_kind(target_kind))
}

fn analyze_tree_rec(q: &mut QueryState, t: &Tree) {
    match t.kind {
        TreeKind::SelectStatement => {
            analyze_select_statement(q, t);
        }
        _ => {
            for c in t.children.as_slice() {
                analyze_child_rec(q, &c);
            }
        }
    }
}

fn analyze_child_rec(q: &mut QueryState, c: &Child) {
    match c {
        Child::Token(t) => {}
        Child::Tree(t) => analyze_tree_rec(q, t),
    }
}

fn analyze_select_statement(q: &mut QueryState, t: &Tree) {
    let select_clause = get_first_tree_with_kind(&t.children, TreeKind::SelectClause);

    if select_clause.is_none() {
        return;
    }

    // FROM keyword
    let from_clause = get_first_tree_with_kind(&t.children, TreeKind::FromClause);
    if from_clause.is_none() {
        return;
    }
    let from_clause = from_clause.unwrap();

    // TableIdentifier node
    let table_identifier = from_clause.children.iter().nth(1);
    let table_identifier = table_identifier.get_tree_with_kind(TreeKind::TableIdentifier);
    if table_identifier.is_none() {
        return;
    }
    let table_identifier = table_identifier.unwrap();

    // Table name bareword
    let table_name = table_identifier.children.iter().nth(1);
    let table_name = table_name.get_token_with_kind(TokenKind::BareWord);
    if table_name.is_none() {
        return;
    }
    let table_name = table_name.unwrap();

    q.tables.push(table_name.text.clone());
}
