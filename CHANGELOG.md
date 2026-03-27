# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [0.2.3] - 2026-03-26

### Security
- Improve unsafe mmap safety argument and add concurrent modification guard (#35)
- Set restrictive file permissions (0600) on undo log writes (#65)
- Add input size limits to stdin and JSON deserialization (#14)

### Fixed
- Fix lock staleness detection breaking acquire_twice and timeout tests (#68)
- Fix formatting and clippy warnings across all crates (#66)
- Fix parallel discovery heuristic only counting top-level entries (#38)
- Fix detect_buffered partial-buffer edge case with fill_buf peek (#44)
- Replace TOCTOU exists+read pair with single read in AtomicBatch::commit (#48)
- Fix hardcoded operation index 0 in Matcher::new error context (#57)
- Use schema::is_supported_version in request validation instead of hardcoded check (#54)
- Use schema::CURRENT_VERSION constant instead of hardcoded version strings (#53)
- Fix lock_path_for producing double-dot for extensionless files (#45)
- Bump crate versions for JSON mode backup fix (#21)
- Propagate Config::discover errors instead of silently returning None (#32)
- Fix spurious change recording for no-op Surround and Indent operations (#29)
- Fix silent acceptance of invalid glob patterns in file discovery (#13)
- Fix uses_crlf normalizing mixed line-ending files to all CRLF (#17)
- Fix JSON mode re-reading files per operation instead of composing results (#16)
- Fix --confirm flag applying all changes regardless of per-change user response (#12)
- Fix silent backup failure in JSON mode causing potential data loss (#15)
- Add missing JSON mode tests for undo, config, gitignore, and atomic write test files (#20)
- Fix crosslink cache files tracked in git blocking crate publish (#11)
- Fix Unicode byte-offset mismatch in case-insensitive literal matching (#1)
- Fix non-atomic batch commit in AtomicBatch::commit (#2)
- Fix discovery reading entire files for binary detection (#3)
- Fix silent undo log write failures in save_undo_log (#4)
- Fix silent file read error swallowing in JSON mode (#6)

### Changed
- Consolidate process::exit into single call site in main() (#8)
- Extract shared record_undo() and build_op_options() helpers (#5)
- Replace wasteful matcher.replace() with is_match() in Transform arm (#7)
- Remove unused read_file_with_encoding and read_file_streaming (#10)
- Add test for Transform no-op edge case (#9)

## [0.3.0] - 2026-03-01

### Added
- Bump crate versions to 0.2.4 (#67)
- Extract shared test helpers into common module in ripsed-cli tests (#61)
- Deduplicate shared file-processing logic between file_mode and script_mode (#25)
- Extract engine apply() match arms into dedicated helper functions (#30)
- Remove upward dependency from ripsed-fs to ripsed-core OpOptions (#40)
- Extract repetitive validate_op match arms into shared validation helper (#42)
- Simplify double-negation JSON mode detection logic in main.rs (#27)
- Extract mode resolution from run() to reduce cyclomatic complexity (#24)
- Add test coverage for detect_buffered function (#55)
- Add test coverage for mmap code path in reader.rs (#47)
- Deduplicate WalkBuilder configuration between serial and parallel discovery (#39)
- Eliminate double JSON deserialization in detect_stdin path (#43)
- Replace boolean force_parallel parameter with WalkStrategy enum (#37)
- Add Default derive to DiscoveryOptions to reduce boilerplate (#36)
- Deduplicate default_true helper between operation.rs and config.rs (#56)
- Remove unnecessary rollback wrapper method from AtomicBatch (#49)
- Replace unwrap_or_else serialization fallback with expect in JSON responses (#51)
- Change pub mod to mod for internal modules in ripsed-cli binary crate (#64)
- Replace process::exit in load_config with Result return (#23)
- Remove unused proptest dev-dependency from ripsed-fs (#46)
- Remove dead case_insensitive field from Matcher::Literal variant (#58)
- Remove dead let _ = replace binding in validate_op (#52)
- Run periodic codebase maintenance pass (#19)
- Add missing JSON mode tests for undo, config, gitignore, and atomic write test files (#20)
- New operation: `--transform` — change case of matched text (upper, lower, title, snake_case, camel_case)
- New operation: `--surround PREFIX SUFFIX` — wrap matching lines with prefix and suffix
- New operation: `--indent N` — add N spaces before matching lines
- New operation: `--dedent N` — remove up to N leading spaces from matching lines
- `.rip` script files: chain multiple operations in a file, run with `--script path.rip`
- Script parser with quoted strings, escape sequences, inline comments, and per-operation `--glob` scoping
- 4 libfuzzer fuzz targets (regex input, JSON request, engine, autodetect)
- CI: cargo-semver-checks job (advisory) for API compatibility checking
- Claude Code `/ripsed` skill for AI-assisted bulk find-and-replace
- 172 new tests (495 total across all crates)

### Changed
- `Op` and `TransformMode` enums are now `#[non_exhaustive]` for forward-compatible API evolution

### Fixed
- Fix silent file read error swallowing in JSON mode (#6)
- Fix silent undo log write failures in save_undo_log (#4)
- Fix discovery reading entire files for binary detection (#3)
- Fix non-atomic batch commit in AtomicBatch::commit (#2)
- Fix Unicode byte-offset mismatch in case-insensitive literal matching (#1)
- Two integration tests that ran JSON mode with `dry_run: false` and no `root`, causing ripsed to modify its own source tree during `cargo test`

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
