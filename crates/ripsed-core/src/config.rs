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
        Self { max_entries: 100 }
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
        let content = std::fs::read_to_string(path)
            .map_err(|e| format!("Cannot read {}: {e}", path.display()))?;
        toml::from_str(&content).map_err(|e| format!("Invalid TOML in {}: {e}", path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    // ── Default values ──

    #[test]
    fn config_default_has_expected_values() {
        let config = Config::default();
        assert!(!config.defaults.backup);
        assert!(config.defaults.gitignore);
        assert!(config.defaults.max_depth.is_none());
        assert!(config.agent.dry_run);
        assert_eq!(config.agent.context_lines, 3);
        assert!(config.ignore.patterns.is_empty());
        assert_eq!(config.undo.max_entries, 100);
    }

    // ── TOML parsing ──

    #[test]
    fn parse_full_config() {
        let toml = r#"
[defaults]
backup = true
gitignore = false
max_depth = 5

[agent]
dry_run = false
context_lines = 5

[ignore]
patterns = ["*.log", "target/"]

[undo]
max_entries = 50
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.defaults.backup);
        assert!(!config.defaults.gitignore);
        assert_eq!(config.defaults.max_depth, Some(5));
        assert!(!config.agent.dry_run);
        assert_eq!(config.agent.context_lines, 5);
        assert_eq!(config.ignore.patterns, vec!["*.log", "target/"]);
        assert_eq!(config.undo.max_entries, 50);
    }

    #[test]
    fn parse_empty_toml_uses_defaults() {
        let config: Config = toml::from_str("").unwrap();
        assert!(config.defaults.gitignore);
        assert!(config.agent.dry_run);
        assert_eq!(config.undo.max_entries, 100);
    }

    #[test]
    fn parse_partial_config_fills_defaults() {
        let toml = r#"
[defaults]
backup = true
"#;
        let config: Config = toml::from_str(toml).unwrap();
        assert!(config.defaults.backup);
        assert!(config.defaults.gitignore); // default preserved
        assert_eq!(config.undo.max_entries, 100); // default preserved
    }

    // ── File discovery ──

    #[test]
    fn discover_finds_config_in_current_dir() {
        let dir = TempDir::new().unwrap();
        let config_path = dir.path().join(".ripsed.toml");
        fs::write(&config_path, "[defaults]\nbackup = true\n").unwrap();

        let (found_path, config) = Config::discover(dir.path()).unwrap();
        assert_eq!(found_path, config_path);
        assert!(config.defaults.backup);
    }

    #[test]
    fn discover_walks_up_to_parent() {
        let dir = TempDir::new().unwrap();
        let child = dir.path().join("sub/deep");
        fs::create_dir_all(&child).unwrap();
        fs::write(
            dir.path().join(".ripsed.toml"),
            "[undo]\nmax_entries = 42\n",
        )
        .unwrap();

        let (_, config) = Config::discover(&child).unwrap();
        assert_eq!(config.undo.max_entries, 42);
    }

    #[test]
    fn discover_returns_none_when_not_found() {
        let dir = TempDir::new().unwrap();
        assert!(Config::discover(dir.path()).is_none());
    }

    #[test]
    fn discover_returns_none_for_invalid_toml() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join(".ripsed.toml"), "not [valid toml!!!").unwrap();
        assert!(Config::discover(dir.path()).is_none());
    }

    // ── Config::load ──

    #[test]
    fn load_reads_valid_config() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("config.toml");
        fs::write(&path, "[agent]\ndry_run = false\n").unwrap();

        let config = Config::load(&path).unwrap();
        assert!(!config.agent.dry_run);
    }

    #[test]
    fn load_returns_error_for_missing_file() {
        let result = Config::load(Path::new("/nonexistent/config.toml"));
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Cannot read"));
    }

    #[test]
    fn load_returns_error_for_invalid_toml() {
        let dir = TempDir::new().unwrap();
        let path = dir.path().join("bad.toml");
        fs::write(&path, "{{{{not valid").unwrap();

        let result = Config::load(&path);
        assert!(result.is_err());
        assert!(result.unwrap_err().contains("Invalid TOML"));
    }
}
