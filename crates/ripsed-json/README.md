# ripsed-json

Agent/JSON interface for [ripsed](https://github.com/dollspace-gay/ripsed) — a fast, modern stream editor.

This crate provides the structured JSON protocol for AI coding agents, editor plugins, and automation pipelines:

- **Request parsing** — versioned JSON request schema with validation and helpful error messages
- **Response building** — structured JSON responses with per-file diffs, change counts, and error details
- **Auto-detection** — determine whether stdin contains JSON or plain text for seamless mode switching
- **Undo protocol** — JSON interface for undo operations

## Example request

```json
{
  "version": "1",
  "operations": [
    {
      "op": "replace",
      "find": "old_function",
      "replace": "new_function",
      "glob": "src/**/*.rs"
    }
  ],
  "options": {
    "dry_run": true,
    "root": "./my-project"
  }
}
```

## License

Licensed under either of [Apache License, Version 2.0](http://www.apache.org/licenses/LICENSE-2.0) or [MIT license](http://opensource.org/licenses/MIT) at your option.
