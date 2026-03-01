/// The old_function does important work.
pub fn old_function(x: i32) -> i32 {
    x * 2
}

/// old_helper is a utility function.
pub fn old_helper() -> &'static str {
    "old_helper result"
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_old_function() {
        assert_eq!(old_function(5), 10);
    }
}
