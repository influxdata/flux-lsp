/// A collection of tools for working with lsp types.
use lspower::lsp;

/// Return true if two Range structs overlap.
pub fn ranges_overlap(a: &lsp::Range, b: &lsp::Range) -> bool {
    position_in_range(&a.start, b) || position_in_range(&a.end, b)
}

/// Return true if a Position is found within the provided Range
pub fn position_in_range(
    position: &lsp::Position,
    range: &lsp::Range,
) -> bool {
    if position.line >= range.start.line
        && position.line <= range.end.line
    {
        if position.line == range.start.line {
            return position.character >= range.start.character;
        }
        if position.line == range.end.line {
            return position.character <= range.end.character;
        }
        return true;
    }
    false
}

#[cfg(test)]
mod test {
    use lspower::lsp;

    use super::*;

    #[test]
    fn position_in_range_line_too_early() {
        let position = lsp::Position {
            line: 5,
            character: 12,
        };
        let range = lsp::Range {
            start: lsp::Position {
                line: 6,
                character: 7,
            },
            end: lsp::Position {
                line: 15,
                character: 18,
            },
        };
        assert!(!position_in_range(&position, &range));
    }

    #[test]
    fn position_in_range_character_too_early() {
        let position = lsp::Position {
            line: 6,
            character: 6,
        };
        let range = lsp::Range {
            start: lsp::Position {
                line: 6,
                character: 7,
            },
            end: lsp::Position {
                line: 15,
                character: 18,
            },
        };
        assert!(!position_in_range(&position, &range));
    }

    #[test]
    fn position_in_range_line_too_late() {
        let position = lsp::Position {
            line: 17,
            character: 12,
        };
        let range = lsp::Range {
            start: lsp::Position {
                line: 6,
                character: 7,
            },
            end: lsp::Position {
                line: 15,
                character: 18,
            },
        };
        assert!(!position_in_range(&position, &range));
    }

    #[test]
    fn position_in_range_character_too_late() {
        let position = lsp::Position {
            line: 15,
            character: 19,
        };
        let range = lsp::Range {
            start: lsp::Position {
                line: 6,
                character: 7,
            },
            end: lsp::Position {
                line: 15,
                character: 18,
            },
        };
        assert!(!position_in_range(&position, &range));
    }

    #[test]
    fn position_in_range_works() {
        let position = lsp::Position {
            line: 7,
            character: 12,
        };
        let range = lsp::Range {
            start: lsp::Position {
                line: 6,
                character: 7,
            },
            end: lsp::Position {
                line: 15,
                character: 18,
            },
        };
        assert!(position_in_range(&position, &range));
    }
}
