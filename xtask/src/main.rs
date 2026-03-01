use serde_json::json;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(|s| s.as_str()) {
        Some("gen-schema") => gen_schema(),
        Some("gen-fixtures") => gen_fixtures(),
        Some("bench") => bench(),
        _ => {
            eprintln!("Usage: cargo xtask <gen-schema|gen-fixtures|bench>");
            std::process::exit(1);
        }
    }
}

fn gen_schema() {
    // Build a JSON description of the JsonRequest schema based on the actual
    // types in ripsed-json and ripsed-core.  We construct a hand-written
    // schema object that mirrors the Rust structs (JsonRequest, Op, OpOptions,
    // etc.) rather than pulling in a full JSON-Schema derive crate.

    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "title": "RipsedJsonRequest",
        "description": "Schema for the ripsed JSON request protocol (version 1).",
        "type": "object",
        "properties": {
            "version": {
                "type": "string",
                "description": "Protocol version. Currently only \"1\" is supported.",
                "default": "1",
                "enum": ["1"]
            },
            "operations": {
                "type": "array",
                "description": "List of operations to apply.",
                "items": {
                    "$ref": "#/$defs/JsonOp"
                }
            },
            "options": {
                "$ref": "#/$defs/OpOptions"
            },
            "undo": {
                "$ref": "#/$defs/UndoRequest"
            }
        },
        "additionalProperties": true,
        "$defs": {
            "JsonOp": {
                "description": "A single operation with an optional per-operation glob.",
                "type": "object",
                "required": ["op", "find"],
                "properties": {
                    "op": {
                        "type": "string",
                        "enum": ["replace", "delete", "insert_after", "insert_before", "replace_line"],
                        "description": "The operation type."
                    },
                    "find": {
                        "type": "string",
                        "description": "The pattern to search for (literal or regex).",
                        "minLength": 1
                    },
                    "replace": {
                        "type": "string",
                        "description": "Replacement text (required for 'replace' op)."
                    },
                    "content": {
                        "type": "string",
                        "description": "Content to insert or replace the line with (required for insert_after, insert_before, replace_line).",
                        "minLength": 1
                    },
                    "regex": {
                        "type": "boolean",
                        "default": false,
                        "description": "Whether 'find' is a regex pattern."
                    },
                    "case_insensitive": {
                        "type": "boolean",
                        "default": false,
                        "description": "Whether matching is case-insensitive."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Per-operation file glob (overrides options.glob)."
                    }
                }
            },
            "OpOptions": {
                "description": "Options that control how operations are applied.",
                "type": "object",
                "properties": {
                    "dry_run": {
                        "type": "boolean",
                        "default": true,
                        "description": "Preview changes without writing."
                    },
                    "root": {
                        "type": "string",
                        "description": "Root directory to operate in."
                    },
                    "gitignore": {
                        "type": "boolean",
                        "default": true,
                        "description": "Respect .gitignore rules."
                    },
                    "backup": {
                        "type": "boolean",
                        "default": false,
                        "description": "Create .bak backup files."
                    },
                    "atomic": {
                        "type": "boolean",
                        "default": false,
                        "description": "Atomic batch mode: all-or-nothing writes."
                    },
                    "glob": {
                        "type": "string",
                        "description": "Global file glob pattern."
                    },
                    "ignore": {
                        "type": "string",
                        "description": "Glob pattern for files to ignore."
                    },
                    "hidden": {
                        "type": "boolean",
                        "default": false,
                        "description": "Include hidden files."
                    },
                    "max_depth": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Maximum directory traversal depth."
                    },
                    "line_range": {
                        "$ref": "#/$defs/LineRange"
                    }
                }
            },
            "LineRange": {
                "description": "A range of lines to operate on (1-indexed, inclusive).",
                "type": "object",
                "required": ["start"],
                "properties": {
                    "start": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Start line (1-indexed, inclusive)."
                    },
                    "end": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "End line (1-indexed, inclusive). If omitted, extends to end of file."
                    }
                }
            },
            "UndoRequest": {
                "description": "Request to undo the last N operations.",
                "type": "object",
                "required": ["last"],
                "properties": {
                    "last": {
                        "type": "integer",
                        "minimum": 1,
                        "description": "Number of operations to undo."
                    }
                }
            }
        }
    });

    // Verify the schema round-trips through our own parser by checking it's
    // valid JSON (it always will be since we built it with serde_json, but
    // this also exercises the ripsed-json crate link).
    let _: ripsed_json::request::JsonRequest = serde_json::from_value(json!({
        "operations": [{"op": "replace", "find": "a", "replace": "b"}]
    }))
    .expect("sanity: example request should parse");

    println!(
        "{}",
        serde_json::to_string_pretty(&schema).expect("schema serialization should not fail")
    );
}

fn gen_fixtures() {
    eprintln!("gen-fixtures: not yet implemented (planned for v0.2)");
    std::process::exit(1);
}

fn bench() {
    let status = std::process::Command::new("cargo")
        .args(["bench", "--workspace"])
        .status()
        .expect("failed to execute cargo bench");

    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
}
