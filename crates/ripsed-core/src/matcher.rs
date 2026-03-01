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
    Regex(Regex),
}

impl Matcher {
    /// Create a new matcher from an operation.
    pub fn new(op: &Op) -> Result<Self, RipsedError> {
        let pattern = op.find_pattern();
        let is_regex = op.is_regex();
        let case_insensitive = op.is_case_insensitive();

        if is_regex {
            let re_pattern = if case_insensitive {
                format!("(?i){pattern}")
            } else {
                pattern.to_string()
            };
            Regex::new(&re_pattern).map(Matcher::Regex).map_err(|e| {
                RipsedError::invalid_regex(0, pattern, &e.to_string())
            })
        } else {
            Ok(Matcher::Literal {
                pattern: pattern.to_string(),
                case_insensitive,
            })
        }
    }

    /// Check if the given text matches.
    pub fn is_match(&self, text: &str) -> bool {
        match self {
            Matcher::Literal {
                pattern,
                case_insensitive,
            } => {
                if *case_insensitive {
                    text.to_lowercase().contains(&pattern.to_lowercase())
                } else {
                    text.contains(pattern.as_str())
                }
            }
            Matcher::Regex(re) => re.is_match(text),
        }
    }

    /// Replace all matches in the given text. Returns None if no matches.
    pub fn replace(&self, text: &str, replacement: &str) -> Option<String> {
        match self {
            Matcher::Literal {
                pattern,
                case_insensitive,
            } => {
                if *case_insensitive {
                    // Case-insensitive literal replace
                    let lower_text = text.to_lowercase();
                    let lower_pattern = pattern.to_lowercase();
                    if !lower_text.contains(&lower_pattern) {
                        return None;
                    }
                    let mut result = String::with_capacity(text.len());
                    let mut search_start = 0;
                    while let Some(pos) = lower_text[search_start..].find(&lower_pattern) {
                        let abs_pos = search_start + pos;
                        result.push_str(&text[search_start..abs_pos]);
                        result.push_str(replacement);
                        search_start = abs_pos + pattern.len();
                    }
                    result.push_str(&text[search_start..]);
                    Some(result)
                } else if text.contains(pattern.as_str()) {
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
}
