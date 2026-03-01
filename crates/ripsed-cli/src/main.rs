mod args;
pub mod file_mode;
mod human;
mod interactive;
pub mod json_mode;
pub mod pipe_mode;
pub mod script_mode;
pub mod shared;

use args::Cli;
use clap::Parser;
use ripsed_json::detect::{InputMode, detect_stdin};
use std::io::{IsTerminal, Read};
use std::process;

fn main() {
    let cli = Cli::parse();

    // Load config from --config or auto-discover
    let config = shared::load_config(&cli);

    // Handle --script before other modes
    if let Some(ref script_path) = cli.script {
        script_mode::run_script_mode(script_path, &cli, &config);
        return;
    }

    // Handle --undo-list before anything else
    if cli.undo_list {
        file_mode::handle_undo_list(&config);
        return;
    }

    // Handle --undo before anything else
    if let Some(count) = cli.undo {
        file_mode::handle_undo(count, &config);
        return;
    }

    // Force pipe mode when --pipe is set
    if cli.pipe {
        let mut data = Vec::new();
        std::io::stdin().read_to_end(&mut data).unwrap_or_else(|e| {
            eprintln!("ripsed: failed to read stdin: {e}");
            process::exit(1);
        });
        pipe_mode::run_pipe_mode(&cli, &data);
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
                file_mode::run_file_mode(&cli, &config);
                return;
            }

            if cli.json {
                json_mode::run_json_mode(&input, &config, cli.jsonl);
            } else {
                // Auto-detect
                let mut cursor = std::io::Cursor::new(input.as_bytes());
                match detect_stdin(&mut cursor) {
                    Ok(InputMode::Json(json)) => {
                        json_mode::run_json_mode(&json, &config, cli.jsonl)
                    }
                    Ok(InputMode::Pipe(data)) => {
                        pipe_mode::run_pipe_mode(&cli, &data);
                    }
                    Err(e) => {
                        eprintln!("ripsed: failed to read stdin: {e}");
                        process::exit(1);
                    }
                }
            }
        } else if let Some(ref json_arg) = cli.json_input {
            json_mode::run_json_mode(json_arg, &config, cli.jsonl);
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
        pipe_mode::run_pipe_mode(&cli, &data);
    } else {
        // File mode
        file_mode::run_file_mode(&cli, &config);
    }
}
