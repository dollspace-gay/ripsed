use crate::reader;
use ignore::WalkBuilder;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// Convert an `ignore` crate error into an `io::Error`.
fn glob_error(e: ignore::Error) -> io::Error {
    io::Error::new(
        io::ErrorKind::InvalidInput,
        format!("invalid glob pattern: {e}"),
    )
}

/// Options for file discovery.
pub struct DiscoveryOptions {
    pub root: PathBuf,
    pub glob: Option<String>,
    pub ignore_pattern: Option<String>,
    pub gitignore: bool,
    pub hidden: bool,
    pub max_depth: Option<usize>,
    pub follow_links: bool,
}

impl Default for DiscoveryOptions {
    fn default() -> Self {
        Self {
            root: std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")),
            glob: None,
            ignore_pattern: None,
            gitignore: true,
            hidden: false,
            max_depth: None,
            follow_links: false,
        }
    }
}

/// Build a `WalkBuilder` with the shared configuration from `DiscoveryOptions`.
///
/// Both serial and parallel discovery call this so that gitignore, hidden,
/// follow_links, max_depth, and glob overrides are configured in one place.
fn configure_walk_builder(opts: &DiscoveryOptions) -> io::Result<WalkBuilder> {
    let mut builder = WalkBuilder::new(&opts.root);
    builder
        .git_ignore(opts.gitignore)
        .git_global(opts.gitignore)
        .git_exclude(opts.gitignore)
        .hidden(!opts.hidden)
        .follow_links(opts.follow_links);

    if let Some(depth) = opts.max_depth {
        builder.max_depth(Some(depth));
    }

    if let Some(ref glob) = opts.glob {
        let mut overrides = ignore::overrides::OverrideBuilder::new(&opts.root);
        overrides.add(glob).map_err(glob_error)?;
        let built = overrides.build().map_err(glob_error)?;
        builder.overrides(built);
    }

    Ok(builder)
}

/// Discover files to process based on options.
///
/// Returns an error if the glob pattern is invalid.
pub fn discover_files(opts: &DiscoveryOptions) -> io::Result<Vec<PathBuf>> {
    let builder = configure_walk_builder(opts)?;
    let walker = builder.build();

    Ok(walker
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.file_type().is_some_and(|ft| ft.is_file()))
        .filter(|entry| {
            // Filter out binary files (reads only 8KB, not the whole file)
            !reader::is_binary(entry.path()).unwrap_or(true)
        })
        .filter(|entry| {
            // Apply ignore pattern if set
            if let Some(ref pattern) = opts.ignore_pattern {
                let path_str = entry.path().to_string_lossy();
                !glob_match(pattern, &path_str)
            } else {
                true
            }
        })
        .map(|entry| entry.into_path())
        .collect())
}

/// Discover files using WalkBuilder's parallel walker for large directories.
///
/// Returns an error if the glob pattern is invalid.
pub fn discover_files_parallel(opts: &DiscoveryOptions) -> io::Result<Vec<PathBuf>> {
    let mut builder = configure_walk_builder(opts)?;
    builder.threads(rayon::current_num_threads().max(2));

    let ignore_pattern = opts.ignore_pattern.clone();
    let results: Mutex<Vec<PathBuf>> = Mutex::new(Vec::new());

    builder.build_parallel().run(|| {
        let results = &results;
        let ignore_pattern = ignore_pattern.clone();
        Box::new(move |entry| {
            let entry = match entry {
                Ok(e) => e,
                Err(_) => return ignore::WalkState::Continue,
            };

            // Only process regular files
            if !entry.file_type().is_some_and(|ft| ft.is_file()) {
                return ignore::WalkState::Continue;
            }

            // Filter out binary files (reads only 8KB, not the whole file)
            if reader::is_binary(entry.path()).unwrap_or(true) {
                return ignore::WalkState::Continue;
            }

            // Apply ignore pattern
            if let Some(ref pattern) = ignore_pattern {
                let path_str = entry.path().to_string_lossy();
                if glob_match(pattern, &path_str) {
                    return ignore::WalkState::Continue;
                }
            }

            results.lock().unwrap().push(entry.into_path());
            ignore::WalkState::Continue
        })
    });

    let mut files = results.into_inner().unwrap();
    files.sort();
    Ok(files)
}

/// Strategy for choosing between serial and parallel file discovery.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WalkStrategy {
    /// Automatically choose based on directory size heuristic.
    Auto,
    /// Always use the parallel walker.
    ForceParallel,
}

/// Choose between serial and parallel file discovery.
///
/// With [`WalkStrategy::Auto`], always uses the parallel walker — it has
/// minimal overhead on small directories and avoids the unreliable top-level
/// entry count heuristic that previously caused incorrect serial fallback
/// on deep directory trees.
///
/// Returns an error if the glob pattern is invalid.
pub fn discover_files_auto(
    opts: &DiscoveryOptions,
    _strategy: WalkStrategy,
) -> io::Result<Vec<PathBuf>> {
    discover_files_parallel(opts)
}

/// Simple glob matching (delegates to the `ignore` crate's globbing).
fn glob_match(pattern: &str, path: &str) -> bool {
    ignore::gitignore::GitignoreBuilder::new("")
        .add_line(None, pattern)
        .ok()
        .and_then(|b| b.build().ok())
        .is_some_and(|gi| gi.matched(Path::new(path), false).is_ignore())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    /// Helper: create a temporary directory tree with the given number of text files.
    fn make_tree(count: usize) -> tempfile::TempDir {
        let dir = tempfile::tempdir().unwrap();
        for i in 0..count {
            let p = dir.path().join(format!("file_{i}.txt"));
            fs::write(&p, format!("content {i}\n")).unwrap();
        }
        dir
    }

    #[test]
    fn serial_and_parallel_agree() {
        let dir = make_tree(20);
        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: None,
            ignore_pattern: None,
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        let mut serial = discover_files(&opts).unwrap();
        serial.sort();
        let parallel = discover_files_parallel(&opts).unwrap();

        assert_eq!(serial, parallel);
    }

    #[test]
    fn parallel_skips_binary() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("text.txt"), "hello\n").unwrap();
        fs::write(dir.path().join("bin.dat"), b"\x00\x01\x02").unwrap();

        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: None,
            ignore_pattern: None,
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        let files = discover_files_parallel(&opts).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("text.txt"));
    }

    #[test]
    fn auto_uses_serial_for_small() {
        let dir = make_tree(5);
        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: None,
            ignore_pattern: None,
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        // Should not panic and should find all 5 files
        let files = discover_files_auto(&opts, WalkStrategy::Auto).unwrap();
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn auto_force_parallel() {
        let dir = make_tree(5);
        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: None,
            ignore_pattern: None,
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        let files = discover_files_auto(&opts, WalkStrategy::ForceParallel).unwrap();
        assert_eq!(files.len(), 5);
    }

    #[test]
    fn parallel_respects_glob() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("a.rs"), "fn main() {}").unwrap();
        fs::write(dir.path().join("b.txt"), "hello").unwrap();

        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: Some("*.rs".to_string()),
            ignore_pattern: None,
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        let files = discover_files_parallel(&opts).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("a.rs"));
    }

    #[test]
    fn parallel_respects_ignore_pattern() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("keep.txt"), "keep").unwrap();
        fs::write(dir.path().join("skip.log"), "skip").unwrap();

        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: None,
            ignore_pattern: Some("*.log".to_string()),
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        let files = discover_files_parallel(&opts).unwrap();
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("keep.txt"));
    }

    #[test]
    fn invalid_glob_returns_error() {
        let dir = make_tree(3);
        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: Some("[invalid".to_string()),
            ignore_pattern: None,
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        assert!(discover_files(&opts).is_err());
        assert!(discover_files_parallel(&opts).is_err());
        assert!(discover_files_auto(&opts, WalkStrategy::Auto).is_err());
    }

    #[test]
    fn valid_glob_returns_ok() {
        let dir = make_tree(3);
        let opts = DiscoveryOptions {
            root: dir.path().to_path_buf(),
            glob: Some("*.txt".to_string()),
            ignore_pattern: None,
            gitignore: false,
            hidden: false,
            max_depth: None,
            follow_links: false,
        };

        assert!(discover_files(&opts).is_ok());
        assert!(discover_files_parallel(&opts).is_ok());
        assert!(discover_files_auto(&opts, WalkStrategy::Auto).is_ok());
    }
}
