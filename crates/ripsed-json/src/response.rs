use ripsed_core::diff::{OpResult, Summary};
use ripsed_core::error::RipsedError;
use serde::{Deserialize, Serialize};

/// The top-level JSON response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonResponse {
    pub version: String,
    pub success: bool,
    pub dry_run: bool,
    pub summary: Summary,
    pub results: Vec<OpResult>,
    pub errors: Vec<RipsedError>,
}

/// An undo response.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoResponse {
    pub version: String,
    pub success: bool,
    pub undo: UndoSummary,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoSummary {
    pub operations_reverted: usize,
    pub files_restored: usize,
    pub log_entries_remaining: usize,
}

impl JsonResponse {
    /// Build a success response.
    pub fn success(dry_run: bool, summary: Summary, results: Vec<OpResult>) -> Self {
        Self {
            version: "1".to_string(),
            success: true,
            dry_run,
            summary,
            results,
            errors: vec![],
        }
    }

    /// Build an error response.
    pub fn error(errors: Vec<RipsedError>) -> Self {
        Self {
            version: "1".to_string(),
            success: false,
            dry_run: false,
            summary: Summary::default(),
            results: vec![],
            errors,
        }
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self).unwrap_or_else(|_| {
            r#"{"version":"1","success":false,"errors":[{"code":"internal_error","message":"Failed to serialize response"}]}"#.to_string()
        })
    }
}
