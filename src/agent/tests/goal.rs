use super::*;
use crate::agent::goal::GoalStatus;

#[test]
fn goal_state_round_trips_through_agent_snapshot() {
    let mut runner = AgentRunner::new(
        test_auth_tokens(),
        tempfile::tempdir().expect("tempdir").path().to_path_buf(),
        crate::DEFAULT_MODEL.to_string(),
        None,
        false,
        false,
        crate::DEFAULT_COMPACT_AFTER_CHARS,
        crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        crate::DEFAULT_MAX_INPUT_CHARS,
        false,
        None,
        false,
        None,
        crate::tools::AgentMode::Work,
    )
    .expect("runner");

    runner
        .set_goal("Migrate the config format and keep tests green")
        .expect("set goal");
    runner.record_goal_progress(
        GoalStatus::Running,
        "Read AGENTS.md and identified validation command",
    );

    let snapshot = runner.snapshot();
    let goal = snapshot.goal.expect("goal should be snapshotted");
    assert_eq!(
        goal.objective,
        "Migrate the config format and keep tests green"
    );
    assert_eq!(goal.status, GoalStatus::Running);
    assert_eq!(goal.checkpoint, 1);
    assert!(goal.progress_log[0].note.contains("validation command"));
}

#[test]
fn goal_marker_updates_status_and_progress() {
    let mut runner = AgentRunner::new(
        test_auth_tokens(),
        tempfile::tempdir().expect("tempdir").path().to_path_buf(),
        crate::DEFAULT_MODEL.to_string(),
        None,
        false,
        false,
        crate::DEFAULT_COMPACT_AFTER_CHARS,
        crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        crate::DEFAULT_MAX_INPUT_CHARS,
        false,
        None,
        false,
        None,
        crate::tools::AgentMode::Work,
    )
    .expect("runner");
    runner.set_goal("Finish the release").expect("set goal");

    runner.record_goal_decision_from_assistant(
        "Validation passed.\n<spark_goal status=\"complete\">release published</spark_goal>",
    );

    let goal = runner.goal().expect("goal");
    assert_eq!(goal.status, GoalStatus::Complete);
    assert_eq!(goal.checkpoint, 1);
    assert_eq!(goal.progress_log[0].note, "release published");
}

#[test]
fn goal_continuation_prompt_carries_contract_and_recent_progress() {
    let mut runner = AgentRunner::new(
        test_auth_tokens(),
        tempfile::tempdir().expect("tempdir").path().to_path_buf(),
        crate::DEFAULT_MODEL.to_string(),
        None,
        false,
        false,
        crate::DEFAULT_COMPACT_AFTER_CHARS,
        crate::DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS,
        crate::DEFAULT_MAX_INPUT_CHARS,
        false,
        None,
        false,
        None,
        crate::tools::AgentMode::Work,
    )
    .expect("runner");
    runner.set_goal("Fix issue #25").expect("set goal");
    runner.record_goal_progress(GoalStatus::Running, "Read the issue body");

    let prompt = runner.goal_continuation_prompt().expect("prompt");

    assert!(prompt.contains("Fix issue #25"));
    assert!(prompt.contains("Read the issue body"));
    assert!(prompt.contains("<spark_goal status=\"continue|complete|blocked\">"));
    assert!(prompt.contains("verifiable stopping condition"));
}
