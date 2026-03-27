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

impl UndoResponse {
    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self)
            .expect("UndoResponse serialization is infallible for known field types")
    }
}

impl JsonResponse {
    /// Build a success response.
    pub fn success(dry_run: bool, summary: Summary, results: Vec<OpResult>) -> Self {
        Self {
            version: crate::schema::CURRENT_VERSION.to_string(),
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
            version: crate::schema::CURRENT_VERSION.to_string(),
            success: false,
            dry_run: false,
            summary: Summary::default(),
            results: vec![],
            errors,
        }
    }

    /// Serialize to JSON string.
    pub fn to_json(&self) -> String {
        serde_json::to_string_pretty(self)
            .expect("JsonResponse serialization is infallible for known field types")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ripsed_core::diff::{Change, FileChanges};

    #[test]
    fn success_response_has_correct_fields() {
        let summary = Summary {
            files_matched: 3,
            files_modified: 2,
            total_replacements: 5,
        };
        let results = vec![OpResult {
            operation_index: 0,
            files: vec![FileChanges {
                path: "src/lib.rs".into(),
                changes: vec![Change {
                    line: 1,
                    before: "old".into(),
                    after: Some("new".into()),
                    context: None,
                }],
            }],
        }];
        let resp = JsonResponse::success(true, summary, results);
        assert_eq!(resp.version, "1");
        assert!(resp.success);
        assert!(resp.dry_run);
        assert_eq!(resp.summary.files_matched, 3);
        assert_eq!(resp.results.len(), 1);
        assert!(resp.errors.is_empty());
    }

    #[test]
    fn error_response_has_correct_fields() {
        let err = RipsedError::invalid_request("bad input", "fix it");
        let resp = JsonResponse::error(vec![err]);
        assert_eq!(resp.version, "1");
        assert!(!resp.success);
        assert!(!resp.dry_run);
        assert_eq!(resp.summary, Summary::default());
        assert!(resp.results.is_empty());
        assert_eq!(resp.errors.len(), 1);
        assert_eq!(resp.errors[0].message, "bad input");
    }

    #[test]
    fn to_json_produces_valid_json() {
        let resp = JsonResponse::success(false, Summary::default(), vec![]);
        let json_str = resp.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["version"], "1");
        assert_eq!(parsed["success"], true);
        assert_eq!(parsed["dry_run"], false);
    }

    #[test]
    fn to_json_error_response_includes_errors() {
        let err = RipsedError::internal_error("oops");
        let resp = JsonResponse::error(vec![err]);
        let json_str = resp.to_json();
        let parsed: serde_json::Value = serde_json::from_str(&json_str).unwrap();
        assert_eq!(parsed["success"], false);
        assert_eq!(parsed["errors"][0]["code"], "internal_error");
    }

    #[test]
    fn response_roundtrips_through_json() {
        let resp = JsonResponse::success(
            true,
            Summary {
                files_matched: 1,
                files_modified: 0,
                total_replacements: 2,
            },
            vec![],
        );
        let json_str = resp.to_json();
        let deserialized: JsonResponse = serde_json::from_str(&json_str).unwrap();
        assert_eq!(deserialized.version, "1");
        assert!(deserialized.success);
        assert!(deserialized.dry_run);
        assert_eq!(deserialized.summary.total_replacements, 2);
    }

    #[test]
    fn undo_response_serializes() {
        let resp = UndoResponse {
            version: "1".into(),
            success: true,
            undo: UndoSummary {
                operations_reverted: 2,
                files_restored: 3,
                log_entries_remaining: 8,
            },
        };
        let json = serde_json::to_value(&resp).unwrap();
        assert_eq!(json["undo"]["operations_reverted"], 2);
        assert_eq!(json["undo"]["files_restored"], 3);
        assert_eq!(json["undo"]["log_entries_remaining"], 8);
    }
}
