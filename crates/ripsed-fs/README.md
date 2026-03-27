# ripsed-fs

File system layer for [ripsed](https://github.com/dollspace-gay/ripsed) — a fast, modern stream editor.

This crate handles all file I/O:

- **File discovery** — recursive parallel directory walking with `.gitignore` support and glob filtering
- **Reading** — UTF-8 file reading with memory-mapped I/O for large files and binary detection
- **Atomic writes** — safe file writes via temp file + rename, with batch mode for all-or-nothing semantics
- **Backups** — `.ripsed.bak` file creation with numbered suffixes
- **File locking** — advisory locks with PID-based staleness detection

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0) or [MIT license](http://opensource.org/licenses/MIT) at your option.
