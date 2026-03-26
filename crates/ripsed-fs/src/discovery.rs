use crate::reader;
use ignore::WalkBuilder;
use ripsed_core::operation::OpOptions;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

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

impl DiscoveryOptions {
    pub fn from_op_options(opts: &OpOptions) -> Self {
        Self {
            root: opts
                .root
                .as_ref()
                .map(PathBuf::from)
                .unwrap_or_else(|| std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))),
            glob: opts.glob.clone(),
            ignore_pattern: opts.ignore.clone(),
            gitignore: opts.gitignore,
            hidden: opts.hidden,
            max_depth: opts.max_depth,
            follow_links: false,
        }
    }
}

/// Threshold at which `discover_files_auto` switches to the parallel walker.
const PARALLEL_THRESHOLD: usize = 1000;

/// Discover files to process based on options.
pub fn discover_files(opts: &DiscoveryOptions) -> Vec<PathBuf> {
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

    // Add glob filter
    if let Some(ref glob) = opts.glob {
        // Use overrides for glob patterns
        let mut overrides = ignore::overrides::OverrideBuilder::new(&opts.root);
        // The glob acts as an include filter
        let _ = overrides.add(glob);
        if let Ok(built) = overrides.build() {
            builder.overrides(built);
        }
    }

    let walker = builder.build();

    walker
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
        .collect()
}

/// Discover files using WalkBuilder's parallel walker for large directories.
///
/// This function uses `build_parallel()` from the `ignore` crate, which spawns
/// multiple threads to walk the directory tree concurrently. The results are
/// collected into a `Vec<PathBuf>` and sorted for deterministic output.
pub fn discover_files_parallel(opts: &DiscoveryOptions) -> Vec<PathBuf> {
    let mut builder = WalkBuilder::new(&opts.root);
    builder
        .git_ignore(opts.gitignore)
        .git_global(opts.gitignore)
        .git_exclude(opts.gitignore)
        .hidden(!opts.hidden)
        .follow_links(opts.follow_links)
        .threads(rayon::current_num_threads().max(2));

    if let Some(depth) = opts.max_depth {
        builder.max_depth(Some(depth));
    }

    if let Some(ref glob) = opts.glob {
        let mut overrides = ignore::overrides::OverrideBuilder::new(&opts.root);
        let _ = overrides.add(glob);
        if let Ok(built) = overrides.build() {
            builder.overrides(built);
        }
    }

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
    files
}

/// Automatically choose serial or parallel discovery based on a heuristic.
///
/// If `force_parallel` is `true`, always use the parallel walker. Otherwise,
/// first do a quick count of directory entries; if the count exceeds
/// [`PARALLEL_THRESHOLD`] (1 000), fall back to the parallel walker.
pub fn discover_files_auto(opts: &DiscoveryOptions, force_parallel: bool) -> Vec<PathBuf> {
    if force_parallel {
        return discover_files_parallel(opts);
    }

    // Quick heuristic: count top-level entries to estimate tree size.
    let entry_count = std::fs::read_dir(&opts.root)
        .map(|rd| rd.count())
        .unwrap_or(0);

    if entry_count > PARALLEL_THRESHOLD {
        discover_files_parallel(opts)
    } else {
        discover_files(opts)
    }
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

        let mut serial = discover_files(&opts);
        serial.sort();
        let parallel = discover_files_parallel(&opts); // already sorted

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

        let files = discover_files_parallel(&opts);
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
        let files = discover_files_auto(&opts, false);
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

        let files = discover_files_auto(&opts, true);
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

        let files = discover_files_parallel(&opts);
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

        let files = discover_files_parallel(&opts);
        assert_eq!(files.len(), 1);
        assert!(files[0].ends_with("keep.txt"));
    }
}
