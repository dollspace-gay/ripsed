use ripsed_core::config::Config;
use ripsed_core::engine;
use ripsed_core::matcher::Matcher;
use ripsed_core::operation::OpOptions;
use ripsed_core::script::{Script, parse_script};
use ripsed_core::undo::UndoRecord;
use ripsed_fs::discovery::{DiscoveryOptions, discover_files_auto};
use ripsed_fs::reader;
use ripsed_fs::writer;
use std::collections::HashSet;
use std::path::PathBuf;
use std::process;

use crate::args::Cli;
use crate::human;
use crate::shared::{load_undo_log, save_undo_log};

/// Run ripsed in script mode: read a .rip file and execute each operation.
pub fn run_script_mode(script_path: &str, cli: &Cli, config: &Config) {
    let script_content = match std::fs::read_to_string(script_path) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("ripsed: cannot read script '{script_path}': {e}");
            process::exit(1);
        }
    };

    let script: Script = match parse_script(&script_content) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("ripsed: {e}");
            process::exit(1);
        }
    };

    if script.operations.is_empty() {
        eprintln!("ripsed: script '{script_path}' contains no operations");
        process::exit(1);
    }

    let mut total_changes = 0usize;
    let mut files_modified_set: HashSet<PathBuf> = HashSet::new();

    // Load undo log for recording changes (only when not dry-run)
    let mut undo_log = if !cli.dry_run {
        Some(load_undo_log(config))
    } else {
        None
    };

    for script_op in &script.operations {
        let op = &script_op.op;

        let matcher = match Matcher::new(op) {
            Ok(m) => m,
            Err(e) => {
                eprintln!("ripsed: {e}");
                process::exit(1);
            }
        };

        // Build options using per-op glob or fall back to CLI glob
        let effective_glob = script_op.glob.clone().or_else(|| cli.glob.clone());

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
            glob: effective_glob,
            ignore: cli.ignore_pattern.clone(),
            hidden: cli.hidden,
            max_depth: cli.max_depth.or(config.defaults.max_depth),
            line_range: cli.line_range,
        };

        let mut discovery_opts = DiscoveryOptions::from_op_options(&options);
        discovery_opts.follow_links = cli.follow;
        let files = discover_files_auto(&discovery_opts, false);

        if files.is_empty() {
            continue;
        }

        for file_path in &files {
            let content = match reader::read_file(file_path) {
                Ok(c) => c,
                Err(e) => {
                    eprintln!("ripsed: {}: {e}", file_path.display());
                    continue;
                }
            };

            let output = match engine::apply(&content, op, &matcher, options.line_range, 3) {
                Ok(o) => o,
                Err(e) => {
                    eprintln!("ripsed: {}: {e}", file_path.display());
                    continue;
                }
            };

            if output.changes.is_empty() {
                continue;
            }

            total_changes += output.changes.len();

            if cli.count {
                // Just count, don't print diffs
            } else if !cli.quiet {
                human::print_file_diff(file_path, &output.changes);
            }

            if !cli.dry_run {
                if options.backup && !files_modified_set.contains(file_path) {
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
                        Ok(()) => {
                            files_modified_set.insert(file_path.clone());
                        }
                        Err(e) => {
                            eprintln!("ripsed: write failed for {}: {e}", file_path.display());
                        }
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
        human::print_summary(files_modified_set.len(), total_changes, cli.dry_run);
    }

    if total_changes == 0 {
        process::exit(1);
    }
}
