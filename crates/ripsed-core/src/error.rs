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
            format!(
                "The path '{path}' does not exist. Did you mean '{}'?",
                suggestions[0]
            )
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
            hint: format!(
                "Check file permissions for '{path}'. Try chmod or run with appropriate permissions."
            ),
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

    pub fn atomic_rollback(operation_index: usize, file: &str, reason: &str) -> Self {
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

#[cfg(test)]
mod tests {
    use super::*;

    // ── ErrorCode serde ──

    #[test]
    fn error_code_serializes_to_snake_case() {
        let json = serde_json::to_string(&ErrorCode::NoMatches).unwrap();
        assert_eq!(json, r#""no_matches""#);

        let json = serde_json::to_string(&ErrorCode::InvalidRegex).unwrap();
        assert_eq!(json, r#""invalid_regex""#);

        let json = serde_json::to_string(&ErrorCode::BinaryFileSkipped).unwrap();
        assert_eq!(json, r#""binary_file_skipped""#);
    }

    // ── Factory methods ──

    #[test]
    fn no_matches_without_suggestions() {
        let err = RipsedError::no_matches(0, "foobar", 10, vec![]);
        assert_eq!(err.code, ErrorCode::NoMatches);
        assert_eq!(err.operation_index, Some(0));
        assert!(err.message.contains("foobar"));
        assert!(err.message.contains("10 files"));
        assert!(err.hint.contains("typos"));
        assert!(!err.context.contains_key("suggestions"));
    }

    #[test]
    fn no_matches_with_suggestions() {
        let err = RipsedError::no_matches(2, "fobar", 5, vec!["foobar".into()]);
        assert!(err.hint.contains("foobar"));
        let suggestions = err.context.get("suggestions").unwrap().as_array().unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0], "foobar");
    }

    #[test]
    fn invalid_regex_has_pattern_context() {
        let err = RipsedError::invalid_regex(1, "[bad", "unclosed bracket");
        assert_eq!(err.code, ErrorCode::InvalidRegex);
        assert_eq!(err.operation_index, Some(1));
        assert!(err.message.contains("unclosed bracket"));
        assert_eq!(err.context["pattern"], "[bad");
    }

    #[test]
    fn invalid_request_has_no_operation_index() {
        let err = RipsedError::invalid_request("bad request", "fix it");
        assert_eq!(err.code, ErrorCode::InvalidRequest);
        assert!(err.operation_index.is_none());
        assert_eq!(err.message, "bad request");
        assert_eq!(err.hint, "fix it");
    }

    #[test]
    fn file_not_found_without_suggestions() {
        let err = RipsedError::file_not_found("/missing/path", vec![]);
        assert_eq!(err.code, ErrorCode::FileNotFound);
        assert!(err.hint.contains("typos"));
        assert_eq!(err.context["path"], "/missing/path");
    }

    #[test]
    fn file_not_found_with_suggestions() {
        let err = RipsedError::file_not_found("/src/mian.rs", vec!["/src/main.rs".into()]);
        assert!(err.hint.contains("/src/main.rs"));
    }

    #[test]
    fn permission_denied_includes_path() {
        let err = RipsedError::permission_denied("/etc/shadow");
        assert_eq!(err.code, ErrorCode::PermissionDenied);
        assert!(err.message.contains("/etc/shadow"));
        assert_eq!(err.context["path"], "/etc/shadow");
    }

    #[test]
    fn binary_file_skipped_includes_path() {
        let err = RipsedError::binary_file_skipped("image.png");
        assert_eq!(err.code, ErrorCode::BinaryFileSkipped);
        assert!(err.message.contains("image.png"));
    }

    #[test]
    fn atomic_rollback_includes_file_and_reason() {
        let err = RipsedError::atomic_rollback(3, "src/lib.rs", "disk full");
        assert_eq!(err.code, ErrorCode::AtomicRollback);
        assert_eq!(err.operation_index, Some(3));
        assert!(err.message.contains("src/lib.rs"));
        assert!(err.message.contains("disk full"));
        assert_eq!(err.context["file"], "src/lib.rs");
        assert_eq!(err.context["reason"], "disk full");
    }

    #[test]
    fn write_failed_includes_os_error() {
        let err = RipsedError::write_failed("/tmp/out", "permission denied");
        assert_eq!(err.code, ErrorCode::WriteFailed);
        assert!(err.message.contains("permission denied"));
        assert_eq!(err.context["os_error"], "permission denied");
    }

    #[test]
    fn internal_error_has_bug_hint() {
        let err = RipsedError::internal_error("unexpected state");
        assert_eq!(err.code, ErrorCode::InternalError);
        assert!(err.hint.contains("bug"));
    }

    // ── Display impl ──

    #[test]
    fn display_shows_message() {
        let err = RipsedError::invalid_request("test message", "hint");
        assert_eq!(format!("{err}"), "test message");
    }

    // ── JSON serialization ──

    #[test]
    fn error_serializes_to_json() {
        let err = RipsedError::no_matches(0, "pat", 1, vec![]);
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "no_matches");
        assert_eq!(json["operation_index"], 0);
        assert!(json["message"].is_string());
        assert!(json["hint"].is_string());
    }

    #[test]
    fn error_without_context_omits_context_field() {
        let err = RipsedError::internal_error("oops");
        let json = serde_json::to_value(&err).unwrap();
        // context should be omitted when empty (skip_serializing_if)
        assert!(json.get("context").is_none());
    }

    #[test]
    fn error_without_operation_index_omits_field() {
        let err = RipsedError::invalid_request("bad", "fix");
        let json = serde_json::to_value(&err).unwrap();
        assert!(json.get("operation_index").is_none());
    }
}
