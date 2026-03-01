use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// All error codes in ripsed's error taxonomy.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    NoMatches,
    InvalidRegex,
    InvalidRequest,
    FileNotFound,
    PermissionDenied,
    BinaryFileSkipped,
    AtomicRollback,
    WriteFailed,
    InternalError,
}

/// A structured error with code, message, hint, and context.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RipsedError {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub operation_index: Option<usize>,
    pub code: ErrorCode,
    pub message: String,
    pub hint: String,
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub context: HashMap<String, serde_json::Value>,
}

impl std::fmt::Display for RipsedError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for RipsedError {}

impl RipsedError {
    pub fn no_matches(
        operation_index: usize,
        pattern: &str,
        files_searched: usize,
        suggestions: Vec<String>,
    ) -> Self {
        let mut context = HashMap::new();
        context.insert(
            "pattern".to_string(),
            serde_json::Value::String(pattern.to_string()),
        );
        context.insert(
            "files_searched".to_string(),
            serde_json::Value::Number(files_searched.into()),
        );
        if !suggestions.is_empty() {
            context.insert(
                "suggestions".to_string(),
                serde_json::Value::Array(
                    suggestions
                        .iter()
                        .map(|s| serde_json::Value::String(s.clone()))
                        .collect(),
                ),
            );
        }

        let hint = if suggestions.is_empty() {
            "Check for typos in the pattern. If using regex, ensure --regex is set.".to_string()
        } else {
            format!(
                "Check for typos in the pattern. Did you mean '{}'? If using regex, ensure --regex is set.",
                suggestions[0]
            )
        };

        Self {
            operation_index: Some(operation_index),
            code: ErrorCode::NoMatches,
            message: format!(
                "Pattern '{}' matched 0 lines across {} files.",
                pattern, files_searched
            ),
            hint,
            context,
        }
    }

    pub fn invalid_regex(operation_index: usize, pattern: &str, error: &str) -> Self {
        let mut context = HashMap::new();
        context.insert(
            "pattern".to_string(),
            serde_json::Value::String(pattern.to_string()),
        );

        Self {
            operation_index: Some(operation_index),
            code: ErrorCode::InvalidRegex,
            message: format!("Regex failed to compile: {error}."),
            hint: format!("Check the regex syntax in pattern '{pattern}'. {error}"),
            context,
        }
    }

    pub fn invalid_request(message: impl Into<String>, hint: impl Into<String>) -> Self {
        Self {
            operation_index: None,
            code: ErrorCode::InvalidRequest,
            message: message.into(),
            hint: hint.into(),
            context: HashMap::new(),
        }
    }

    pub fn file_not_found(path: &str, suggestions: Vec<String>) -> Self {
        let mut context = HashMap::new();
        context.insert(
            "path".to_string(),
            serde_json::Value::String(path.to_string()),
        );

        let hint = if suggestions.is_empty() {
            format!("The path '{path}' does not exist. Check for typos.")
        } else {
            format!("The path '{path}' does not exist. Did you mean '{}'?", suggestions[0])
        };

        Self {
            operation_index: None,
            code: ErrorCode::FileNotFound,
            message: format!("Target path '{path}' does not exist."),
            hint,
            context,
        }
    }

    pub fn permission_denied(path: &str) -> Self {
        let mut context = HashMap::new();
        context.insert(
            "path".to_string(),
            serde_json::Value::String(path.to_string()),
        );

        Self {
            operation_index: None,
            code: ErrorCode::PermissionDenied,
            message: format!("Cannot read or write '{path}'."),
            hint: format!("Check file permissions for '{path}'. Try chmod or run with appropriate permissions."),
            context,
        }
    }

    pub fn binary_file_skipped(path: &str) -> Self {
        let mut context = HashMap::new();
        context.insert(
            "path".to_string(),
            serde_json::Value::String(path.to_string()),
        );

        Self {
            operation_index: None,
            code: ErrorCode::BinaryFileSkipped,
            message: format!("Binary file '{path}' was skipped."),
            hint: "Binary files are skipped by default. Use --binary to include them.".to_string(),
            context,
        }
    }

    pub fn atomic_rollback(
        operation_index: usize,
        file: &str,
        reason: &str,
    ) -> Self {
        let mut context = HashMap::new();
        context.insert(
            "file".to_string(),
            serde_json::Value::String(file.to_string()),
        );
        context.insert(
            "reason".to_string(),
            serde_json::Value::String(reason.to_string()),
        );

        Self {
            operation_index: Some(operation_index),
            code: ErrorCode::AtomicRollback,
            message: format!(
                "Batch operation failed at '{file}': {reason}. All changes reverted."
            ),
            hint: "Nothing was written. Consider splitting into smaller batches or fixing the failing operation.".to_string(),
            context,
        }
    }

    pub fn write_failed(path: &str, os_error: &str) -> Self {
        let mut context = HashMap::new();
        context.insert(
            "path".to_string(),
            serde_json::Value::String(path.to_string()),
        );
        context.insert(
            "os_error".to_string(),
            serde_json::Value::String(os_error.to_string()),
        );

        Self {
            operation_index: None,
            code: ErrorCode::WriteFailed,
            message: format!("Could not write to '{path}': {os_error}."),
            hint: "Check disk space and path validity.".to_string(),
            context,
        }
    }

    pub fn internal_error(message: impl Into<String>) -> Self {
        Self {
            operation_index: None,
            code: ErrorCode::InternalError,
            message: message.into(),
            hint: "This is a bug in ripsed. Please report it.".to_string(),
            context: HashMap::new(),
        }
    }
}
