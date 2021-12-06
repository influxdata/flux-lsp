use flux::ast;
use flux::semantic::walk::Node;
use lspower::lsp;

/// Convert a flux::ast::Position to a lsp::Position
/// https://microsoft.github.io/language-server-protocol/specification#position
// XXX: rockstar (28 Jul 2021) - This can't be implemented in a From trait
// without some clownshoes type aliasing, so this conversion function will work.
fn ast_to_lsp_position(position: &ast::Position) -> lsp::Position {
    lsp::Position {
        line: position.line - 1,
        character: position.column - 1,
    }
}
/// Convert a flux::ast::SourceLocation to a lsp::Range.
pub fn ast_to_lsp_range(loc: &ast::SourceLocation) -> lsp::Range {
    lsp::Range {
        start: ast_to_lsp_position(&loc.start),
        end: ast_to_lsp_position(&loc.end),
    }
}

/// Convert a flux::semantic::walk::Node to a lsp::Location
/// https://microsoft.github.io/language-server-protocol/specification#location
// XXX: rockstar (28 Jul 2021) - This can't be implemented in a From trait
// without some clownshoes type aliasing, so this conversion function will work.
pub fn node_to_location(node: &Node, uri: lsp::Url) -> lsp::Location {
    let node_location = node.loc();
    lsp::Location {
        uri,
        range: lsp::Range {
            start: ast_to_lsp_position(&node_location.start),
            end: ast_to_lsp_position(&node_location.end),
        },
    }
}

#[cfg(test)]
mod tests {
    use flux::ast;
    use flux::semantic::nodes::IdentifierExpr;
    use flux::semantic::types::MonoType;
    use flux::semantic::walk::Node;

    use super::*;

    #[test]
    fn test_ast_to_lsp_position() {
        let expected = lsp::Position {
            line: 22,
            character: 7,
        };

        let ast_position = ast::Position {
            line: 23,
            column: 8,
        };
        let result = ast_to_lsp_position(&ast_position);

        assert_eq!(expected, result);
    }

    #[test]
    fn test_node_to_location() {
        let expected = lsp::Location {
            uri: lsp::Url::parse("file:///path/to/file.flux")
                .unwrap(),
            range: lsp::Range {
                start: lsp::Position {
                    line: 22,
                    character: 7,
                },
                end: lsp::Position {
                    line: 22,
                    character: 8,
                },
            },
        };

        let expr = IdentifierExpr {
            loc: ast::SourceLocation {
                file: None,
                start: ast::Position {
                    line: 23,
                    column: 8,
                },
                end: ast::Position {
                    line: 23,
                    column: 9,
                },
                source: None,
            },
            typ: MonoType::String,
            name: "a".into(),
        };
        let node = Node::IdentifierExpr(&expr);

        let result = node_to_location(
            &node,
            lsp::Url::parse("file:///path/to/file.flux").unwrap(),
        );

        assert_eq!(expected, result);
    }
}
