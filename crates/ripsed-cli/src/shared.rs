use ripsed_core::config::Config;
use ripsed_core::undo::UndoLog;
use std::path::{Path, PathBuf};

use crate::args::Cli;

/// Load configuration from --config path or auto-discover from cwd.
pub fn load_config(cli: &Cli) -> Config {
    if let Some(ref path_str) = cli.config {
        match Config::load(Path::new(path_str)) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("ripsed: {e}");
                std::process::exit(1);
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
pub fn undo_dir() -> PathBuf {
    let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    cwd.join(".ripsed")
}

pub fn undo_log_path() -> PathBuf {
    undo_dir().join("undo.jsonl")
}

/// Load the undo log from disk.
pub fn load_undo_log(config: &Config) -> UndoLog {
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
pub fn save_undo_log(log: &UndoLog) {
    let dir = undo_dir();
    let _ = std::fs::create_dir_all(&dir);
    let path = undo_log_path();
    let _ = std::fs::write(&path, log.to_jsonl());
}
