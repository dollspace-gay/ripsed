mod args;
mod human;
mod interactive;

use args::Cli;
use clap::Parser;
use interactive::ConfirmAction;
use ripsed_core::config::Config;
use ripsed_core::diff::Summary;
use ripsed_core::engine;
use ripsed_core::matcher::Matcher;
use ripsed_core::operation::{Op, OpOptions};
use ripsed_core::undo::{UndoLog, UndoRecord};
use ripsed_fs::discovery::{DiscoveryOptions, discover_files};
use ripsed_fs::reader;
use ripsed_fs::writer;
use ripsed_json::detect::{InputMode, detect_stdin};
use ripsed_json::request::JsonRequest;
use ripsed_json::response::JsonResponse;
use std::io::{IsTerminal, Read};
use std::path::{Path, PathBuf};
use std::process;

fn main() {
    let cli = Cli::parse();

    // Load config from --config or auto-discover
    let config = load_config(&cli);

    // Handle --undo-list before anything else
    if cli.undo_list {
        handle_undo_list(&config);
        return;
    }

    // Handle --undo before anything else
    if let Some(count) = cli.undo {
        handle_undo(count, &config);
        return;
    }

    // Check if stdin has data (pipe mode detection)
    let stdin_is_tty = std::io::stdin().is_terminal();

    if cli.json || (!stdin_is_tty && !cli.no_json) {
        // Attempt JSON/agent mode
        if !stdin_is_tty {
            let mut input = String::new();
            std::io::stdin()
                .read_to_string(&mut input)
                .unwrap_or_else(|e| {
                    eprintln!("ripsed: failed to read stdin: {e}");
                    process::exit(1);
                });

            // If stdin was empty and --json wasn't explicitly requested,
            // fall through to file mode (subprocess/test environments often
            // have stdin as a closed pipe rather than a tty).
            if input.is_empty() && !cli.json {
                run_file_mode(&cli, &config);
                return;
            }

            if cli.json {
                run_json_mode(&input);
            } else {
                // Auto-detect
                let mut cursor = std::io::Cursor::new(input.as_bytes());
                match detect_stdin(&mut cursor) {
                    Ok(InputMode::Json(json)) => run_json_mode(&json),
                    Ok(InputMode::Pipe(data)) => {
                        run_pipe_mode(&cli, &data);
                    }
                    Err(e) => {
                        eprintln!("ripsed: failed to read stdin: {e}");
                        process::exit(1);
                    }
                }
            }
        } else if let Some(ref json_arg) = cli.json_input {
            run_json_mode(json_arg);
        } else {
            eprintln!("ripsed: --json requires input via stdin or argument");
            process::exit(1);
        }
    } else if !stdin_is_tty {
        // Pipe mode: stdin -> stdout (--no-json was set)
        let mut data = Vec::new();
        std::io::stdin().read_to_end(&mut data).unwrap_or_else(|e| {
            eprintln!("ripsed: failed to read stdin: {e}");
            process::exit(1);
        });
        run_pipe_mode(&cli, &data);
    } else {
        // File mode
        run_file_mode(&cli, &config);
    }
}

/// Load configuration from --config path or auto-discover from cwd.
fn load_config(cli: &Cli) -> Config {
    if let Some(ref path_str) = cli.config {
        match Config::load(Path::new(path_str)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("ripsed: {e}");
                process::exit(1);
            }
        }
    } else {
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        Config::discover(&cwd)
            .map(|(_path, config)| config)
            .unwrap_or_default()
    }
}

/// Resolve the undo directory: `.ripsed/` next to the config file, or in cwd.
fn undo_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(".ripsed")
}

fn undo_log_path() -> PathBuf {
    undo_dir().join("undo.jsonl")
}

/// Load the undo log from disk.
fn load_undo_log(config: &Config) -> UndoLog {
    let path = undo_log_path();
    if path.exists() {
        match std::fs::read_to_string(&path) {
            Ok(content) => UndoLog::from_jsonl(&content, config.undo.max_entries),
            Err(_) => UndoLog::new(config.undo.max_entries),
        }
    } else {
        UndoLog::new(config.undo.max_entries)
    }
}

/// Save the undo log to disk.
fn save_undo_log(log: &UndoLog) {
    let dir = undo_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = undo_log_path();
    let _ = std::fs::write(&path, log.to_jsonl());
}

/// Handle `--undo N`: restore the last N files from the undo log.
fn handle_undo(count: usize, config: &Config) {
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
        let path = Path::new(&record.file_path);
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
fn handle_undo_list(config: &Config) {
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

fn run_json_mode(input: &str) {
    let request = match JsonRequest::parse(input) {
        Ok(r) => r,
        Err(e) => {
            let response = JsonResponse::error(vec![e]);
            println!("{}", response.to_json());
            process::exit(1);
        }
    };

    let dry_run = request.options.dry_run;
    let (ops, options) = request.into_ops();
    let discovery_opts = DiscoveryOptions::from_op_options(&options);
    let files = discover_files(&discovery_opts);

    let mut summary = Summary::default();
    let mut results = Vec::new();
    let mut errors = Vec::new();

    for (op_index, (op, op_glob)) in ops.iter().enumerate() {
        let matcher = match Matcher::new(op) {
            Ok(m) => m,
            Err(mut e) => {
                e.operation_index = Some(op_index);
                errors.push(e);
                continue;
            }
        };

        // Build a glob matcher for per-operation filtering
        let glob_matcher = op_glob.as_ref().and_then(|g| {
            globset::GlobBuilder::new(g)
                .literal_separator(true)
                .build()
                .ok()
                .map(|glob| glob.compile_matcher())
        });

        for file_path in &files {
            // Skip files that don't match the per-operation glob
            if let Some(ref gm) = glob_matcher {
                if !gm.is_match(file_path) {
                    // Also try matching just the file name
                    let matches_name = file_path
                        .file_name()
                        .map(|n| gm.is_match(n))
                        .unwrap_or(false);
                    if !matches_name {
                        continue;
                    }
                }
            }
            let content = match reader::read_file(file_path) {
                Ok(c) => c,
                Err(_) => continue,
            };

            let output = match engine::apply(&content, op, &matcher, options.line_range, 3) {
                Ok(o) => o,
                Err(e) => {
                    errors.push(e);
                    continue;
                }
            };

            if !output.changes.is_empty() {
                summary.files_matched += 1;
                summary.total_replacements += output.changes.len();

                let result =
                    engine::build_op_result(op_index, &file_path.to_string_lossy(), output.changes);
                results.push(result);

                if !dry_run {
                    if let Some(ref text) = output.text {
                        if writer::write_atomic(file_path, text).is_ok() {
                            summary.files_modified += 1;
                        }
                    }
                }
            }
        }
    }

    let response = if errors.is_empty() {
        JsonResponse::success(dry_run, summary, results)
    } else {
        let mut resp = JsonResponse::success(dry_run, summary, results);
        resp.errors = errors;
        resp.success = resp.errors.is_empty();
        resp
    };

    println!("{}", response.to_json());
    if !response.success {
        process::exit(1);
    }
}

fn run_pipe_mode(cli: &Cli, data: &[u8]) {
    let text = match std::str::from_utf8(data) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("ripsed: stdin is not valid UTF-8: {e}");
            process::exit(1);
        }
    };

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

    match engine::apply(text, &op, &matcher, cli.line_range, 0) {
        Ok(output) => {
            if cli.count {
                println!("{}", output.changes.len());
            } else {
                print!("{}", output.text.as_deref().unwrap_or(text));
            }
        }
        Err(e) => {
            eprintln!("ripsed: {e}");
            process::exit(1);
        }
    }
}

fn run_file_mode(cli: &Cli, config: &Config) {
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
        gitignore: !cli.no_gitignore,
        backup: cli.backup,
        atomic: false,
        glob: cli.glob.clone(),
        ignore: cli.ignore_pattern.clone(),
        hidden: cli.hidden,
        max_depth: cli.max_depth,
        line_range: cli.line_range,
    };

    let discovery_opts = DiscoveryOptions::from_op_options(&options);
    let files = discover_files(&discovery_opts);

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
            if cli.backup {
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

fn build_op_from_cli(cli: &Cli, find: &str) -> Op {
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
