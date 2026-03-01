use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

/// Project-level configuration from `.ripsed.toml`.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Config {
    #[serde(default)]
    pub defaults: DefaultsConfig,
    #[serde(default)]
    pub agent: AgentConfig,
    #[serde(default)]
    pub ignore: IgnoreConfig,
    #[serde(default)]
    pub undo: UndoConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DefaultsConfig {
    #[serde(default)]
    pub backup: bool,
    #[serde(default = "default_true")]
    pub gitignore: bool,
    pub max_depth: Option<usize>,
}

impl Default for DefaultsConfig {
    fn default() -> Self {
        Self {
            backup: false,
            gitignore: true,
            max_depth: None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentConfig {
    #[serde(default = "default_true")]
    pub dry_run: bool,
    #[serde(default = "default_context_lines")]
    pub context_lines: usize,
}

impl Default for AgentConfig {
    fn default() -> Self {
        Self {
            dry_run: true,
            context_lines: 3,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct IgnoreConfig {
    #[serde(default)]
    pub patterns: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UndoConfig {
    #[serde(default = "default_max_entries")]
    pub max_entries: usize,
}

impl Default for UndoConfig {
    fn default() -> Self {
        Self {
            max_entries: 100,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_context_lines() -> usize {
    3
}

fn default_max_entries() -> usize {
    100
}

impl Config {
    /// Load configuration by walking up from `start_dir` looking for `.ripsed.toml`.
    pub fn discover(start_dir: &Path) -> Option<(PathBuf, Config)> {
        let mut dir = start_dir.to_path_buf();
        loop {
            let config_path = dir.join(".ripsed.toml");
            if config_path.exists() {
                match std::fs::read_to_string(&config_path) {
                    Ok(content) => match toml::from_str::<Config>(&content) {
                        Ok(config) => return Some((config_path, config)),
                        Err(_) => return None,
                    },
                    Err(_) => return None,
                }
            }
            if !dir.pop() {
                break;
            }
        }
        None
    }

    /// Load from a specific path.
    pub fn load(path: &Path) -> Result<Config, String> {
        let content =
            std::fs::read_to_string(path).map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        toml::from_str(&content).map_err(|e| format!("Invalid TOML in {}: {e}", path.display()))
    }
}
