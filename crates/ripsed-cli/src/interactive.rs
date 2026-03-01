use ripsed_core::diff::Change;
use std::io::{self, Write};
use std::path::Path;

/// Prompt the user to confirm a change.
/// Returns true if the user confirms.
pub fn confirm_change(path: &Path, change: &Change) -> bool {
    println!("\x1b[1m{}\x1b[0m line {}:", path.display(), change.line);
    println!("\x1b[31m- {}\x1b[0m", change.before);
    if let Some(ref after) = change.after {
        println!("\x1b[32m+ {after}\x1b[0m");
    }
    print!("Apply this change? [y/N] ");
    io::stdout().flush().ok();

    let mut response = String::new();
    if io::stdin().read_line(&mut response).is_ok() {
        let trimmed = response.trim().to_lowercase();
        trimmed == "y" || trimmed == "yes"
    } else {
        false
    }
}
