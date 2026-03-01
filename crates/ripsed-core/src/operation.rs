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
