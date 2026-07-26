use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::agent::{AgentDisplay, AgentRunner};
use crate::profiler::AgentProfiler;
use crate::tools::{AgentMode, ToolResult};

pub(crate) const ADVANCED_SUBAGENT_MODEL: &str = "gpt-5.5";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SubagentKind {
    Explore,
    Research,
    Review,
    Plan,
}

impl SubagentKind {
    pub(crate) fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "explore" | "inspect" => Some(Self::Explore),
            "research" | "web" | "search" => Some(Self::Research),
            "review" | "code-review" => Some(Self::Review),
            "plan" | "planner" => Some(Self::Plan),
            _ => None,
        }
    }

    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Explore => "explore",
            Self::Research => "research",
            Self::Review => "review",
            Self::Plan => "plan",
        }
    }

    pub(crate) fn spec(self) -> SubagentSpec {
        match self {
            Self::Explore => SubagentSpec {
                mode: AgentMode::Ask,
                model_policy: SubagentModelPolicy::Parent,
                reasoning_effort: "medium",
                system_prompt: "You are a Spark explore subagent. You are read-only. Inspect local evidence quickly, do not edit files, and return only a compact brief for the parent loop.",
            },
            Self::Research => SubagentSpec {
                mode: AgentMode::Ask,
                model_policy: SubagentModelPolicy::Advanced,
                reasoning_effort: "high",
                system_prompt: "You are a Spark research subagent. Prefer hosted web search for current external facts, keep citations and source names in the brief, do not edit files, and separate verified facts from inference.",
            },
            Self::Review => SubagentSpec {
                mode: AgentMode::Ask,
                model_policy: SubagentModelPolicy::Advanced,
                reasoning_effort: "high",
                system_prompt: "You are a Spark review subagent. You are read-only. Check diffs, source, tests, and behavioral risks. Prioritize concrete findings with file and line evidence.",
            },
            Self::Plan => SubagentSpec {
                mode: AgentMode::Ask,
                model_policy: SubagentModelPolicy::Advanced,
                reasoning_effort: "high",
                system_prompt: "You are a Spark planning subagent. You are read-only. Turn evidence into a minimal phased plan with risks, validation, and explicit non-goals.",
            },
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SubagentSpec {
    pub(crate) mode: AgentMode,
    pub(crate) model_policy: SubagentModelPolicy,
    pub(crate) reasoning_effort: &'static str,
    pub(crate) system_prompt: &'static str,
}

impl SubagentSpec {
    pub(crate) fn runtime_config(
        self,
        parent_model: &str,
        parent_reasoning_effort: &str,
        options: &SubagentRunOptions,
    ) -> SubagentRuntimeConfig {
        let model = match options.model.as_deref().map(str::trim) {
            Some(model) if model.eq_ignore_ascii_case("parent") => parent_model.to_string(),
            Some(model) if !model.is_empty() => model.to_string(),
            _ => match self.model_policy {
                SubagentModelPolicy::Parent => parent_model.to_string(),
                SubagentModelPolicy::Advanced => std::env::var("SPARK_ADVANCED_SUBAGENT_MODEL")
                    .ok()
                    .filter(|model| !model.trim().is_empty())
                    .unwrap_or_else(|| ADVANCED_SUBAGENT_MODEL.to_string()),
            },
        };
        let reasoning_effort =
            options
                .reasoning_effort
                .clone()
                .unwrap_or_else(|| match self.model_policy {
                    SubagentModelPolicy::Parent => parent_reasoning_effort.to_string(),
                    SubagentModelPolicy::Advanced => self.reasoning_effort.to_string(),
                });
        SubagentRuntimeConfig {
            model,
            reasoning_effort,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SubagentModelPolicy {
    Parent,
    Advanced,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SubagentRuntimeConfig {
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(crate) struct SubagentRunOptions {
    pub(crate) model: Option<String>,
    pub(crate) reasoning_effort: Option<String>,
}

impl SubagentRunOptions {
    pub(crate) fn from_tool_args(args: &Value) -> Result<Self> {
        Ok(Self {
            model: optional_string(args, "model")?,
            reasoning_effort: optional_reasoning_effort(args)?,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubagentReport {
    pub(crate) kind: SubagentKind,
    pub(crate) task: String,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) summary: String,
    pub(crate) profile: Value,
}

impl AgentRunner {
    #[allow(dead_code)]
    pub(crate) async fn run_subagent(
        &self,
        kind: SubagentKind,
        task: &str,
    ) -> Result<SubagentReport> {
        self.run_subagent_with_options(kind, task, SubagentRunOptions::default())
            .await
    }

    pub(crate) async fn run_subagent_with_options(
        &self,
        kind: SubagentKind,
        task: &str,
        options: SubagentRunOptions,
    ) -> Result<SubagentReport> {
        let task = task.trim();
        if task.is_empty() {
            bail!("subagent task is required");
        }

        let spec = kind.spec();
        let runtime = spec.runtime_config(
            self.client.model(),
            self.client.reasoning_effort(),
            &options,
        );
        let mut child = AgentRunner {
            client: self
                .client
                .clone_with_model_and_reasoning_effort(&runtime.model, &runtime.reasoning_effort),
            cwd: self.cwd.clone(),
            read_roots: self.read_roots.clone(),
            input: Vec::new(),
            trace: None,
            compact_after_chars: self.compact_after_chars,
            compact_after_tool_only_turns: self.compact_after_tool_only_turns,
            max_input_chars: self.max_input_chars,
            request_seq: 0,
            profile: false,
            display: AgentDisplay::Buffered(Vec::new()),
            profiler: AgentProfiler::default(),
            readonly_tool_cache: Default::default(),
            loaded_skills: Vec::new(),
            mode: spec.mode,
            goal: None,
            memory_enabled: self.memory_enabled,
            subagent_depth: self.subagent_depth + 1,
            mcp_registry: None,
        };
        child.refresh_memory_context()?;
        child.set_system_prompt(Some(spec.system_prompt.to_string()));
        child.run(&subagent_prompt(kind, task)).await?;

        Ok(SubagentReport {
            kind,
            task: task.to_string(),
            model: runtime.model,
            reasoning_effort: runtime.reasoning_effort,
            summary: child.latest_assistant_text().unwrap_or_default(),
            profile: child.profile_summary(),
        })
    }

    fn latest_assistant_text(&self) -> Option<String> {
        self.input
            .iter()
            .rev()
            .filter_map(assistant_text_from_item)
            .find(|text| !text.trim().is_empty())
    }

    pub(crate) fn record_subagent_report(&mut self, report: &SubagentReport) {
        self.input.push(json!({
            "role": "user",
            "content": [{
                "type": "input_text",
                "text": report_prompt(report),
            }]
        }));
        if let Some(trace) = &mut self.trace {
            let _ = trace.write(
                self.request_seq,
                "subagent-report",
                &json!({
                    "kind": report.kind.name(),
                    "task": report.task,
                    "model": report.model,
                    "reasoning_effort": report.reasoning_effort,
                    "profile": report.profile,
                    "summary_chars": report.summary.len(),
                }),
            );
        }
    }

    pub(crate) async fn invoke_subagent_tool(&mut self, args: Value) -> ToolResult {
        match self.invoke_subagent_tool_inner(&args).await {
            Ok(report) => ToolResult {
                ok: true,
                data: json!({
                    "kind": report.kind.name(),
                    "task": report.task,
                    "model": report.model,
                    "reasoning_effort": report.reasoning_effort,
                    "summary": report.summary,
                    "profile": report.profile,
                }),
                error: None,
            },
            Err(error) => subagent_error_tool_result(&args, &error.to_string()),
        }
    }

    async fn invoke_subagent_tool_inner(&self, args: &Value) -> Result<SubagentReport> {
        if self.subagent_depth > 0 {
            bail!("subagents cannot spawn nested subagents");
        }
        let kind = args
            .get("kind")
            .and_then(Value::as_str)
            .and_then(SubagentKind::parse)
            .ok_or_else(|| {
                anyhow::anyhow!("subagent kind must be explore, research, review, or plan")
            })?;
        let task = args
            .get("task")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|task| !task.is_empty())
            .ok_or_else(|| anyhow::anyhow!("subagent task is required"))?;
        let options = SubagentRunOptions::from_tool_args(args)?;
        self.run_subagent_with_options(kind, task, options).await
    }
}

pub(crate) fn subagent_error_tool_result(args: &Value, message: &str) -> ToolResult {
    ToolResult {
        ok: false,
        data: json!({
            "error_kind": "subagent_failed",
            "tool": "subagent.run",
            "args": args,
            "message": message,
            "hint": "Use kind=explore|research|review|plan with a non-empty task. Optionally set model=parent or a concrete model and reasoning_effort=low|medium|high|xhigh.",
        }),
        error: Some(message.to_string()),
    }
}

pub(crate) fn subagent_prompt(kind: SubagentKind, task: &str) -> String {
    format!(
        "Run as the Spark {} subagent for this task:\n\n{}\n\nReturn a compact brief to the parent loop with these sections:\n- Answer\n- Evidence\n- Risks or unknowns\n- Recommended next step\n\nDo not edit files. Keep the brief dense enough that the parent loop does not need your full transcript.",
        kind.name(),
        task.trim()
    )
}

fn assistant_text_from_item(item: &Value) -> Option<String> {
    let content = item.get("content")?.as_array()?;
    let parts = content
        .iter()
        .filter_map(|part| {
            let text = part.get("text")?.as_str()?;
            let kind = part.get("type").and_then(Value::as_str);
            (kind == Some("output_text") || kind == Some("text")).then(|| text.to_string())
        })
        .collect::<Vec<_>>();
    (!parts.is_empty()).then(|| parts.join("\n"))
}

pub(crate) fn report_prompt(report: &SubagentReport) -> String {
    format!(
        "[spark subagent report: {}]\nModel: {}\nReasoning: {}\nTask: {}\n\n{}\n\nProfile:\n{}",
        report.kind.name(),
        report.model,
        report.reasoning_effort,
        report.task,
        report.summary.trim(),
        serde_json::to_string_pretty(&report.profile).unwrap_or_else(|_| "{}".to_string())
    )
}

fn optional_string(args: &Value, key: &str) -> Result<Option<String>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.trim().to_string())),
        Some(_) => bail!("{key} must be a string"),
    }
}

fn optional_reasoning_effort(args: &Value) -> Result<Option<String>> {
    let Some(reasoning_effort) = optional_string(args, "reasoning_effort")? else {
        return Ok(None);
    };
    match reasoning_effort.as_str() {
        "low" | "medium" | "high" | "xhigh" => Ok(Some(reasoning_effort)),
        _ => bail!("reasoning_effort must be low, medium, high, or xhigh"),
    }
}
