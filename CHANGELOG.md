# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.1.0] - 2026-03-01

Initial release. Full MVP implementation of the ripsed stream editor.

### Added

#### Core Engine (`ripsed-core`)
- Edit engine supporting five operations: replace, delete, insert_after, insert_before, replace_line
- Regex and literal pattern matching via `Matcher` with case-insensitive support
- CRLF line ending detection and preservation
- Line range filtering (`-n N:M`) to restrict operations to specific lines
- Structured diff output with contextual before/after lines
- Undo log with JSONL persistence and configurable max entries
- Config file discovery (`.ripsed.toml`) with directory walk-up
- Near-miss suggestions using Levenshtein distance for typo detection
- Comprehensive error taxonomy with codes, hints, and structured context
- 55 unit tests including property-based tests (proptest)

#### File System (`ripsed-fs`)
- Recursive file discovery using the `ignore` crate (respects `.gitignore`)
- Parallel file discovery via `WalkBuilder::build_parallel()` for large directories
- Memory-mapped file reading for files over 1MB
- UTF-8 BOM detection and stripping
- Streaming `BufReader` for very large files
- Atomic writes using tempfile + rename pattern
- Transactional multi-file writes (`write_atomic_batch`)
- Numbered backup files (`.ripsed.bak`, `.ripsed.bak.1`, `.ripsed.bak.2`)
- Advisory file locking with configurable timeout and exponential backoff
- Binary file detection (first 8KB scan)
- 38 unit tests

#### JSON Interface (`ripsed-json`)
- JSON request/response protocol (version "1") for AI coding agents
- Batch operations with per-operation `operation_index` tracking
- Comprehensive request validation (op types, regex patterns, glob syntax)
- Forward-compatible field handling via `#[serde(flatten)]`
- Auto-detection of JSON vs plain text stdin
- Per-operation glob patterns
- 54 unit tests

#### CLI (`ripsed-cli`)
- Human mode: recursive find-and-replace with colored diff output
- Pipe mode: stdin-to-stdout processing (like traditional sed)
- Agent mode: structured JSON input/output with `--json` flag
- Auto-detection: piped JSON is automatically routed to agent mode
- `--dry-run` for previewing changes without writing
- `--backup` for creating `.ripsed.bak` files before modification
- `--glob` and `--ignore` for file filtering
- `--hidden` to include dotfiles
- `--case-insensitive` for case-folded matching
- `--delete`, `--after`, `--before`, `--replace-line` operation flags
- `--confirm` for interactive yes/no/all/skip/quit prompting
- `--undo` and `--undo-list` for operation reversal
- `--config` for explicit config file path
- `-c` / `--count` for match counting
- `-q` / `--quiet` for suppressing output
- Cross-platform colored output via `anstream` / `anstyle`
- 71 integration tests (human mode, JSON mode, auto-detection, error hints)

#### Infrastructure
- GitHub Actions CI workflow (Ubuntu, macOS, Windows; stable + MSRV 1.85)
- Release workflow with cross-platform binary builds
- Criterion benchmarks for engine performance (100/1K/10K line files)
- `xtask gen-schema` for generating JSON request schema
- MIT OR Apache-2.0 dual license
