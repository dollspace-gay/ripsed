use ripsed_core::config::Config;
use ripsed_core::engine;
use ripsed_core::matcher::Matcher;
use ripsed_core::operation::{Op, OpOptions};
use ripsed_core::undo::UndoRecord;
use ripsed_fs::discovery::{DiscoveryOptions, discover_files_auto};
use ripsed_fs::reader;
use ripsed_fs::writer;
use std::process;

use crate::args::Cli;
use crate::human;
use crate::interactive::{self, ConfirmAction};
use crate::shared::{load_undo_log, save_undo_log};

/// Handle `--undo N`: restore the last N files from the undo log.
pub fn handle_undo(count: usize, config: &Config) {
    let mut log = load_undo_log(config);
    if log.is_empty() {
        eprintln!("ripsed: nothing to undo");
        process::exit(1);
    }

    let records = log.pop(count);
    if records.is_empty() {
        eprintln!("ripsed: nothing to undo");
        process::exit(1);
    }

    for record in &records {
        let path = std::path::Path::new(&record.file_path);
        match writer::write_atomic(path, &record.entry.original_text) {
            Ok(()) => {
                eprintln!("ripsed: restored {}", record.file_path);
            }
            Err(e) => {
                eprintln!("ripsed: failed to restore {}: {e}", record.file_path);
            }
        }
    }

    save_undo_log(&log);
}

/// Handle `--undo-list`: display recent undo log entries.
pub fn handle_undo_list(config: &Config) {
    let log = load_undo_log(config);
    if log.is_empty() {
        eprintln!("ripsed: undo log is empty");
        return;
    }

    let recent = log.recent(20);
    for (i, record) in recent.iter().enumerate() {
        println!("  {} {} ({})", i + 1, record.file_path, record.timestamp);
    }
}

pub fn run_file_mode(cli: &Cli, config: &Config) {
    let Some(ref find) = cli.find else {
        eprintln!("ripsed: missing FIND pattern");
        process::exit(1);
    };

    let op = build_op_from_cli(cli, find);
    let matcher = match Matcher::new(&op) {
        Ok(m) => m,
        Err(e) => {
            eprintln!("ripsed: {e}");
            process::exit(1);
        }
    };

    let options = OpOptions {
        dry_run: cli.dry_run,
        root: None,
        gitignore: if cli.no_gitignore {
            false
        } else {
            config.defaults.gitignore
        },
        backup: cli.backup || config.defaults.backup,
        atomic: false,
        glob: cli.glob.clone(),
        ignore: cli.ignore_pattern.clone(),
        hidden: cli.hidden,
        max_depth: cli.max_depth.or(config.defaults.max_depth),
        line_range: cli.line_range,
    };

    let mut discovery_opts = DiscoveryOptions::from_op_options(&options);
    discovery_opts.follow_links = cli.follow;
    let files = discover_files_auto(&discovery_opts, false);

    if files.is_empty() {
        eprintln!("ripsed: no files found");
        process::exit(1);
    }

    let mut total_changes = 0usize;
    let mut files_modified = 0usize;

    // Load undo log for recording changes (only when not dry-run)
    let mut undo_log = if !cli.dry_run {
        Some(load_undo_log(config))
    } else {
        None
    };

    let mut apply_all = false;

    for file_path in &files {
        let content = match reader::read_file(file_path) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("ripsed: {}: {e}", file_path.display());
                continue;
            }
        };

        let output = match engine::apply(&content, &op, &matcher, options.line_range, 3) {
            Ok(o) => o,
            Err(e) => {
                eprintln!("ripsed: {}: {e}", file_path.display());
                continue;
            }
        };

        if output.changes.is_empty() {
            continue;
        }

        // Handle --confirm: prompt for each change in this file
        if cli.confirm && !apply_all {
            let mut skip_file = false;
            for change in &output.changes {
                let action = interactive::confirm_change(file_path, change);
                match action {
                    ConfirmAction::Yes => {}
                    ConfirmAction::No => continue,
                    ConfirmAction::ApplyAll => {
                        apply_all = true;
                        break;
                    }
                    ConfirmAction::SkipFile => {
                        skip_file = true;
                        break;
                    }
                    ConfirmAction::Quit => {
                        // Save undo log before quitting
                        if let Some(ref log) = undo_log {
                            save_undo_log(log);
                        }
                        process::exit(0);
                    }
                }
            }
            if skip_file {
                continue;
            }
        }

        total_changes += output.changes.len();

        if cli.count {
            // Just count, don't print diffs
        } else if !cli.quiet {
            human::print_file_diff(file_path, &output.changes);
        }

        if !cli.dry_run {
            if options.backup {
                if let Err(e) = writer::create_backup(file_path) {
                    eprintln!("ripsed: backup failed for {}: {e}", file_path.display());
                    continue;
                }
            }
            if let Some(ref text) = output.text {
                // Record undo entry before writing
                if let Some(ref mut log) = undo_log {
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .map(|d| {
                            // ISO 8601-ish timestamp
                            let secs = d.as_secs();
                            format!("{secs}")
                        })
                        .unwrap_or_else(|_| "0".to_string());

                    if let Some(ref undo_entry) = output.undo {
                        log.push(UndoRecord {
                            timestamp: now,
                            file_path: file_path.to_string_lossy().to_string(),
                            entry: undo_entry.clone(),
                        });
                    }
                }

                match writer::write_atomic(file_path, text) {
                    Ok(()) => files_modified += 1,
                    Err(e) => {
                        eprintln!("ripsed: write failed for {}: {e}", file_path.display());
                    }
                }
            }
        }
    }

    // Save undo log after all changes
    if let Some(ref log) = undo_log {
        save_undo_log(log);
    }

    if cli.count {
        println!("{total_changes}");
    } else if !cli.quiet {
        human::print_summary(files_modified, total_changes, cli.dry_run);
    }

    if total_changes == 0 {
        process::exit(1);
    }
}

pub fn build_op_from_cli(cli: &Cli, find: &str) -> Op {
    if cli.delete {
        Op::Delete {
            find: find.to_string(),
            regex: cli.regex,
            case_insensitive: cli.case_insensitive,
        }
    } else if let Some(ref content) = cli.after {
        Op::InsertAfter {
            find: find.to_string(),
            content: content.clone(),
            regex: cli.regex,
            case_insensitive: cli.case_insensitive,
        }
    } else if let Some(ref content) = cli.before {
        Op::InsertBefore {
            find: find.to_string(),
            content: content.clone(),
            regex: cli.regex,
            case_insensitive: cli.case_insensitive,
        }
    } else if let Some(ref content) = cli.replace_line {
        Op::ReplaceLine {
            find: find.to_string(),
            content: content.clone(),
            regex: cli.regex,
            case_insensitive: cli.case_insensitive,
        }
    } else {
        Op::Replace {
            find: find.to_string(),
            replace: cli.replace.clone().unwrap_or_default(),
            regex: cli.regex,
            case_insensitive: cli.case_insensitive,
        }
    }
}
