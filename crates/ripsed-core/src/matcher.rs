use crate::error::RipsedError;
use crate::operation::Op;
use regex::Regex;

/// Abstraction over literal and regex matching.
#[derive(Debug)]
pub enum Matcher {
    Literal {
        pattern: String,
        case_insensitive: bool,
    },
    /// A regex matcher — used for both explicit `--regex` patterns and as the
    /// implementation backing case-insensitive literal matching (via
    /// `regex::escape` + `(?i)`), which avoids byte-offset mismatches from
    /// `str::to_lowercase()` on multi-byte Unicode characters.
    Regex(Regex),
}

impl Matcher {
    /// Create a new matcher from an operation.
    pub fn new(op: &Op) -> Result<Self, RipsedError> {
        let pattern = op.find_pattern();
        let is_regex = op.is_regex();
        let case_insensitive = op.is_case_insensitive();

        if is_regex || case_insensitive {
            // For case-insensitive literals, escape the pattern and delegate to
            // the regex engine which handles Unicode casing correctly.
            let re_src = if is_regex {
                pattern.to_string()
            } else {
                regex::escape(pattern)
            };
            let re_pattern = if case_insensitive {
                format!("(?i){re_src}")
            } else {
                re_src
            };
            Regex::new(&re_pattern)
                .map(Matcher::Regex)
                .map_err(|e| RipsedError::invalid_regex(0, pattern, &e.to_string()))
        } else {
            Ok(Matcher::Literal {
                pattern: pattern.to_string(),
                case_insensitive: false,
            })
        }
    }

    /// Check if the given text matches.
    pub fn is_match(&self, text: &str) -> bool {
        match self {
            Matcher::Literal { pattern, .. } => text.contains(pattern.as_str()),
            Matcher::Regex(re) => re.is_match(text),
        }
    }

    /// Replace all matches in the given text. Returns None if no matches.
    pub fn replace(&self, text: &str, replacement: &str) -> Option<String> {
        match self {
            Matcher::Literal { pattern, .. } => {
                if text.contains(pattern.as_str()) {
                    Some(text.replace(pattern.as_str(), replacement))
                } else {
                    None
                }
            }
            Matcher::Regex(re) => {
                if re.is_match(text) {
                    Some(re.replace_all(text, replacement).into_owned())
                } else {
                    None
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_match() {
        let op = Op::Replace {
            find: "hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.is_match("say hello world"));
        assert!(!m.is_match("say Hi world"));
    }

    #[test]
    fn test_literal_case_insensitive() {
        let op = Op::Replace {
            find: "hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: true,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.is_match("say HELLO world"));
        assert!(m.is_match("say Hello world"));
    }

    #[test]
    fn test_regex_match() {
        let op = Op::Replace {
            find: r"fn\s+(\w+)".to_string(),
            replace: "fn new_$1".to_string(),
            regex: true,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.is_match("fn old_func() {"));
        assert!(!m.is_match("let x = 5;"));
    }

    #[test]
    fn test_regex_replace_with_captures() {
        let op = Op::Replace {
            find: r"fn\s+old_(\w+)".to_string(),
            replace: "fn new_$1".to_string(),
            regex: true,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        let result = m.replace("fn old_function() {", "fn new_$1");
        assert_eq!(result, Some("fn new_function() {".to_string()));
    }

    #[test]
    fn test_invalid_regex() {
        let op = Op::Replace {
            find: "fn (foo".to_string(),
            replace: "bar".to_string(),
            regex: true,
            case_insensitive: false,
        };
        let err = Matcher::new(&op).unwrap_err();
        assert_eq!(err.code, crate::error::ErrorCode::InvalidRegex);
    }

    // ---------------------------------------------------------------
    // Empty pattern behavior
    // ---------------------------------------------------------------

    #[test]
    fn test_empty_pattern_literal_matches_everything() {
        let op = Op::Replace {
            find: "".to_string(),
            replace: "x".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        // An empty string is contained in every string
        assert!(m.is_match("anything"));
        assert!(m.is_match(""));
    }

    #[test]
    fn test_empty_pattern_literal_replace() {
        let op = Op::Replace {
            find: "".to_string(),
            replace: "x".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        // Rust's str::replace("", "x") inserts "x" between every char and at start/end
        let result = m.replace("ab", "x");
        assert_eq!(result, Some("xaxbx".to_string()));
    }

    #[test]
    fn test_empty_pattern_regex_matches_everything() {
        let op = Op::Replace {
            find: "".to_string(),
            replace: "x".to_string(),
            regex: true,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.is_match("anything"));
        assert!(m.is_match(""));
    }

    // ---------------------------------------------------------------
    // Pattern that matches entire line
    // ---------------------------------------------------------------

    #[test]
    fn test_pattern_matches_entire_line_literal() {
        let op = Op::Replace {
            find: "hello world".to_string(),
            replace: "goodbye".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        let result = m.replace("hello world", "goodbye");
        assert_eq!(result, Some("goodbye".to_string()));
    }

    #[test]
    fn test_pattern_matches_entire_line_regex() {
        let op = Op::Replace {
            find: r"^.*$".to_string(),
            replace: "replaced".to_string(),
            regex: true,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        let result = m.replace("anything here", "replaced");
        assert_eq!(result, Some("replaced".to_string()));
    }

    #[test]
    fn test_regex_anchored_full_line() {
        let op = Op::Replace {
            find: r"^fn main\(\)$".to_string(),
            replace: "fn start()".to_string(),
            regex: true,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.is_match("fn main()"));
        assert!(!m.is_match("  fn main()")); // leading whitespace
        assert!(!m.is_match("fn main() {")); // trailing content
    }

    // ---------------------------------------------------------------
    // Case-insensitive with unicode (Turkish I problem, etc.)
    // ---------------------------------------------------------------

    #[test]
    fn test_case_insensitive_ascii() {
        let op = Op::Replace {
            find: "Hello".to_string(),
            replace: "hi".to_string(),
            regex: false,
            case_insensitive: true,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.is_match("HELLO"));
        assert!(m.is_match("hello"));
        assert!(m.is_match("HeLLo"));
        let result = m.replace("say HELLO there", "hi");
        assert_eq!(result, Some("say hi there".to_string()));
    }

    #[test]
    fn test_case_insensitive_german_eszett() {
        // German sharp-s: lowercase to_lowercase() of "SS" is "ss",
        // and to_lowercase() of "\u{00DF}" (sharp-s) is "\u{00DF}"
        // This tests that the engine handles non-trivial unicode casing
        let op = Op::Replace {
            find: "stra\u{00DF}e".to_string(), // "strasse" with sharp-s
            replace: "street".to_string(),
            regex: false,
            case_insensitive: true,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.is_match("STRA\u{00DF}E"));
    }

    #[test]
    fn test_case_insensitive_turkish_i_lowercase() {
        // Turkish dotted I: \u{0130} (capital I with dot above)
        // This is a known edge case. We test that the matcher doesn't panic
        // and behaves consistently with Unicode simple case folding.
        let op = Op::Replace {
            find: "i".to_string(),
            replace: "x".to_string(),
            regex: false,
            case_insensitive: true,
        };
        let m = Matcher::new(&op).unwrap();
        // Standard ASCII: "I" simple-folds to "i", so this matches
        assert!(m.is_match("I"));
        // \u{0130} (İ) has no simple case fold to "i" in Unicode — the full
        // fold is "i\u{0307}" but the regex engine only uses simple folds.
        // This correctly does NOT match, avoiding false positives from the
        // old to_lowercase()-based byte-offset approach.
        assert!(!m.is_match("\u{0130}"));
    }

    // ---------------------------------------------------------------
    // Regex special characters in literal mode
    // ---------------------------------------------------------------

    #[test]
    fn test_literal_mode_regex_metacharacters() {
        // All these are regex metacharacters but should be treated literally
        let patterns = vec![
            (".", "dot"),
            ("*", "star"),
            ("+", "plus"),
            ("?", "question"),
            ("(", "paren"),
            ("[", "bracket"),
            ("{", "brace"),
            ("^", "caret"),
            ("$", "dollar"),
            ("|", "pipe"),
            ("\\", "backslash"),
        ];
        for (pat, name) in patterns {
            let op = Op::Replace {
                find: pat.to_string(),
                replace: "X".to_string(),
                regex: false,
                case_insensitive: false,
            };
            let m = Matcher::new(&op).unwrap();
            let text = format!("before {pat} after");
            assert!(
                m.is_match(&text),
                "Literal mode should match '{name}' ({pat}) as a literal character"
            );
            let result = m.replace(&text, "X");
            assert_eq!(
                result,
                Some("before X after".to_string()),
                "Literal mode should replace '{name}' ({pat}) as a literal"
            );
        }
    }

    // ---------------------------------------------------------------
    // Multiple matches on same line
    // ---------------------------------------------------------------

    #[test]
    fn test_multiple_matches_same_line() {
        let op = Op::Replace {
            find: "ab".to_string(),
            replace: "X".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        let result = m.replace("ab cd ab ef ab", "X");
        assert_eq!(result, Some("X cd X ef X".to_string()));
    }

    #[test]
    fn test_replace_with_empty_string() {
        let op = Op::Replace {
            find: "remove".to_string(),
            replace: "".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        let result = m.replace("please remove this", "");
        assert_eq!(result, Some("please  this".to_string()));
    }

    #[test]
    fn test_no_match_returns_none() {
        let op = Op::Replace {
            find: "xyz".to_string(),
            replace: "abc".to_string(),
            regex: false,
            case_insensitive: false,
        };
        let m = Matcher::new(&op).unwrap();
        assert!(m.replace("nothing here", "abc").is_none());
    }
}
