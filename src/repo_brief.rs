use std::collections::BTreeSet;
use std::path::{Component, Path, PathBuf};
use std::time::Instant;

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use tokio_util::sync::CancellationToken;

use crate::agent::AgentRunner;
use crate::tools::AgentMode;
use crate::{DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS, DEFAULT_MAX_INPUT_CHARS, DEFAULT_MODEL};

const MAX_TASK_CHARS: usize = 16_000;
const MAX_CONTEXT_CHARS: usize = 64_000;
const MAX_STARTING_PATHS: usize = 32;
const MAX_INSTRUCTION_CHARS: usize = 64_000;
pub(crate) const DEFAULT_TIMEOUT_SECONDS: u64 = 120;
pub(crate) const DEFAULT_LOCAL_FILESYSTEM_TOOL_BUDGET: usize = 16;
pub(crate) const REPO_BRIEF_COMPACT_AFTER_CHARS: usize = 320_000;
const MAX_TIMEOUT_SECONDS: u64 = 3_600;

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct RepoBriefRequest {
    #[serde(alias = "task")]
    pub(crate) question: String,
    #[serde(default)]
    pub(crate) cwd: Option<PathBuf>,
    #[serde(default, alias = "starting_paths")]
    pub(crate) paths: Vec<String>,
    #[serde(default)]
    pub(crate) context: Option<String>,
    #[serde(default)]
    pub(crate) reasoning_effort: Option<String>,
    #[serde(default)]
    pub(crate) trace: bool,
}

#[derive(Debug)]
struct InstructionContext {
    body: String,
    paths: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum RepoBriefStatus {
    Completed,
    ContractIncomplete,
    Limit,
    Error,
}

enum RunOutcome {
    Finished(Result<String>),
    Limit,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct ContractDiagnostic {
    pub(crate) passed: bool,
    pub(crate) required_sections: RequiredSections,
    pub(crate) citation_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub(crate) struct RequiredSections {
    pub(crate) answer: bool,
    pub(crate) evidence: bool,
    pub(crate) risks_unknowns: bool,
    pub(crate) next_inspection: bool,
}

#[derive(Debug, Serialize)]
pub(crate) struct RepoBriefReport {
    pub(crate) schema_version: u8,
    #[serde(rename = "type")]
    pub(crate) kind: &'static str,
    pub(crate) status: RepoBriefStatus,
    pub(crate) question: String,
    pub(crate) workspace: String,
    pub(crate) starting_paths: Vec<String>,
    pub(crate) answer_markdown: String,
    pub(crate) duration_ms: u64,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) trace_dir: Option<String>,
    pub(crate) profile: Value,
    pub(crate) safety_capabilities: Value,
    pub(crate) contract_diagnostic: ContractDiagnostic,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) error: Option<String>,
}

impl RepoBriefReport {
    pub(crate) fn exit_code(&self) -> i32 {
        match self.status {
            RepoBriefStatus::Completed => 0,
            RepoBriefStatus::ContractIncomplete => 2,
            RepoBriefStatus::Limit => 3,
            RepoBriefStatus::Error => 1,
        }
    }
}

pub(crate) fn validate_request(request: &RepoBriefRequest) -> Result<()> {
    let question = request.question.trim();
    if question.is_empty() {
        bail!("question is required");
    }
    if question.len() > MAX_TASK_CHARS {
        bail!("question exceeds {MAX_TASK_CHARS} characters");
    }
    if request.paths.len() > MAX_STARTING_PATHS {
        bail!("starting_paths exceeds {MAX_STARTING_PATHS} entries");
    }
    for path in &request.paths {
        let path = Path::new(path);
        if is_outside_workspace_path(path) {
            bail!(
                "starting_paths must stay relative to cwd: {}",
                path.display()
            );
        }
    }
    if request
        .context
        .as_deref()
        .is_some_and(|context| context.len() > MAX_CONTEXT_CHARS)
    {
        bail!("context exceeds {MAX_CONTEXT_CHARS} characters");
    }
    if let Some(reasoning_effort) = request.reasoning_effort.as_deref()
        && !matches!(reasoning_effort, "low" | "medium" | "high" | "xhigh")
    {
        bail!("reasoning_effort must be low, medium, high, or xhigh");
    }
    Ok(())
}

pub(crate) fn resolve_workspace(cwd: Option<&Path>) -> Result<PathBuf> {
    let cwd = cwd
        .map(Path::to_path_buf)
        .unwrap_or(std::env::current_dir()?);
    let cwd = std::fs::canonicalize(&cwd)
        .with_context(|| format!("failed to resolve workspace {}", cwd.display()))?;
    if !cwd.is_dir() {
        bail!("workspace is not a directory: {}", cwd.display());
    }
    Ok(cwd)
}

pub(crate) fn build_prompt(request: &RepoBriefRequest, instructions: &str) -> String {
    let starting_paths = if request.paths.is_empty() {
        "(none supplied)".to_string()
    } else {
        request
            .paths
            .iter()
            .map(|path| format!("- {path}"))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let parent_context = request
        .context
        .as_deref()
        .map(str::trim)
        .filter(|context| !context.is_empty())
        .unwrap_or("(none supplied)");
    let instruction_context = if instructions.is_empty() {
        "(no applicable AGENTS.md files found)"
    } else {
        instructions
    };

    format!(
        "Question:\n{}\n\nStarting paths:\n{}\n\nTask-relevant context forwarded by native Codex:\n{}\n\nApplicable repository instructions:\n{}\n\nReturn exactly these Markdown sections:\n## Answer\n## Evidence\n## Risks/unknowns\n## Next inspection\n\nEvidence must cite repository-relative file paths with line numbers, such as `src/main.rs:42`. Keep the brief compact. Do not edit files, execute commands, delegate, or use the network.",
        request.question.trim(),
        starting_paths,
        parent_context,
        instruction_context,
    )
}

pub(crate) async fn run_mcp(
    request: RepoBriefRequest,
    cancellation: CancellationToken,
) -> Result<String> {
    validate_request(&request)?;
    let workspace = resolve_workspace(request.cwd.as_deref())?;
    let instructions = load_instruction_context(&workspace, &request.paths)?;
    let mut runner = build_readonly_runner(&request, workspace, &instructions)?;
    runner
        .run_with_cancel_to_text(&build_prompt(&request, &instructions.body), cancellation)
        .await
}

pub(crate) async fn run_standalone(
    request: RepoBriefRequest,
    timeout_seconds: u64,
) -> Result<RepoBriefReport> {
    validate_request(&request)?;
    if !(1..=MAX_TIMEOUT_SECONDS).contains(&timeout_seconds) {
        bail!("timeout_seconds must be between 1 and {MAX_TIMEOUT_SECONDS}");
    }
    let workspace = resolve_workspace(request.cwd.as_deref())?;
    let instructions = load_instruction_context(&workspace, &request.paths)?;
    let mut runner = build_readonly_runner(&request, workspace.clone(), &instructions)?;
    let started = Instant::now();
    let prompt = build_prompt(&request, &instructions.body);
    let cancellation = CancellationToken::new();
    let result = {
        let run = runner.run_with_cancel_to_text(&prompt, cancellation.clone());
        tokio::pin!(run);
        tokio::select! {
            result = &mut run => RunOutcome::Finished(result),
            _ = tokio::time::sleep(std::time::Duration::from_secs(timeout_seconds)) => {
                cancellation.cancel();
                let _ = tokio::time::timeout(std::time::Duration::from_secs(2), &mut run).await;
                RunOutcome::Limit
            }
        }
    };
    let duration_ms = started.elapsed().as_millis().min(u64::MAX as u128) as u64;
    let trace_dir = runner.trace_dir().map(|path| path.display().to_string());
    let profile = runner.profile_summary();
    let reasoning_effort = request
        .reasoning_effort
        .unwrap_or_else(|| "medium".to_string());
    Ok(report_from_result(
        &request.question,
        workspace,
        request.paths,
        reasoning_effort,
        trace_dir,
        duration_ms,
        profile,
        result,
    ))
}

pub(crate) fn standalone_error_report(
    request: &RepoBriefRequest,
    error: &anyhow::Error,
) -> RepoBriefReport {
    let workspace = request
        .cwd
        .as_deref()
        .and_then(|cwd| std::fs::canonicalize(cwd).ok())
        .unwrap_or_else(|| request.cwd.clone().unwrap_or_else(|| PathBuf::from(".")));
    report(
        RepoBriefStatus::Error,
        &request.question,
        workspace,
        request.paths.clone(),
        String::new(),
        0,
        request
            .reasoning_effort
            .clone()
            .unwrap_or_else(|| "medium".to_string()),
        None,
        json!({}),
        contract_diagnostic(""),
        Some(format!("{error:#}")),
    )
}

fn build_readonly_runner(
    request: &RepoBriefRequest,
    workspace: PathBuf,
    instructions: &InstructionContext,
) -> Result<AgentRunner> {
    let auth = crate::config::load_auth()?;
    let reasoning_effort = request
        .reasoning_effort
        .clone()
        .unwrap_or_else(|| "medium".to_string());
    let mut runner = AgentRunner::new_with_reasoning_effort(
        auth,
        workspace,
        DEFAULT_MODEL.to_string(),
        reasoning_effort,
        request.trace,
        false,
        REPO_BRIEF_COMPACT_AFTER_CHARS,
        DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        DEFAULT_MAX_INPUT_CHARS,
        false,
        None,
        false,
        Some(json!({
            "repo_brief": true,
            "starting_paths": request.paths,
            "instruction_files": instructions.paths,
        })),
        AgentMode::Ask,
    )?;
    runner.use_buffered_display();
    runner.enforce_local_filesystem_only();
    runner.set_local_filesystem_tool_budget(DEFAULT_LOCAL_FILESYSTEM_TOOL_BUDGET);
    runner.set_system_prompt(Some(
        "You are a read-only local repository brief agent. Use only local filesystem evidence. Do not use web search, edit files, execute commands, or delegate. Return the requested Markdown brief."
            .to_string(),
    ));
    Ok(runner)
}

fn report_from_result(
    question: &str,
    workspace: PathBuf,
    starting_paths: Vec<String>,
    reasoning_effort: String,
    trace_dir: Option<String>,
    duration_ms: u64,
    profile: Value,
    result: RunOutcome,
) -> RepoBriefReport {
    let (status, answer_markdown, error) = match result {
        RunOutcome::Finished(Ok(answer)) => {
            let diagnostic = contract_diagnostic(&answer);
            let status = if diagnostic.passed {
                RepoBriefStatus::Completed
            } else {
                RepoBriefStatus::ContractIncomplete
            };
            return report(
                status,
                question,
                workspace,
                starting_paths,
                answer,
                duration_ms,
                reasoning_effort,
                trace_dir,
                profile,
                diagnostic,
                None,
            );
        }
        RunOutcome::Finished(Err(error)) => (
            RepoBriefStatus::Error,
            String::new(),
            Some(format!("{error:#}")),
        ),
        RunOutcome::Limit => (
            RepoBriefStatus::Limit,
            String::new(),
            Some("standalone deadline exceeded".to_string()),
        ),
    };
    report(
        status,
        question,
        workspace,
        starting_paths,
        answer_markdown,
        duration_ms,
        reasoning_effort,
        trace_dir,
        profile,
        contract_diagnostic(""),
        error,
    )
}

fn report(
    status: RepoBriefStatus,
    question: &str,
    workspace: PathBuf,
    starting_paths: Vec<String>,
    answer_markdown: String,
    duration_ms: u64,
    reasoning_effort: String,
    trace_dir: Option<String>,
    profile: Value,
    contract_diagnostic: ContractDiagnostic,
    error: Option<String>,
) -> RepoBriefReport {
    RepoBriefReport {
        schema_version: 1,
        kind: "repo_brief",
        status,
        question: question.trim().to_string(),
        workspace: workspace.display().to_string(),
        starting_paths,
        answer_markdown,
        duration_ms,
        model: DEFAULT_MODEL.to_string(),
        reasoning_effort,
        trace_dir,
        profile,
        safety_capabilities: json!({
            "local_filesystem_only": true,
            "tools": ["fs.read", "fs.list", "fs.stat", "fs.search"],
            "local_filesystem_tool_budget": {
                "max": DEFAULT_LOCAL_FILESYSTEM_TOOL_BUDGET,
                "scope": "local_filesystem_tool_invocations",
            },
            "mcp": false,
            "subagents": false,
            "hosted_web_search": false,
            "shell": false,
            "browser": false,
            "writes": false,
        }),
        contract_diagnostic,
        error,
    }
}

pub(crate) fn contract_diagnostic(answer: &str) -> ContractDiagnostic {
    let normalized = answer.to_ascii_lowercase();
    let required_sections = RequiredSections {
        answer: has_heading(&normalized, "answer"),
        evidence: has_heading(&normalized, "evidence"),
        risks_unknowns: has_heading(&normalized, "risks/unknowns")
            || has_heading(&normalized, "risks / unknowns"),
        next_inspection: has_heading(&normalized, "next inspection"),
    };
    let citation_count = regex::Regex::new(
        r"(?m)(?:^|[\s\[(`])([A-Za-z0-9][A-Za-z0-9_./-]*\.[A-Za-z0-9_-]+):([1-9][0-9]*)",
    )
    .expect("constant citation regex")
    .captures_iter(answer)
    .count();
    let passed = required_sections.answer
        && required_sections.evidence
        && required_sections.risks_unknowns
        && required_sections.next_inspection
        && citation_count >= 1;
    ContractDiagnostic {
        passed,
        required_sections,
        citation_count,
    }
}

fn has_heading(answer: &str, wanted: &str) -> bool {
    answer.lines().any(|line| {
        let line = line.trim_start();
        let heading = line.trim_start_matches('#').trim();
        line.starts_with('#') && heading == wanted
    })
}

fn is_outside_workspace_path(path: &Path) -> bool {
    if path.is_absolute() {
        return true;
    }
    let text = path.as_os_str().to_string_lossy();
    let windows_absolute =
        text.starts_with('\\') || text.starts_with("//") || text.as_bytes().get(1) == Some(&b':');
    windows_absolute
        || text.replace('\\', "/").split('/').any(|part| part == "..")
        || path.components().any(|part| part == Component::ParentDir)
}

fn load_instruction_context(cwd: &Path, starting_paths: &[String]) -> Result<InstructionContext> {
    let repo_root = cwd
        .ancestors()
        .find(|path| path.join(".git").exists())
        .unwrap_or(cwd);
    let mut candidates = BTreeSet::new();
    collect_agents_between(repo_root, cwd, &mut candidates);
    for starting_path in starting_paths {
        let joined = cwd.join(starting_path);
        let directory = if joined.is_dir() {
            joined
        } else {
            joined.parent().unwrap_or(cwd).to_path_buf()
        };
        if directory.starts_with(repo_root) {
            collect_agents_between(repo_root, &directory, &mut candidates);
        }
    }

    let mut body = String::new();
    let mut paths = Vec::new();
    for path in candidates {
        if !path.is_file() {
            continue;
        }
        let raw = std::fs::read_to_string(&path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        let relative = path
            .strip_prefix(repo_root)
            .unwrap_or(&path)
            .display()
            .to_string();
        let section = format!("\n### {relative}\n{}\n", raw.trim());
        if body.len() + section.len() > MAX_INSTRUCTION_CHARS {
            break;
        }
        body.push_str(&section);
        paths.push(relative);
    }
    Ok(InstructionContext {
        body: body.trim().to_string(),
        paths,
    })
}

fn collect_agents_between(root: &Path, target: &Path, candidates: &mut BTreeSet<PathBuf>) {
    if !target.starts_with(root) {
        return;
    }
    let mut current = root.to_path_buf();
    candidates.insert(current.join("AGENTS.md"));
    let Ok(relative) = target.strip_prefix(root) else {
        return;
    };
    for component in relative.components() {
        current.push(component.as_os_str());
        candidates.insert(current.join("AGENTS.md"));
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use clap::Parser;
    use serde_json::json;

    use super::{
        DEFAULT_LOCAL_FILESYSTEM_TOOL_BUDGET, REPO_BRIEF_COMPACT_AFTER_CHARS, RepoBriefRequest,
        RepoBriefStatus, RunOutcome, contract_diagnostic, report,
    };
    use crate::cli::{Cli, Command};

    #[test]
    fn cli_parses_brief_defaults_and_repeated_paths() {
        let cli = Cli::try_parse_from([
            "spark",
            "brief",
            "Trace commands",
            "--path",
            "src",
            "--path",
            "README.md",
        ])
        .expect("cli");
        let Command::Brief {
            question,
            cwd,
            paths,
            format,
            reasoning_effort,
            trace,
            timeout_seconds,
        } = cli.command
        else {
            panic!("brief command");
        };
        assert_eq!(question, "Trace commands");
        assert_eq!(cwd, PathBuf::from("."));
        assert_eq!(paths, vec!["src", "README.md"]);
        assert_eq!(format.to_string(), "text");
        assert_eq!(reasoning_effort, "medium");
        assert!(!trace);
        assert_eq!(timeout_seconds, super::DEFAULT_TIMEOUT_SECONDS);
    }

    #[test]
    fn rejects_absolute_and_parent_paths_before_runner_setup() {
        for path in [
            "../outside",
            "..\\outside",
            "C:\\outside",
            "C:drive-relative",
            "\\\\server\\share",
        ] {
            let request = RepoBriefRequest {
                question: "Inspect".to_string(),
                cwd: None,
                paths: vec![path.to_string()],
                context: None,
                reasoning_effort: None,
                trace: false,
            };
            assert!(super::validate_request(&request).is_err());
        }
    }

    #[test]
    fn contract_detection_requires_all_sections_and_citation() {
        let passing = "## Answer\nYes\n## Evidence\n- `src/main.rs:42`\n## Risks/unknowns\nNone\n## Next inspection\nTests";
        assert!(contract_diagnostic(passing).passed);
        let missing = "## Answer\nYes\n## Evidence\nNone";
        let diagnostic = contract_diagnostic(missing);
        assert!(!diagnostic.passed);
        assert!(!diagnostic.required_sections.next_inspection);
        assert_eq!(diagnostic.citation_count, 0);
    }

    #[test]
    fn nested_agents_files_are_loaded_in_scope_order_and_outside_paths_are_ignored() {
        let dir = tempfile::tempdir().expect("tempdir");
        std::fs::create_dir_all(dir.path().join(".git")).expect("git dir");
        std::fs::create_dir_all(dir.path().join("src/parser")).expect("nested dir");
        std::fs::write(dir.path().join("AGENTS.md"), "root rules").expect("root agents");
        std::fs::write(dir.path().join("src/AGENTS.md"), "src rules").expect("src agents");

        let context = super::load_instruction_context(
            dir.path(),
            &[
                "src/parser/input.rs".to_string(),
                "../outside.rs".to_string(),
            ],
        )
        .expect("instruction context");
        let paths = context
            .paths
            .iter()
            .map(|path| path.replace('\\', "/"))
            .collect::<Vec<_>>();
        assert_eq!(paths, vec!["AGENTS.md", "src/AGENTS.md"]);
        assert!(context.body.find("root rules") < context.body.find("src rules"));
    }

    #[test]
    fn report_serializes_stable_json_envelope() {
        let report = report(
            RepoBriefStatus::Completed,
            "Question",
            PathBuf::from("C:/repo"),
            vec!["src".to_string()],
            "## Answer".to_string(),
            7,
            "medium".to_string(),
            None,
            json!({"response_usage": {"source": "provider_responses"}}),
            contract_diagnostic(
                "## Answer\n## Evidence\n`src/main.rs:1`\n## Risks/unknowns\nnone\n## Next inspection\nnone",
            ),
            None,
        );
        let value = serde_json::to_value(report).expect("json");
        assert_eq!(value["schema_version"], 1);
        assert_eq!(value["type"], "repo_brief");
        assert_eq!(value["status"], "completed");
        assert!(value.get("trace_dir").is_some());
        assert!(value.get("profile").is_some());
        assert_eq!(
            value["safety_capabilities"]["local_filesystem_tool_budget"]["max"],
            DEFAULT_LOCAL_FILESYSTEM_TOOL_BUDGET
        );
        assert_eq!(
            value["safety_capabilities"]["local_filesystem_tool_budget"]["scope"],
            "local_filesystem_tool_invocations"
        );
    }

    #[test]
    fn repo_brief_keeps_evidence_until_the_higher_compaction_threshold() {
        assert_eq!(REPO_BRIEF_COMPACT_AFTER_CHARS, 320_000);
    }

    #[tokio::test]
    async fn timeout_outcome_produces_deterministic_limit_envelope() {
        let report = super::report_from_result(
            "Question",
            PathBuf::from("C:/repo"),
            Vec::new(),
            "medium".to_string(),
            None,
            0,
            json!({}),
            RunOutcome::Limit,
        );
        assert_eq!(report.status, RepoBriefStatus::Limit);
        assert_eq!(
            report.error.as_deref(),
            Some("standalone deadline exceeded")
        );
        assert_eq!(report.exit_code(), 3);
    }
}
