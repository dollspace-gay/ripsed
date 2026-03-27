use serde::{Deserialize, Serialize};

/// Text transformation modes for the Transform operation.
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
#[non_exhaustive]
pub enum TransformMode {
    Upper,
    Lower,
    Title,
    SnakeCase,
    CamelCase,
}

impl std::fmt::Display for TransformMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            TransformMode::Upper => write!(f, "upper"),
            TransformMode::Lower => write!(f, "lower"),
            TransformMode::Title => write!(f, "title"),
            TransformMode::SnakeCase => write!(f, "snake_case"),
            TransformMode::CamelCase => write!(f, "camel_case"),
        }
    }
}

impl std::str::FromStr for TransformMode {
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "upper" => Ok(TransformMode::Upper),
            "lower" => Ok(TransformMode::Lower),
            "title" => Ok(TransformMode::Title),
            "snake_case" | "snake" => Ok(TransformMode::SnakeCase),
            "camel_case" | "camel" => Ok(TransformMode::CamelCase),
            _ => Err(format!(
                "unknown transform mode '{s}'. Valid modes: upper, lower, title, snake_case, camel_case"
            )),
        }
    }
}

/// The intermediate representation for all ripsed operations.
/// Both CLI args and JSON requests are normalized into this form.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "op", rename_all = "snake_case")]
#[non_exhaustive]
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
    Transform {
        find: String,
        mode: TransformMode,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    Surround {
        find: String,
        prefix: String,
        suffix: String,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    Indent {
        find: String,
        #[serde(default = "default_indent_amount")]
        amount: usize,
        #[serde(default)]
        use_tabs: bool,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
    Dedent {
        find: String,
        #[serde(default = "default_indent_amount")]
        amount: usize,
        #[serde(default)]
        use_tabs: bool,
        #[serde(default)]
        regex: bool,
        #[serde(default)]
        case_insensitive: bool,
    },
}

fn default_indent_amount() -> usize {
    4
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

use crate::default_true;

impl Op {
    /// Extract the find pattern from the operation.
    pub fn find_pattern(&self) -> &str {
        match self {
            Op::Replace { find, .. }
            | Op::Delete { find, .. }
            | Op::InsertAfter { find, .. }
            | Op::InsertBefore { find, .. }
            | Op::ReplaceLine { find, .. }
            | Op::Transform { find, .. }
            | Op::Surround { find, .. }
            | Op::Indent { find, .. }
            | Op::Dedent { find, .. } => find,
        }
    }

    pub fn is_regex(&self) -> bool {
        match self {
            Op::Replace { regex, .. }
            | Op::Delete { regex, .. }
            | Op::InsertAfter { regex, .. }
            | Op::InsertBefore { regex, .. }
            | Op::ReplaceLine { regex, .. }
            | Op::Transform { regex, .. }
            | Op::Surround { regex, .. }
            | Op::Indent { regex, .. }
            | Op::Dedent { regex, .. } => *regex,
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
            }
            | Op::Transform {
                case_insensitive, ..
            }
            | Op::Surround {
                case_insensitive, ..
            }
            | Op::Indent {
                case_insensitive, ..
            }
            | Op::Dedent {
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

    // ── New Op serde roundtrip tests ──

    #[test]
    fn transform_roundtrips_through_json() {
        let op = Op::Transform {
            find: "myVar".into(),
            mode: TransformMode::SnakeCase,
            regex: false,
            case_insensitive: true,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn transform_serializes_with_op_tag() {
        let op = Op::Transform {
            find: "hello".into(),
            mode: TransformMode::Upper,
            regex: true,
            case_insensitive: false,
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "transform");
        assert_eq!(json["find"], "hello");
        assert_eq!(json["mode"], "upper");
        assert_eq!(json["regex"], true);
    }

    #[test]
    fn transform_all_modes_roundtrip() {
        let modes = [
            TransformMode::Upper,
            TransformMode::Lower,
            TransformMode::Title,
            TransformMode::SnakeCase,
            TransformMode::CamelCase,
        ];
        for mode in modes {
            let op = Op::Transform {
                find: "test".into(),
                mode,
                regex: false,
                case_insensitive: false,
            };
            let json = serde_json::to_string(&op).unwrap();
            let deserialized: Op = serde_json::from_str(&json).unwrap();
            assert_eq!(op, deserialized, "Failed roundtrip for mode {:?}", mode);
        }
    }

    #[test]
    fn transform_deserialize_with_defaults() {
        let json = r#"{"op": "transform", "find": "x", "mode": "upper"}"#;
        let op: Op = serde_json::from_str(json).unwrap();
        assert!(!op.is_regex());
        assert!(!op.is_case_insensitive());
    }

    #[test]
    fn transform_missing_mode_fails() {
        let json = r#"{"op": "transform", "find": "a"}"#;
        let result = serde_json::from_str::<Op>(json);
        assert!(result.is_err());
    }

    #[test]
    fn surround_roundtrips_through_json() {
        let op = Op::Surround {
            find: "TODO".into(),
            prefix: "<<".into(),
            suffix: ">>".into(),
            regex: true,
            case_insensitive: false,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn surround_serializes_with_op_tag() {
        let op = Op::Surround {
            find: "word".into(),
            prefix: "[".into(),
            suffix: "]".into(),
            regex: false,
            case_insensitive: false,
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "surround");
        assert_eq!(json["find"], "word");
        assert_eq!(json["prefix"], "[");
        assert_eq!(json["suffix"], "]");
    }

    #[test]
    fn surround_deserialize_with_defaults() {
        let json = r#"{"op": "surround", "find": "x", "prefix": "<", "suffix": ">"}"#;
        let op: Op = serde_json::from_str(json).unwrap();
        assert!(!op.is_regex());
        assert!(!op.is_case_insensitive());
    }

    #[test]
    fn indent_roundtrips_through_json() {
        let op = Op::Indent {
            find: "fn ".into(),
            amount: 8,
            use_tabs: true,
            regex: false,
            case_insensitive: false,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn indent_serializes_with_op_tag() {
        let op = Op::Indent {
            find: "line".into(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "indent");
        assert_eq!(json["find"], "line");
        assert_eq!(json["amount"], 4);
    }

    #[test]
    fn indent_deserialize_with_defaults() {
        let json = r#"{"op": "indent", "find": "x"}"#;
        let op: Op = serde_json::from_str(json).unwrap();
        assert!(!op.is_regex());
        assert!(!op.is_case_insensitive());
        // amount should default to 4
        match op {
            Op::Indent {
                amount, use_tabs, ..
            } => {
                assert_eq!(amount, 4);
                assert!(!use_tabs);
            }
            _ => panic!("Expected Indent variant"),
        }
    }

    #[test]
    fn dedent_roundtrips_through_json() {
        let op = Op::Dedent {
            find: "code".into(),
            amount: 2,
            use_tabs: false,
            regex: true,
            case_insensitive: true,
        };
        let json = serde_json::to_string(&op).unwrap();
        let deserialized: Op = serde_json::from_str(&json).unwrap();
        assert_eq!(op, deserialized);
    }

    #[test]
    fn dedent_serializes_with_op_tag() {
        let op = Op::Dedent {
            find: "line".into(),
            amount: 4,
            use_tabs: false,
            regex: false,
            case_insensitive: false,
        };
        let json = serde_json::to_value(&op).unwrap();
        assert_eq!(json["op"], "dedent");
        assert_eq!(json["find"], "line");
        assert_eq!(json["amount"], 4);
    }

    #[test]
    fn dedent_deserialize_with_defaults() {
        let json = r#"{"op": "dedent", "find": "x"}"#;
        let op: Op = serde_json::from_str(json).unwrap();
        assert!(!op.is_regex());
        assert!(!op.is_case_insensitive());
        // amount should default to 4
        match op {
            Op::Dedent { amount, .. } => {
                assert_eq!(amount, 4);
            }
            _ => panic!("Expected Dedent variant"),
        }
    }

    // ── Accessor methods for new variants ──

    #[test]
    fn find_pattern_returns_find_for_new_variants() {
        let ops = [
            Op::Transform {
                find: "t".into(),
                mode: TransformMode::Upper,
                regex: false,
                case_insensitive: false,
            },
            Op::Surround {
                find: "s".into(),
                prefix: "<".into(),
                suffix: ">".into(),
                regex: false,
                case_insensitive: false,
            },
            Op::Indent {
                find: "i".into(),
                amount: 4,
                use_tabs: false,
                regex: false,
                case_insensitive: false,
            },
            Op::Dedent {
                find: "d".into(),
                amount: 4,
                use_tabs: false,
                regex: false,
                case_insensitive: false,
            },
        ];
        let expected = ["t", "s", "i", "d"];
        for (op, exp) in ops.iter().zip(expected.iter()) {
            assert_eq!(op.find_pattern(), *exp);
        }
    }

    #[test]
    fn is_regex_reflects_field_for_new_variants() {
        let ops = [
            Op::Transform {
                find: "x".into(),
                mode: TransformMode::Upper,
                regex: true,
                case_insensitive: false,
            },
            Op::Surround {
                find: "x".into(),
                prefix: "<".into(),
                suffix: ">".into(),
                regex: true,
                case_insensitive: false,
            },
            Op::Indent {
                find: "x".into(),
                amount: 4,
                use_tabs: false,
                regex: true,
                case_insensitive: false,
            },
            Op::Dedent {
                find: "x".into(),
                amount: 4,
                use_tabs: false,
                regex: true,
                case_insensitive: false,
            },
        ];
        for op in &ops {
            assert!(op.is_regex());
        }
    }

    #[test]
    fn is_case_insensitive_reflects_field_for_new_variants() {
        let ops = [
            Op::Transform {
                find: "x".into(),
                mode: TransformMode::Upper,
                regex: false,
                case_insensitive: true,
            },
            Op::Surround {
                find: "x".into(),
                prefix: "<".into(),
                suffix: ">".into(),
                regex: false,
                case_insensitive: true,
            },
            Op::Indent {
                find: "x".into(),
                amount: 4,
                use_tabs: false,
                regex: false,
                case_insensitive: true,
            },
            Op::Dedent {
                find: "x".into(),
                amount: 4,
                use_tabs: false,
                regex: false,
                case_insensitive: true,
            },
        ];
        for op in &ops {
            assert!(op.is_case_insensitive());
        }
    }

    // ── TransformMode Display and FromStr ──

    #[test]
    fn transform_mode_display_roundtrip() {
        let modes = [
            TransformMode::Upper,
            TransformMode::Lower,
            TransformMode::Title,
            TransformMode::SnakeCase,
            TransformMode::CamelCase,
        ];
        for mode in modes {
            let s = mode.to_string();
            let parsed: TransformMode = s.parse().unwrap();
            assert_eq!(mode, parsed);
        }
    }

    #[test]
    fn transform_mode_from_str_aliases() {
        assert_eq!(
            "snake".parse::<TransformMode>().unwrap(),
            TransformMode::SnakeCase
        );
        assert_eq!(
            "camel".parse::<TransformMode>().unwrap(),
            TransformMode::CamelCase
        );
    }

    #[test]
    fn transform_mode_from_str_unknown_fails() {
        assert!("unknown".parse::<TransformMode>().is_err());
    }
}
