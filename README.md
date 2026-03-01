# ripsed

A fast, modern stream editor built in Rust. Like [ripgrep](https://github.com/BurntSushi/ripgrep) is to grep, ripsed is to sed.

Designed for humans **and** machines — with first-class JSON support for AI coding agents.

## Features

- **Sensible defaults.** Recursive, `.gitignore`-aware, UTF-8. No flags needed for the common case.
- **No escape hell.** Standard Rust regex syntax. No sed-style delimiters.
- **Agent-native.** Structured JSON I/O as a first-class interface, not an afterthought.
- **Safe by default.** Dry-run previews, atomic writes, undo log, backup files.
- **Fast.** Parallel file discovery, memory-mapped I/O, same philosophy as ripgrep.

## Installation

### From source

```bash
cargo install --path crates/ripsed-cli
```

Requires Rust 1.85+.

## Quick Start

```bash
# Find-and-replace across all files (recursive, respects .gitignore)
ripsed 'old_function' 'new_function'

# Regex with capture groups
ripsed -e 'fn\s+old_(\w+)' 'fn new_$1'

# Scope to specific files
ripsed 'TODO' 'DONE' --glob '*.rs'

# Delete lines matching a pattern
ripsed -d 'console\.log'

# Insert text after matching lines
ripsed 'use serde;' --after 'use serde_json;'

# Preview changes without applying
ripsed 'foo' 'bar' --dry-run

# Pipe mode (stdin/stdout, like traditional sed)
echo 'hello world' | ripsed 'hello' 'goodbye'
```

## CLI Reference

```
USAGE:
    ripsed [OPTIONS] <FIND> [REPLACE]

ARGS:
    <FIND>       Pattern to search for (literal by default, regex with -e)
    [REPLACE]    Replacement string

OPTIONS:
    -e, --regex              Treat FIND as a regex
    -d, --delete             Delete matching lines
        --dry-run            Preview changes without writing
        --backup             Create .ripsed.bak files before modifying
        --glob <PATTERN>     Only process files matching glob
        --ignore <PATTERN>   Skip files matching glob
        --hidden             Include hidden files
        --no-gitignore       Don't respect .gitignore
        --case-insensitive   Case-insensitive matching
        --after <TEXT>       Insert text after matching lines
        --before <TEXT>      Insert text before matching lines
        --replace-line <TEXT> Replace entire matching line
    -n, --line-range <N:M>   Only operate on lines N through M
        --max-depth <N>      Maximum directory recursion depth
    -c, --count              Print count of matches only
    -q, --quiet              Suppress all non-error output
        --confirm            Interactive confirmation before each change
        --undo [N]           Undo the last N operations (default: 1)
        --undo-list          Show recent undo log entries
        --config <PATH>      Path to .ripsed.toml config file
    -j, --json               Enable agent/JSON mode
        --no-json            Force human mode even if stdin looks like JSON
```

## Agent / JSON Mode

ripsed has a structured JSON interface designed for AI coding agents, editor plugins, and automation pipelines. In agent mode, `dry_run` defaults to `true` for safety.

### Request

```bash
ripsed --json << 'EOF'
{
  "version": "1",
  "operations": [
    {
      "op": "replace",
      "find": "old_function",
      "replace": "new_function",
      "glob": "src/**/*.rs"
    },
    {
      "op": "delete",
      "find": "^\\s*//\\s*TODO:.*$",
      "regex": true
    }
  ],
  "options": {
    "dry_run": true,
    "root": "./my-project"
  }
}
EOF
```

### Response

```json
{
  "version": "1",
  "success": true,
  "dry_run": true,
  "summary": {
    "files_matched": 12,
    "files_modified": 0,
    "total_replacements": 34
  },
  "results": [
    {
      "operation_index": 0,
      "files": [
        {
          "path": "src/lib.rs",
          "changes": [
            {
              "line": 42,
              "before": "    let result = old_function(x);",
              "after": "    let result = new_function(x);",
              "context": {
                "before": ["fn main() {", "    let x = 5;"],
                "after": ["    println!(\"{}\", result);", "}"]
              }
            }
          ]
        }
      ]
    }
  ],
  "errors": []
}
```

### Operations

| Operation | JSON `op` | Human flag | Description |
|---|---|---|---|
| Replace | `replace` | `ripsed 'find' 'replace'` | Find and replace text |
| Delete | `delete` | `-d` | Remove lines matching pattern |
| Insert after | `insert_after` | `--after` | Insert text after matching lines |
| Insert before | `insert_before` | `--before` | Insert text before matching lines |
| Replace line | `replace_line` | `--replace-line` | Replace entire matching line |

### Error Handling

Every error includes a machine-readable `code`, human-readable `message`, and actionable `hint`:

| Code | Description |
|---|---|
| `no_matches` | Pattern matched nothing |
| `invalid_regex` | Regex failed to compile |
| `invalid_request` | Malformed JSON or missing fields |
| `file_not_found` | Target path doesn't exist |
| `permission_denied` | Can't read/write target files |
| `binary_file_skipped` | Binary file was skipped |
| `write_failed` | Could not write output file |

## Configuration

Create a `.ripsed.toml` in your project root:

```toml
[defaults]
backup = true
max_depth = 10

[undo]
max_entries = 100
```

ripsed discovers this file by walking up from the current directory, similar to `.gitignore`.

## Architecture

ripsed is organized as a Rust workspace with four crates:

| Crate | Description |
|---|---|
| `ripsed-core` | Pure logic: edit engine, matcher, operation IR, error taxonomy |
| `ripsed-fs` | File I/O: discovery, reading (with mmap), atomic writes, locking |
| `ripsed-json` | Agent interface: request/response schemas, auto-detection |
| `ripsed-cli` | Binary: CLI args, human output formatting, interactive confirm |

## License

Licensed under either of

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Contributing

Contributions are welcome! Please open an issue or submit a pull request.

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in this project by you, as defined in the Apache-2.0 license, shall be dual licensed as above, without any additional terms or conditions.
