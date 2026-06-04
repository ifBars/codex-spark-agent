use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::Result;
use serde_json::{Value, json};

use crate::profiler::{SPARK_CONTEXT_WINDOW_TOKENS, approx_token_count_from_chars};

pub(super) struct TraceWriter {
    pub(in crate::agent) dir: PathBuf,
    pub(in crate::agent) file_counts: HashMap<String, usize>,
}

pub(super) struct TraceMetadata {
    pub(super) cwd: PathBuf,
    pub(super) model: String,
    pub(super) max_turns: Option<usize>,
    pub(super) compact_after_chars: usize,
    pub(super) compact_after_tool_only_turns: usize,
    pub(super) max_input_chars: usize,
    pub(super) profile: bool,
    pub(super) interactive: bool,
    pub(super) session_name: Option<String>,
    pub(super) new_session: bool,
    pub(super) context: Option<Value>,
}

impl TraceWriter {
    pub(super) fn new(cwd: PathBuf, metadata: TraceMetadata) -> Result<Self> {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis();
        let dir = cwd.join(".spark-runs").join(format!("run-{now_ms}"));
        std::fs::create_dir_all(&dir)?;
        std::fs::write(
            dir.join("000-trace-metadata.json"),
            serde_json::to_vec_pretty(&json!({
                "schema_version": 1,
                "started_at_unix_ms": now_ms,
                "cwd": metadata.cwd,
                "model": metadata.model,
                "max_turns": metadata.max_turns,
                "profile": metadata.profile,
                "interactive": metadata.interactive,
                "session": metadata.session_name,
                "new_session": metadata.new_session,
                "context": metadata.context,
                "compact_after_chars": metadata.compact_after_chars,
                "compact_after_approx_tokens": approx_token_count_from_chars(metadata.compact_after_chars),
                "compact_after_tool_only_turns": metadata.compact_after_tool_only_turns,
                "max_input_chars": metadata.max_input_chars,
                "max_input_approx_tokens": approx_token_count_from_chars(metadata.max_input_chars),
                "context_window_tokens": SPARK_CONTEXT_WINDOW_TOKENS,
            }))?,
        )?;
        eprintln!("trace: {}", dir.display());
        Ok(Self {
            dir,
            file_counts: HashMap::new(),
        })
    }

    pub(super) fn write(&mut self, turn: usize, kind: &str, value: &Value) -> Result<()> {
        let key = format!("{turn:03}-{kind}");
        let count = self.file_counts.entry(key.clone()).or_insert(0);
        *count += 1;
        let filename = if *count == 1 {
            format!("{key}.json")
        } else {
            format!("{key}-{count:03}.json")
        };
        let path = self.dir.join(filename);
        std::fs::write(path, serde_json::to_vec_pretty(value)?)?;
        Ok(())
    }
}
