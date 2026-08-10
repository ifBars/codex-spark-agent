use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolDescriptor {
    pub name: String,
    pub description: String,
    pub input_schema: Value,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosted_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hosted_config: Option<Value>,
}

pub fn builtin_tools() -> Vec<ToolDescriptor> {
    vec![
        ToolDescriptor {
            name: "fs.read".to_string(),
            description: "Read a bounded UTF-8 line window from a regular text file up to 1 MiB. Larger, binary, or invalid text files fail quickly; use fs.stat and a narrower source file instead.".to_string(),
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
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "fs.list".to_string(),
            description: "List files and directories under the workspace with bounded depth, directory count, and a short traversal deadline.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "path": {"type": "string"},
                    "recursive": {"type": "boolean"},
                    "max_depth": {"type": "integer", "minimum": 0, "maximum": 6},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200}
                },
                "additionalProperties": false
            }),
            hosted_type: None,
            hosted_config: None,
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
            hosted_type: None,
            hosted_config: None,
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
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "fs.search".to_string(),
            description: "Search bounded text files under the workspace with ripgrep when available. Searches use strict file/depth/result budgets and a short deadline; narrow path/query first. Query is literal by default; set regex=true for regular expressions.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string"},
                    "path": {"type": "string"},
                    "regex": {"type": "boolean"},
                    "case_sensitive": {"type": "boolean"},
                    "max_depth": {"type": "integer", "minimum": 0, "maximum": 6},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100},
                    "context_lines": {"type": "integer", "minimum": 0, "maximum": 3}
                },
                "required": ["query"],
                "additionalProperties": false
            }),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "fs.replace".to_string(),
            description: "Replace UTF-8 text in one workspace file. Matches exact text first, then safe line-ending or leading-indent equivalent blocks when unambiguous. Optionally require an expected replacement count.".to_string(),
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
            hosted_type: None,
            hosted_config: None,
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
            hosted_type: None,
            hosted_config: None,
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
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "cmd.exec".to_string(),
            description: "Execute a shell command in the workspace. On Windows this runs through powershell -NoProfile -Command; do not use && because Windows PowerShell 5.1 rejects it. Run dependent commands as separate cmd.exec calls or use PowerShell-compatible control flow.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "command": {"type": "string"},
                    "workdir": {"type": "string"},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 30000}
                },
                "additionalProperties": false
            }),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "gh.read".to_string(),
            description: "Run a bounded, non-interactive, read-only GitHub CLI operation in the current workspace. Pass the arguments that follow `gh`, for example [\"pr\",\"view\",\"42\",\"--json\",\"title,files,reviews,statusCheckRollup\",\"--repo\",\"owner/repo\"]. Prefer this over web.search for identifiable GitHub repositories, pull requests, issues, Actions runs, workflows, and releases. Mutating verbs, GraphQL or non-GET API calls, browser-opening flags, and token output are rejected.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "args": {
                        "type": "array",
                        "items": {"type": "string"},
                        "minItems": 1,
                        "maxItems": 48
                    },
                    "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 30000}
                },
                "required": ["args"],
                "additionalProperties": false
            }),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "browser.run".to_string(),
            description: "Run a stateless Playwright Chromium browser pass from the workspace. Opens a URL, optionally performs simple CSS-selector actions, and returns bounded page text, ARIA snapshot, console/page errors, status, final URL, and an optional screenshot path. Use for local web UI smoke checks and browser-backed inspection; prefer cmd.exec for arbitrary scripts.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "url": {"type": "string"},
                    "actions": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "type": {
                                    "type": "string",
                                    "enum": ["click", "fill", "select", "press", "wait"]
                                },
                                "selector": {"type": "string"},
                                "value": {
                                    "description": "Text for fill, or option value for select.",
                                    "type": "string"
                                },
                                "key": {"type": "string"},
                                "ms": {"type": "integer", "minimum": 0, "maximum": 30000},
                                "timeout_ms": {"type": "integer", "minimum": 100, "maximum": 60000}
                            },
                            "required": ["type"],
                            "additionalProperties": false
                        }
                    },
                    "capture_screenshot": {"type": "boolean"},
                    "screenshot_path": {"type": "string"},
                    "headless": {"type": "boolean"},
                    "viewport_width": {"type": "integer", "minimum": 320, "maximum": 3840},
                    "viewport_height": {"type": "integer", "minimum": 240, "maximum": 2160},
                    "wait_until": {
                        "type": "string",
                        "enum": ["load", "domcontentloaded", "networkidle", "commit"]
                    },
                    "text_limit": {"type": "integer", "minimum": 0, "maximum": 40000},
                    "timeout_ms": {"type": "integer", "minimum": 1000, "maximum": 30000}
                },
                "required": ["url"],
                "additionalProperties": false
            }),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "web.search".to_string(),
            description: "Search the public web through OpenAI's hosted web search tool for current information. Use this when local repo files are insufficient or the user asks for latest/current external facts; cite sources in the final answer.".to_string(),
            input_schema: json!({}),
            hosted_type: Some("web_search".to_string()),
            hosted_config: Some(json!({
                "search_context_size": "medium"
            })),
        },
        ToolDescriptor {
            name: "subagent.run".to_string(),
            description: "Run one isolated helper to completion. For concurrent work prefer subagent.spawn then subagent.wait. Helpers default to read-only; a work helper requires explicit ownership and inherits the parent model, mode, memory, and context limits unless overridden. Explore uses the parent Spark model; research, review, and plan use gpt-5.6-luna by default, overridable with SPARK_ADVANCED_SUBAGENT_MODEL.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["explore", "research", "review", "plan"]
                    },
                    "task": {"type": "string"},
                    "model": {
                        "type": "string",
                        "description": "Optional model override. Use parent to inherit the main thread model, or pass a concrete model such as gpt-5.6-luna."
                    },
                    "reasoning_effort": {
                        "type": "string",
                        "enum": ["low", "medium", "high", "xhigh"]
                    },
                    "mode": {
                        "type": "string",
                        "enum": ["ask", "work"],
                        "description": "Defaults to ask. work requires a work-mode parent plus ownership and permits only bounded native fs writes inside those paths."
                    },
                    "ownership": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "Relative workspace paths exclusively owned by a work helper. Required for mode=work."
                    }
                },
                "required": ["kind", "task"],
                "additionalProperties": false
            }),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "subagent.spawn".to_string(),
            description: "Start an isolated helper without blocking the parent. Use only for independent, concrete work; default workers are read-only. The harness caps concurrent workers at SPARK_SUBAGENT_MAX_CONCURRENCY (default 3, maximum 8). Follow with subagent.wait to receive a compact brief.".to_string(),
            input_schema: json!({
                "type": "object",
                "properties": {
                    "kind": {"type": "string", "enum": ["explore", "research", "review", "plan"]},
                    "task": {"type": "string"},
                    "model": {"type": "string"},
                    "reasoning_effort": {"type": "string", "enum": ["low", "medium", "high", "xhigh"]},
                    "mode": {"type": "string", "enum": ["ask", "work"]},
                    "ownership": {"type": "array", "items": {"type": "string"}}
                },
                "required": ["kind", "task"],
                "additionalProperties": false
            }),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "subagent.wait".to_string(),
            description: "Wait for one spawned helper and return its compact report with worker profile data. This records a parent trace lifecycle event and attaches only the compact brief to the parent context.".to_string(),
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}}, "required": ["id"], "additionalProperties": false}),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "subagent.followup".to_string(),
            description: "Start a follow-up worker from a completed helper's compact brief. Follow-ups require the prior worker to be completed; use a focused new task and wait on the returned worker id.".to_string(),
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}, "task": {"type": "string"}}, "required": ["id", "task"], "additionalProperties": false}),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "subagent.steer".to_string(),
            description: "Change a worker's direction. For a completed worker this starts a follow-up from its compact brief. For a running worker the harness cancels it and starts a replacement with the same role, model, reasoning, mode, and ownership plus the new direction.".to_string(),
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}, "task": {"type": "string"}}, "required": ["id", "task"], "additionalProperties": false}),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "subagent.cancel".to_string(),
            description: "Cancel one running helper by id, or omit id to cancel every running helper. Cancellation is cooperative at loop boundaries and the harness also aborts the local worker task.".to_string(),
            input_schema: json!({"type": "object", "properties": {"id": {"type": "string"}}, "additionalProperties": false}),
            hosted_type: None,
            hosted_config: None,
        },
        ToolDescriptor {
            name: "subagent.list".to_string(),
            description: "List worker ids, states, role/model/reasoning settings, ownership, and the active concurrency limit. Use this before waiting, following up, or cancelling.".to_string(),
            input_schema: json!({"type": "object", "properties": {}, "additionalProperties": false}),
            hosted_type: None,
            hosted_config: None,
        },
    ]
}
