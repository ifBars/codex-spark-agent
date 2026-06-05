use serde_json::json;

use super::command::bounded_text;
use super::fs::{fs_list, fs_read, fs_search, fs_stat};
use super::*;

#[test]
fn builtin_tools_do_not_include_synthetic_completion_tool() {
    let names = builtin_tools()
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();

    assert_eq!(names.len(), 10);
    assert!(!names.iter().any(|name| name == "agent.complete"));
    assert!(names.iter().any(|name| name == "fs.stat"));
    assert!(names.iter().any(|name| name == "fs.rename"));
    assert!(names.iter().any(|name| name == "cmd.exec"));
    assert!(names.iter().any(|name| name == "web.search"));
}

#[test]
fn ask_mode_advertises_only_readonly_tools() {
    let names = tools_for_mode(builtin_tools(), AgentMode::Ask)
        .into_iter()
        .map(|tool| tool.name)
        .collect::<Vec<_>>();

    assert_eq!(
        names,
        vec!["fs.read", "fs.list", "fs.stat", "fs.search", "web.search"]
    );
}

#[tokio::test]
async fn invoke_returns_structured_error_for_missing_required_args() {
    let dir = tempfile::tempdir().expect("tempdir");

    let result = invoke(dir.path(), AgentMode::Work, "fs.read", json!({"limit": 10})).await;

    assert!(!result.ok);
    assert_eq!(result.data["error_kind"], "invalid_arguments");
    assert_eq!(result.data["tool"], "fs.read");
    assert_eq!(result.data["args_shape"]["limit"], "number");
    assert!(
        result.data["hint"]
            .as_str()
            .expect("hint")
            .contains("required schema fields")
    );
    assert!(
        result
            .error
            .as_deref()
            .expect("error")
            .contains("path is required")
    );
}

#[tokio::test]
async fn invoke_returns_structured_error_for_unknown_tool() {
    let dir = tempfile::tempdir().expect("tempdir");

    let result = invoke(
        dir.path(),
        AgentMode::Work,
        "fs.missing",
        json!({"path": "README.md"}),
    )
    .await;

    assert!(!result.ok);
    assert_eq!(result.data["error_kind"], "unknown_tool");
    assert_eq!(result.data["tool"], "fs.missing");
    assert_eq!(result.data["args_shape"]["path"], "string");
    assert!(
        result.data["hint"]
            .as_str()
            .expect("hint")
            .contains("advertised native tool names")
    );
}

#[tokio::test]
async fn invoke_blocks_mutating_tools_in_ask_mode() {
    let dir = tempfile::tempdir().expect("tempdir");

    let result = invoke(
        dir.path(),
        AgentMode::Ask,
        "fs.write",
        json!({"path": "sample.txt", "content": "blocked"}),
    )
    .await;

    assert!(!result.ok);
    assert!(result.error.as_deref().expect("error").contains("ask mode"));
    assert!(!dir.path().join("sample.txt").exists());
}

#[tokio::test]
async fn invoke_readonly_tools_can_use_reference_roots_but_writes_stay_local() {
    let workspace = tempfile::tempdir().expect("workspace");
    let source = tempfile::tempdir().expect("source");
    std::fs::create_dir_all(source.path().join("src")).expect("src");
    std::fs::write(
        source.path().join("src").join("lib.rs"),
        "pub fn reference_marker() {}\n",
    )
    .expect("write source");

    let read = invoke_with_read_roots(
        workspace.path(),
        &[source.path().to_path_buf()],
        AgentMode::Work,
        "fs.read",
        json!({"path": "src/lib.rs", "limit": 10}),
    )
    .await;
    assert!(read.ok);
    assert!(
        read.data["content"]
            .as_str()
            .expect("content")
            .contains("reference_marker")
    );

    let write = invoke_with_read_roots(
        workspace.path(),
        &[source.path().to_path_buf()],
        AgentMode::Work,
        "fs.write",
        json!({"path": "src/generated.rs", "content": "local only\n"}),
    )
    .await;
    assert!(write.ok);
    assert!(workspace.path().join("src/generated.rs").exists());
    assert!(!source.path().join("src/generated.rs").exists());
}

#[test]
fn fs_search_returns_matching_line_snippets() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("sample.txt"), "alpha\nneedle here\nomega\n")
        .expect("write sample");

    let result = fs_search(
        dir.path(),
        json!({"query": "needle", "path": ".", "context_lines": 1}),
    )
    .expect("search");

    assert!(result.ok);
    assert_eq!(result.data["matches"][0]["path"], "sample.txt");
    assert_eq!(result.data["matches"][0]["line"], 2);
    assert!(
        result.data["matches"][0]["snippet"]
            .as_str()
            .expect("snippet")
            .contains("1: alpha")
    );
}

#[test]
fn fs_search_keeps_literal_default_and_supports_regex_mode() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("sample.txt"),
        "alpha 123\nalpha digits\nomega\n",
    )
    .expect("write sample");

    let literal_result =
        fs_search(dir.path(), json!({"query": r"\d+", "path": "."})).expect("literal search");
    assert_eq!(
        literal_result.data["matches"]
            .as_array()
            .expect("literal matches")
            .len(),
        0
    );
    assert_eq!(literal_result.data["regex"], false);

    let regex_result = fs_search(
        dir.path(),
        json!({"query": r"\d+", "path": ".", "regex": true}),
    )
    .expect("regex search");
    let matches = regex_result.data["matches"].as_array().expect("matches");
    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["path"], "sample.txt");
    assert_eq!(matches[0]["line"], 1);
    assert_eq!(regex_result.data["regex"], true);
}

#[test]
fn fs_read_reports_window_metadata() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("sample.txt"), "one\ntwo\nthree\n").expect("write sample");

    let result = fs_read(
        dir.path(),
        json!({"path": "sample.txt", "offset": 1, "limit": 2}),
    )
    .expect("read");

    assert_eq!(result.data["returned_lines"], 2);
    assert_eq!(result.data["total_lines"], 3);
    assert_eq!(result.data["has_more"], true);
    assert_eq!(result.data["next_offset"], 3);
}

#[test]
fn fs_read_decodes_utf16le_with_bom() {
    let dir = tempfile::tempdir().expect("tempdir");
    let mut bytes = vec![0xFF, 0xFE];
    for unit in "total=5\r\nfailed=2\r\n".encode_utf16() {
        bytes.extend_from_slice(&unit.to_le_bytes());
    }
    std::fs::write(dir.path().join("summary.txt"), bytes).expect("write summary");

    let result = fs_read(
        dir.path(),
        json!({"path": "summary.txt", "line_numbers": false}),
    )
    .expect("read");

    let content = result.data["content"].as_str().expect("content");
    assert!(content.contains("total=5"));
    assert!(content.contains("failed=2"));
}

#[test]
fn fs_read_defaults_to_small_windows_and_clamps_large_limits() {
    let dir = tempfile::tempdir().expect("tempdir");
    let content = (1..=600)
        .map(|line| format!("line {line}"))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.path().join("sample.txt"), content).expect("write sample");

    let default_result = fs_read(dir.path(), json!({"path": "sample.txt"})).expect("read");
    assert_eq!(default_result.data["limit"], 120);
    assert_eq!(default_result.data["returned_lines"], 120);
    assert_eq!(default_result.data["has_more"], true);
    assert_eq!(default_result.data["next_offset"], 121);

    let clamped_result =
        fs_read(dir.path(), json!({"path": "sample.txt", "limit": 2000})).expect("read");
    assert_eq!(clamped_result.data["limit"], 400);
    assert_eq!(clamped_result.data["returned_lines"], 400);
    assert_eq!(clamped_result.data["next_offset"], 401);
}

#[test]
fn fs_read_caps_content_chars_even_when_lines_are_long() {
    let dir = tempfile::tempdir().expect("tempdir");
    let content = (1..=20)
        .map(|line| format!("line {line} {}", "x".repeat(1000)))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.path().join("sample.txt"), content).expect("write sample");

    let result = fs_read(
        dir.path(),
        json!({"path": "sample.txt", "limit": 20, "line_numbers": false}),
    )
    .expect("read");

    assert_eq!(result.data["content_truncated"], true);
    assert!(
        result.data["returned_lines"]
            .as_u64()
            .expect("returned lines")
            < 20
    );
    assert_eq!(result.data["has_more"], true);
    assert!(result.data["content"].as_str().expect("content").len() <= 12_000);
}

#[test]
fn fs_stat_reports_existing_file_metadata_without_contents() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("sample.txt"), "hello").expect("write sample");

    let result = fs_stat(dir.path(), json!({"path": "sample.txt"})).expect("stat");

    assert!(result.ok);
    assert_eq!(result.data["path"], "sample.txt");
    assert_eq!(result.data["exists"], true);
    assert_eq!(result.data["is_file"], true);
    assert_eq!(result.data["is_dir"], false);
    assert_eq!(result.data["size"], 5);
    assert!(result.data.get("content").is_none());
}

#[test]
fn fs_stat_reports_missing_workspace_path_without_error() {
    let dir = tempfile::tempdir().expect("tempdir");

    let result = fs_stat(dir.path(), json!({"path": "missing/sample.txt"})).expect("stat");

    assert!(result.ok);
    assert_eq!(result.data["path"], "missing/sample.txt");
    assert_eq!(result.data["exists"], false);
}

#[test]
fn fs_list_skips_generated_dirs_during_recursive_discovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("target/debug")).expect("create target");
    std::fs::create_dir_all(dir.path().join(".spark-runs/run-1")).expect("create traces");
    std::fs::create_dir_all(dir.path().join("src")).expect("create src");
    std::fs::write(dir.path().join("src/main.rs"), "fn main() {}\n").expect("write src");
    std::fs::write(dir.path().join("target/debug/generated.txt"), "generated\n")
        .expect("write generated");

    let result = fs_list(
        dir.path(),
        json!({"path": ".", "recursive": true, "max_depth": 4, "limit": 100}),
    )
    .expect("list");
    let entries = result.data["entries"].as_array().expect("entries");
    let paths = entries
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect::<Vec<_>>();

    assert!(paths.contains(&"src"));
    assert!(paths.contains(&"src/main.rs"));
    assert!(!paths.iter().any(|path| path.starts_with("target")));
    assert!(!paths.iter().any(|path| path.starts_with(".spark-runs")));
}

#[test]
fn fs_list_clamps_limits_and_caps_result_chars() {
    let dir = tempfile::tempdir().expect("tempdir");
    for index in 0..300 {
        let path = dir.path().join(format!(
            "src/module_{index:03}/very_long_named_file_{index:03}.rs"
        ));
        std::fs::create_dir_all(path.parent().expect("parent")).expect("create parent");
        std::fs::write(path, "fn demo() {}\n").expect("write file");
    }

    let result = fs_list(
        dir.path(),
        json!({"path": ".", "recursive": true, "max_depth": 4, "limit": 500}),
    )
    .expect("list");

    assert_eq!(result.data["limit"], 200);
    assert_eq!(result.data["truncated"], true);
    assert_eq!(result.data["entries_truncated_by_chars"], true);
    assert!(
        serde_json::to_string(result.data["entries"].as_array().expect("entries"))
            .expect("serialize entries")
            .len()
            <= 12_000
    );
}

#[test]
fn fs_search_skips_generated_dirs_during_recursive_discovery() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join("target")).expect("create target");
    std::fs::create_dir_all(dir.path().join(".spark-scenarios/case")).expect("create scenarios");
    std::fs::create_dir_all(dir.path().join("src")).expect("create src");
    std::fs::write(dir.path().join("target/generated.txt"), "needle\n").expect("write target");
    std::fs::write(
        dir.path().join(".spark-scenarios/case/generated.txt"),
        "needle\n",
    )
    .expect("write scenario");
    std::fs::write(dir.path().join("src/main.rs"), "needle\n").expect("write src");

    let result = fs_search(
        dir.path(),
        json!({"query": "needle", "path": ".", "max_depth": 4}),
    )
    .expect("search");
    let matches = result.data["matches"].as_array().expect("matches");
    let paths = matches
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect::<Vec<_>>();

    assert_eq!(paths, vec!["src/main.rs"]);
}

#[test]
fn fs_search_allows_explicit_generated_fixture_path() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::create_dir_all(dir.path().join(".spark-scenarios/case")).expect("create scenario");
    std::fs::write(
        dir.path().join(".spark-scenarios/case/source.txt"),
        "needle\n",
    )
    .expect("write scenario");

    let result = fs_search(
        dir.path(),
        json!({"query": "needle", "path": ".spark-scenarios/case", "max_depth": 4}),
    )
    .expect("search");
    let matches = result.data["matches"].as_array().expect("matches");

    assert_eq!(matches.len(), 1);
    assert_eq!(matches[0]["path"], ".spark-scenarios/case/source.txt");
    assert!(
        result.data["files_scanned"]
            .as_u64()
            .expect("files scanned")
            >= 1
    );
}

#[test]
fn fs_search_clamps_limits_and_truncates_large_snippets() {
    let dir = tempfile::tempdir().expect("tempdir");
    let content = (1..=150)
        .map(|line| format!("needle {line} {}", "x".repeat(1000)))
        .collect::<Vec<_>>()
        .join("\n");
    std::fs::write(dir.path().join("sample.txt"), content).expect("write sample");

    let result = fs_search(
        dir.path(),
        json!({"query": "needle", "path": ".", "limit": 500, "context_lines": 5}),
    )
    .expect("search");

    let matches = result.data["matches"].as_array().expect("matches");
    assert_eq!(result.data["limit"], 100);
    assert_eq!(matches.len(), 100);
    assert_eq!(result.data["truncated"], true);
    assert_eq!(result.data["snippets_truncated"], 100);
    assert!(matches[0]["snippet"].as_str().expect("snippet").len() <= 600);
    assert_eq!(matches[0]["snippet_truncated"], true);
}

#[test]
fn fs_write_reports_created_file_and_creates_parent_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");

    let result = fs_write(
        dir.path(),
        json!({
            "path": "nested/sample.txt",
            "content": "hello"
        }),
    )
    .expect("write");

    assert!(result.ok);
    assert!(
        result.data["path"]
            .as_str()
            .expect("path")
            .ends_with("sample.txt")
    );
    assert_eq!(result.data["bytes"], 5);
    assert_eq!(result.data["previous_bytes"], Value::Null);
    assert_eq!(result.data["created"], true);
    assert_eq!(result.data["created_parent_dirs"], json!(["nested"]));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("nested/sample.txt")).expect("read"),
        "hello"
    );
}

#[test]
fn fs_write_reports_overwritten_file_size() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("sample.txt"), "old").expect("write old");

    let result = fs_write(
        dir.path(),
        json!({
            "path": "sample.txt",
            "content": "new content"
        }),
    )
    .expect("write");

    assert!(result.ok);
    assert_eq!(result.data["bytes"], 11);
    assert_eq!(result.data["previous_bytes"], 3);
    assert_eq!(result.data["created"], false);
    assert_eq!(result.data["created_parent_dirs"], json!([]));
}

#[test]
fn fs_rename_moves_file_and_creates_parent_dirs() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("old.txt"), "hello").expect("write old");

    let result = fs_rename(
        dir.path(),
        json!({"from": "old.txt", "to": "nested/new.txt"}),
    )
    .expect("rename");

    assert!(result.ok);
    assert_eq!(result.data["from"], "old.txt");
    assert_eq!(result.data["to"], "nested/new.txt");
    assert_eq!(result.data["is_file"], true);
    assert_eq!(result.data["created_parent_dirs"], json!(["nested"]));
    assert!(!dir.path().join("old.txt").exists());
    assert_eq!(
        std::fs::read_to_string(dir.path().join("nested/new.txt")).expect("read new"),
        "hello"
    );
}

#[test]
fn fs_rename_refuses_to_overwrite_destination() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(dir.path().join("old.txt"), "old").expect("write old");
    std::fs::write(dir.path().join("new.txt"), "new").expect("write new");

    let error = fs_rename(dir.path(), json!({"from": "old.txt", "to": "new.txt"}))
        .expect_err("rename should fail");

    assert!(error.to_string().contains("destination already exists"));
    assert_eq!(
        std::fs::read_to_string(dir.path().join("old.txt")).expect("read old"),
        "old"
    );
    assert_eq!(
        std::fs::read_to_string(dir.path().join("new.txt")).expect("read new"),
        "new"
    );
}

#[test]
fn fs_write_rejects_parent_dir_escape_for_missing_paths() {
    let dir = tempfile::tempdir().expect("tempdir");

    let error = fs_write(
        dir.path(),
        json!({
            "path": "../outside.txt",
            "content": "bad"
        }),
    )
    .expect_err("escape should fail");

    assert!(error.to_string().contains("path escapes workspace"));
}

#[test]
fn bounded_text_preserves_short_streams() {
    let output = bounded_text("short output", 100);

    assert_eq!(output.text, "short output");
    assert_eq!(output.original_chars, 12);
    assert!(!output.truncated);
}

#[test]
fn bounded_text_keeps_head_tail_and_metadata_for_long_streams() {
    let raw = format!("{}{}", "a".repeat(80), "z".repeat(80));
    let output = bounded_text(&raw, 80);

    assert!(output.truncated);
    assert_eq!(output.original_chars, 160);
    assert!(output.text.contains("command stream truncated"));
    assert!(output.text.starts_with("aaa"));
    assert!(output.text.ends_with("zzz"));
    assert!(output.text.chars().count() <= 80);
}

#[test]
fn fs_replace_requires_expected_count_before_writing() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.txt");
    std::fs::write(&path, "alpha beta beta\n").expect("write sample");

    let error = fs_replace(
        dir.path(),
        json!({
            "path": "sample.txt",
            "old": "beta",
            "new": "gamma",
            "expected_replacements": 1
        }),
    )
    .expect_err("expected count mismatch");

    assert!(error.to_string().contains("expected 1 replacements"));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read unchanged"),
        "alpha beta beta\n"
    );
}

#[test]
fn fs_replace_updates_exact_text() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.txt");
    std::fs::write(&path, "alpha beta\n").expect("write sample");

    let result = fs_replace(
        dir.path(),
        json!({
            "path": "sample.txt",
            "old": "beta",
            "new": "gamma",
            "expected_replacements": 1
        }),
    )
    .expect("replace");

    assert!(result.ok);
    assert_eq!(result.data["replacements"], 1);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read updated"),
        "alpha gamma\n"
    );
}

#[test]
fn fs_edit_replaces_inclusive_line_range() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.txt");
    std::fs::write(&path, "one\ntwo\nthree\n").expect("write sample");

    let result = fs_edit(
        dir.path(),
        json!({
            "path": "sample.txt",
            "start_line": 2,
            "end_line": 2,
            "replacement": "TWO",
            "expected_old": "two"
        }),
    )
    .expect("edit");

    assert!(result.ok);
    assert_eq!(result.data["old_lines"], 1);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read updated"),
        "one\nTWO\nthree\n"
    );
}

#[test]
fn fs_edit_inserts_when_end_line_precedes_start_line() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.txt");
    std::fs::write(&path, "one\nthree\n").expect("write sample");

    let result = fs_edit(
        dir.path(),
        json!({
            "path": "sample.txt",
            "start_line": 2,
            "end_line": 1,
            "replacement": "two"
        }),
    )
    .expect("insert");

    assert!(result.ok);
    assert_eq!(result.data["old_lines"], 0);
    assert_eq!(
        std::fs::read_to_string(&path).expect("read updated"),
        "one\ntwo\nthree\n"
    );
}

#[test]
fn fs_edit_expected_old_mismatch_does_not_write() {
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("sample.txt");
    std::fs::write(&path, "one\ntwo\nthree\n").expect("write sample");

    let error = fs_edit(
        dir.path(),
        json!({
            "path": "sample.txt",
            "start_line": 2,
            "end_line": 2,
            "replacement": "TWO",
            "expected_old": "wrong"
        }),
    )
    .expect_err("expected mismatch");

    assert!(error.to_string().contains("expected_old did not match"));
    assert_eq!(
        std::fs::read_to_string(&path).expect("read unchanged"),
        "one\ntwo\nthree\n"
    );
}

#[tokio::test]
async fn cmd_exec_reports_timeouts_as_tool_failures() {
    let dir = tempfile::tempdir().expect("tempdir");
    let command = if cfg!(target_os = "windows") {
        "Start-Sleep -Milliseconds 1000"
    } else {
        "sleep 1"
    };

    let result = cmd_exec(
        dir.path(),
        json!({
            "command": command,
            "timeout_ms": 100
        }),
    )
    .await
    .expect("cmd result");

    assert!(!result.ok);
    assert_eq!(result.data["timed_out"], true);
    assert_eq!(result.data["timeout_ms"], 100);
    assert_eq!(
        result.error.as_deref(),
        Some("command timed out after 100ms")
    );
}

#[tokio::test]
async fn cmd_exec_reports_nonzero_exit_as_tool_error() {
    let dir = tempfile::tempdir().expect("tempdir");
    let command = if cfg!(target_os = "windows") {
        "Write-Output nope; exit 7"
    } else {
        "printf nope; exit 7"
    };

    let result = cmd_exec(
        dir.path(),
        json!({
            "command": command,
            "timeout_ms": 5000
        }),
    )
    .await
    .expect("cmd result");

    assert!(!result.ok);
    assert_eq!(result.data["code"], 7);
    assert_eq!(result.error.as_deref(), Some("command exited with code 7"));
    let stdout = result.data["stdout"].as_str().expect("stdout");
    assert!(stdout.contains("nope"));
}

#[tokio::test]
async fn cmd_exec_reports_windows_separator_hint() {
    if !cfg!(target_os = "windows") {
        return;
    }
    let dir = tempfile::tempdir().expect("tempdir");

    let result = cmd_exec(
        dir.path(),
        json!({
            "command": "Write-Output one && Write-Output two",
            "timeout_ms": 5000
        }),
    )
    .await
    .expect("cmd result");

    assert!(!result.ok);
    assert_eq!(result.data["shell"], "powershell");
    let hint = result.data["hint"].as_str().expect("hint");
    assert!(hint.contains("does not support &&"));
    assert!(
        result
            .error
            .as_deref()
            .expect("error")
            .contains("does not support &&")
    );
}
