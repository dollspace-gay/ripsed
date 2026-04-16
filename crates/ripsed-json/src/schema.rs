/// Current protocol version.
pub const CURRENT_VERSION: &str = "1";

/// Supported protocol versions.
pub const SUPPORTED_VERSIONS: &[&str] = &["1"];

/// Check if a version string is supported.
pub fn is_supported_version(version: &str) -> bool {
    SUPPORTED_VERSIONS.contains(&version)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Protocol behavior: known supported versions are accepted.
    #[test]
    fn version_1_is_supported() {
        assert!(is_supported_version("1"));
    }

    /// Protocol behavior: unknown versions, including future/legacy ones
    /// and the empty string, are rejected. This is the real behavioral
    /// invariant — being permissive would cause silent protocol drift.
    #[test]
    fn unknown_versions_are_rejected() {
        assert!(!is_supported_version("0"));
        assert!(!is_supported_version("2"));
        assert!(!is_supported_version(""));
        assert!(!is_supported_version("1.0"));
        assert!(!is_supported_version("v1"));
        assert!(!is_supported_version(" 1"));
    }
}
