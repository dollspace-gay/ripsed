---
allowed-tools: Bash(ripsed *), Bash(cat *), Bash(ls *), Write
description: Bulk find-and-replace across multiple files using ripsed. Use instead of repeated Edit calls.
---

## When to use this skill

Use `/ripsed` when the task involves:
- **The same change across many files** — renaming a function, updating an import, changing a config value project-wide
- **Pattern-based text transformations** — uppercasing keywords, wrapping lines, adjusting indentation
- **Multi-step scripted refactors** — several operations in sequence, optionally scoped to different globs

Do NOT use `/ripsed` for:
- Single targeted edits in one file (use Edit tool instead)
- Changes that require understanding surrounding context to get right

## Quick reference

```bash
# Basic replace (recursive from cwd, respects .gitignore)
ripsed FIND REPLACE

# Regex replace with capture groups
ripsed -e 'old_(\w+)' 'new_$1'

# Case-insensitive
ripsed --case-insensitive oldName newName

# Scope to specific files
ripsed --glob '*.rs' FIND REPLACE
ripsed --glob 'src/**/*.ts' FIND REPLACE

# Delete matching lines
ripsed -d 'pattern_to_remove'

# Insert before/after matching lines
ripsed --after 'import React' 'import { useState } from "react";'
ripsed --before 'fn main' '#[allow(unused)]'

# Replace entire matching line
ripsed --replace-line 'version = ".*"' 'version = "2.0.0"' -e

# Transform matched text
ripsed --transform upper 'select|from|where|join' -e
ripsed --transform snake_case 'myFunction'

# Surround matching lines
ripsed --surround '<!-- ' ' -->' 'TODO'

# Indent/dedent matching lines
ripsed --indent 4 'match_pattern'
ripsed --dedent 2 'over_indented'

# Line range (1-indexed, inclusive)
ripsed -n 10:20 FIND REPLACE

# Preview without writing
ripsed --dry-run FIND REPLACE

# Undo last operation
ripsed --undo
```

## User's request

$ARGUMENTS

## Your task

1. **Understand the change**: What pattern needs to match? What's the replacement? Which files?

2. **Choose the right mode**:
   - Simple find/replace: `ripsed FIND REPLACE [--glob PATTERN]`
   - Regex needed: add `-e` flag, use `$1`, `$2` for capture groups
   - Line operations (delete, insert, replace-line, surround, indent): use the corresponding flag
   - Text transforms (case changes): use `--transform MODE`
   - Multiple operations: write a `.rip` script file

3. **Always dry-run first**: Run with `--dry-run` to preview changes before applying. Show the user the diff output.

4. **Apply**: If the dry-run looks correct, run without `--dry-run` to apply.

5. **Verify**: Spot-check a modified file to confirm the change landed correctly.

## .rip script files

For multi-step refactors, write a temporary `.rip` script:

```bash
# Each line is: operation ARGS [FLAGS]
# Comments start with #
# Strings with spaces use quotes

replace "oldName" "newName" --glob "*.rs"
delete "deprecated_function" -e --glob "*.py"
transform "TODO" --mode upper
surround "WARNING" "<!-- " " -->"
indent "nested_block" --amount 4
```

Run with: `ripsed --script /tmp/refactor.rip --dry-run`

## JSON mode (complex cases)

For operations that are hard to express on the command line:

```bash
echo '{
  "version": "1",
  "operations": [
    {"op": "replace", "find": "old", "replace": "new", "regex": true, "glob": "*.rs"},
    {"op": "delete", "find": "^\\s*//\\s*HACK:.*$", "regex": true}
  ],
  "options": {"dry_run": true, "root": "/path/to/project"}
}' | ripsed --json
```

## Constraints

- Always `--dry-run` first for non-trivial changes
- Use `--glob` to scope changes when the pattern might match unintended files
- Regex mode (`-e`) uses Rust regex syntax — `$1` not `\1` for capture groups
- ripsed respects `.gitignore` by default; use `--no-gitignore` to override
- Changes can be undone with `ripsed --undo`
