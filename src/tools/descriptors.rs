use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
}

pub fn builtin_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "fs.read".to_string(),
            description: "Read a bounded UTF-8 line window under the workspace. Use offset/limit to page through larger files instead of reading them all at once.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "offset": {"type": "integer", "minimum": 1},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 400},
                    "line_numbers": {"type": "boolean"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.list".to_string(),
            description: "List files and directories under the workspace.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "recursive": {"type": "boolean"},
                    "max_depth": {"type": "integer", "minimum": 0, "maximum": 8},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200}
                },
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.stat".to_string(),
            description: "Return compact metadata for one workspace path without reading file contents.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"}
                },
                "required": ["path"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.write".to_string(),
            description: "Write a UTF-8 text file under the workspace, creating parent directories if needed.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "content": {"type": "string"}
                },
                "required": ["path", "content"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.search".to_string(),
            description: "Search UTF-8 files under the workspace with ripgrep when available and return bounded matching line snippets. Query is literal by default; set regex=true for regular expressions. Narrow path/query first, then page with more specific searches when needed.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "path": {"type": "string"},
                    "regex": {"type": "boolean"},
                    "case_sensitive": {"type": "boolean"},
                    "max_depth": {"type": "integer", "minimum": 0, "maximum": 12},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100},
                    "context_lines": {"type": "integer", "minimum": 0, "maximum": 3}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.replace".to_string(),
            description: "Replace exact UTF-8 text in one workspace file. Optionally require an expected replacement count.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "old": {"type": "string"},
                    "new": {"type": "string"},
                    "expected_replacements": {"type": "integer", "minimum": 1}
                },
                "required": ["path", "old", "new"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.edit".to_string(),
            description: "Edit one UTF-8 file by replacing an inclusive 1-based line range. Use end_line one less than start_line to insert before start_line.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "start_line": {"type": "integer", "minimum": 1},
                    "end_line": {"type": "integer", "minimum": 0},
                    "replacement": {"type": "string"},
                    "expected_old": {"type": "string"}
                },
                "required": ["path", "start_line", "end_line", "replacement"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "fs.rename".to_string(),
            description: "Rename or move one file or directory inside the workspace. Refuses to overwrite an existing destination.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "from": {"type": "string"},
                    "to": {"type": "string"}
                },
                "required": ["from", "to"],
                "additionalProperties": false
            }),
        },
        ToolDescriptor {
            name: "cmd.exec".to_string(),
            description: "Execute a shell command in the workspace. Use PowerShell-compatible commands on Windows.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "workdir": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 120000}
                },
                "additionalProperties": false
            }),
        },
    ]
}
