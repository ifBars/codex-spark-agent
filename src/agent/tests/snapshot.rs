use super::*;

#[test]
fn agent_snapshot_round_trips_history_and_profile() {
    let snapshot = AgentSnapshot {
        schema_version: AGENT_SNAPSHOT_SCHEMA_VERSION,
        input: vec![json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "hello"}]
        })],
        request_seq: 7,
        profiler: AgentProfiler::default(),
        loaded_skills: vec!["demo".to_string()],
        mode: crate::tools::AgentMode::Ask,
        reasoning_effort: "high".to_string(),
        goal: Some(crate::agent::goal::GoalState::new(
            "Keep benchmark quality above 95",
        )),
        memory_enabled: true,
    };

    let encoded = serde_json::to_string(&snapshot).expect("serialize snapshot");
    let decoded = serde_json::from_str::<AgentSnapshot>(&encoded).expect("deserialize snapshot");

    assert_eq!(decoded.schema_version, AGENT_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(decoded.input, snapshot.input);
    assert_eq!(decoded.request_seq, 7);
    assert_eq!(decoded.loaded_skills, vec!["demo"]);
    assert_eq!(decoded.mode, crate::tools::AgentMode::Ask);
    assert_eq!(decoded.reasoning_effort, "high");
    assert_eq!(
        decoded.goal.expect("goal").objective,
        "Keep benchmark quality above 95"
    );
    assert!(decoded.memory_enabled);
    assert_eq!(decoded.profiler.to_json()["requests"], 0);
}

#[test]
fn agent_snapshot_defaults_schema_version_for_existing_sessions() {
    let decoded = serde_json::from_value::<AgentSnapshot>(json!({
        "input": [{
            "role": "user",
            "content": [{"type": "input_text", "text": "hello"}]
        }],
        "request_seq": 1,
        "profiler": AgentProfiler::default(),
        "loaded_skills": []
    }))
    .expect("deserialize old snapshot");

    assert_eq!(decoded.schema_version, AGENT_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(decoded.request_seq, 1);
    assert_eq!(decoded.mode, crate::tools::AgentMode::Work);
    assert_eq!(
        decoded.reasoning_effort,
        crate::client::DEFAULT_SPARK_AGENT_REASONING_EFFORT
    );
    assert!(decoded.goal.is_none());
    assert!(!decoded.memory_enabled);
}
