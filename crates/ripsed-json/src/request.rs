use ripsed_core::error::RipsedError;
use ripsed_core::operation::{Op, OpOptions};
use serde::{Deserialize, Serialize};

/// A structured JSON request from an agent.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonRequest {
    #[serde(default = "default_version")]
    pub version: String,
    pub operations: Vec<JsonOp>,
    #[serde(default)]
    pub options: OpOptions,
    /// Undo request (mutually exclusive with operations).
    pub undo: Option<UndoRequest>,
}

/// A single operation in a JSON request, with per-operation glob.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JsonOp {
    #[serde(flatten)]
    pub op: Op,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub glob: Option<String>,
}

/// An undo request.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoRequest {
    pub last: usize,
}

fn default_version() -> String {
    "1".to_string()
}

impl JsonRequest {
    /// Parse and validate a JSON request from a string.
    pub fn parse(input: &str) -> Result<Self, RipsedError> {
        let request: JsonRequest = serde_json::from_str(input).map_err(|e| {
            RipsedError::invalid_request(
                format!("Failed to parse JSON request: {e}"),
                "Check that the JSON is well-formed and matches the ripsed request schema.",
            )
        })?;

        request.validate()?;
        Ok(request)
    }

    /// Validate the request after parsing.
    fn validate(&self) -> Result<(), RipsedError> {
        if self.version != "1" {
            return Err(RipsedError::invalid_request(
                format!("Unknown version '{}'. Supported versions: 1", self.version),
                "Set \"version\": \"1\" in your request.",
            ));
        }

        if self.undo.is_some() && !self.operations.is_empty() {
            return Err(RipsedError::invalid_request(
                "Request cannot contain both 'operations' and 'undo'.",
                "Send undo and operations as separate requests.",
            ));
        }

        if self.undo.is_none() && self.operations.is_empty() {
            return Err(RipsedError::invalid_request(
                "Request must contain 'operations' or 'undo'.",
                "Add at least one operation or an undo request.",
            ));
        }

        Ok(())
    }

    /// Extract the list of operations with their effective globs.
    pub fn into_ops(self) -> (Vec<(Op, Option<String>)>, OpOptions) {
        let ops = self
            .operations
            .into_iter()
            .map(|json_op| {
                let glob = json_op.glob.or_else(|| self.options.glob.clone());
                (json_op.op, glob)
            })
            .collect();
        (ops, self.options)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_simple_replace() {
        let input = r#"{
            "operations": [{"op": "replace", "find": "foo", "replace": "bar"}]
        }"#;
        let req = JsonRequest::parse(input).unwrap();
        assert_eq!(req.operations.len(), 1);
        assert!(req.options.dry_run); // default
    }

    #[test]
    fn test_parse_invalid_json() {
        let result = JsonRequest::parse("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_empty_operations() {
        let input = r#"{"operations": []}"#;
        let result = JsonRequest::parse(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_unknown_version() {
        let input = r#"{"version": "99", "operations": [{"op": "replace", "find": "a", "replace": "b"}]}"#;
        let result = JsonRequest::parse(input);
        assert!(result.is_err());
    }
}
