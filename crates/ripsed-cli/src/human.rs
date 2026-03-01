use ripsed_core::diff::Change;
use std::path::Path;

const BOLD: anstyle::Style = anstyle::Style::new().bold();
const RED: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red)));
const GREEN: anstyle::Style =
    anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green)));
const RESET: anstyle::Reset = anstyle::Reset;

/// Print a colored diff for a file's changes.
pub fn print_file_diff(path: &Path, changes: &[Change]) {
    anstream::println!("{BOLD}{}{RESET}", path.display());

    for change in changes {
        // Print context before
        if let Some(ref ctx) = change.context {
            for line in &ctx.before {
                anstream::println!("  {line}");
            }
        }

        // Print the change
        anstream::println!("{RED}- {}{RESET}", change.before);
        if let Some(ref after) = change.after {
            // For insert operations, after may contain newlines
            for line in after.lines() {
                anstream::println!("{GREEN}+ {line}{RESET}");
            }
        }

        // Print context after
        if let Some(ref ctx) = change.context {
            for line in &ctx.after {
                anstream::println!("  {line}");
            }
        }

        anstream::println!();
    }
}

/// Print a summary line.
pub fn print_summary(files_matched: usize, total_changes: usize, dry_run: bool) {
    if dry_run {
        anstream::eprintln!(
            "ripsed: dry run — {total_changes} change(s) in {files_matched} file(s) (not applied)"
        );
    } else {
        anstream::eprintln!("ripsed: {total_changes} change(s) applied in {files_matched} file(s)");
    }
}
