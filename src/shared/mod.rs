mod signatures;

use combinations::Combinations;
use lspower::lsp;

pub use signatures::{
    get_argument_names, FunctionInfo, FunctionSignature,
};

#[derive(Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<String>,
}

pub fn all_combos<T>(l: Vec<T>) -> Vec<Vec<T>>
where
    T: std::cmp::Ord + Clone,
{
    let mut result = vec![];
    let length = l.len();

    for i in 1..length {
        let c: Vec<Vec<T>> =
            Combinations::new(l.clone(), i).collect();
        result.extend(c);
    }

    result.push(l);

    result
}

pub fn get_package_name(name: &str) -> Option<String> {
    let items = name.split('/');
    items.last().map(|n| n.to_string())
}

pub fn flux_position_to_position(
    pos: flux::ast::Position,
) -> lsp::Position {
    lsp::Position {
        line: pos.line - 1,
        character: pos.column - 1,
    }
}

pub fn is_in_node(
    pos: lsp::Position,
    base: &flux::ast::BaseNode,
) -> bool {
    let start_line = base.location.start.line - 1;
    let start_col = base.location.start.column - 1;
    let end_line = base.location.end.line - 1;
    let end_col = base.location.end.column - 1;

    if pos.line < start_line {
        return false;
    }

    if pos.line > end_line {
        return false;
    }

    if pos.line == start_line && pos.character < start_col {
        return false;
    }

    if pos.line == end_line && pos.character > end_col {
        return false;
    }

    true
}

#[cfg(test)]
mod tests {
    use super::all_combos;

    #[test]
    fn test_all_combos() {
        let array = vec!["1", "2", "3"];
        let result = all_combos(array);

        assert_eq!(result.len(), 7);
    }
}
