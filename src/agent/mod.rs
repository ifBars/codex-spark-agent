use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::auth::{self, AuthTokens};
use crate::client::SparkClient;
use crate::config;
use crate::mcp::McpRegistry;
use crate::profiler::AgentProfiler;
use crate::session::store::SessionStore;
use crate::tools::AgentMode;

pub(in crate::agent) mod cache;
pub(in crate::agent) mod compaction;
mod goal;
mod run_loop;
mod subagent;
mod team;
pub(in crate::agent) mod trace;

#[cfg(test)]
mod tests;

use cache::CachedToolObservation;
use compaction::context_pressure_json;
use goal::GoalState;
pub(crate) use subagent::{SubagentKind, SubagentReport, SubagentRunOptions, report_prompt};
use team::SubagentTeam;
use trace::{TraceMetadata, TraceWriter};

const AGENT_SNAPSHOT_SCHEMA_VERSION: u32 = 1;
pub(super) const TOOL_ONLY_STREAK_COMPACTION_TRIGGER: &str = "tool_only_streak";

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum AgentDisplayEvent {
    RequestStart {
        turn: usize,
        input_chars: usize,
    },
    Assistant(String),
    AssistantDelta(String),
    ResponseComplete {
        duration_ms: u64,
        output_tokens: Option<u64>,
        time_to_first_token_ms: Option<u64>,
        average_tokens_per_second: Option<f64>,
    },
    ReasoningStart,
    ReasoningSummary(String),
    ReasoningFinish,
    CompactionStart {
        trigger: Option<String>,
        input_chars: usize,
    },
    CompactionFinish {
        notice: String,
    },
    ToolBatchStart {
        count: usize,
    },
    ToolCall {
        name: String,
        args: String,
    },
    ToolResult {
        name: String,
        ok: bool,
        duration_ms: u64,
        output_chars: usize,
        error: Option<String>,
    },
    ConnectionRetry {
        attempt: u64,
        max_attempts: u64,
        delay_ms: u64,
        error: String,
    },
    ConnectionRecovered {
        attempts: u64,
    },
    TransportFallback {
        from: &'static str,
        to: &'static str,
        error: String,
    },
    System(String),
    Warning(String),
    Profile(String),
}

#[derive(Debug, Clone, Default)]
pub(in crate::agent) enum AgentDisplay {
    #[default]
    Plain,
    Markdown,
    Buffered(Vec<AgentDisplayEvent>),
    Shared(SharedDisplayEvents),
}

pub(crate) type SharedDisplayEvents = Arc<Mutex<Vec<AgentDisplayEvent>>>;

pub struct AgentRunner {
    pub(in crate::agent) client: SparkClient,
    pub(in crate::agent) cwd: PathBuf,
    pub(in crate::agent) read_roots: Vec<PathBuf>,
    pub(in crate::agent) input: Vec<Value>,
    pub(in crate::agent) trace: Option<TraceWriter>,
    pub(in crate::agent) compact_after_chars: usize,
    pub(in crate::agent) compact_after_tool_only_turns: usize,
    pub(in crate::agent) max_input_chars: usize,
    pub(in crate::agent) request_seq: usize,
    pub(in crate::agent) profile: bool,
    pub(in crate::agent) display: AgentDisplay,
    pub(in crate::agent) profiler: AgentProfiler,
    pub(in crate::agent) readonly_tool_cache: HashMap<String, CachedToolObservation>,
    pub(in crate::agent) loaded_skills: Vec<String>,
    pub(in crate::agent) mode: AgentMode,
    pub(in crate::agent) goal: Option<GoalState>,
    pub(in crate::agent) memory_enabled: bool,
    pub(in crate::agent) subagent_depth: usize,
    pub(in crate::agent) subagent_team: SubagentTeam,
    /// Present only for explicitly delegated write workers. This is a harness
    /// guard, not an OS sandbox: it limits native file mutations and disables
    /// shell/browser/MCP execution for that worker.
    pub(in crate::agent) delegated_write_ownership: Option<Vec<String>>,
    pub(in crate::agent) mcp_registry: Option<McpRegistry>,
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
    #[serde(default = "default_agent_mode")]
    pub mode: AgentMode,
    #[serde(default = "default_reasoning_effort")]
    pub reasoning_effort: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub goal: Option<GoalState>,
    #[serde(default)]
    pub memory_enabled: bool,
}

impl AgentRunner {
    pub fn new(
        auth_tokens: AuthTokens,
        cwd: PathBuf,
        model: String,
        trace: bool,
        profile: bool,
        compact_after_chars: usize,
        compact_after_tool_only_turns: usize,
        max_input_chars: usize,
        interactive: bool,
        session_name: Option<String>,
        new_session: bool,
        trace_context: Option<Value>,
        mode: AgentMode,
    ) -> Result<Self> {
        Self::new_with_reasoning_effort(
            auth_tokens,
            cwd,
            model,
            crate::client::DEFAULT_SPARK_AGENT_REASONING_EFFORT.to_string(),
            trace,
            profile,
            compact_after_chars,
            compact_after_tool_only_turns,
            max_input_chars,
            interactive,
            session_name,
            new_session,
            trace_context,
            mode,
        )
    }

    pub fn new_with_reasoning_effort(
        mut auth_tokens: AuthTokens,
        cwd: PathBuf,
        model: String,
        reasoning_effort: String,
        trace: bool,
        profile: bool,
        compact_after_chars: usize,
        compact_after_tool_only_turns: usize,
        max_input_chars: usize,
        interactive: bool,
        session_name: Option<String>,
        new_session: bool,
        trace_context: Option<Value>,
        mode: AgentMode,
    ) -> Result<Self> {
        if auth::is_expired(&auth_tokens) {
            eprintln!("Refreshing ChatGPT token...");
            auth_tokens = tokio::task::block_in_place(|| {
                tokio::runtime::Handle::current().block_on(auth::refresh(&auth_tokens))
            })?;
            config::save_auth(&auth_tokens)?;
        }

        let trace_metadata = TraceMetadata {
            cwd: cwd.clone(),
            model: model.clone(),
            compact_after_chars,
            compact_after_tool_only_turns,
            max_input_chars,
            profile,
            interactive,
            session_name,
            new_session,
            context: trace_context,
            mode,
        };

        Ok(Self {
            client: SparkClient::new_with_reasoning_effort(auth_tokens, model, reasoning_effort),
            cwd: cwd.clone(),
            read_roots: Vec::new(),
            input: Vec::new(),
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
            display: if interactive {
                AgentDisplay::Markdown
            } else {
                AgentDisplay::Plain
            },
            profiler: AgentProfiler::default(),
            readonly_tool_cache: HashMap::new(),
            loaded_skills: Vec::new(),
            mode,
            goal: None,
            memory_enabled: false,
            subagent_depth: 0,
            subagent_team: SubagentTeam::from_environment(),
            delegated_write_ownership: None,
            mcp_registry: None,
        })
    }

    pub async fn run(&mut self, prompt: &str) -> Result<()> {
        self.run_with_cancel(prompt, CancellationToken::new()).await
    }

    pub async fn run_with_cancel(
        &mut self,
        prompt: &str,
        cancellation: CancellationToken,
    ) -> Result<()> {
        self.run_with_cancel_to_text(prompt, cancellation)
            .await
            .map(|_| ())
    }

    pub(crate) async fn run_with_cancel_to_text(
        &mut self,
        prompt: &str,
        cancellation: CancellationToken,
    ) -> Result<String> {
        self.refresh_memory_context()?;
        self.push_user_message(prompt);
        let assistant_text = self.run_until_idle(cancellation).await?;
        self.record_memory_exchange(prompt, &assistant_text)?;
        Ok(assistant_text)
    }

    pub(crate) fn use_buffered_display(&mut self) {
        self.display = AgentDisplay::Buffered(Vec::new());
    }

    pub(crate) fn use_shared_display(&mut self) -> SharedDisplayEvents {
        let events = Arc::new(Mutex::new(Vec::new()));
        self.display = AgentDisplay::Shared(Arc::clone(&events));
        events
    }

    pub(crate) fn take_display_events(&mut self) -> Vec<AgentDisplayEvent> {
        match &mut self.display {
            AgentDisplay::Buffered(events) => std::mem::take(events),
            AgentDisplay::Shared(events) => take_shared_display_events(events),
            AgentDisplay::Plain | AgentDisplay::Markdown => Vec::new(),
        }
    }

    pub(crate) fn emit_assistant_message(&mut self, text: &str) {
        match &mut self.display {
            AgentDisplay::Plain => println!("{text}"),
            AgentDisplay::Markdown => crate::chat::markdown::print_assistant_message(text),
            AgentDisplay::Buffered(events) => {
                events.push(AgentDisplayEvent::Assistant(text.to_string()));
            }
            AgentDisplay::Shared(events) => {
                push_shared_display_event(events, AgentDisplayEvent::Assistant(text.to_string()))
            }
        }
    }

    pub(crate) fn emit_assistant_delta(&mut self, text: &str) {
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {
                print!("{text}");
                let _ = std::io::Write::flush(&mut std::io::stdout());
            }
            AgentDisplay::Buffered(events) => {
                events.push(AgentDisplayEvent::AssistantDelta(text.to_string()));
            }
            AgentDisplay::Shared(events) => push_shared_display_event(
                events,
                AgentDisplayEvent::AssistantDelta(text.to_string()),
            ),
        }
    }

    pub(crate) fn emit_response_complete(
        &mut self,
        duration_ms: u64,
        output_tokens: Option<u64>,
        time_to_first_token_ms: Option<u64>,
        average_tokens_per_second: Option<f64>,
    ) {
        let event = AgentDisplayEvent::ResponseComplete {
            duration_ms,
            output_tokens,
            time_to_first_token_ms,
            average_tokens_per_second,
        };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {}
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_reasoning_start(&mut self) {
        let event = AgentDisplayEvent::ReasoningStart;
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {}
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_reasoning_summary(&mut self, text: &str) {
        let text = text.trim();
        if text.is_empty() {
            return;
        }
        let event = AgentDisplayEvent::ReasoningSummary(text.to_string());
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {}
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_reasoning_finish(&mut self) {
        let event = AgentDisplayEvent::ReasoningFinish;
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {}
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_request_start(&mut self, turn: usize, input_chars: usize) {
        let event = AgentDisplayEvent::RequestStart { turn, input_chars };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {}
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_compaction_start(
        &mut self,
        trigger: Option<&'static str>,
        input_chars: usize,
    ) {
        let event = AgentDisplayEvent::CompactionStart {
            trigger: trigger.map(str::to_string),
            input_chars,
        };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {
                let trigger = trigger.unwrap_or("manual");
                eprintln!("compaction starting: trigger={trigger} input={input_chars} chars");
            }
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_compaction_finish(&mut self, notice: impl Into<String>) {
        let notice = notice.into();
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => eprintln!("{notice}"),
            AgentDisplay::Buffered(events) => {
                events.push(AgentDisplayEvent::CompactionFinish { notice });
            }
            AgentDisplay::Shared(events) => {
                push_shared_display_event(events, AgentDisplayEvent::CompactionFinish { notice });
            }
        }
    }

    pub(crate) fn emit_tool_batch_start(&mut self, count: usize) {
        let event = AgentDisplayEvent::ToolBatchStart { count };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {}
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_tool_call(&mut self, name: &str, args: &str) {
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => println!("\n> {name} {args}"),
            AgentDisplay::Buffered(events) => {
                events.push(AgentDisplayEvent::ToolCall {
                    name: name.to_string(),
                    args: args.to_string(),
                });
            }
            AgentDisplay::Shared(events) => push_shared_display_event(
                events,
                AgentDisplayEvent::ToolCall {
                    name: name.to_string(),
                    args: args.to_string(),
                },
            ),
        }
    }

    pub(crate) fn emit_tool_result(
        &mut self,
        name: &str,
        ok: bool,
        duration_ms: u64,
        output_chars: usize,
        error: Option<&str>,
    ) {
        let event = AgentDisplayEvent::ToolResult {
            name: name.to_string(),
            ok,
            duration_ms,
            output_chars,
            error: error.map(str::to_string),
        };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {
                let status = if ok { "ok" } else { "failed" };
                println!("  {status} {duration_ms}ms {output_chars} chars");
            }
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_system_message(&mut self, text: impl Into<String>) {
        let text = text.into();
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => println!("{text}"),
            AgentDisplay::Buffered(events) => events.push(AgentDisplayEvent::System(text)),
            AgentDisplay::Shared(events) => {
                push_shared_display_event(events, AgentDisplayEvent::System(text));
            }
        }
    }

    pub(crate) fn emit_warning(&mut self, text: impl Into<String>) {
        let text = text.into();
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => eprintln!("{text}"),
            AgentDisplay::Buffered(events) => events.push(AgentDisplayEvent::Warning(text)),
            AgentDisplay::Shared(events) => {
                push_shared_display_event(events, AgentDisplayEvent::Warning(text));
            }
        }
    }

    pub(crate) fn emit_connection_retry(
        &mut self,
        attempt: u64,
        max_attempts: u64,
        delay_ms: u64,
        error: impl Into<String>,
    ) {
        let event = AgentDisplayEvent::ConnectionRetry {
            attempt,
            max_attempts,
            delay_ms,
            error: error.into(),
        };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => eprintln!(
                "connection interrupted; retrying {attempt}/{max_attempts} in {delay_ms}ms"
            ),
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_connection_recovered(&mut self, attempts: u64) {
        let event = AgentDisplayEvent::ConnectionRecovered { attempts };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {
                eprintln!("connection recovered after {attempts} retries")
            }
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_transport_fallback(
        &mut self,
        from: &'static str,
        to: &'static str,
        error: impl Into<String>,
    ) {
        let error = error.into();
        let event = AgentDisplayEvent::TransportFallback {
            from,
            to,
            error: error.clone(),
        };
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => {
                eprintln!("falling back from {from} to {to}: {error}")
            }
            AgentDisplay::Buffered(events) => events.push(event),
            AgentDisplay::Shared(events) => push_shared_display_event(events, event),
        }
    }

    pub(crate) fn emit_profile_message(&mut self, text: impl Into<String>) {
        let text = text.into();
        match &mut self.display {
            AgentDisplay::Plain | AgentDisplay::Markdown => println!("{text}"),
            AgentDisplay::Buffered(events) => events.push(AgentDisplayEvent::Profile(text)),
            AgentDisplay::Shared(events) => {
                push_shared_display_event(events, AgentDisplayEvent::Profile(text));
            }
        }
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
            "{}; mode={}, reasoning={}, memory={}, live_input_chars={}, live_approx_input_tokens={} ({:.1}% of 128k), compact_after_chars={}, compact_after_tool_only_turns={}, max_input_chars={}",
            self.profiler.status_line(),
            self.mode.name(),
            self.reasoning_effort(),
            if self.memory_enabled() { "on" } else { "off" },
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
            object.insert("mode".to_string(), json!(self.mode.name()));
            object.insert(
                "reasoning_effort".to_string(),
                json!(self.reasoning_effort()),
            );
            object.insert("memory_enabled".to_string(), json!(self.memory_enabled));
            object.insert(
                "live_context".to_string(),
                context_pressure_json(&self.input, self.compact_after_chars, self.max_input_chars)
                    .unwrap_or_else(|error| json!({"error": error.to_string()})),
            );
            if let Some(goal) = &self.goal {
                object.insert("goal".to_string(), json!(goal));
            }
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
            mode: self.mode,
            reasoning_effort: self.reasoning_effort().to_string(),
            goal: self.goal.clone(),
            memory_enabled: self.memory_enabled,
        }
    }

    pub fn restore_snapshot(&mut self, snapshot: AgentSnapshot) {
        self.input = snapshot.input;
        self.request_seq = snapshot.request_seq;
        self.profiler = snapshot.profiler;
        self.loaded_skills = snapshot.loaded_skills;
        self.mode = snapshot.mode;
        self.goal = snapshot.goal;
        self.memory_enabled = snapshot.memory_enabled;
        self.set_reasoning_effort(snapshot.reasoning_effort);
        let _ = self.refresh_memory_context();
        self.readonly_tool_cache.clear();
    }

    pub fn loaded_skills(&self) -> &[String] {
        &self.loaded_skills
    }

    pub fn mode(&self) -> AgentMode {
        self.mode
    }

    pub fn set_mode(&mut self, mode: AgentMode) {
        if self.mode != mode {
            self.readonly_tool_cache.clear();
        }
        self.mode = mode;
    }

    pub fn set_read_roots(&mut self, read_roots: Vec<PathBuf>) {
        self.read_roots = read_roots;
        self.readonly_tool_cache.clear();
    }

    pub(crate) fn disable_mcp(&mut self) {
        self.mcp_registry = Some(McpRegistry::default());
    }

    pub(crate) fn disable_subagents(&mut self) {
        self.subagent_depth = 1;
    }

    pub(in crate::agent) async fn ensure_mcp_registry(&mut self) {
        if self.mcp_registry.is_some() {
            return;
        }
        let registry = McpRegistry::discover(&self.cwd).await;
        for warning in registry.warnings() {
            self.emit_warning(warning.clone());
        }
        self.mcp_registry = Some(registry);
    }

    pub(crate) fn invalidate_mcp_registry(&mut self) {
        self.mcp_registry = None;
    }

    pub(crate) async fn refresh_mcp_registry(&mut self) -> (usize, Vec<String>) {
        let registry = McpRegistry::discover(&self.cwd).await;
        let tool_count = registry.tools().len();
        let warnings = registry.warnings().to_vec();
        self.mcp_registry = Some(registry);
        (tool_count, warnings)
    }

    pub fn reasoning_effort(&self) -> &str {
        self.client.reasoning_effort()
    }

    pub fn set_reasoning_effort(&mut self, reasoning_effort: impl Into<String>) {
        self.client.set_reasoning_effort(reasoning_effort);
    }

    pub fn set_system_prompt(&mut self, system_prompt: impl Into<Option<String>>) {
        self.client.set_system_prompt(system_prompt);
    }

    pub fn memory_enabled(&self) -> bool {
        self.memory_enabled
    }

    pub fn set_memory_enabled(&mut self, enabled: bool) -> Result<()> {
        self.memory_enabled = enabled;
        if enabled {
            crate::memory::MemoryStore::open_default()?.ensure_files()?;
        }
        self.refresh_memory_context()
    }

    pub fn memory_status(&self) -> Result<String> {
        let paths = crate::memory::MemoryStore::open_default()?.paths();
        Ok(format!(
            "memory: {}\nMEMORY.md: {}\nchronicle.md: {}",
            if self.memory_enabled { "on" } else { "off" },
            paths.memory.display(),
            paths.chronicle.display()
        ))
    }

    pub fn memory_context_preview(&self) -> Result<String> {
        Ok(crate::memory::MemoryStore::open_default()?
            .read_context()?
            .unwrap_or_else(|| "memory is empty".to_string()))
    }

    pub fn append_memory_note(&mut self, note: &str) -> Result<()> {
        crate::memory::MemoryStore::open_default()?.append_note(note)?;
        self.refresh_memory_context()
    }

    fn refresh_memory_context(&mut self) -> Result<()> {
        if self.memory_enabled {
            let context = crate::memory::MemoryStore::open_default()?.read_context()?;
            self.client.set_memory_context(context);
        } else {
            self.client.set_memory_context(None);
        }
        Ok(())
    }

    fn record_memory_exchange(&mut self, prompt: &str, assistant_text: &str) -> Result<()> {
        if !self.memory_enabled || assistant_text.trim().is_empty() {
            return Ok(());
        }
        crate::memory::MemoryStore::open_default()?
            .append_chronicle_entry(prompt, assistant_text)?;
        self.refresh_memory_context()
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

    pub fn load_session_named(&mut self, name: &str) -> Result<bool> {
        let Some(snapshot) = SessionStore::open_default()?.load(name)? else {
            return Ok(false);
        };
        self.restore_snapshot(snapshot);
        Ok(true)
    }

    pub fn save_session_named(&self, name: &str) -> Result<()> {
        SessionStore::open_default()?.save(name, &self.snapshot())
    }
}

fn default_agent_snapshot_schema_version() -> u32 {
    AGENT_SNAPSHOT_SCHEMA_VERSION
}

fn default_agent_mode() -> AgentMode {
    AgentMode::Work
}

fn default_reasoning_effort() -> String {
    crate::client::DEFAULT_SPARK_AGENT_REASONING_EFFORT.to_string()
}

pub(crate) fn take_shared_display_events(events: &SharedDisplayEvents) -> Vec<AgentDisplayEvent> {
    match events.lock() {
        Ok(mut events) => std::mem::take(&mut *events),
        Err(poisoned) => std::mem::take(&mut *poisoned.into_inner()),
    }
}

fn push_shared_display_event(events: &SharedDisplayEvents, event: AgentDisplayEvent) {
    match events.lock() {
        Ok(mut events) => events.push(event),
        Err(poisoned) => poisoned.into_inner().push(event),
    }
}
