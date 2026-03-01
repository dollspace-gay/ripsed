use ignore::WalkBuilder;
use ripsed_core::operation::OpOptions;
use std::path::{Path, PathBuf};

/// Options for file discovery.
pub struct DiscoveryOptions {
    pub root: PathBuf,
    pub glob: Option<String>,
    pub ignore_pattern: Option<String>,
    pub gitignore: bool,
    pub hidden: bool,
    pub max_depth: Option<usize>,
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
        }
    }
}

/// Discover files to process based on options.
pub fn discover_files(opts: &DiscoveryOptions) -> Vec<PathBuf> {
    let mut builder = WalkBuilder::new(&opts.root);
    builder
        .git_ignore(opts.gitignore)
        .git_global(opts.gitignore)
        .git_exclude(opts.gitignore)
        .hidden(!opts.hidden)
        .follow_links(false);

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
            // Filter out binary files by checking for null bytes in first 8KB
            if let Ok(content) = std::fs::read(entry.path()) {
                let check_len = content.len().min(8192);
                !content[..check_len].contains(&0)
            } else {
                false
            }
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

/// Simple glob matching (delegates to the `ignore` crate's globbing).
fn glob_match(pattern: &str, path: &str) -> bool {
    ignore::gitignore::GitignoreBuilder::new("")
        .add_line(None, pattern)
        .ok()
        .and_then(|b| b.build().ok())
        .is_some_and(|gi| {
            gi.matched(Path::new(path), false).is_ignore()
        })
}
