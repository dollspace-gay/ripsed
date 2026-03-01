mod args;
mod human;
mod interactive;

use args::Cli;
use clap::Parser;
use ripsed_core::diff::Summary;
use ripsed_core::engine;
use ripsed_core::matcher::Matcher;
use ripsed_core::operation::{Op, OpOptions};
use ripsed_fs::discovery::{DiscoveryOptions, discover_files};
use ripsed_fs::reader;
use ripsed_fs::writer;
use ripsed_json::detect::{InputMode, detect_stdin};
use ripsed_json::request::JsonRequest;
use ripsed_json::response::JsonResponse;
use std::io::{IsTerminal, Read};
use std::process;

fn main() {
    let cli = Cli::parse();

    // Check if stdin has data (pipe mode detection)
    let stdin_is_tty = std::io::stdin().is_terminal();

    if cli.json || (!stdin_is_tty && !cli.no_json) {
        // Attempt JSON/agent mode
        if !stdin_is_tty {
            let mut input = String::new();
            std::io::stdin().read_to_string(&mut input).unwrap_or_else(|e| {
                eprintln!("ripsed: failed to read stdin: {e}");
                process::exit(1);
            });

            // If stdin was empty and --json wasn't explicitly requested,
            // fall through to file mode (subprocess/test environments often
            // have stdin as a closed pipe rather than a tty).
            if input.is_empty() && !cli.json {
                run_file_mode(&cli);
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
        run_file_mode(&cli);
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

    for (op_index, (op, _glob)) in ops.iter().enumerate() {
        let matcher = match Matcher::new(op) {
            Ok(m) => m,
            Err(mut e) => {
                e.operation_index = Some(op_index);
                errors.push(e);
                continue;
            }
        };

        for file_path in &files {
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

                let result = engine::build_op_result(
                    op_index,
                    &file_path.to_string_lossy(),
                    output.changes,
                );
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

    match engine::apply(text, &op, &matcher, None, 0) {
        Ok(output) => {
            print!("{}", output.text.as_deref().unwrap_or(text));
        }
        Err(e) => {
            eprintln!("ripsed: {e}");
            process::exit(1);
        }
    }
}

fn run_file_mode(cli: &Cli) {
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
        line_range: None,
    };

    let discovery_opts = DiscoveryOptions::from_op_options(&options);
    let files = discover_files(&discovery_opts);

    if files.is_empty() {
        eprintln!("ripsed: no files found");
        process::exit(1);
    }

    let mut total_changes = 0usize;
    let mut files_modified = 0usize;

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
                match writer::write_atomic(file_path, text) {
                    Ok(()) => files_modified += 1,
                    Err(e) => {
                        eprintln!("ripsed: write failed for {}: {e}", file_path.display());
                    }
                }
            }
        }
    }

    if cli.count {
        println!("{total_changes}");
    } else if !cli.quiet {
        if cli.dry_run {
            eprintln!(
                "ripsed: dry run — {} change(s) across {} file(s) (not applied)",
                total_changes,
                files.len()
            );
        } else {
            eprintln!(
                "ripsed: {} change(s) applied across {} file(s)",
                total_changes, files_modified
            );
        }
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
