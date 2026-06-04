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
    };

    let encoded = serde_json::to_string(&snapshot).expect("serialize snapshot");
    let decoded = serde_json::from_str::<AgentSnapshot>(&encoded).expect("deserialize snapshot");

    assert_eq!(decoded.schema_version, AGENT_SNAPSHOT_SCHEMA_VERSION);
    assert_eq!(decoded.input, snapshot.input);
    assert_eq!(decoded.request_seq, 7);
    assert_eq!(decoded.loaded_skills, vec!["demo"]);
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
}
