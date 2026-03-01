use ripsed_core::diff::Change;
use std::io::{self, Write};
use std::path::Path;

/// Actions the user can take when confirming a change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmAction {
    /// Apply this change.
    Yes,
    /// Skip this change.
    No,
    /// Apply all remaining changes without further prompts.
    ApplyAll,
    /// Skip all remaining changes in the current file.
    SkipFile,
    /// Abort the entire operation immediately.
    Quit,
}

/// Prompt the user to confirm a change.
/// Returns a `ConfirmAction` indicating what to do.
///
/// Accepted inputs:
///   y / yes  -> Yes
///   n / no   -> No  (default on empty input)
///   a / all  -> ApplyAll
///   s / skip -> SkipFile
///   q / quit -> Quit
pub fn confirm_change(path: &Path, change: &Change) -> ConfirmAction {
    let bold = anstyle::Style::new().bold();
    let red = anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Red)));
    let green =
        anstyle::Style::new().fg_color(Some(anstyle::Color::Ansi(anstyle::AnsiColor::Green)));
    let reset = anstyle::Reset;

    eprintln!(
        "{bold}{}{reset} line {}:",
        path.display(),
        change.line
    );
    eprintln!("{red}- {}{reset}", change.before);
    if let Some(ref after) = change.after {
        eprintln!("{green}+ {after}{reset}");
    }
    eprint!("Apply this change? [y/n/a/s/q] ");
    io::stderr().flush().ok();

    let mut response = String::new();
    if io::stdin().read_line(&mut response).is_ok() {
        match response.trim().to_lowercase().as_str() {
            "y" | "yes" => ConfirmAction::Yes,
            "a" | "all" => ConfirmAction::ApplyAll,
            "s" | "skip" => ConfirmAction::SkipFile,
            "q" | "quit" => ConfirmAction::Quit,
            _ => ConfirmAction::No,
        }
    } else {
        ConfirmAction::Quit
    }
}
