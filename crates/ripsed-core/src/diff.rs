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
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Summary {
    pub files_matched: usize,
    pub files_modified: usize,
    pub total_replacements: usize,
}
