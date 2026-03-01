use serde::{Deserialize, Serialize};

/// A single change within a file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Change {
    /// 1-indexed line number where the change occurs.
    pub line: usize,
    /// The original line content.
    pub before: String,
    /// The modified line content (None for deletions).
    pub after: Option<String>,
    /// Surrounding context lines.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context: Option<ChangeContext>,
}

/// Context lines surrounding a change for display.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ChangeContext {
    pub before: Vec<String>,
    pub after: Vec<String>,
}

/// All changes applied to a single file.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct FileChanges {
    pub path: String,
    pub changes: Vec<Change>,
}

/// The result of applying an operation, including all file changes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpResult {
    pub operation_index: usize,
    pub files: Vec<FileChanges>,
}

/// Summary statistics for the full run.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Summary {
    pub files_matched: usize,
    pub files_modified: usize,
    pub total_replacements: usize,
}

/// Compute an aggregate summary from a slice of `OpResult`s.
///
/// `files_matched` counts the number of unique file paths that appear in any result.
/// `files_modified` counts the number of unique file paths that have at least one change.
/// `total_replacements` counts the total number of individual changes across all files.
pub fn compute_summary(results: &[OpResult]) -> Summary {
    use std::collections::HashSet;

    let mut matched_paths: HashSet<&str> = HashSet::new();
    let mut modified_paths: HashSet<&str> = HashSet::new();
    let mut total_replacements: usize = 0;

    for result in results {
        for file_changes in &result.files {
            matched_paths.insert(&file_changes.path);
            if !file_changes.changes.is_empty() {
                modified_paths.insert(&file_changes.path);
                total_replacements += file_changes.changes.len();
            }
        }
    }

    Summary {
        files_matched: matched_paths.len(),
        files_modified: modified_paths.len(),
        total_replacements,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compute_summary_empty() {
        let results: Vec<OpResult> = vec![];
        let summary = compute_summary(&results);
        assert_eq!(
            summary,
            Summary {
                files_matched: 0,
                files_modified: 0,
                total_replacements: 0,
            }
        );
    }

    #[test]
    fn test_compute_summary_single_result() {
        let results = vec![OpResult {
            operation_index: 0,
            files: vec![FileChanges {
                path: "src/main.rs".to_string(),
                changes: vec![
                    Change {
                        line: 1,
                        before: "old".to_string(),
                        after: Some("new".to_string()),
                        context: None,
                    },
                    Change {
                        line: 5,
                        before: "old2".to_string(),
                        after: Some("new2".to_string()),
                        context: None,
                    },
                ],
            }],
        }];
        let summary = compute_summary(&results);
        assert_eq!(
            summary,
            Summary {
                files_matched: 1,
                files_modified: 1,
                total_replacements: 2,
            }
        );
    }

    #[test]
    fn test_compute_summary_multiple_files() {
        let results = vec![
            OpResult {
                operation_index: 0,
                files: vec![
                    FileChanges {
                        path: "a.rs".to_string(),
                        changes: vec![Change {
                            line: 1,
                            before: "x".to_string(),
                            after: Some("y".to_string()),
                            context: None,
                        }],
                    },
                    FileChanges {
                        path: "b.rs".to_string(),
                        changes: vec![Change {
                            line: 2,
                            before: "x".to_string(),
                            after: Some("y".to_string()),
                            context: None,
                        }],
                    },
                ],
            },
            OpResult {
                operation_index: 1,
                files: vec![FileChanges {
                    path: "a.rs".to_string(),
                    changes: vec![Change {
                        line: 3,
                        before: "z".to_string(),
                        after: Some("w".to_string()),
                        context: None,
                    }],
                }],
            },
        ];
        let summary = compute_summary(&results);
        assert_eq!(
            summary,
            Summary {
                files_matched: 2,
                files_modified: 2,
                total_replacements: 3,
            }
        );
    }

    #[test]
    fn test_compute_summary_file_with_no_changes() {
        let results = vec![OpResult {
            operation_index: 0,
            files: vec![FileChanges {
                path: "empty.rs".to_string(),
                changes: vec![],
            }],
        }];
        let summary = compute_summary(&results);
        assert_eq!(
            summary,
            Summary {
                files_matched: 1,
                files_modified: 0,
                total_replacements: 0,
            }
        );
    }

    #[test]
    fn test_compute_summary_deletions_counted() {
        let results = vec![OpResult {
            operation_index: 0,
            files: vec![FileChanges {
                path: "file.rs".to_string(),
                changes: vec![
                    Change {
                        line: 1,
                        before: "deleted line".to_string(),
                        after: None, // deletion
                        context: None,
                    },
                    Change {
                        line: 3,
                        before: "replaced".to_string(),
                        after: Some("new".to_string()),
                        context: None,
                    },
                ],
            }],
        }];
        let summary = compute_summary(&results);
        assert_eq!(summary.total_replacements, 2);
    }
}
