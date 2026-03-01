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

    #[test]
    fn current_version_is_supported() {
        assert!(is_supported_version(CURRENT_VERSION));
    }

    #[test]
    fn version_1_is_supported() {
        assert!(is_supported_version("1"));
    }

    #[test]
    fn unknown_version_is_not_supported() {
        assert!(!is_supported_version("0"));
        assert!(!is_supported_version("2"));
        assert!(!is_supported_version(""));
    }

    #[test]
    fn current_version_is_1() {
        assert_eq!(CURRENT_VERSION, "1");
    }
}
