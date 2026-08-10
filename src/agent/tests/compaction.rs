use super::*;
use crate::agent::compaction::retention::{
    append_post_compaction_verification_notice, compact_input_locally, compact_message_item,
    install_remote_compaction_history, message_text_from_value, parse_native_tool_action,
    post_compaction_verification_text, process_remote_compaction_output, retained_intent_block,
    retained_intent_lines,
};
use crate::agent::compaction::{
    compact_remote_history_to_threshold, compaction_trigger_for_turn, format_compaction_notice,
};

#[test]
fn remote_compaction_summary_alias_normalizes_to_codex_compaction_item() {
    let output = vec![
        json!({
            "type": "message",
            "id": "msg_keep_id_out",
            "status": "completed",
            "role": "user",
            "content": [{"type": "input_text", "text": "keep the user request"}]
        }),
        json!({
            "type": "compaction_summary",
            "id": "cmp_drop_id",
            "encrypted_content": "encrypted-summary"
        }),
    ];

    let replacement = process_remote_compaction_output(output);

    assert_eq!(replacement.len(), 2);
    assert_eq!(replacement[0]["role"], "user");
    assert!(replacement[0].get("id").is_none());
    assert!(replacement[0].get("status").is_none());
    assert_eq!(
        replacement[1],
        json!({
            "type": "compaction",
            "encrypted_content": "encrypted-summary"
        })
    );
}

#[test]
fn remote_compaction_output_drops_stale_developer_and_tool_items() {
    let output = vec![
        json!({
            "type": "message",
            "role": "developer",
            "content": [{"type": "input_text", "text": "stale instructions"}]
        }),
        json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "{}"
        }),
        json!({
            "type": "message",
            "role": "assistant",
            "content": [{"type": "output_text", "text": "kept assistant"}]
        }),
        json!({
            "type": "compaction",
            "encrypted_content": "encrypted-summary"
        }),
    ];

    let replacement = process_remote_compaction_output(output);

    assert_eq!(replacement.len(), 2);
    assert_eq!(replacement[0]["role"], "assistant");
    assert_eq!(replacement[1]["type"], "compaction");
}

#[test]
fn fallback_remote_compaction_history_retains_recent_user_messages_under_budget() {
    let prompt_input = vec![
        json!({
            "role": "developer",
            "content": [{"type": "input_text", "text": "not retained"}]
        }),
        json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "first"}]
        }),
        json!({
            "type": "function_call_output",
            "call_id": "call_1",
            "output": "{}"
        }),
        json!({
            "role": "user",
            "content": [{"type": "input_text", "text": "second"}]
        }),
    ];

    let replacement = install_remote_compaction_history(&prompt_input, Vec::new());

    assert_eq!(replacement.len(), 2);
    assert_eq!(message_text_from_value(&replacement[0]), "first");
    assert_eq!(message_text_from_value(&replacement[1]), "second");
}

#[test]
fn remote_compaction_above_threshold_gets_local_pressure_pass() {
    let remote_output = (0..12)
        .map(|index| {
            json!({
                "type": "message",
                "role": "user",
                "content": [{
                    "type": "input_text",
                    "text": format!("remote retained message {index} {}", "x".repeat(10_000))
                }]
            })
        })
        .collect::<Vec<_>>();

    let (replacement, pressure) = compact_remote_history_to_threshold(&[], remote_output, 90_000)
        .expect("compact remote history");
    let final_chars = serde_json::to_string(&replacement)
        .expect("serialize replacement")
        .len();
    let pressure = pressure.expect("local pressure report");
    let remote_after_chars = pressure["remote_after_chars"]
        .as_u64()
        .expect("remote after chars") as usize;

    assert_eq!(pressure["reason"], "remote_compaction_above_threshold");
    assert!(remote_after_chars > 90_000);
    assert!(final_chars < remote_after_chars);
    assert_eq!(pressure["made_progress"], true);
    assert_eq!(
        pressure["final_chars"].as_u64().expect("final chars") as usize,
        final_chars
    );
}

#[test]
fn remote_compaction_summary_compacts_replayed_large_user_message() {
    let remote_output = vec![
        json!({
            "type": "message",
            "role": "user",
            "status": "completed",
            "id": "msg_replayed",
            "content": [{
                "type": "input_text",
                "text": format!("important instruction\n{}\nfinal instruction", "x".repeat(180_000))
            }]
        }),
        json!({
            "type": "compaction_summary",
            "encrypted_content": "encrypted-summary",
        }),
    ];

    let (replacement, pressure) = compact_remote_history_to_threshold(&[], remote_output, 100_000)
        .expect("compact remote history");
    let pressure = pressure.expect("local pressure report");
    let final_chars = serde_json::to_string(&replacement)
        .expect("serialize replacement")
        .len();

    assert_eq!(pressure["made_progress"], true);
    assert!(final_chars < 100_000);
    assert_eq!(replacement[1]["type"], "compaction");
    let retained = message_text_from_value(&replacement[0]);
    assert!(retained.contains("important instruction"));
    assert!(retained.contains("final instruction"));
    assert!(retained.contains("[spark local message compaction]"));
    assert!(retained.contains("exact_content=omitted"));
}

#[test]
fn local_compaction_report_splits_tool_outputs_and_messages() {
    let mut input = (0..12)
        .flat_map(|index| {
            [
                json!({
                    "role": "user",
                    "content": [{
                        "type": "input_text",
                        "text": format!("message {index} {}", "m".repeat(10_000))
                    }]
                }),
                json!({
                    "type": "function_call_output",
                    "call_id": format!("call_{index}"),
                    "output": "o".repeat(10_000)
                }),
            ]
        })
        .collect::<Vec<_>>();

    let report = compact_input_locally(&mut input, 40_000)
        .expect("local compact")
        .expect("report");

    assert!(
        report["compacted_tool_outputs"]
            .as_u64()
            .expect("tool outputs")
            > 0
    );
    assert!(report["compacted_messages"].as_u64().expect("messages") > 0);
    assert_eq!(
        report["compacted_outputs"],
        json!(
            report["compacted_tool_outputs"].as_u64().unwrap()
                + report["compacted_messages"].as_u64().unwrap()
        )
    );
}

#[test]
fn local_compaction_can_shrink_single_large_recent_user_message() {
    let mut input = vec![json!({
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": format!("must keep start\n{}\nmust keep end", "x".repeat(180_000))
        }]
    })];

    let report = compact_input_locally(&mut input, 40_000)
        .expect("local compact")
        .expect("report");
    let final_chars = serde_json::to_string(&input)
        .expect("serialize compacted input")
        .len();
    let retained = message_text_from_value(&input[0]);

    assert_eq!(report["compacted_tool_outputs"], 0);
    assert_eq!(report["compacted_messages"], 1);
    assert!(final_chars < 40_000);
    assert!(retained.contains("must keep start"));
    assert!(retained.contains("must keep end"));
    assert!(retained.contains("[spark local message compaction]"));
    assert!(retained.contains("retained=head+tail"));
}

#[test]
fn compact_message_item_is_idempotent_for_local_handoff() {
    let mut item = json!({
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": format!("first\n{}\nlast", "x".repeat(20_000))
        }]
    });

    assert!(compact_message_item(&mut item, 1200).expect("first compact"));
    let once = message_text_from_value(&item);
    assert!(!compact_message_item(&mut item, 1200).expect("second compact"));
    let twice = message_text_from_value(&item);

    assert_eq!(once, twice);
    assert!(once.contains("preview_chars="));
}

#[test]
fn post_compaction_notice_tells_agent_to_reconfirm_exact_state() {
    let prompt_input = vec![json!({
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": "After any compaction, use fs.list on src with recursive=false, then answer.\nNext, use fs.read on README.md.\nrow 00001: filler"
        }]
    })];

    let notice = post_compaction_verification_text(&prompt_input).expect("post-compaction notice");

    assert!(notice.contains("[spark post-compaction verification]"));
    assert!(notice.contains("Treat compacted summaries as memory hints, not proof."));
    assert!(notice.contains("run the smallest fresh confirmation tool call"));
    assert!(notice.contains("required_actions=2"));
    assert!(notice.contains("action_1=tool=fs.list path=src recursive=false"));
    assert!(notice.contains("action_2=tool=fs.read path=README.md"));
    assert!(!notice.contains("row 00001"));
}

#[test]
fn post_compaction_notice_is_appended_once_after_retained_history() {
    let prompt_input = vec![json!({
        "role": "user",
        "content": [{
            "type": "input_text",
            "text": "After any compaction, use fs.list on src with recursive=false, then answer."
        }]
    })];
    let mut compacted = vec![json!({
        "type": "compaction",
        "encrypted_content": "encrypted-summary"
    })];

    let first = append_post_compaction_verification_notice(&mut compacted, &prompt_input)
        .expect("first notice");
    let second = append_post_compaction_verification_notice(&mut compacted, &prompt_input)
        .expect("second notice");
    let notices = compacted
        .iter()
        .filter(|item| {
            message_text_from_value(item).contains("[spark post-compaction verification]")
        })
        .count();

    assert_eq!(first.required_actions, 1);
    assert_eq!(second.required_actions, 1);
    assert_eq!(notices, 1);
    assert_eq!(compacted.len(), 2);
    assert_eq!(compacted[0]["type"], "compaction");
}

#[test]
fn local_compaction_handoff_retains_intent_lines_without_filler_rows() {
    let raw = format!(
        "Profile scenario: compaction-pressure.\n\
         This prompt intentionally creates long-context pressure below Spark's 128k context window.\n\
         Let the harness compact automatically if its threshold is crossed.\n\
         Do not restate the synthetic payload. After any compaction, use fs.list on src with recursive=false, then answer with:\n\
         - whether the task remained understandable,\n\
         - which tool you used,\n\
         Next, use fs.read on README.md.\n\
         Then use fs.stat on .spark-scenarios/file-ops/final/report.md.\n\
         Then use fs.write on .spark-scenarios/file-ops/drafts/report-draft.md.\n\
         Then use fs.rename to move .spark-scenarios/file-ops/drafts/report-draft.md to .spark-scenarios/file-ops/final/report.md.\n\
         Synthetic payload follows. Preserve the high-level instruction above; payload rows are intentionally repetitive profiling filler.\n\
         row 00001: {}\n\
         row 00002: {}\n",
        "x".repeat(4000),
        "y".repeat(4000)
    );

    let lines = retained_intent_lines(&raw, 12);
    let block = retained_intent_block(&raw);

    assert!(lines.iter().any(|line| line.contains("Profile scenario")));
    assert!(lines.iter().any(|line| line.contains("fs.list on src")));
    assert!(lines.iter().any(|line| line.contains("which tool")));
    assert!(!lines.iter().any(|line| line.starts_with("row ")));
    assert!(block.contains("retained_intent_lines="));
    assert!(block.contains("intent_1=Profile scenario: compaction-pressure."));
    assert!(block.contains("required_actions=5"));
    assert!(block.contains("action_1=tool=fs.list path=src recursive=false"));
    assert!(block.contains("action_2=tool=fs.read path=README.md"));
    assert!(block.contains(
        "action_3=tool=fs.rename from=.spark-scenarios/file-ops/drafts/report-draft.md to=.spark-scenarios/file-ops/final/report.md"
    ));
    assert!(block.contains("action_4=tool=fs.stat path=.spark-scenarios/file-ops/final/report.md"));
    assert!(
        block.contains(
            "action_5=tool=fs.write path=.spark-scenarios/file-ops/drafts/report-draft.md"
        )
    );
}

#[test]
fn parses_required_native_file_tool_actions_from_intent_lines() {
    let list_action = parse_native_tool_action(
        "Do not restate the synthetic payload. After any compaction, use fs.list on src with recursive=false, then answer with:",
    )
    .expect("list action");
    let read_action =
        parse_native_tool_action("Next, use fs.read on `README.md`.").expect("read action");
    let stat_action =
        parse_native_tool_action("Then use fs.stat on `src/main.rs`.").expect("stat action");
    let search_action =
        parse_native_tool_action("Then run fs.search in src for compact.").expect("search action");
    let write_action = parse_native_tool_action(
        "Then use fs.write on .spark-scenarios/file-ops/drafts/report-draft.md with a short markdown report.",
    )
    .expect("write action");
    let rename_action = parse_native_tool_action(
        "Then use fs.rename to move .spark-scenarios/file-ops/drafts/report-draft.md to .spark-scenarios/file-ops/final/report.md.",
    )
    .expect("rename action");

    assert_eq!(list_action, "tool=fs.list path=src recursive=false");
    assert_eq!(read_action, "tool=fs.read path=README.md");
    assert_eq!(stat_action, "tool=fs.stat path=src/main.rs");
    assert_eq!(search_action, "tool=fs.search path=src");
    assert_eq!(
        write_action,
        "tool=fs.write path=.spark-scenarios/file-ops/drafts/report-draft.md"
    );
    assert_eq!(
        rename_action,
        "tool=fs.rename from=.spark-scenarios/file-ops/drafts/report-draft.md to=.spark-scenarios/file-ops/final/report.md"
    );
}

#[test]
fn local_compaction_report_keeps_aggregate_output_count() {
    let mut input = (0..4)
        .map(|index| {
            json!({
                "type": "function_call_output",
                "call_id": format!("call_{index}"),
                "output": "o".repeat(20_000)
            })
        })
        .collect::<Vec<_>>();

    let report = compact_input_locally(&mut input, 40_000)
        .expect("local compact")
        .expect("report");

    assert_eq!(
        report["compacted_outputs"],
        report["compacted_tool_outputs"]
    );
    assert_eq!(report["compacted_messages"], 0);
}

#[test]
fn compaction_notice_summarizes_remote_report() {
    let notice = format_compaction_notice(&json!({
        "method": "responses_compact",
        "duration_ms": 1234,
        "before_chars": 220_000,
        "after_chars": 80_000
    }));

    assert_eq!(
        notice,
        "compaction: responses_compact 220000->80000 chars in 1234ms"
    );
}

#[test]
fn compaction_notice_marks_local_pressure() {
    let notice = format_compaction_notice(&json!({
        "method": "responses_compact",
        "duration_ms": 1234,
        "before_chars": 220_000,
        "after_chars": 100_000,
        "local_pressure": {
            "made_progress": true
        }
    }));

    assert!(notice.contains("local_pressure=applied"));
}

#[test]
fn context_pressure_reports_live_thresholds() {
    let input = vec![json!({
        "role": "user",
        "content": [{"type": "input_text", "text": "x".repeat(120)}]
    })];

    let pressure = context_pressure_json(&input, 40, 10_000).expect("context pressure");

    assert!(pressure["input_chars"].as_u64().expect("input chars") > 40);
    assert_eq!(pressure["compact_after_exceeded"], true);
    assert_eq!(pressure["max_input_exceeded"], false);
    assert_eq!(pressure["context_window_tokens"], 128_000);
}

#[test]
fn compaction_trigger_prefers_size_then_tool_only_streak() {
    let input = vec![json!({
        "role": "user",
        "content": [{"type": "input_text", "text": "x".repeat(120)}]
    })];

    let trigger = compaction_trigger_for_turn(40, 12, 12, 0, &input).expect("trigger decision");
    assert_eq!(trigger, Some("size_threshold"));

    let trigger = compaction_trigger_for_turn(10_000, 12, 12, 0, &input).expect("trigger decision");
    assert_eq!(trigger, None);

    let large_input = vec![json!({
        "role": "user",
        "content": [{"type": "input_text", "text": "x".repeat(90_000)}]
    })];
    let trigger =
        compaction_trigger_for_turn(100_000, 12, 12, 0, &large_input).expect("trigger decision");
    assert_eq!(trigger, Some(TOOL_ONLY_STREAK_COMPACTION_TRIGGER));

    let trigger =
        compaction_trigger_for_turn(100_000, 12, 12, 12, &large_input).expect("trigger decision");
    assert_eq!(trigger, None);

    let trigger = compaction_trigger_for_turn(10_000, 0, 100, 0, &input).expect("trigger decision");
    assert_eq!(trigger, None);
}
