use strsim::levenshtein;

/// Maximum edit distance for a suggestion to be considered relevant.
const MAX_DISTANCE: usize = 2;

/// Find near-miss suggestions for a pattern among candidates.
///
/// Returns candidates sorted by edit distance (closest first),
/// filtered to those within `MAX_DISTANCE`.
pub fn find_suggestions(pattern: &str, candidates: &[&str]) -> Vec<String> {
    let mut scored: Vec<(usize, &str)> = candidates
        .iter()
        .filter_map(|&candidate| {
            let distance = levenshtein(pattern, candidate);
            if distance > 0 && distance <= MAX_DISTANCE {
                Some((distance, candidate))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by_key(|(d, _)| *d);
    scored.into_iter().map(|(_, s)| s.to_string()).collect()
}

/// Find near-miss path suggestions.
pub fn find_path_suggestions(path: &str, existing_paths: &[&str]) -> Vec<String> {
    // For paths, compare just the filename component
    let filename = std::path::Path::new(path)
        .file_name()
        .and_then(|f| f.to_str())
        .unwrap_or(path);

    let mut scored: Vec<(usize, &str)> = existing_paths
        .iter()
        .filter_map(|&existing| {
            let existing_filename = std::path::Path::new(existing)
                .file_name()
                .and_then(|f| f.to_str())
                .unwrap_or(existing);
            let distance = levenshtein(filename, existing_filename);
            if distance > 0 && distance <= MAX_DISTANCE {
                Some((distance, existing))
            } else {
                None
            }
        })
        .collect();

    scored.sort_by_key(|(d, _)| *d);
    scored.into_iter().map(|(_, s)| s.to_string()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_finds_close_match() {
        let suggestions =
            find_suggestions("old_fnuction", &["old_function", "new_function", "xyz"]);
        assert_eq!(suggestions, vec!["old_function"]);
    }

    #[test]
    fn test_no_suggestions_for_distant() {
        let suggestions = find_suggestions("abc", &["xyz", "completely_different"]);
        assert!(suggestions.is_empty());
    }

    #[test]
    fn test_exact_match_excluded() {
        let suggestions = find_suggestions("hello", &["hello", "helo"]);
        assert_eq!(suggestions, vec!["helo"]);
    }

    #[test]
    fn test_path_suggestions() {
        let suggestions = find_path_suggestions("src/lbi.rs", &["src/lib.rs", "src/main.rs"]);
        assert_eq!(suggestions, vec!["src/lib.rs"]);
    }
}
