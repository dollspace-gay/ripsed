# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.0] - 2026-03-01

### Added
- JSON undo dispatch: send `{"undo": {"last": N}}` to undo operations via JSON mode
- JSONL streaming output with `--jsonl` flag for real-time per-file results
- Atomic batch mode: all-or-nothing writes when `options.atomic` is true in JSON mode
- Undo logging in JSON mode (previously only file mode recorded undo entries)
- Parallel file discovery: auto-switches to parallel walker for large directories
- Config defaults merging: `.ripsed.toml` defaults now apply to CLI invocations
- `--pipe` flag to force pipe mode regardless of TTY detection
- `--follow` flag to follow symbolic links during file discovery
- Integration tests for undo, gitignore, config, cross-platform, and atomic writes
- CI: cargo-deny (license + advisory auditing)
- CI: cargo-audit (CVE checking)
- CI: Miri job (advisory, for undefined behavior detection)
- Release: aarch64-unknown-linux-gnu target
- Release: SHA256 checksum generation

### Changed
- File discovery now uses auto-switching heuristic (serial for small dirs, parallel for large)
- Refactored CLI into separate modules (json_mode, file_mode, pipe_mode, shared)

### Removed
- `--in-place` flag (redundant; file mode writes in-place by default)

## [0.1.0] - 2026-03-01

### Added
- Initial release
- Four-crate workspace architecture (ripsed-core, ripsed-fs, ripsed-json, ripsed-cli)
- JSON agent mode with auto-detection from stdin
- File mode with colored diffs and dry-run preview
- Pipe mode (stdin -> stdout) for Unix pipeline integration
- Operations: replace, delete, insert_after, insert_before, replace_line
- Regex support with capture group replacement
- Case-insensitive matching
- Per-operation glob filtering in JSON mode
- File discovery with .gitignore support
- Atomic file writes with temp file + rename
- Backup file creation (`.ripsed.bak`)
- Undo support (`--undo`, `--undo-list`)
- Interactive confirmation mode (`--confirm`)
- Configuration via `.ripsed.toml` with directory discovery
- CRLF line ending preservation
- Binary file detection and skipping
- Memory-mapped I/O for large files
- Cross-platform support (Linux, macOS, Windows)
- 273 tests across all crates
