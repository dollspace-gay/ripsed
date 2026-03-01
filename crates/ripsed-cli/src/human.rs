use ripsed_core::diff::Change;
use std::path::Path;

/// Print a colored diff for a file's changes.
pub fn print_file_diff(path: &Path, changes: &[Change]) {
    println!("\x1b[1m{}\x1b[0m", path.display());

    for change in changes {
        // Print context before
        if let Some(ref ctx) = change.context {
            for line in &ctx.before {
                println!("  {line}");
            }
        }

        // Print the change
        println!("\x1b[31m- {}\x1b[0m", change.before);
        if let Some(ref after) = change.after {
            // For insert operations, after may contain newlines
            for line in after.lines() {
                println!("\x1b[32m+ {line}\x1b[0m");
            }
        }

        // Print context after
        if let Some(ref ctx) = change.context {
            for line in &ctx.after {
                println!("  {line}");
            }
        }

        println!();
    }
}

/// Print a summary line.
pub fn print_summary(files_matched: usize, total_changes: usize, dry_run: bool) {
    if dry_run {
        eprintln!(
            "ripsed: dry run — {total_changes} change(s) in {files_matched} file(s) (not applied)"
        );
    } else {
        eprintln!("ripsed: {total_changes} change(s) applied in {files_matched} file(s)");
    }
}
