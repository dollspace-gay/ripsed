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

    let modified_text = if changes.is_empty() {
        None
    } else {
        // Preserve trailing newline if original had one
        let mut joined = result_lines.join("\n");
        if text.ends_with('\n') {
            joined.push('\n');
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
}
