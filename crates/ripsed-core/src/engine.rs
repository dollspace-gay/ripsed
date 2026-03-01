use crate::diff::{Change, ChangeContext, FileChanges, OpResult};
use crate::error::RipsedError;
use crate::matcher::Matcher;
use crate::operation::{LineRange, Op};
use crate::undo::UndoEntry;

/// The result of applying operations to a text buffer.
#[derive(Debug)]
pub struct EngineOutput {
    /// The modified text (None if unchanged).
    pub text: Option<String>,
    /// Structured diff of changes made.
    pub changes: Vec<Change>,
    /// Undo entry to reverse this operation.
    pub undo: Option<UndoEntry>,
}

/// Detect whether the text uses CRLF line endings.
///
/// Returns true if CRLF (`\r\n`) is found before any bare LF (`\n`).
fn uses_crlf(text: &str) -> bool {
    text.contains("\r\n")
}

/// Apply a single operation to a text buffer.
///
/// Returns the modified text and a structured diff.
/// If `dry_run` is true, the text is computed but flagged as preview-only.
pub fn apply(
    text: &str,
    op: &Op,
    matcher: &Matcher,
    line_range: Option<LineRange>,
    context_lines: usize,
) -> Result<EngineOutput, RipsedError> {
    let crlf = uses_crlf(text);
    let lines: Vec<&str> = text.lines().collect();
    let mut result_lines: Vec<String> = Vec::with_capacity(lines.len());
    let mut changes: Vec<Change> = Vec::new();

    for (idx, &line) in lines.iter().enumerate() {
        let line_num = idx + 1; // 1-indexed

        // Skip lines outside the line range
        if let Some(range) = line_range {
            if !range.contains(line_num) {
                result_lines.push(line.to_string());
                continue;
            }
        }

        match op {
            Op::Replace {
                replace, ..
            } => {
                if let Some(replaced) = matcher.replace(line, replace) {
                    let ctx = build_context(&lines, idx, context_lines);
                    changes.push(Change {
                        line: line_num,
                        before: line.to_string(),
                        after: Some(replaced.clone()),
                        context: Some(ctx),
                    });
                    result_lines.push(replaced);
                } else {
                    result_lines.push(line.to_string());
                }
            }
            Op::Delete { .. } => {
                if matcher.is_match(line) {
                    let ctx = build_context(&lines, idx, context_lines);
                    changes.push(Change {
                        line: line_num,
                        before: line.to_string(),
                        after: None,
                        context: Some(ctx),
                    });
                    // Don't push — line is deleted
                } else {
                    result_lines.push(line.to_string());
                }
            }
            Op::InsertAfter { content, .. } => {
                result_lines.push(line.to_string());
                if matcher.is_match(line) {
                    let ctx = build_context(&lines, idx, context_lines);
                    changes.push(Change {
                        line: line_num,
                        before: line.to_string(),
                        after: Some(format!("{line}\n{content}")),
                        context: Some(ctx),
                    });
                    result_lines.push(content.clone());
                }
            }
            Op::InsertBefore { content, .. } => {
                if matcher.is_match(line) {
                    let ctx = build_context(&lines, idx, context_lines);
                    changes.push(Change {
                        line: line_num,
                        before: line.to_string(),
                        after: Some(format!("{content}\n{line}")),
                        context: Some(ctx),
                    });
                    result_lines.push(content.clone());
                }
                result_lines.push(line.to_string());
            }
            Op::ReplaceLine { content, .. } => {
                if matcher.is_match(line) {
                    let ctx = build_context(&lines, idx, context_lines);
                    changes.push(Change {
                        line: line_num,
                        before: line.to_string(),
                        after: Some(content.clone()),
                        context: Some(ctx),
                    });
                    result_lines.push(content.clone());
                } else {
                    result_lines.push(line.to_string());
                }
            }
        }
    }

    let line_sep = if crlf { "\r\n" } else { "\n" };
    let modified_text = if changes.is_empty() {
        None
    } else {
        // Preserve line ending style and trailing newline
        let mut joined = result_lines.join(line_sep);
        if text.ends_with('\n') || text.ends_with("\r\n") {
            joined.push_str(line_sep);
        }
        Some(joined)
    };

    let undo = if !changes.is_empty() {
        Some(UndoEntry {
            original_text: text.to_string(),
        })
    } else {
        None
    };

    Ok(EngineOutput {
        text: modified_text,
        changes,
        undo,
    })
}

fn build_context(lines: &[&str], idx: usize, context_lines: usize) -> ChangeContext {
    let start = idx.saturating_sub(context_lines);
    let end = (idx + context_lines + 1).min(lines.len());

    let before = lines[start..idx].iter().map(|s| s.to_string()).collect();
    let after = if idx + 1 < end {
        lines[idx + 1..end].iter().map(|s| s.to_string()).collect()
    } else {
        vec![]
    };

    ChangeContext { before, after }
}

/// Build an OpResult from file-level changes.
pub fn build_op_result(operation_index: usize, path: &str, changes: Vec<Change>) -> OpResult {
    OpResult {
        operation_index,
        files: if changes.is_empty() {
            vec![]
        } else {
            vec![FileChanges {
                path: path.to_string(),
                changes,
            }]
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::matcher::Matcher;

    #[test]
    fn test_simple_replace() {
        let text = "hello world\nfoo bar\nhello again\n";
        let op = Op::Replace {
            find: "hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 2).unwrap();
        assert_eq!(result.text.unwrap(), "hi world\nfoo bar\nhi again\n");
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_delete_lines() {
        let text = "keep\ndelete me\nkeep too\n";
        let op = Op::Delete {
            find: "delete".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "keep\nkeep too\n");
    }

    #[test]
    fn test_no_changes() {
        let text = "nothing matches here\n";
        let op = Op::Replace {
            find: "zzz".to_string(),
            replace: "aaa".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_line_range() {
        let text = "line1\nline2\nline3\nline4\n";
        let op = Op::Replace {
            find: "line".to_string(),
            replace: "row".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let range = Some(LineRange {
            start: 2,
            end: Some(3),
        });
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, range, 0).unwrap();
        assert_eq!(result.text.unwrap(), "line1\nrow2\nrow3\nline4\n");
    }

    // ---------------------------------------------------------------
    // CRLF handling tests
    // ---------------------------------------------------------------

    #[test]
    fn test_crlf_replace_preserves_crlf() {
        let text = "hello world\r\nfoo bar\r\nhello again\r\n";
        let op = Op::Replace {
            find: "hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "hi world\r\nfoo bar\r\nhi again\r\n");
    }

    #[test]
    fn test_crlf_delete_preserves_crlf() {
        let text = "keep\r\ndelete me\r\nkeep too\r\n";
        let op = Op::Delete {
            find: "delete".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "keep\r\nkeep too\r\n");
    }

    #[test]
    fn test_crlf_no_trailing_newline() {
        let text = "hello world\r\nfoo bar";
        let op = Op::Replace {
            find: "hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert_eq!(output, "hi world\r\nfoo bar");
        // No trailing CRLF since original didn't have one
        assert!(!output.ends_with("\r\n"));
    }

    #[test]
    fn test_uses_crlf_detection() {
        assert!(uses_crlf("a\r\nb\r\n"));
        assert!(uses_crlf("a\r\n"));
        assert!(!uses_crlf("a\nb\n"));
        assert!(!uses_crlf("no newline at all"));
        assert!(!uses_crlf(""));
    }

    // ---------------------------------------------------------------
    // Edge-case tests
    // ---------------------------------------------------------------

    #[test]
    fn test_empty_input_text() {
        let text = "";
        let op = Op::Replace {
            find: "anything".to_string(),
            replace: "something".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_single_line_no_trailing_newline() {
        let text = "hello world";
        let op = Op::Replace {
            find: "hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert_eq!(output, "hi world");
        // Should NOT add a trailing newline that wasn't there
        assert!(!output.ends_with('\n'));
    }

    #[test]
    fn test_whitespace_only_lines() {
        let text = "  \n\t\n   \t  \n";
        let op = Op::Replace {
            find: "\t".to_string(),
            replace: "TAB".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert!(output.contains("TAB"));
        assert_eq!(result.changes.len(), 2); // lines 2 and 3 have tabs
    }

    #[test]
    fn test_very_long_line() {
        let long_word = "x".repeat(100_000);
        let text = format!("before\n{long_word}\nafter\n");
        let op = Op::Replace {
            find: "x".to_string(),
            replace: "y".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(&text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        let expected_long = "y".repeat(100_000);
        assert!(output.contains(&expected_long));
    }

    #[test]
    fn test_unicode_emoji() {
        let text = "hello world\n";
        let op = Op::Replace {
            find: "world".to_string(),
            replace: "\u{1F30D}".to_string(), // earth globe emoji
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "hello \u{1F30D}\n");
    }

    #[test]
    fn test_unicode_cjk() {
        let text = "\u{4F60}\u{597D}\u{4E16}\u{754C}\n"; // "hello world" in Chinese
        let op = Op::Replace {
            find: "\u{4E16}\u{754C}".to_string(), // "world"
            replace: "\u{5730}\u{7403}".to_string(), // "earth"
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(
            result.text.unwrap(),
            "\u{4F60}\u{597D}\u{5730}\u{7403}\n"
        );
    }

    #[test]
    fn test_unicode_combining_characters() {
        // e + combining acute accent = e-acute
        let text = "caf\u{0065}\u{0301}\n";
        let op = Op::Replace {
            find: "caf\u{0065}\u{0301}".to_string(),
            replace: "coffee".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "coffee\n");
    }

    #[test]
    fn test_regex_special_chars_in_literal_mode() {
        // In literal mode, regex metacharacters should be treated as literals
        let text = "price is $10.00 (USD)\n";
        let op = Op::Replace {
            find: "$10.00".to_string(),
            replace: "$20.00".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "price is $20.00 (USD)\n");
    }

    #[test]
    fn test_overlapping_matches_in_single_line() {
        // "aaa" with pattern "aa" — standard str::replace does non-overlapping left-to-right
        let text = "aaa\n";
        let op = Op::Replace {
            find: "aa".to_string(),
            replace: "b".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        // Rust's str::replace: "aaa".replace("aa", "b") == "ba"
        assert_eq!(result.text.unwrap(), "ba\n");
    }

    #[test]
    fn test_replace_line_count_preserved() {
        let text = "line1\nline2\nline3\nline4\nline5\n";
        let input_line_count = text.lines().count();
        let op = Op::Replace {
            find: "line".to_string(),
            replace: "row".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        let output_line_count = output.lines().count();
        assert_eq!(input_line_count, output_line_count);
    }

    #[test]
    fn test_replace_preserves_empty_result_on_non_match() {
        // Pattern that exists nowhere in text
        let text = "alpha\nbeta\ngamma\n";
        let op = Op::Replace {
            find: "zzzzzz".to_string(),
            replace: "y".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.undo.is_none());
    }

    #[test]
    fn test_undo_entry_stores_original() {
        let text = "hello\nworld\n";
        let op = Op::Replace {
            find: "hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let undo = result.undo.unwrap();
        assert_eq!(undo.original_text, text);
    }

    #[test]
    fn test_determinism_same_input_same_output() {
        let text = "foo bar baz\nhello world\nfoo again\n";
        let op = Op::Replace {
            find: "foo".to_string(),
            replace: "qux".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let r1 = apply(text, &op, &matcher, None, 0).unwrap();
        let r2 = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(r1.text, r2.text);
        assert_eq!(r1.changes.len(), r2.changes.len());
        for (c1, c2) in r1.changes.iter().zip(r2.changes.iter()) {
            assert_eq!(c1, c2);
        }
    }
}

// ---------------------------------------------------------------
// Property-based tests (proptest)
// ---------------------------------------------------------------
#[cfg(test)]
mod proptests {
    use super::*;
    use crate::matcher::Matcher;
    use crate::operation::Op;
    use proptest::prelude::*;

    /// Strategy for generating text that is multiple lines with a trailing newline.
    fn arb_multiline_text() -> impl Strategy<Value = String> {
        prop::collection::vec("[^\n\r]{0,80}", 1..10)
            .prop_map(|lines| lines.join("\n") + "\n")
    }

    /// Strategy for generating a non-empty find pattern (plain literal).
    fn arb_find_pattern() -> impl Strategy<Value = String> {
        "[a-zA-Z0-9]{1,8}"
    }

    proptest! {
        /// Round-trip: applying a Replace then undoing (restoring original_text)
        /// should give back the original text.
        #[test]
        fn prop_roundtrip_undo(
            text in arb_multiline_text(),
            find in arb_find_pattern(),
            replace in "[a-zA-Z0-9]{0,8}",
        ) {
            let op = Op::Replace {
                find: find.clone(),
                replace: replace.clone(),
                regex: false,
                case_insensitive: false,
            };
            let matcher = Matcher::new(&op).unwrap();
            let result = apply(&text, &op, &matcher, None, 0).unwrap();

            if let Some(undo) = &result.undo {
                // Undo should restore original text
                prop_assert_eq!(&undo.original_text, &text);
            }
            // If no changes, text should be None
            if result.text.is_none() {
                prop_assert!(result.changes.is_empty());
            }
        }

        /// No-op: applying with a pattern that cannot match leaves text unchanged.
        #[test]
        fn prop_noop_nonmatching_pattern(text in arb_multiline_text()) {
            // Use a pattern with a NUL byte which will never appear in text generated
            // by arb_multiline_text
            let op = Op::Replace {
                find: "\x00\x00NOMATCH\x00\x00".to_string(),
                replace: "replacement".to_string(),
                regex: false,
                case_insensitive: false,
            };
            let matcher = Matcher::new(&op).unwrap();
            let result = apply(&text, &op, &matcher, None, 0).unwrap();
            prop_assert!(result.text.is_none(), "Non-matching pattern should not modify text");
            prop_assert!(result.changes.is_empty());
            prop_assert!(result.undo.is_none());
        }

        /// Determinism: same input always produces same output.
        #[test]
        fn prop_deterministic(
            text in arb_multiline_text(),
            find in arb_find_pattern(),
            replace in "[a-zA-Z0-9]{0,8}",
        ) {
            let op = Op::Replace {
                find,
                replace,
                regex: false,
                case_insensitive: false,
            };
            let matcher = Matcher::new(&op).unwrap();
            let r1 = apply(&text, &op, &matcher, None, 0).unwrap();
            let r2 = apply(&text, &op, &matcher, None, 0).unwrap();
            prop_assert_eq!(&r1.text, &r2.text);
            prop_assert_eq!(r1.changes.len(), r2.changes.len());
        }

        /// Line count: for Replace ops, output line count == input line count.
        #[test]
        fn prop_replace_preserves_line_count(
            text in arb_multiline_text(),
            find in arb_find_pattern(),
            replace in "[a-zA-Z0-9]{0,8}",
        ) {
            let op = Op::Replace {
                find,
                replace,
                regex: false,
                case_insensitive: false,
            };
            let matcher = Matcher::new(&op).unwrap();
            let result = apply(&text, &op, &matcher, None, 0).unwrap();
            if let Some(ref output) = result.text {
                let input_lines = text.lines().count();
                let output_lines = output.lines().count();
                prop_assert_eq!(
                    input_lines,
                    output_lines,
                    "Replace should preserve line count: input={} output={}",
                    input_lines,
                    output_lines
                );
            }
        }
    }
}
