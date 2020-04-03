use crate::protocol::{
    properties::{Position, Range, TextEdit},
    requests::FormattingOptions,
};

use flux::formatter::Formatter;
use flux::parser::Parser;

pub fn format_str(
    src: &str,
    _fo: &FormattingOptions,
) -> Vec<TextEdit> {
    let file = Parser::new(src).parse_file("".to_string());
    let mut fmt = Formatter::default();
    fmt.format_file(&file, true);
    if let Ok(output) = fmt.output() {
        let mismatches = make_diff(src, &output);
        create_text_edits(mismatches)
    } else {
        return vec![];
    }
}

fn create_text_edits(mismatches: Vec<Mismatch>) -> Vec<TextEdit> {
    mismatches
        .into_iter()
        .map(|mismatch| {
            let lines = mismatch.lines.iter();
            let num_removed = lines
                .filter(|line| match line {
                    DiffLine::Resulting(_) => true,
                    _ => false,
                })
                .count();

            let new_lines: Vec<String> = mismatch
                .lines
                .into_iter()
                .filter_map(|line| match line {
                    DiffLine::Resulting(_) => None,
                    DiffLine::Expected(str) => Some(str + "\n"),
                })
                .collect();
            let start_line = mismatch.line_number_orig - 1;
            let end_line = start_line + (num_removed as u32);

            let new_text = new_lines.join("");

            TextEdit {
                range: Range {
                    start: Position::new(start_line, 0),
                    end: Position::new(end_line, 0),
                },
                new_text,
            }
        })
        .collect()
}

#[derive(Debug, PartialEq)]
enum DiffLine {
    Expected(String),
    Resulting(String),
}

#[derive(Debug, PartialEq)]
struct Mismatch {
    /// The line number in the formatted version.
    pub line_number: u32,
    /// The line number in the original version.
    pub line_number_orig: u32,
    /// The set of lines (context and old/new) in the mismatch.
    pub lines: Vec<DiffLine>,
}

impl Mismatch {
    fn new(line_number: u32, line_number_orig: u32) -> Mismatch {
        Mismatch {
            line_number,
            line_number_orig,
            lines: Vec::new(),
        }
    }
}

// Produces a diff between the expected output and actual output.
fn make_diff(expected: &str, actual: &str) -> Vec<Mismatch> {
    let mut line_number = 1;
    let mut line_number_orig = 1;
    let mut lines_since_mismatch = 1;
    let mut results = Vec::new();
    let mut mismatch = Mismatch::new(0, 0);

    for result in diff::lines(expected, actual) {
        match result {
            diff::Result::Left(str) => {
                if lines_since_mismatch > 0 {
                    results.push(mismatch);
                    mismatch =
                        Mismatch::new(line_number, line_number_orig);
                }

                mismatch
                    .lines
                    .push(DiffLine::Resulting(str.to_owned()));
                line_number_orig += 1;
                lines_since_mismatch = 0;
            }
            diff::Result::Right(str) => {
                if lines_since_mismatch > 0 {
                    results.push(mismatch);
                    mismatch =
                        Mismatch::new(line_number, line_number_orig);
                }

                mismatch
                    .lines
                    .push(DiffLine::Expected(str.to_owned()));
                line_number += 1;
                lines_since_mismatch = 0;
            }
            diff::Result::Both(_, _) => {
                line_number += 1;
                line_number_orig += 1;
                lines_since_mismatch += 1;
            }
        }
    }

    results.push(mismatch);
    results.remove(0);

    results
}

#[cfg(test)]
mod test {
    use super::DiffLine::*;
    use super::{make_diff, Mismatch};

    #[test]
    fn diff_simple() {
        let src = "one\ntwo\nthree\nfour\nfive\n";
        let dest = "one\ntwo\ntrois\nfour\nfive\n";
        let diff = make_diff(src, dest);
        assert_eq!(
            diff,
            vec![Mismatch {
                line_number: 3,
                line_number_orig: 3,
                lines: vec![
                    Resulting("three".to_owned()),
                    Expected("trois".to_owned()),
                ],
            }]
        );
    }

    #[test]
    fn diff_multiple() {
        let src = "one\ntwo\nthree\nfour\nfive\nsix\n";
        let dest = "one\ntwo\ntrois\nfour\nfive\nxis\n";
        let diff = make_diff(src, dest);
        assert_eq!(
            diff,
            vec![
                Mismatch {
                    line_number: 3,
                    line_number_orig: 3,
                    lines: vec![
                        Resulting("three".to_owned()),
                        Expected("trois".to_owned())
                    ],
                },
                Mismatch {
                    line_number: 6,
                    line_number_orig: 6,
                    lines: vec![
                        Resulting("six".to_owned()),
                        Expected("xis".to_owned())
                    ],
                }
            ]
        );
    }

    #[test]
    fn diff_trailing_newline() {
        let src = "one\ntwo\nthree\nfour\nfive";
        let dest = "one\ntwo\nthree\nfour\nfive\n";
        let diff = make_diff(src, dest);
        assert_eq!(
            diff,
            vec![Mismatch {
                line_number: 6,
                line_number_orig: 6,
                lines: vec![Expected("".to_owned())],
            }]
        );
    }

    #[test]
    fn diff_missing_trailing_newline() {
        let src = "one\ntwo\nthree\nfour\nfive\n";
        let dest = "one\ntwo\nthree\nfour\nfive";
        let diff = make_diff(src, dest);
        assert_eq!(
            diff,
            vec![Mismatch {
                line_number: 6,
                line_number_orig: 6,
                lines: vec![Resulting("".to_owned())],
            }]
        );
    }
}
