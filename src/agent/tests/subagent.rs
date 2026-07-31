use serde_json::json;

use crate::agent::SubagentReport;
use crate::agent::subagent::{
    ADVANCED_SUBAGENT_MODEL, SubagentKind, SubagentModelPolicy, SubagentRunOptions, subagent_prompt,
};
use crate::agent::team::{SubagentTeam, WorkerMetadata};
use crate::tools::AgentMode;

#[test]
fn subagent_kind_parses_common_names() {
    assert_eq!(SubagentKind::parse("explore"), Some(SubagentKind::Explore));
    assert_eq!(
        SubagentKind::parse("research"),
        Some(SubagentKind::Research)
    );
    assert_eq!(SubagentKind::parse("review"), Some(SubagentKind::Review));
    assert_eq!(SubagentKind::parse("plan"), Some(SubagentKind::Plan));
    assert_eq!(SubagentKind::parse("unknown"), None);
}

#[test]
fn subagent_specs_keep_exploration_read_only_and_research_web_aware() {
    let explore = SubagentKind::Explore.spec();
    assert_eq!(explore.mode, AgentMode::Ask);
    assert_eq!(explore.model_policy, SubagentModelPolicy::Parent);
    assert!(explore.system_prompt.contains("read-only"));

    let research = SubagentKind::Research.spec();
    assert_eq!(research.mode, AgentMode::Ask);
    assert_eq!(research.model_policy, SubagentModelPolicy::Advanced);
    assert_eq!(research.reasoning_effort, "high");
    assert!(research.system_prompt.contains("hosted web search"));
    assert!(research.system_prompt.contains("citations"));
}

#[test]
fn advanced_subagents_default_to_luna_with_kind_reasoning() {
    let review = SubagentKind::Review.spec();
    let runtime = review.runtime_config(
        "gpt-5.3-codex-spark",
        "medium",
        &SubagentRunOptions::default(),
    );

    assert_eq!(ADVANCED_SUBAGENT_MODEL, "gpt-5.6-luna");
    assert_eq!(runtime.model, "gpt-5.6-luna");
    assert_eq!(runtime.reasoning_effort, "high");
}

#[test]
fn subagent_tool_options_allow_parent_model_reasoning_and_owned_work() {
    let options = SubagentRunOptions::from_tool_args(&json!({
        "kind": "research",
        "task": "Find current docs",
        "model": "parent",
        "reasoning_effort": "medium",
        "mode": "work",
        "ownership": ["src/agent", "README.md"]
    }))
    .expect("options");
    let runtime =
        SubagentKind::Research
            .spec()
            .runtime_config("gpt-5.3-codex-spark", "low", &options);

    assert_eq!(runtime.model, "gpt-5.3-codex-spark");
    assert_eq!(runtime.reasoning_effort, "medium");
    assert_eq!(options.mode, Some(AgentMode::Work));
    assert_eq!(options.ownership, ["src/agent", "README.md"]);
}

#[test]
fn ownership_rejects_workspace_escape_paths() {
    let error = SubagentRunOptions::from_tool_args(&json!({
        "ownership": ["../outside"]
    }))
    .expect_err("escape must fail");
    assert!(error.to_string().contains("relative workspace paths"));
}

#[test]
fn subagent_prompt_returns_compact_brief_and_work_constraints() {
    let prompt = subagent_prompt(
        SubagentKind::Review,
        "Review PR #30 for lifecycle regressions",
        AgentMode::Work,
        &["src/agent".to_string()],
    );

    assert!(prompt.contains("Review PR #30"));
    assert!(prompt.contains("Return a compact brief"));
    assert!(prompt.contains("src/agent"));
    assert!(prompt.contains("Do not use shell"));
    assert!(prompt.contains("Evidence"));
}

#[tokio::test]
async fn worker_team_enforces_concurrency_and_preserves_completed_report() {
    let mut team = SubagentTeam::from_environment();
    let limit = team.max_concurrency();
    for index in 1..=limit {
        let id = team.reserve_id().expect("reserve worker");
        let report = report(&id);
        team.insert_running(
            metadata(&id),
            tokio_util::sync::CancellationToken::new(),
            tokio::spawn(async move { Ok(report) }),
        );
        assert_eq!(id, format!("worker-{index}"));
    }
    assert!(team.reserve_id().is_err());

    let completed = team.wait("worker-1").await.expect("wait worker");
    assert_eq!(completed.id.as_deref(), Some("worker-1"));
    assert_eq!(
        team.completed_report("worker-1")
            .expect("stored report")
            .summary,
        "brief"
    );
}

#[tokio::test]
async fn worker_team_cancellation_is_idempotent_for_finished_entries() {
    let mut team = SubagentTeam::from_environment();
    let id = team.reserve_id().expect("reserve worker");
    team.insert_running(
        metadata(&id),
        tokio_util::sync::CancellationToken::new(),
        tokio::spawn(async { std::future::pending::<anyhow::Result<SubagentReport>>().await }),
    );

    assert_eq!(team.cancel(Some(&id)).expect("cancel"), vec![id.clone()]);
    assert!(team.cancel(Some(&id)).expect("repeat cancel").is_empty());
    assert!(team.wait(&id).await.is_err());
}

fn metadata(id: &str) -> WorkerMetadata {
    WorkerMetadata {
        id: id.to_string(),
        kind: "review".to_string(),
        task: "Check the patch".to_string(),
        model: "gpt-5.6-luna".to_string(),
        reasoning_effort: "high".to_string(),
        mode: "ask".to_string(),
        ownership: Vec::new(),
    }
}

fn report(id: &str) -> SubagentReport {
    SubagentReport {
        id: Some(id.to_string()),
        kind: SubagentKind::Review,
        task: "Check the patch".to_string(),
        model: "gpt-5.6-luna".to_string(),
        reasoning_effort: "high".to_string(),
        mode: AgentMode::Ask,
        ownership: Vec::new(),
        summary: "brief".to_string(),
        profile: json!({"requests": 1}),
    }
}
