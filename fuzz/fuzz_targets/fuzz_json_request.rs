#![no_main]

use libfuzzer_sys::fuzz_target;
use ripsed_json::request::JsonRequest;

fuzz_target!(|data: &[u8]| {
    // Convert random bytes to a string; skip non-UTF-8 inputs.
    let input = match std::str::from_utf8(data) {
        Ok(s) => s,
        Err(_) => return,
    };

    // This must never panic. It either succeeds with a valid JsonRequest
    // or returns a RipsedError for any malformed input.
    let _ = JsonRequest::parse(input);
});
