use serde_json::json;

use crate::agent::subagent::{
    ADVANCED_SUBAGENT_MODEL, SubagentKind, SubagentModelPolicy, SubagentRunOptions,
    subagent_error_tool_result, subagent_prompt,
};

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
    assert_eq!(explore.mode, crate::tools::AgentMode::Ask);
    assert_eq!(explore.model_policy, SubagentModelPolicy::Parent);
    assert!(explore.system_prompt.contains("read-only"));

    let research = SubagentKind::Research.spec();
    assert_eq!(research.mode, crate::tools::AgentMode::Ask);
    assert_eq!(research.model_policy, SubagentModelPolicy::Advanced);
    assert_eq!(research.reasoning_effort, "high");
    assert!(research.system_prompt.contains("hosted web search"));
    assert!(research.system_prompt.contains("citations"));
}

#[test]
fn difficult_subagents_default_to_gpt55_with_kind_reasoning() {
    let review = SubagentKind::Review.spec();
    let runtime = review.runtime_config(
        "gpt-5.3-codex-spark",
        "medium",
        &SubagentRunOptions::default(),
    );

    assert_eq!(runtime.model, ADVANCED_SUBAGENT_MODEL);
    assert_eq!(runtime.reasoning_effort, "high");
    assert_eq!(runtime.max_turns, review.max_turns);
}

#[test]
fn subagent_tool_options_allow_parent_model_and_budget_override() {
    let options = SubagentRunOptions::from_tool_args(&json!({
        "kind": "research",
        "task": "Find current docs",
        "model": "parent",
        "reasoning_effort": "medium",
        "max_turns": 2
    }))
    .expect("options");
    let runtime =
        SubagentKind::Research
            .spec()
            .runtime_config("gpt-5.3-codex-spark", "low", &options);

    assert_eq!(runtime.model, "gpt-5.3-codex-spark");
    assert_eq!(runtime.reasoning_effort, "medium");
    assert_eq!(runtime.max_turns, 2);
}

#[test]
fn subagent_budget_exhaustion_returns_incomplete_observation() {
    let result = subagent_error_tool_result(
        &json!({
            "kind": "explore",
            "task": "Find the file",
            "max_turns": 2
        }),
        "stopped after 2 turns without completion",
    );

    assert!(result.ok);
    assert!(result.error.is_none());
    assert_eq!(result.data["status"], "incomplete");
    assert_eq!(result.data["error_kind"], "subagent_incomplete");
    assert!(
        result.data["hint"]
            .as_str()
            .expect("hint")
            .contains("Do not retry the same bounded subagent call")
    );
}

#[test]
fn subagent_prompt_returns_compact_brief_to_parent_loop() {
    let prompt = subagent_prompt(
        SubagentKind::Review,
        "Review PR #30 for lifecycle regressions",
    );

    assert!(prompt.contains("Review PR #30"));
    assert!(prompt.contains("Return a compact brief"));
    assert!(prompt.contains("Do not edit files"));
    assert!(prompt.contains("Evidence"));
}
