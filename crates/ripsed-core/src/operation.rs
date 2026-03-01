use serde::{Deserialize, Serialize};

/// The intermediate representation for all ripsed operations.
/// Both CLI args and JSON requests are normalized into this form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
pub enum Op {
    Replace {
        find: String,
        replace: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    Delete {
        find: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    InsertAfter {
        find: String,
        content: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    InsertBefore {
        find: String,
        content: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    ReplaceLine {
        find: String,
        content: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
}

/// Options that control how operations are applied.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OpOptions {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    pub root: Option<String>,
    #[serde(default = "default_true")]
    pub gitignore: bool,
    #[serde(default)]
    pub backup: bool,
    #[serde(default)]
    pub atomic: bool,
    pub glob: Option<String>,
    pub ignore: Option<String>,
    #[serde(default)]
    pub hidden: bool,
    pub max_depth: Option<usize>,
    pub line_range: Option<LineRange>,
}

impl Default for OpOptions {
    fn default() -> Self {
        Self {
            dry_run: true,
            root: None,
            gitignore: true,
            backup: false,
            atomic: false,
            glob: None,
            ignore: None,
            hidden: false,
            max_depth: None,
            line_range: None,
        }
    }
}

/// A range of lines to operate on (1-indexed, inclusive).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
pub struct LineRange {
    pub start: usize,
    pub end: Option<usize>,
}

impl LineRange {
    pub fn contains(&self, line: usize) -> bool {
        line >= self.start && self.end.is_none_or(|end| line <= end)
    }
}

fn default_true() -> bool {
    true
}

impl Op {
    /// Extract the glob pattern from the operation, if present in JSON requests.
    /// In CLI mode, globs come from OpOptions instead.
    pub fn find_pattern(&self) -> &str {
        match self {
            Op::Replace { find, .. }
            | Op::Delete { find, .. }
            | Op::InsertAfter { find, .. }
            | Op::InsertBefore { find, .. }
            | Op::ReplaceLine { find, .. } => find,
        }
    }

    pub fn is_regex(&self) -> bool {
        match self {
            Op::Replace { regex, .. }
            | Op::Delete { regex, .. }
            | Op::InsertAfter { regex, .. }
            | Op::InsertBefore { regex, .. }
            | Op::ReplaceLine { regex, .. } => *regex,
        }
    }

    pub fn is_case_insensitive(&self) -> bool {
        match self {
            Op::Replace {
                case_insensitive, ..
            }
            | Op::Delete {
                case_insensitive, ..
            }
            | Op::InsertAfter {
                case_insensitive, ..
            }
            | Op::InsertBefore {
                case_insensitive, ..
            }
            | Op::ReplaceLine {
                case_insensitive, ..
            } => *case_insensitive,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // ── Op serde roundtrip ──

    #[test]
    fn replace_serializes_with_op_tag() {
        let op = Op::Replace {
            find: "foo".into(),
            replace: "bar".into(),
            regex: false,
            case_insensitive: false,
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "replace");
        assert_eq!(json["find"], "foo");
        assert_eq!(json["replace"], "bar");
    }

    #[test]
    fn delete_roundtrips_through_json() {
        let op = Op::Delete {
            find: "TODO".into(),
            regex: true,
            case_insensitive: false,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn insert_after_roundtrips_through_json() {
        let op = Op::InsertAfter {
            find: "use serde;".into(),
            content: "use serde_json;".into(),
            regex: false,
            case_insensitive: true,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn insert_before_roundtrips_through_json() {
        let op = Op::InsertBefore {
            find: "fn main".into(),
            content: "// entry".into(),
            regex: false,
            case_insensitive: false,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn replace_line_roundtrips_through_json() {
        let op = Op::ReplaceLine {
            find: "old".into(),
            content: "new".into(),
            regex: true,
            case_insensitive: true,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn deserialize_with_default_booleans() {
        let json = r#"{"op": "replace", "find": "a", "replace": "b"}"#;
        let op: Op = serde_json::from_str(json).unwrap();
        assert!(!op.is_regex());
        assert!(!op.is_case_insensitive());
    }

    #[test]
    fn unknown_op_tag_fails_deserialization() {
        let json = r#"{"op": "transform", "find": "a"}"#;
        let result = serde_json::from_str::<Op>(json);
        assert!(result.is_err());
    }

    // ── Accessor methods ──

    #[test]
    fn find_pattern_returns_find_for_all_variants() {
        let ops = [
            Op::Replace {
                find: "a".into(),
                replace: "b".into(),
                regex: false,
                case_insensitive: false,
            },
            Op::Delete {
                find: "c".into(),
                regex: false,
                case_insensitive: false,
            },
            Op::InsertAfter {
                find: "d".into(),
                content: "e".into(),
                regex: false,
                case_insensitive: false,
            },
            Op::InsertBefore {
                find: "f".into(),
                content: "g".into(),
                regex: false,
                case_insensitive: false,
            },
            Op::ReplaceLine {
                find: "h".into(),
                content: "i".into(),
                regex: false,
                case_insensitive: false,
            },
        ];
        let expected = ["a", "c", "d", "f", "h"];
        for (op, exp) in ops.iter().zip(expected.iter()) {
            assert_eq!(op.find_pattern(), *exp);
        }
    }

    #[test]
    fn is_regex_reflects_field() {
        let op = Op::Delete {
            find: "x".into(),
            regex: true,
            case_insensitive: false,
        };
        assert!(op.is_regex());
    }

    #[test]
    fn is_case_insensitive_reflects_field() {
        let op = Op::Replace {
            find: "x".into(),
            replace: "y".into(),
            regex: false,
            case_insensitive: true,
        };
        assert!(op.is_case_insensitive());
    }

    // ── LineRange ──

    #[test]
    fn line_range_contains_bounded() {
        let range = LineRange {
            start: 5,
            end: Some(10),
        };
        assert!(!range.contains(4));
        assert!(range.contains(5));
        assert!(range.contains(7));
        assert!(range.contains(10));
        assert!(!range.contains(11));
    }

    #[test]
    fn line_range_contains_unbounded_end() {
        let range = LineRange {
            start: 3,
            end: None,
        };
        assert!(!range.contains(2));
        assert!(range.contains(3));
        assert!(range.contains(1000));
    }

    #[test]
    fn line_range_single_line() {
        let range = LineRange {
            start: 7,
            end: Some(7),
        };
        assert!(!range.contains(6));
        assert!(range.contains(7));
        assert!(!range.contains(8));
    }

    #[test]
    fn line_range_roundtrips_through_json() {
        let range = LineRange {
            start: 1,
            end: Some(50),
        };
        let json = serde_json::to_string(&range).unwrap();
        let deserialized: LineRange = serde_json::from_str(&json).unwrap();
        assert_eq!(range, deserialized);
    }

    // ── OpOptions ──

    #[test]
    fn op_options_default_values() {
        let opts = OpOptions::default();
        assert!(opts.dry_run);
        assert!(opts.gitignore);
        assert!(!opts.backup);
        assert!(!opts.atomic);
        assert!(!opts.hidden);
        assert!(opts.root.is_none());
        assert!(opts.glob.is_none());
        assert!(opts.ignore.is_none());
        assert!(opts.max_depth.is_none());
        assert!(opts.line_range.is_none());
    }

    #[test]
    fn op_options_deserializes_with_defaults() {
        let json = "{}";
        let opts: OpOptions = serde_json::from_str(json).unwrap();
        assert!(opts.dry_run);
        assert!(opts.gitignore);
    }

    #[test]
    fn op_options_overrides_defaults() {
        let json = r#"{"dry_run": false, "gitignore": false, "backup": true}"#;
        let opts: OpOptions = serde_json::from_str(json).unwrap();
        assert!(!opts.dry_run);
        assert!(!opts.gitignore);
        assert!(opts.backup);
    }
}
