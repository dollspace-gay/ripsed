use std::io::{self, BufRead, Read};

/// The detected input mode based on stdin content.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    /// Valid ripsed JSON request detected.
    Json(String),
    /// Plain text (pipe mode).
    Pipe(Vec<u8>),
}

/// Peek at stdin to determine whether the input is a JSON request or plain text.
///
/// Reads the first chunk of stdin, checks if it starts with `{` and contains
/// an `"operations"` key, and returns the appropriate mode.
pub fn detect_stdin(stdin: &mut impl Read) -> io::Result<InputMode> {
    let mut buffer = Vec::new();
    stdin.read_to_end(&mut buffer)?;

    if buffer.is_empty() {
        return Ok(InputMode::Pipe(buffer));
    }

    // Find first non-whitespace byte
    let first_nonws = buffer.iter().position(|&b| !b.is_ascii_whitespace());

    match first_nonws {
        Some(pos) if buffer[pos] == b'{' => {
            // Try to parse as JSON
            if let Ok(text) = std::str::from_utf8(&buffer) {
                if is_ripsed_json(text) {
                    return Ok(InputMode::Json(text.to_string()));
                }
            }
            Ok(InputMode::Pipe(buffer))
        }
        _ => Ok(InputMode::Pipe(buffer)),
    }
}

/// Check if a JSON string looks like a ripsed request (has "operations" key).
fn is_ripsed_json(text: &str) -> bool {
    // Quick check: does it contain the "operations" key?
    // We parse it to validate it's actually valid JSON with that key.
    if let Ok(value) = serde_json::from_str::<serde_json::Value>(text) {
        value.get("operations").is_some()
    } else {
        false
    }
}

/// Detect input mode from a buffered reader (for streaming stdin).
pub fn detect_buffered(reader: &mut impl BufRead) -> io::Result<InputMode> {
    let buf = reader.fill_buf()?;
    if buf.is_empty() {
        return Ok(InputMode::Pipe(vec![]));
    }

    // Peek at first non-whitespace
    let first_nonws = buf.iter().position(|&b| !b.is_ascii_whitespace());

    if first_nonws.is_some_and(|pos| buf[pos] == b'{') {
        // Read everything and try to parse
        let mut full = Vec::new();
        reader.read_to_end(&mut full)?;
        if let Ok(text) = std::str::from_utf8(&full) {
            if is_ripsed_json(text) {
                return Ok(InputMode::Json(text.to_string()));
            }
        }
        Ok(InputMode::Pipe(full))
    } else {
        let mut full = Vec::new();
        reader.read_to_end(&mut full)?;
        Ok(InputMode::Pipe(full))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_json() {
        let input = r#"{"operations": [{"op": "replace", "find": "a", "replace": "b"}]}"#;
        let mut cursor = io::Cursor::new(input.as_bytes());
        let mode = detect_stdin(&mut cursor).unwrap();
        assert!(matches!(mode, InputMode::Json(_)));
    }

    #[test]
    fn test_detect_plain_text() {
        let input = "just some plain text\n";
        let mut cursor = io::Cursor::new(input.as_bytes());
        let mode = detect_stdin(&mut cursor).unwrap();
        assert!(matches!(mode, InputMode::Pipe(_)));
    }

    #[test]
    fn test_detect_json_without_operations() {
        let input = r#"{"key": "value"}"#;
        let mut cursor = io::Cursor::new(input.as_bytes());
        let mode = detect_stdin(&mut cursor).unwrap();
        assert!(matches!(mode, InputMode::Pipe(_)));
    }

    #[test]
    fn test_detect_empty() {
        let input = "";
        let mut cursor = io::Cursor::new(input.as_bytes());
        let mode = detect_stdin(&mut cursor).unwrap();
        assert!(matches!(mode, InputMode::Pipe(_)));
    }
}
