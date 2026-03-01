#![no_main]

use libfuzzer_sys::fuzz_target;
use ripsed_json::detect::{detect_stdin, InputMode};
use std::io::Cursor;

fuzz_target!(|data: &[u8]| {
    // Wrap the random bytes in a Cursor to simulate stdin.
    let mut cursor = Cursor::new(data);

    // This must never panic and must never hang.
    let result = detect_stdin(&mut cursor);

    // detect_stdin returns io::Result<InputMode>.
    // It should always succeed (Cursor I/O never fails).
    match result {
        Ok(InputMode::Json(_)) => {
            // Valid: dispatched to JSON mode.
        }
        Ok(InputMode::Pipe(_)) => {
            // Valid: dispatched to pipe mode.
        }
        Err(_) => {
            // An I/O error from a Cursor is unexpected but not a panic.
        }
    }
});
