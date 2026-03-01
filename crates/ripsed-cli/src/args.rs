use clap::Parser;

/// ripsed — a fast, modern stream editor. Like sed, but better.
#[derive(Parser, Debug)]
#[command(name = "ripsed", version, about)]
pub struct Cli {
    /// Pattern to search for
    pub find: Option<String>,

    /// Replacement string
    pub replace: Option<String>,

    /// Treat FIND as a regex pattern
    #[arg(short = 'e', long)]
    pub regex: bool,

    /// Delete matching lines
    #[arg(short, long)]
    pub delete: bool,

    /// Modify files in place
    #[arg(short, long)]
    pub in_place: bool,

    /// Read from stdin, write to stdout
    #[arg(short, long)]
    pub pipe: bool,

    /// Preview changes without writing
    #[arg(long)]
    pub dry_run: bool,

    /// Create .bak files before modifying
    #[arg(long)]
    pub backup: bool,

    /// Only process files matching glob
    #[arg(long)]
    pub glob: Option<String>,

    /// Skip files matching glob
    #[arg(long = "ignore")]
    pub ignore_pattern: Option<String>,

    /// Include hidden files
    #[arg(long)]
    pub hidden: bool,

    /// Don't respect .gitignore
    #[arg(long)]
    pub no_gitignore: bool,

    /// Enable agent/JSON mode
    #[arg(short, long)]
    pub json: bool,

    /// JSON input as argument (for --json mode)
    #[arg(long, hide = true)]
    pub json_input: Option<String>,

    /// Force human mode even if stdin looks like JSON
    #[arg(long)]
    pub no_json: bool,

    /// Stream results as JSON Lines
    #[arg(long)]
    pub jsonl: bool,

    /// Insert text after matching lines
    #[arg(long)]
    pub after: Option<String>,

    /// Insert text before matching lines
    #[arg(long)]
    pub before: Option<String>,

    /// Replace entire matching line with new content
    #[arg(long)]
    pub replace_line: Option<String>,

    /// Only operate on lines N through M (format: N:M)
    #[arg(short = 'n', long)]
    pub line_range: Option<String>,

    /// Maximum directory recursion depth
    #[arg(long)]
    pub max_depth: Option<usize>,

    /// Case-insensitive matching
    #[arg(long)]
    pub case_insensitive: bool,

    /// Print count of matches/replacements only
    #[arg(short, long)]
    pub count: bool,

    /// Suppress all non-error output
    #[arg(short, long)]
    pub quiet: bool,

    /// Interactive confirmation before each change
    #[arg(long)]
    pub confirm: bool,
}
