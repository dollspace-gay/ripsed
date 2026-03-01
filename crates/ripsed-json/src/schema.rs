/// Current protocol version.
pub const CURRENT_VERSION: &str = "1";

/// Supported protocol versions.
pub const SUPPORTED_VERSIONS: &[&str] = &["1"];

/// Check if a version string is supported.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_VERSIONS.contains(&version)
}
