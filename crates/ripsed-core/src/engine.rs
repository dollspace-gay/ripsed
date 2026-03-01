use crate::diff::{Change, ChangeContext, FileChanges, OpResult};
use crate::error::RipsedError;
use crate::matcher::Matcher;
use crate::operation::{LineRange, Op, TransformMode};
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
            Op::Replace { replace, .. } => {
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
            Op::Transform { mode, .. } => {
                if let Some(transformed) = matcher.replace(line, "") {
                    // Replace matched text with transformed version
                    let _ = transformed;
                    let new_line = apply_transform(line, matcher, *mode);
                    if new_line != line {
                        let ctx = build_context(&lines, idx, context_lines);
                        changes.push(Change {
                            line: line_num,
                            before: line.to_string(),
                            after: Some(new_line.clone()),
                            context: Some(ctx),
                        });
                        result_lines.push(new_line);
                    } else {
                        result_lines.push(line.to_string());
                    }
                } else {
                    result_lines.push(line.to_string());
                }
            }
            Op::Surround { prefix, suffix, .. } => {
                if matcher.is_match(line) {
                    let new_line = format!("{prefix}{line}{suffix}");
                    let ctx = build_context(&lines, idx, context_lines);
                    changes.push(Change {
                        line: line_num,
                        before: line.to_string(),
                        after: Some(new_line.clone()),
                        context: Some(ctx),
                    });
                    result_lines.push(new_line);
                } else {
                    result_lines.push(line.to_string());
                }
            }
            Op::Indent {
                amount, use_tabs, ..
            } => {
                if matcher.is_match(line) {
                    let indent = if *use_tabs {
                        "\t".repeat(*amount)
                    } else {
                        " ".repeat(*amount)
                    };
                    let new_line = format!("{indent}{line}");
                    let ctx = build_context(&lines, idx, context_lines);
                    changes.push(Change {
                        line: line_num,
                        before: line.to_string(),
                        after: Some(new_line.clone()),
                        context: Some(ctx),
                    });
                    result_lines.push(new_line);
                } else {
                    result_lines.push(line.to_string());
                }
            }
            Op::Dedent { amount, .. } => {
                if matcher.is_match(line) {
                    let new_line = dedent_line(line, *amount);
                    if new_line != line {
                        let ctx = build_context(&lines, idx, context_lines);
                        changes.push(Change {
                            line: line_num,
                            before: line.to_string(),
                            after: Some(new_line.clone()),
                            context: Some(ctx),
                        });
                        result_lines.push(new_line);
                    } else {
                        result_lines.push(line.to_string());
                    }
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

/// Apply a text transformation to matched portions of a line.
fn apply_transform(line: &str, matcher: &Matcher, mode: TransformMode) -> String {
    match matcher {
        Matcher::Literal {
            pattern,
            case_insensitive,
        } => {
            if *case_insensitive {
                let lower_line = line.to_lowercase();
                let lower_pat = pattern.to_lowercase();
                let mut result = String::with_capacity(line.len());
                let mut search_start = 0;
                while let Some(pos) = lower_line[search_start..].find(&lower_pat) {
                    let abs_pos = search_start + pos;
                    result.push_str(&line[search_start..abs_pos]);
                    let matched = &line[abs_pos..abs_pos + pattern.len()];
                    result.push_str(&transform_text(matched, mode));
                    search_start = abs_pos + pattern.len();
                }
                result.push_str(&line[search_start..]);
                result
            } else {
                line.replace(pattern.as_str(), &transform_text(pattern, mode))
            }
        }
        Matcher::Regex(re) => {
            let result = re.replace_all(line, |caps: &regex::Captures| {
                transform_text(&caps[0], mode)
            });
            result.into_owned()
        }
    }
}

/// Transform a text string according to the given mode.
fn transform_text(text: &str, mode: TransformMode) -> String {
    match mode {
        TransformMode::Upper => text.to_uppercase(),
        TransformMode::Lower => text.to_lowercase(),
        TransformMode::Title => {
            let mut result = String::with_capacity(text.len());
            let mut capitalize_next = true;
            for ch in text.chars() {
                if ch.is_whitespace() || ch == '_' || ch == '-' {
                    result.push(ch);
                    capitalize_next = true;
                } else if capitalize_next {
                    for upper in ch.to_uppercase() {
                        result.push(upper);
                    }
                    capitalize_next = false;
                } else {
                    result.push(ch);
                }
            }
            result
        }
        TransformMode::SnakeCase => {
            let mut result = String::with_capacity(text.len() + 4);
            let mut prev_was_lower = false;
            for ch in text.chars() {
                if ch.is_uppercase() {
                    if prev_was_lower {
                        result.push('_');
                    }
                    for lower in ch.to_lowercase() {
                        result.push(lower);
                    }
                    prev_was_lower = false;
                } else if ch == '-' || ch == ' ' {
                    result.push('_');
                    prev_was_lower = false;
                } else {
                    result.push(ch);
                    prev_was_lower = ch.is_lowercase();
                }
            }
            result
        }
        TransformMode::CamelCase => {
            let mut result = String::with_capacity(text.len());
            let mut capitalize_next = false;
            let mut first = true;
            for ch in text.chars() {
                if ch == '_' || ch == '-' || ch == ' ' {
                    capitalize_next = true;
                } else if capitalize_next {
                    for upper in ch.to_uppercase() {
                        result.push(upper);
                    }
                    capitalize_next = false;
                } else if first {
                    for lower in ch.to_lowercase() {
                        result.push(lower);
                    }
                    first = false;
                } else {
                    result.push(ch);
                    first = false;
                }
            }
            result
        }
    }
}

/// Remove up to `amount` leading spaces from a line.
fn dedent_line(line: &str, amount: usize) -> String {
    let leading_spaces = line.len() - line.trim_start_matches(' ').len();
    let remove = leading_spaces.min(amount);
    line[remove..].to_string()
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
            find: "\u{4E16}\u{754C}".to_string(),    // "world"
            replace: "\u{5730}\u{7403}".to_string(), // "earth"
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "\u{4F60}\u{597D}\u{5730}\u{7403}\n");
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

    // ---------------------------------------------------------------
    // Transform operation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_transform_upper() {
        let text = "hello world\nfoo bar\n";
        let op = Op::Transform {
            find: "hello".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "HELLO world\nfoo bar\n");
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].line, 1);
    }

    #[test]
    fn test_transform_lower() {
        let text = "HELLO WORLD\nFOO BAR\n";
        let op = Op::Transform {
            find: "HELLO".to_string(),
            mode: TransformMode::Lower,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "hello WORLD\nFOO BAR\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_transform_title() {
        let text = "hello world\nfoo bar\n";
        let op = Op::Transform {
            find: "hello world".to_string(),
            mode: TransformMode::Title,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "Hello World\nfoo bar\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_transform_snake_case() {
        let text = "let myVariable = 1;\nother line\n";
        let op = Op::Transform {
            find: "myVariable".to_string(),
            mode: TransformMode::SnakeCase,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "let my_variable = 1;\nother line\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_transform_camel_case() {
        let text = "let my_variable = 1;\nother line\n";
        let op = Op::Transform {
            find: "my_variable".to_string(),
            mode: TransformMode::CamelCase,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "let myVariable = 1;\nother line\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_transform_upper_multiple_matches_on_line() {
        let text = "hello and hello again\n";
        let op = Op::Transform {
            find: "hello".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "HELLO and HELLO again\n");
    }

    #[test]
    fn test_transform_no_match() {
        let text = "hello world\n";
        let op = Op::Transform {
            find: "zzz".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_transform_empty_text() {
        let text = "";
        let op = Op::Transform {
            find: "anything".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_transform_with_regex() {
        let text = "let fooBar = 1;\nlet bazQux = 2;\n";
        let op = Op::Transform {
            find: r"[a-z]+[A-Z]\w*".to_string(),
            mode: TransformMode::SnakeCase,
            regex: true,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert!(output.contains("foo_bar"));
        assert!(output.contains("baz_qux"));
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_transform_case_insensitive() {
        let text = "Hello HELLO hello\n";
        let op = Op::Transform {
            find: "hello".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: true,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "HELLO HELLO HELLO\n");
    }

    #[test]
    fn test_transform_crlf_preserved() {
        let text = "hello world\r\nfoo bar\r\n";
        let op = Op::Transform {
            find: "hello".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "HELLO world\r\nfoo bar\r\n");
    }

    #[test]
    fn test_transform_with_line_range() {
        let text = "hello\nhello\nhello\nhello\n";
        let op = Op::Transform {
            find: "hello".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let range = Some(LineRange {
            start: 2,
            end: Some(3),
        });
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, range, 0).unwrap();
        assert_eq!(result.text.unwrap(), "hello\nHELLO\nHELLO\nhello\n");
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_transform_title_with_underscores() {
        let text = "my_func_name\n";
        let op = Op::Transform {
            find: "my_func_name".to_string(),
            mode: TransformMode::Title,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        // Title case capitalizes after underscores
        assert_eq!(result.text.unwrap(), "My_Func_Name\n");
    }

    #[test]
    fn test_transform_snake_case_from_multi_word() {
        let text = "my-kebab-case\n";
        let op = Op::Transform {
            find: "my-kebab-case".to_string(),
            mode: TransformMode::SnakeCase,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "my_kebab_case\n");
    }

    #[test]
    fn test_transform_camel_case_from_snake() {
        let text = "my_var_name\n";
        let op = Op::Transform {
            find: "my_var_name".to_string(),
            mode: TransformMode::CamelCase,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "myVarName\n");
    }

    #[test]
    fn test_transform_camel_case_from_kebab() {
        let text = "my-var-name\n";
        let op = Op::Transform {
            find: "my-var-name".to_string(),
            mode: TransformMode::CamelCase,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "myVarName\n");
    }

    // ---------------------------------------------------------------
    // Surround operation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_surround_basic() {
        let text = "hello world\nfoo bar\n";
        let op = Op::Surround {
            find: "hello".to_string(),
            prefix: "<<".to_string(),
            suffix: ">>".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "<<hello world>>\nfoo bar\n");
        assert_eq!(result.changes.len(), 1);
        assert_eq!(result.changes[0].line, 1);
    }

    #[test]
    fn test_surround_multiple_lines() {
        let text = "foo line 1\nbar line 2\nfoo line 3\n";
        let op = Op::Surround {
            find: "foo".to_string(),
            prefix: "[".to_string(),
            suffix: "]".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(
            result.text.unwrap(),
            "[foo line 1]\nbar line 2\n[foo line 3]\n"
        );
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_surround_no_match() {
        let text = "hello world\n";
        let op = Op::Surround {
            find: "zzz".to_string(),
            prefix: "<".to_string(),
            suffix: ">".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_surround_empty_text() {
        let text = "";
        let op = Op::Surround {
            find: "anything".to_string(),
            prefix: "<".to_string(),
            suffix: ">".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_surround_with_regex() {
        let text = "fn main() {\n    let x = 1;\n}\n";
        let op = Op::Surround {
            find: r"fn\s+\w+".to_string(),
            prefix: "/* ".to_string(),
            suffix: " */".to_string(),
            regex: true,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(
            result.text.unwrap(),
            "/* fn main() { */\n    let x = 1;\n}\n"
        );
    }

    #[test]
    fn test_surround_case_insensitive() {
        let text = "Hello world\nhello world\nHELLO world\n";
        let op = Op::Surround {
            find: "hello".to_string(),
            prefix: "(".to_string(),
            suffix: ")".to_string(),
            regex: false,
            case_insensitive: true,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert_eq!(output, "(Hello world)\n(hello world)\n(HELLO world)\n");
        assert_eq!(result.changes.len(), 3);
    }

    #[test]
    fn test_surround_crlf_preserved() {
        let text = "hello world\r\nfoo bar\r\n";
        let op = Op::Surround {
            find: "hello".to_string(),
            prefix: "[".to_string(),
            suffix: "]".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "[hello world]\r\nfoo bar\r\n");
    }

    #[test]
    fn test_surround_with_line_range() {
        let text = "foo\nfoo\nfoo\nfoo\n";
        let op = Op::Surround {
            find: "foo".to_string(),
            prefix: "<".to_string(),
            suffix: ">".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let range = Some(LineRange {
            start: 2,
            end: Some(3),
        });
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, range, 0).unwrap();
        assert_eq!(result.text.unwrap(), "foo\n<foo>\n<foo>\nfoo\n");
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_surround_with_empty_prefix_and_suffix() {
        let text = "hello world\n";
        let op = Op::Surround {
            find: "hello".to_string(),
            prefix: String::new(),
            suffix: String::new(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        // Surround always records a change when is_match is true, even if
        // prefix and suffix are empty (the new_line equals the original).
        assert!(result.text.is_some());
        let output = result.text.unwrap();
        assert_eq!(output, "hello world\n");
    }

    // ---------------------------------------------------------------
    // Indent operation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_indent_basic() {
        let text = "hello\nworld\n";
        let op = Op::Indent {
            find: "hello".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "    hello\nworld\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_indent_multiple_lines() {
        let text = "foo line 1\nbar line 2\nfoo line 3\n";
        let op = Op::Indent {
            find: "foo".to_string(),
            amount: 2,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(
            result.text.unwrap(),
            "  foo line 1\nbar line 2\n  foo line 3\n"
        );
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_indent_with_tabs() {
        let text = "hello\nworld\n";
        let op = Op::Indent {
            find: "hello".to_string(),
            amount: 2,
            use_tabs: true,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "\t\thello\nworld\n");
    }

    #[test]
    fn test_indent_no_match() {
        let text = "hello world\n";
        let op = Op::Indent {
            find: "zzz".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_indent_empty_text() {
        let text = "";
        let op = Op::Indent {
            find: "anything".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_indent_zero_amount() {
        let text = "hello\n";
        let op = Op::Indent {
            find: "hello".to_string(),
            amount: 0,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        // Indent by 0 means prepend "" which produces the same line,
        // but the engine still records it as a change because it always
        // pushes when is_match is true for Indent.
        assert!(result.text.is_some());
        assert_eq!(result.text.unwrap(), "hello\n");
    }

    #[test]
    fn test_indent_with_regex() {
        let text = "fn main() {\nlet x = 1;\n}\n";
        let op = Op::Indent {
            find: r"let\s+".to_string(),
            amount: 4,
            use_tabs: false,
            regex: true,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "fn main() {\n    let x = 1;\n}\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_indent_case_insensitive() {
        let text = "Hello\nhello\nHELLO\n";
        let op = Op::Indent {
            find: "hello".to_string(),
            amount: 2,
            use_tabs: false,
            regex: false,
            case_insensitive: true,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "  Hello\n  hello\n  HELLO\n");
        assert_eq!(result.changes.len(), 3);
    }

    #[test]
    fn test_indent_crlf_preserved() {
        let text = "hello\r\nworld\r\n";
        let op = Op::Indent {
            find: "hello".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "    hello\r\nworld\r\n");
    }

    #[test]
    fn test_indent_with_line_range() {
        let text = "foo\nfoo\nfoo\nfoo\n";
        let op = Op::Indent {
            find: "foo".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let range = Some(LineRange {
            start: 2,
            end: Some(3),
        });
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, range, 0).unwrap();
        assert_eq!(result.text.unwrap(), "foo\n    foo\n    foo\nfoo\n");
        assert_eq!(result.changes.len(), 2);
    }

    // ---------------------------------------------------------------
    // Dedent operation tests
    // ---------------------------------------------------------------

    #[test]
    fn test_dedent_basic() {
        let text = "    hello\nworld\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "hello\nworld\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_dedent_partial() {
        // Only 2 spaces of leading whitespace, dedent by 4 should remove only 2
        let text = "  hello\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "hello\n");
    }

    #[test]
    fn test_dedent_no_leading_spaces() {
        // Line matches but has no leading spaces -- nothing to remove
        let text = "hello\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        // No actual change because line has no leading spaces
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_dedent_multiple_lines() {
        let text = "    foo line 1\n    bar line 2\n    foo line 3\n";
        let op = Op::Dedent {
            find: "foo".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(
            result.text.unwrap(),
            "foo line 1\n    bar line 2\nfoo line 3\n"
        );
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_dedent_no_match() {
        let text = "    hello world\n";
        let op = Op::Dedent {
            find: "zzz".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_dedent_empty_text() {
        let text = "";
        let op = Op::Dedent {
            find: "anything".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert!(result.text.is_none());
        assert!(result.changes.is_empty());
    }

    #[test]
    fn test_dedent_with_regex() {
        let text = "    let x = 1;\n    fn main() {\n";
        let op = Op::Dedent {
            find: r"let\s+".to_string(),
            amount: 4,
            regex: true,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "let x = 1;\n    fn main() {\n");
        assert_eq!(result.changes.len(), 1);
    }

    #[test]
    fn test_dedent_case_insensitive() {
        let text = "    Hello\n    hello\n    HELLO\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 2,
            regex: false,
            case_insensitive: true,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "  Hello\n  hello\n  HELLO\n");
        assert_eq!(result.changes.len(), 3);
    }

    #[test]
    fn test_dedent_crlf_preserved() {
        let text = "    hello\r\nworld\r\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.text.unwrap(), "hello\r\nworld\r\n");
    }

    #[test]
    fn test_dedent_with_line_range() {
        let text = "    foo\n    foo\n    foo\n    foo\n";
        let op = Op::Dedent {
            find: "foo".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let range = Some(LineRange {
            start: 2,
            end: Some(3),
        });
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, range, 0).unwrap();
        assert_eq!(result.text.unwrap(), "    foo\nfoo\nfoo\n    foo\n");
        assert_eq!(result.changes.len(), 2);
    }

    #[test]
    fn test_dedent_only_removes_spaces_not_tabs() {
        // Dedent only strips leading spaces, not tabs
        let text = "\t\thello\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        // The dedent_line function only strips spaces (trim_start_matches(' ')),
        // tabs are not removed.
        assert!(result.text.is_none());
    }

    // ---------------------------------------------------------------
    // Indent then Dedent roundtrip
    // ---------------------------------------------------------------

    #[test]
    fn test_indent_then_dedent_roundtrip() {
        let original = "hello world\nfoo bar\n";

        // Step 1: Indent by 4
        let indent_op = Op::Indent {
            find: "hello".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let indent_matcher = Matcher::new(&indent_op).unwrap();
        let indented = apply(original, &indent_op, &indent_matcher, None, 0).unwrap();
        let indented_text = indented.text.unwrap();
        assert_eq!(indented_text, "    hello world\nfoo bar\n");

        // Step 2: Dedent by 4 (the find still matches because "hello" is in the line)
        let dedent_op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let dedent_matcher = Matcher::new(&dedent_op).unwrap();
        let dedented = apply(&indented_text, &dedent_op, &dedent_matcher, None, 0).unwrap();
        assert_eq!(dedented.text.unwrap(), original);
    }

    // ---------------------------------------------------------------
    // Undo entry tests for new ops
    // ---------------------------------------------------------------

    #[test]
    fn test_transform_undo_stores_original() {
        let text = "hello world\n";
        let op = Op::Transform {
            find: "hello".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.undo.unwrap().original_text, text);
    }

    #[test]
    fn test_surround_undo_stores_original() {
        let text = "hello world\n";
        let op = Op::Surround {
            find: "hello".to_string(),
            prefix: "<".to_string(),
            suffix: ">".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.undo.unwrap().original_text, text);
    }

    #[test]
    fn test_indent_undo_stores_original() {
        let text = "hello world\n";
        let op = Op::Indent {
            find: "hello".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.undo.unwrap().original_text, text);
    }

    #[test]
    fn test_dedent_undo_stores_original() {
        let text = "    hello world\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        assert_eq!(result.undo.unwrap().original_text, text);
    }

    // ---------------------------------------------------------------
    // Line preservation tests for new ops
    // ---------------------------------------------------------------

    #[test]
    fn test_transform_preserves_line_count() {
        let text = "hello\nworld\nfoo\n";
        let op = Op::Transform {
            find: "hello".to_string(),
            mode: TransformMode::Upper,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert_eq!(text.lines().count(), output.lines().count());
    }

    #[test]
    fn test_surround_preserves_line_count() {
        let text = "hello\nworld\nfoo\n";
        let op = Op::Surround {
            find: "hello".to_string(),
            prefix: "<".to_string(),
            suffix: ">".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert_eq!(text.lines().count(), output.lines().count());
    }

    #[test]
    fn test_indent_preserves_line_count() {
        let text = "hello\nworld\nfoo\n";
        let op = Op::Indent {
            find: "hello".to_string(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert_eq!(text.lines().count(), output.lines().count());
    }

    #[test]
    fn test_dedent_preserves_line_count() {
        let text = "    hello\n    world\n    foo\n";
        let op = Op::Dedent {
            find: "hello".to_string(),
            amount: 4,
            regex: false,
            case_insensitive: false,
        };
        let matcher = Matcher::new(&op).unwrap();
        let result = apply(text, &op, &matcher, None, 0).unwrap();
        let output = result.text.unwrap();
        assert_eq!(text.lines().count(), output.lines().count());
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
        prop::collection::vec("[^\n\r]{0,80}", 1..10).prop_map(|lines| lines.join("\n") + "\n")
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

        /// Indent then Dedent by the same amount should restore the original text
        /// when every line contains the find pattern and starts with enough spaces.
        #[test]
        fn prop_indent_dedent_roundtrip(
            amount in 1usize..=16,
        ) {
            // Use a known find pattern that appears on every line
            let find = "marker".to_string();
            let text = "marker line one\nmarker line two\nmarker line three\n";

            let indent_op = Op::Indent {
                find: find.clone(),
                amount,
                use_tabs: false,
                regex: false,
                case_insensitive: false,
            };
            let indent_matcher = Matcher::new(&indent_op).unwrap();
            let indented = apply(text, &indent_op, &indent_matcher, None, 0).unwrap();
            let indented_text = indented.text.unwrap();

            // Every line should now start with `amount` spaces
            for line in indented_text.lines() {
                let leading = line.len() - line.trim_start_matches(' ').len();
                prop_assert!(leading >= amount, "Expected at least {} leading spaces, got {}", amount, leading);
            }

            let dedent_op = Op::Dedent {
                find: find.clone(),
                amount,
                regex: false,
                case_insensitive: false,
            };
            let dedent_matcher = Matcher::new(&dedent_op).unwrap();
            let dedented = apply(&indented_text, &dedent_op, &dedent_matcher, None, 0).unwrap();
            prop_assert_eq!(dedented.text.unwrap(), text);
        }

        /// Transform Upper then Lower should restore the original when
        /// the text is already all lowercase ASCII.
        #[test]
        fn prop_transform_upper_lower_roundtrip(
            find in "[a-z]{1,8}",
        ) {
            let text = format!("prefix {find} suffix\n");

            let upper_op = Op::Transform {
                find: find.clone(),
                mode: crate::operation::TransformMode::Upper,
                regex: false,
                case_insensitive: false,
            };
            let upper_matcher = Matcher::new(&upper_op).unwrap();
            let uppered = apply(&text, &upper_op, &upper_matcher, None, 0).unwrap();

            if let Some(ref upper_text) = uppered.text {
                let upper_find = find.to_uppercase();
                let lower_op = Op::Transform {
                    find: upper_find,
                    mode: crate::operation::TransformMode::Lower,
                    regex: false,
                    case_insensitive: false,
                };
                let lower_matcher = Matcher::new(&lower_op).unwrap();
                let lowered = apply(upper_text, &lower_op, &lower_matcher, None, 0).unwrap();
                prop_assert_eq!(lowered.text.unwrap(), text);
            }
        }

        /// Surround preserves line count.
        #[test]
        fn prop_surround_preserves_line_count(
            text in arb_multiline_text(),
            find in arb_find_pattern(),
        ) {
            let op = Op::Surround {
                find,
                prefix: "<<".to_string(),
                suffix: ">>".to_string(),
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
                    "Surround should preserve line count: input={} output={}",
                    input_lines,
                    output_lines
                );
            }
        }

        /// Transform preserves line count.
        #[test]
        fn prop_transform_preserves_line_count(
            text in arb_multiline_text(),
            find in arb_find_pattern(),
        ) {
            let op = Op::Transform {
                find,
                mode: crate::operation::TransformMode::Upper,
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
                    "Transform should preserve line count: input={} output={}",
                    input_lines,
                    output_lines
                );
            }
        }

        /// Indent preserves line count.
        #[test]
        fn prop_indent_preserves_line_count(
            text in arb_multiline_text(),
            find in arb_find_pattern(),
            amount in 1usize..=16,
        ) {
            let op = Op::Indent {
                find,
                amount,
                use_tabs: false,
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
                    "Indent should preserve line count: input={} output={}",
                    input_lines,
                    output_lines
                );
            }
        }
    }
}
