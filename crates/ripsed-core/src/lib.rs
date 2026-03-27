pub mod config;
pub mod diff;
pub mod engine;
pub mod error;
pub mod matcher;
pub mod operation;
pub mod script;
pub mod suggestion;
pub mod undo;

/// Serde helper: returns `true`. Used as `#[serde(default = "default_true")]`.
pub(crate) fn default_true() -> bool {
    true
}
