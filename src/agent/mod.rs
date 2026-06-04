use std::collections::HashMap;
use std::path::PathBuf;

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::auth::{self, AuthTokens};
use crate::client::SparkClient;
use crate::config;
use crate::profiler::AgentProfiler;

pub(in crate::agent) mod cache;
pub(in crate::agent) mod compaction;
mod run_loop;
pub(in crate::agent) mod trace;

#[cfg(test)]
mod tests;

use cache::CachedToolObservation;
use compaction::context_pressure_json;
use trace::{TraceMetadata, TraceWriter};

const AGENT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub(super) const TOOL_ONLY_STREAK_COMPACTION_TRIGGER: &str = "tool_only_streak";

pub struct AgentRunner {
    pub(in crate::agent) client: SparkClient,
    pub(in crate::agent) cwd: PathBuf,
    pub(in crate::agent) input: Vec<Value>,
    pub(in crate::agent) max_turns: Option<usize>,
    pub(in crate::agent) trace: Option<TraceWriter>,
    pub(in crate::agent) compact_after_chars: usize,
    pub(in crate::agent) compact_after_tool_only_turns: usize,
    pub(in crate::agent) max_input_chars: usize,
    pub(in crate::agent) request_seq: usize,
    pub(in crate::agent) profile: bool,
    pub(in crate::agent) profiler: AgentProfiler,
    pub(in crate::agent) readonly_tool_cache: HashMap<String, CachedToolObservation>,
    pub(in crate::agent) loaded_skills: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSnapshot {
    #[serde(default = "default_agent_snapshot_schema_version")]
    pub schema_version: u32,
    pub input: Vec<Value>,
    pub request_seq: usize,
    pub profiler: AgentProfiler,
    #[serde(default)]
    pub loaded_skills: Vec<String>,
}

impl AgentRunner {
    pub fn new(
        mut auth_tokens: AuthTokens,
        cwd: PathBuf,
        model: String,
        max_turns: Option<usize>,
        trace: bool,
        profile: bool,
        compact_after_chars: usize,
        compact_after_tool_only_turns: usize,
        max_input_chars: usize,
        interactive: bool,
        session_name: Option<String>,
        new_session: bool,
        trace_context: Option<Value>,
    ) -> Result<Self> {
        if auth::is_expired(&auth_tokens) {
            println!("Refreshing ChatGPT token...");
            auth_tokens = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(auth::refresh(&auth_tokens))
            })?;
            config::save_auth(&auth_tokens)?;
        }

        let trace_metadata = TraceMetadata {
            cwd: cwd.clone(),
            model: model.clone(),
            max_turns,
            compact_after_chars,
            compact_after_tool_only_turns,
            max_input_chars,
            profile,
            interactive,
            session_name,
            new_session,
            context: trace_context,
        };

        Ok(Self {
            client: SparkClient::new(auth_tokens, model),
            cwd: cwd.clone(),
            input: Vec::new(),
            max_turns,
            trace: if trace {
                Some(TraceWriter::new(cwd, trace_metadata)?)
            } else {
                None
            },
            compact_after_chars,
            compact_after_tool_only_turns,
            max_input_chars,
            request_seq: 0,
            profile,
            profiler: AgentProfiler::default(),
            readonly_tool_cache: HashMap::new(),
            loaded_skills: Vec::new(),
        })
    }

    pub async fn run(&mut self, prompt: &str) -> Result<()> {
        self.push_user_message(prompt);
        self.run_until_idle().await
    }

    pub fn clear_conversation(&mut self) {
        self.input.clear();
        self.request_seq = 0;
        self.profiler = AgentProfiler::default();
        self.readonly_tool_cache.clear();
        self.loaded_skills.clear();
    }

    pub fn input_chars(&self) -> Result<usize> {
        Ok(serde_json::to_string(&self.input)?.len())
    }

    pub fn profile_status(&self) -> String {
        let live =
            context_pressure_json(&self.input, self.compact_after_chars, self.max_input_chars)
                .unwrap_or_else(|error| json!({"error": error.to_string()}));
        let live_input_chars = live
            .get("input_chars")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let live_approx_tokens = live
            .get("approx_input_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default();
        let live_context_pct = live
            .get("context_window_pct")
            .and_then(Value::as_f64)
            .unwrap_or_default();
        format!(
            "{}; live_input_chars={}, live_approx_input_tokens={} ({:.1}% of 128k), compact_after_chars={}, compact_after_tool_only_turns={}, max_input_chars={}",
            self.profiler.status_line(),
            live_input_chars,
            live_approx_tokens,
            live_context_pct,
            self.compact_after_chars,
            self.compact_after_tool_only_turns,
            self.max_input_chars
        )
    }

    pub fn profile_summary(&self) -> Value {
        let mut summary = self.profiler.to_json();
        if let Some(object) = summary.as_object_mut() {
            object.insert(
                "live_context".to_string(),
                context_pressure_json(&self.input, self.compact_after_chars, self.max_input_chars)
                    .unwrap_or_else(|error| json!({"error": error.to_string()})),
            );
        }
        summary
    }

    pub fn snapshot(&self) -> AgentSnapshot {
        AgentSnapshot {
            schema_version: AGENT_SNAPSHOT_SCHEMA_VERSION,
            input: self.input.clone(),
            request_seq: self.request_seq,
            profiler: self.profiler.clone(),
            loaded_skills: self.loaded_skills.clone(),
        }
    }

    pub fn restore_snapshot(&mut self, snapshot: AgentSnapshot) {
        self.input = snapshot.input;
        self.request_seq = snapshot.request_seq;
        self.profiler = snapshot.profiler;
        self.loaded_skills = snapshot.loaded_skills;
        self.readonly_tool_cache.clear();
    }

    pub fn loaded_skills(&self) -> &[String] {
        &self.loaded_skills
    }

    pub fn load_skill_context(&mut self, name: &str, summary: &str) -> bool {
        if self.loaded_skills.iter().any(|loaded| loaded == name) {
            return false;
        }
        self.input.push(json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": format!("[spark skill loaded: {name}]\n{summary}")
            }]
        }));
        self.loaded_skills.push(name.to_string());
        true
    }

    pub async fn compile_skill_summary(&self, name: &str, raw_skill: &str) -> Result<String> {
        self.client.compile_skill_summary(name, raw_skill).await
    }

    pub fn load_session(&mut self, path: &std::path::Path) -> Result<bool> {
        if !path.exists() {
            return Ok(false);
        }
        let body = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read session {}", path.display()))?;
        let snapshot = serde_json::from_str::<AgentSnapshot>(&body)
            .with_context(|| format!("failed to parse session {}", path.display()))?;
        self.restore_snapshot(snapshot);
        Ok(true)
    }

    pub fn save_session(&self, path: &std::path::Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        std::fs::write(path, serde_json::to_vec_pretty(&self.snapshot())?)
            .with_context(|| format!("failed to write session {}", path.display()))?;
        Ok(())
    }
}

fn default_agent_snapshot_schema_version() -> u32 {
    AGENT_SNAPSHOT_SCHEMA_VERSION
}
