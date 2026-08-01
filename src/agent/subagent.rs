use std::path::{Component, Path};

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::agent::team::WorkerMetadata;
use crate::agent::{AgentDisplay, AgentRunner};
use crate::profiler::AgentProfiler;
use crate::tools::{AgentMode, ToolResult};

pub(crate) const ADVANCED_SUBAGENT_MODEL: &str = "gpt-5.6-luna";
const MAX_SUBAGENT_SUMMARY_CHARS: usize = 6_000;

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
                system_prompt: "You are a Spark explore worker. You are read-only. Inspect local evidence quickly and return only a compact evidence brief for the parent.",
            },
            Self::Research => SubagentSpec {
                mode: AgentMode::Ask,
                model_policy: SubagentModelPolicy::Advanced,
                reasoning_effort: "high",
                system_prompt: "You are a Spark research worker. Prefer hosted web search for current external facts, keep citations and source names in the brief, do not edit files, and separate verified facts from inference.",
            },
            Self::Review => SubagentSpec {
                mode: AgentMode::Ask,
                model_policy: SubagentModelPolicy::Advanced,
                reasoning_effort: "high",
                system_prompt: "You are a Spark review worker. You are read-only. Check diffs, source, tests, and behavioral risks. Prioritize concrete findings with file and line evidence.",
            },
            Self::Plan => SubagentSpec {
                mode: AgentMode::Ask,
                model_policy: SubagentModelPolicy::Advanced,
                reasoning_effort: "high",
                system_prompt: "You are a Spark planning worker. You are read-only. Turn evidence into a minimal phased plan with risks, validation, and explicit non-goals.",
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
    pub(crate) mode: Option<AgentMode>,
    pub(crate) ownership: Vec<String>,
}

impl SubagentRunOptions {
    pub(crate) fn from_tool_args(args: &Value) -> Result<Self> {
        let mode = match optional_string(args, "mode")?.as_deref() {
            None => None,
            Some("ask") => Some(AgentMode::Ask),
            Some("work") => Some(AgentMode::Work),
            Some(_) => bail!("mode must be ask or work"),
        };
        let ownership = optional_string_list(args, "ownership")?;
        validate_ownership(&ownership)?;
        Ok(Self {
            model: optional_string(args, "model")?,
            reasoning_effort: optional_reasoning_effort(args)?,
            mode,
            ownership,
        })
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SubagentReport {
    pub(crate) id: Option<String>,
    pub(crate) kind: SubagentKind,
    pub(crate) task: String,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) mode: AgentMode,
    pub(crate) ownership: Vec<String>,
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
        self.run_subagent_with_cancel(kind, task, options, CancellationToken::new())
            .await
    }

    async fn run_subagent_with_cancel(
        &self,
        kind: SubagentKind,
        task: &str,
        options: SubagentRunOptions,
        cancellation: CancellationToken,
    ) -> Result<SubagentReport> {
        let (mut child, mut report) = self.build_subagent(kind, task, options, None)?;
        let summary = child
            .run_with_cancel_to_text(
                &subagent_prompt(kind, &report.task, report.mode, &report.ownership),
                cancellation,
            )
            .await?;
        report.summary = compact_summary(&summary);
        report.profile = child.profile_summary();
        Ok(report)
    }

    fn build_subagent(
        &self,
        kind: SubagentKind,
        task: &str,
        options: SubagentRunOptions,
        id: Option<String>,
    ) -> Result<(AgentRunner, SubagentReport)> {
        let task = task.trim();
        if task.is_empty() {
            bail!("subagent task is required");
        }
        if self.subagent_depth > 0 {
            bail!("subagents cannot spawn nested subagents");
        }
        let spec = kind.spec();
        let mode = options.mode.unwrap_or(spec.mode);
        if mode == AgentMode::Work {
            if self.mode != AgentMode::Work {
                bail!("work subagents require the parent to be in work mode");
            }
            if options.ownership.is_empty() {
                bail!(
                    "work subagents require a non-empty ownership list of relative workspace paths"
                );
            }
        } else if !options.ownership.is_empty() {
            bail!("ownership is only valid for a work subagent");
        }
        let runtime = spec.runtime_config(
            self.client.model(),
            self.client.reasoning_effort(),
            &options,
        );
        let ownership_prompt = if mode == AgentMode::Work {
            format!(
                "\nYou may modify only these owned workspace paths with native fs tools: {}. Shell commands, browser actions, MCP tools, and nested workers are disabled for this delegated write task.",
                options.ownership.join(", ")
            )
        } else {
            "".to_string()
        };
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
            mode,
            goal: None,
            memory_enabled: self.memory_enabled,
            subagent_depth: self.subagent_depth + 1,
            subagent_team: Default::default(),
            delegated_write_ownership: (mode == AgentMode::Work).then(|| options.ownership.clone()),
            mcp_registry: None,
            local_filesystem_only: false,
            local_filesystem_tool_budget: None,
        };
        child.refresh_memory_context()?;
        child.set_system_prompt(Some(format!("{}{}", spec.system_prompt, ownership_prompt)));
        let report = SubagentReport {
            id,
            kind,
            task: task.to_string(),
            model: runtime.model,
            reasoning_effort: runtime.reasoning_effort,
            mode,
            ownership: options.ownership,
            summary: String::new(),
            profile: Value::Null,
        };
        Ok((child, report))
    }

    pub(crate) fn spawn_subagent(
        &mut self,
        kind: SubagentKind,
        task: &str,
        options: SubagentRunOptions,
    ) -> Result<Value> {
        let id = self.subagent_team.reserve_id()?;
        let (mut child, report) = self.build_subagent(kind, task, options, Some(id.clone()))?;
        let metadata = WorkerMetadata {
            id: id.clone(),
            kind: kind.name().to_string(),
            task: report.task.clone(),
            model: report.model.clone(),
            reasoning_effort: report.reasoning_effort.clone(),
            mode: report.mode.name().to_string(),
            ownership: report.ownership.clone(),
        };
        let cancellation = CancellationToken::new();
        let child_cancellation = cancellation.clone();
        let handle = tokio::spawn(async move {
            let summary = child
                .run_with_cancel_to_text(
                    &subagent_prompt(kind, &report.task, report.mode, &report.ownership),
                    child_cancellation,
                )
                .await?;
            Ok(SubagentReport {
                summary: compact_summary(&summary),
                profile: child.profile_summary(),
                ..report
            })
        });
        self.subagent_team
            .insert_running(metadata, cancellation, handle);
        self.record_subagent_event("spawn", &json!({"id": id, "kind": kind.name(), "task": task, "max_concurrency": self.subagent_team.max_concurrency()}));
        Ok(json!({
            "id": id,
            "status": "running",
            "max_concurrency": self.subagent_team.max_concurrency(),
            "hint": "Use subagent.wait with this id before relying on the brief. Other independent workers may run in parallel."
        }))
    }

    pub(crate) async fn wait_subagent(&mut self, id: &str) -> Result<SubagentReport> {
        let report = self.subagent_team.wait(id).await?;
        self.record_subagent_event(
            "wait",
            &json!({
                "id": id,
                "status": "completed",
                "summary_chars": report.summary.len(),
                "profile": report.profile,
            }),
        );
        Ok(report)
    }

    pub(crate) fn followup_subagent(&mut self, id: &str, followup: &str) -> Result<Value> {
        let previous = self.subagent_team.completed_report(id)?;
        let task = format!(
            "Continue the completed worker task below. Keep the same role and constraints.\n\nOriginal task:\n{}\n\nPrevious compact brief:\n{}\n\nFollow-up:\n{}",
            previous.task,
            previous.summary,
            followup.trim()
        );
        let options = SubagentRunOptions {
            model: Some(previous.model),
            reasoning_effort: Some(previous.reasoning_effort),
            mode: Some(previous.mode),
            ownership: previous.ownership,
        };
        let result = self.spawn_subagent(previous.kind, &task, options)?;
        self.record_subagent_event("followup", &json!({"from": id, "next": result.get("id")}));
        Ok(result)
    }

    pub(crate) fn steer_subagent(&mut self, id: &str, direction: &str) -> Result<Value> {
        if self.subagent_team.completed_report(id).is_ok() {
            return self.followup_subagent(id, direction);
        }
        let metadata = self.subagent_team.metadata(id)?;
        self.subagent_team.cancel(Some(id))?;
        let kind = SubagentKind::parse(&metadata.kind)
            .ok_or_else(|| anyhow::anyhow!("stored subagent `{id}` has an unsupported role"))?;
        let mode = match metadata.mode.as_str() {
            "ask" => AgentMode::Ask,
            "work" => AgentMode::Work,
            _ => bail!("stored subagent `{id}` has an unsupported mode"),
        };
        let task = format!(
            "Replacement worker after steering `{id}`. Preserve the role and safety constraints.\n\nOriginal task:\n{}\n\nNew direction:\n{}",
            metadata.task,
            direction.trim()
        );
        let result = self.spawn_subagent(
            kind,
            &task,
            SubagentRunOptions {
                model: Some(metadata.model),
                reasoning_effort: Some(metadata.reasoning_effort),
                mode: Some(mode),
                ownership: metadata.ownership,
            },
        )?;
        self.record_subagent_event("steer", &json!({"from": id, "next": result.get("id")}));
        Ok(result)
    }

    pub(crate) fn cancel_subagents(&mut self, id: Option<&str>) -> Result<Value> {
        let cancelled = self.subagent_team.cancel(id)?;
        self.record_subagent_event("cancel", &json!({"id": id, "cancelled": cancelled}));
        Ok(json!({"cancelled": cancelled}))
    }

    pub(crate) fn subagent_status(&self) -> Value {
        self.subagent_team.status_json()
    }

    pub(crate) fn record_subagent_report(&mut self, report: &SubagentReport) {
        self.input.push(json!({"role": "user", "content": [{"type": "input_text", "text": report_prompt(report)}]}));
        self.record_subagent_event(
            "report",
            &json!({
                "id": report.id,
                "kind": report.kind.name(),
                "task": report.task,
                "model": report.model,
                "reasoning_effort": report.reasoning_effort,
                "mode": report.mode.name(),
                "ownership": report.ownership,
                "profile": report.profile,
                "summary_chars": report.summary.len(),
            }),
        );
    }

    fn record_subagent_event(&mut self, event: &str, detail: &Value) {
        if let Some(trace) = &mut self.trace {
            let _ = trace.write(self.request_seq, &format!("subagent-{event}"), detail);
        }
    }

    pub(crate) async fn invoke_subagent_tool(
        &mut self,
        tool_name: &str,
        args: Value,
    ) -> ToolResult {
        let result = match tool_name {
            "subagent.run" => self.invoke_subagent_run(&args).await.map(report_to_json),
            "subagent.spawn" => self.invoke_subagent_spawn(&args),
            "subagent.wait" => self.invoke_subagent_wait(&args).await.map(report_to_json),
            "subagent.followup" => self.invoke_subagent_followup(&args),
            "subagent.steer" => self.invoke_subagent_steer(&args),
            "subagent.cancel" => self.invoke_subagent_cancel(&args),
            "subagent.list" => Ok(self.subagent_status()),
            _ => Err(anyhow::anyhow!("unknown subagent tool `{tool_name}`")),
        };
        match result {
            Ok(data) => ToolResult {
                ok: true,
                data,
                error: None,
            },
            Err(error) => subagent_error_tool_result(tool_name, &args, &error.to_string()),
        }
    }

    async fn invoke_subagent_run(&mut self, args: &Value) -> Result<SubagentReport> {
        let (kind, task, options) = parse_subagent_request(args)?;
        self.run_subagent_with_options(kind, task, options).await
    }

    fn invoke_subagent_spawn(&mut self, args: &Value) -> Result<Value> {
        let (kind, task, options) = parse_subagent_request(args)?;
        self.spawn_subagent(kind, task, options)
    }

    async fn invoke_subagent_wait(&mut self, args: &Value) -> Result<SubagentReport> {
        self.wait_subagent(required_string(args, "id")?).await
    }

    fn invoke_subagent_followup(&mut self, args: &Value) -> Result<Value> {
        self.followup_subagent(required_string(args, "id")?, required_string(args, "task")?)
    }

    fn invoke_subagent_steer(&mut self, args: &Value) -> Result<Value> {
        self.steer_subagent(required_string(args, "id")?, required_string(args, "task")?)
    }

    fn invoke_subagent_cancel(&mut self, args: &Value) -> Result<Value> {
        let id = optional_string(args, "id")?;
        self.cancel_subagents(id.as_deref())
    }

    pub(in crate::agent) fn delegated_tool_scope_error(
        &self,
        tool_name: &str,
        args: &Value,
    ) -> Option<String> {
        let ownership = self.delegated_write_ownership.as_ref()?;
        if matches!(tool_name, "cmd.exec" | "browser.run") || tool_name.starts_with("mcp__") {
            return Some(format!(
                "delegated write workers cannot use `{tool_name}`; use bounded native fs tools within the assigned ownership paths"
            ));
        }
        let paths = match tool_name {
            "fs.write" | "fs.replace" | "fs.edit" => vec![args.get("path").and_then(Value::as_str)],
            "fs.rename" => vec![
                args.get("from").and_then(Value::as_str),
                args.get("to").and_then(Value::as_str),
            ],
            _ => return None,
        };
        for path in paths {
            let Some(path) = path else {
                return Some(format!(
                    "delegated write worker supplied no path for `{tool_name}`"
                ));
            };
            if !path_is_owned(path, ownership) {
                return Some(format!(
                    "delegated write worker cannot mutate `{path}`; assigned ownership is {}",
                    ownership.join(", ")
                ));
            }
        }
        None
    }
}

fn parse_subagent_request(args: &Value) -> Result<(SubagentKind, &str, SubagentRunOptions)> {
    let kind = args
        .get("kind")
        .and_then(Value::as_str)
        .and_then(SubagentKind::parse)
        .ok_or_else(|| {
            anyhow::anyhow!("subagent kind must be explore, research, review, or plan")
        })?;
    let task = required_string(args, "task")?;
    Ok((kind, task, SubagentRunOptions::from_tool_args(args)?))
}

fn required_string<'a>(args: &'a Value, key: &str) -> Result<&'a str> {
    args.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("subagent {key} is required"))
}

pub(crate) fn subagent_error_tool_result(
    tool_name: &str,
    args: &Value,
    message: &str,
) -> ToolResult {
    ToolResult {
        ok: false,
        data: json!({
            "error_kind": "subagent_failed", "tool": tool_name, "args": args, "message": message,
            "hint": "Use subagent.spawn for parallel read-only workers, subagent.wait for a compact brief, subagent.followup or subagent.steer for a focused continuation, and subagent.cancel to stop running workers. Work mode requires explicit ownership.",
        }),
        error: Some(message.to_string()),
    }
}

pub(crate) fn subagent_prompt(
    kind: SubagentKind,
    task: &str,
    mode: AgentMode,
    ownership: &[String],
) -> String {
    let edit_constraint = if mode == AgentMode::Work {
        format!(
            "You may edit only: {}. Do not use shell, browser, MCP, or nested workers.",
            ownership.join(", ")
        )
    } else {
        "Do not edit files.".to_string()
    };
    format!(
        "Run as the Spark {} worker for this task:\n\n{}\n\nReturn a compact brief to the parent with these sections:\n- Answer\n- Evidence\n- Risks or unknowns\n- Recommended next step\n\n{} Keep the brief dense enough that the parent does not need your full transcript.",
        kind.name(),
        task.trim(),
        edit_constraint
    )
}

pub(crate) fn report_prompt(report: &SubagentReport) -> String {
    format!(
        "[spark subagent report: {}{}]\nModel: {}\nReasoning: {}\nMode: {}\nOwnership: {}\nTask: {}\n\n{}\n\nProfile:\n{}",
        report.kind.name(),
        report
            .id
            .as_ref()
            .map(|id| format!(" {id}"))
            .unwrap_or_default(),
        report.model,
        report.reasoning_effort,
        report.mode.name(),
        if report.ownership.is_empty() {
            "read-only".to_string()
        } else {
            report.ownership.join(", ")
        },
        report.task,
        report.summary.trim(),
        serde_json::to_string_pretty(&report.profile).unwrap_or_else(|_| "{}".to_string())
    )
}

fn report_to_json(report: SubagentReport) -> Value {
    json!({"id": report.id, "kind": report.kind.name(), "task": report.task, "model": report.model,
        "reasoning_effort": report.reasoning_effort, "mode": report.mode.name(), "ownership": report.ownership,
        "summary": report.summary, "profile": report.profile})
}

fn compact_summary(text: &str) -> String {
    let text = text.trim();
    if text.chars().count() <= MAX_SUBAGENT_SUMMARY_CHARS {
        return text.to_string();
    }
    let prefix = text
        .chars()
        .take(MAX_SUBAGENT_SUMMARY_CHARS)
        .collect::<String>();
    format!("{prefix}\n\n[worker brief truncated at {MAX_SUBAGENT_SUMMARY_CHARS} characters]")
}

fn optional_string(args: &Value, key: &str) -> Result<Option<String>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) if value.trim().is_empty() => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.trim().to_string())),
        Some(_) => bail!("{key} must be a string"),
    }
}

fn optional_string_list(args: &Value, key: &str) -> Result<Vec<String>> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(Vec::new()),
        Some(Value::Array(values)) => values
            .iter()
            .map(|value| {
                value
                    .as_str()
                    .map(str::trim)
                    .filter(|value| !value.is_empty())
                    .map(str::to_string)
                    .ok_or_else(|| anyhow::anyhow!("{key} must contain non-empty strings"))
            })
            .collect(),
        Some(_) => bail!("{key} must be an array of strings"),
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

fn validate_ownership(ownership: &[String]) -> Result<()> {
    for path in ownership {
        let path = Path::new(path);
        if path.is_absolute()
            || path.as_os_str().is_empty()
            || path.components().any(|part| {
                matches!(
                    part,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            bail!("ownership paths must be non-empty relative workspace paths without `..`");
        }
    }
    Ok(())
}

fn path_is_owned(path: &str, ownership: &[String]) -> bool {
    let path = Path::new(path);
    !path.is_absolute()
        && !path.components().any(|part| {
            matches!(
                part,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
        && ownership
            .iter()
            .any(|root| path.starts_with(Path::new(root)))
}
