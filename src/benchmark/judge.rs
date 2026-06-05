use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::{client, config};

pub(crate) const DEFAULT_JUDGE_MODEL: &str = "gpt-5.5";
const MAX_SNIPPET_CHARS: usize = 8_000;
const MAX_EVIDENCE_CHARS_PER_RUN: usize = 22_000;

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkJudgeOptions {
    pub(crate) cwd: PathBuf,
    pub(crate) comparison_report: PathBuf,
    pub(crate) model: String,
    pub(crate) reasoning_effort: String,
    pub(crate) output_dir: PathBuf,
    pub(crate) limit: Option<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct BenchmarkJudgeOutput {
    pub(crate) json_path: PathBuf,
    pub(crate) rows: usize,
}

#[derive(Debug, Clone, Deserialize)]
struct ComparisonReport {
    suite: String,
    rows: Vec<ComparisonRunRow>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
struct ComparisonRunRow {
    runner: String,
    suite: String,
    scenario: String,
    model: String,
    completion_score: f64,
    quality_score: f64,
    process_score: f64,
    success: bool,
    validation_exit_code: Option<i32>,
    validation_timed_out: bool,
    duration_ms: u128,
    tool_or_item_calls: u64,
    input_tokens: u64,
    output_tokens: u64,
    source_files: u64,
    source_bytes: u64,
    failure_points: String,
    source: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BenchmarkJudgeReport {
    pub(crate) suite: String,
    pub(crate) comparison_report: String,
    pub(crate) generated_at_unix_ms: u128,
    pub(crate) rows: Vec<BenchmarkJudgeScenario>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BenchmarkJudgeScenario {
    pub(crate) scenario: String,
    pub(crate) scores: Vec<BenchmarkJudgeRunnerScore>,
    pub(crate) verdict: String,
    pub(crate) rationale: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub(crate) raw_response: String,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub(crate) struct BenchmarkJudgeRunnerScore {
    pub(crate) runner: String,
    pub(crate) solution_score: f64,
    pub(crate) process_score: f64,
    pub(crate) confidence: f64,
    pub(crate) notes: String,
}

pub(crate) async fn write_llm_judge_report(
    options: BenchmarkJudgeOptions,
) -> Result<BenchmarkJudgeOutput> {
    let raw = std::fs::read_to_string(&options.comparison_report).with_context(|| {
        format!(
            "failed to read comparison report {}",
            options.comparison_report.display()
        )
    })?;
    let comparison: ComparisonReport = serde_json::from_str(&raw).with_context(|| {
        format!(
            "failed to parse comparison report {}",
            options.comparison_report.display()
        )
    })?;
    let pairs = matched_scenario_pairs(&comparison.rows);
    if pairs.is_empty() {
        anyhow::bail!("comparison report has no matched Spark/Codex scenario pairs to judge");
    }

    std::fs::create_dir_all(&options.output_dir).with_context(|| {
        format!(
            "failed to create benchmark judge directory {}",
            options.output_dir.display()
        )
    })?;

    let auth = config::load_auth()?;
    let judge = client::SparkClient::new(auth, options.model.clone());
    let mut rows = Vec::new();
    for (scenario, runs) in pairs.into_iter().take(options.limit.unwrap_or(usize::MAX)) {
        let prompt = judge_prompt(&options.cwd, &scenario, &runs)?;
        let input = [json!({
            "role": "user",
            "content": [{"type": "input_text", "text": prompt}]
        })];
        let (response, _) = judge
            .responses_create_judge(&input, &options.reasoning_effort, |_| {})
            .await?;
        let text = client::response_text(&response);
        let mut scenario_score = parse_judge_response(&scenario, &text)?;
        scenario_score.raw_response = text;
        rows.push(scenario_score);
    }

    let stamp = unix_millis();
    let report = BenchmarkJudgeReport {
        suite: comparison.suite,
        comparison_report: options.comparison_report.display().to_string(),
        generated_at_unix_ms: stamp,
        rows,
    };
    let json_path = options
        .output_dir
        .join(format!("{}-llm-judge-{stamp}.json", report.suite));
    std::fs::write(&json_path, serde_json::to_string_pretty(&report)?)
        .with_context(|| format!("failed to write {}", json_path.display()))?;

    Ok(BenchmarkJudgeOutput {
        rows: report.rows.len(),
        json_path,
    })
}

fn matched_scenario_pairs(
    rows: &[ComparisonRunRow],
) -> Vec<(String, BTreeMap<String, ComparisonRunRow>)> {
    let mut by_scenario = BTreeMap::<String, BTreeMap<String, ComparisonRunRow>>::new();
    for row in rows {
        by_scenario
            .entry(row.scenario.clone())
            .or_default()
            .insert(row.runner.clone(), row.clone());
    }
    by_scenario
        .into_iter()
        .filter(|(_, runners)| {
            has_runner_family(runners, "spark-harness") && has_runner_family(runners, "codex-cli")
        })
        .collect()
}

fn has_runner_family(runners: &BTreeMap<String, ComparisonRunRow>, family: &str) -> bool {
    let prefix = format!("{family}/");
    runners
        .keys()
        .any(|runner| runner == family || runner.starts_with(&prefix))
}

fn judge_prompt(
    cwd: &Path,
    scenario: &str,
    runs: &BTreeMap<String, ComparisonRunRow>,
) -> Result<String> {
    let mut evidence = Vec::new();
    for row in runs.values() {
        evidence.push(json!({
            "row": row,
            "artifacts": run_artifact_evidence(cwd, &row.source)?,
        }));
    }
    let score_template = runs
        .keys()
        .map(|runner| {
            json!({
                "runner": runner,
                "solution_score": 0,
                "process_score": 0,
                "confidence": 0,
                "notes": "short evidence-backed note",
            })
        })
        .collect::<Vec<_>>();
    let verdict_options = runs
        .keys()
        .map(String::as_str)
        .chain(["tie", "inconclusive"])
        .collect::<Vec<_>>()
        .join("|");
    Ok(format!(
        r#"You are judging a benchmark comparison for real coding-agent work.

Scenario: {scenario}

Rubric:
- solution_score answers this generic real-world quality question: "Did the model provide a real solution to the prompted request/problem/task?"
- Score solution_score only from the delivered answer, files, artifacts, and validation evidence for the original prompt. Do not include speed, token usage, request counts, turn counts, or how hard the run seemed to work.
- solution_score bands:
  - 0-19: no usable task solution, empty output, irrelevant output, or failed run with no recoverable result.
  - 20-39: recognizes the task but mostly diagnoses, plans, or produces fragments instead of a working solution.
  - 40-59: partial solution with important missing requirements, broken artifacts, or weak evidence that it works.
  - 60-79: plausible real solution but incomplete, under-validated, over-broad, or risky for production use.
  - 80-94: real solution that satisfies the prompt with minor gaps or limited validation.
  - 95-100: complete production-quality solution with clear evidence, correct scope, and no material missing requirements.
- process_score is 0-100 and should heavily consider whether the runner self-tested, inspected failures, recovered cleanly, avoided irrelevant exploration, and stopped after completion.
- External validation is important evidence, but do not blindly trust a single automated score when artifact evidence or final messages contradict it.
- A fast broken solution must score below a slower complete solution.
- Do not award points for post-run repair; the run evidence here is the benchmark run plus external validation only.
- Do not use or cite request counts or turn counts; those protocol units are intentionally not comparable between runners.
- Compare runners on the same scenario only.
- Return exactly one scores entry for every runner present in the evidence JSON.

Return JSON only with this exact shape:
{{
  "scenario": "{scenario}",
  "scores": {},
  "verdict": "{}",
  "rationale": "short comparison rationale"
}}

Evidence JSON:
```json
{}
```"#,
        serde_json::to_string_pretty(&score_template)?,
        verdict_options,
        serde_json::to_string_pretty(&evidence)?
    ))
}

fn run_artifact_evidence(cwd: &Path, source: &str) -> Result<Value> {
    let source_path = resolve_source_path(cwd, source);
    let mut total = 0usize;
    let mut artifacts = Vec::new();
    for name in [
        "scenario-validation.json",
        "last-message.txt",
        "prompt.txt",
        "stderr.txt",
        "stdout.jsonl",
    ] {
        push_artifact(&source_path, name, &mut artifacts, &mut total)?;
    }
    if let Some(summary) = profile_summary_path(&source_path)? {
        push_sanitized_json_artifact_path(&summary, &mut artifacts, &mut total)?;
    }
    if let Some(final_message) = spark_final_message(&source_path)? {
        let snippet = bounded_chars(&final_message, MAX_SNIPPET_CHARS);
        total += snippet.len();
        artifacts.push(json!({
            "path": source_path.join("<spark-final-message>").display().to_string(),
            "chars": final_message.len(),
            "snippet": snippet,
            "truncated": final_message.len() > MAX_SNIPPET_CHARS,
        }));
    }
    Ok(json!({
        "source_path": source_path.display().to_string(),
        "files": artifacts,
        "truncated_after_chars": total >= MAX_EVIDENCE_CHARS_PER_RUN,
    }))
}

fn resolve_source_path(cwd: &Path, source: &str) -> PathBuf {
    let path = PathBuf::from(source);
    if path.is_absolute() {
        path
    } else {
        cwd.join(path)
    }
}

fn push_artifact(
    source_path: &Path,
    name: &str,
    artifacts: &mut Vec<Value>,
    total: &mut usize,
) -> Result<()> {
    push_artifact_path(&source_path.join(name), artifacts, total)
}

fn push_artifact_path(path: &Path, artifacts: &mut Vec<Value>, total: &mut usize) -> Result<()> {
    if *total >= MAX_EVIDENCE_CHARS_PER_RUN || !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read judge evidence {}", path.display()))?;
    let remaining = MAX_EVIDENCE_CHARS_PER_RUN.saturating_sub(*total);
    let max_chars = MAX_SNIPPET_CHARS.min(remaining);
    let snippet = bounded_chars(&raw, max_chars);
    *total += snippet.len();
    artifacts.push(json!({
        "path": path.display().to_string(),
        "chars": raw.len(),
        "snippet": snippet,
        "truncated": raw.len() > max_chars,
    }));
    Ok(())
}

fn push_sanitized_json_artifact_path(
    path: &Path,
    artifacts: &mut Vec<Value>,
    total: &mut usize,
) -> Result<()> {
    if *total >= MAX_EVIDENCE_CHARS_PER_RUN || !path.exists() {
        return Ok(());
    }
    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read judge evidence {}", path.display()))?;
    let sanitized = match serde_json::from_str::<Value>(&raw) {
        Ok(mut value) => {
            redact_non_comparable_protocol_units(&mut value);
            serde_json::to_string_pretty(&value)?
        }
        Err(_) => raw,
    };
    let remaining = MAX_EVIDENCE_CHARS_PER_RUN.saturating_sub(*total);
    let max_chars = MAX_SNIPPET_CHARS.min(remaining);
    let snippet = bounded_chars(&sanitized, max_chars);
    *total += snippet.len();
    artifacts.push(json!({
        "path": path.display().to_string(),
        "chars": sanitized.len(),
        "snippet": snippet,
        "truncated": sanitized.len() > max_chars,
        "redacted": "request and turn counters removed because they are not comparable across runners",
    }));
    Ok(())
}

fn redact_non_comparable_protocol_units(value: &mut Value) {
    match value {
        Value::Object(map) => {
            map.retain(|key, nested| {
                let normalized = key.to_ascii_lowercase();
                let remove = normalized.contains("request") || normalized.contains("turn");
                if !remove {
                    redact_non_comparable_protocol_units(nested);
                }
                !remove
            });
        }
        Value::Array(items) => {
            for item in items {
                redact_non_comparable_protocol_units(item);
            }
        }
        _ => {}
    }
}

fn profile_summary_path(source_path: &Path) -> Result<Option<PathBuf>> {
    if !source_path.exists() {
        return Ok(None);
    }
    for entry in std::fs::read_dir(source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with("-profile-summary.json") {
            return Ok(Some(path));
        }
    }
    Ok(None)
}

fn spark_final_message(source_path: &Path) -> Result<Option<String>> {
    let Some(path) = latest_response_path(source_path)? else {
        return Ok(None);
    };
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("failed to read Spark response {}", path.display()))?;
    let value: Value = serde_json::from_str(&raw)
        .with_context(|| format!("failed to parse Spark response {}", path.display()))?;
    let mut texts = Vec::new();
    collect_output_text(&value, &mut texts);
    let text = texts
        .into_iter()
        .filter(|text| !text.trim().is_empty())
        .last()
        .unwrap_or_default();
    Ok((!text.trim().is_empty()).then_some(text))
}

fn latest_response_path(source_path: &Path) -> Result<Option<PathBuf>> {
    if !source_path.exists() {
        return Ok(None);
    }
    let mut paths = Vec::new();
    for entry in std::fs::read_dir(source_path)
        .with_context(|| format!("failed to read {}", source_path.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
            continue;
        };
        if name.ends_with("-response.json") {
            paths.push(path);
        }
    }
    paths.sort();
    Ok(paths.pop())
}

fn collect_output_text(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(object) => {
            if object.get("type").and_then(Value::as_str) == Some("output_text")
                && let Some(text) = object.get("text").and_then(Value::as_str)
            {
                out.push(text.to_string());
            }
            for value in object.values() {
                collect_output_text(value, out);
            }
        }
        Value::Array(values) => {
            for value in values {
                collect_output_text(value, out);
            }
        }
        _ => {}
    }
}

fn bounded_chars(raw: &str, max_chars: usize) -> String {
    if raw.chars().count() <= max_chars {
        return raw.to_string();
    }
    let mut out = raw.chars().take(max_chars).collect::<String>();
    out.push_str("\n...<truncated>");
    out
}

fn parse_judge_response(scenario: &str, text: &str) -> Result<BenchmarkJudgeScenario> {
    let json_text = extract_json_object(text).ok_or_else(|| {
        anyhow::anyhow!("judge response for {scenario} did not contain a JSON object: {text}")
    })?;
    let mut parsed: BenchmarkJudgeScenario = serde_json::from_str(json_text)
        .with_context(|| format!("failed to parse judge JSON for {scenario}: {json_text}"))?;
    parsed.scenario = scenario.to_string();
    for score in &mut parsed.scores {
        score.solution_score = score.solution_score.clamp(0.0, 100.0);
        score.process_score = score.process_score.clamp(0.0, 100.0);
        score.confidence = score.confidence.clamp(0.0, 100.0);
    }
    Ok(parsed)
}

fn extract_json_object(text: &str) -> Option<&str> {
    let start = text.find('{')?;
    let end = text.rfind('}')?;
    (end > start).then_some(&text[start..=end])
}

fn unix_millis() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn judge_prompt_scores_real_world_task_solution() {
        let prompt =
            judge_prompt(Path::new("."), "repo-survey", &BTreeMap::new()).expect("build prompt");

        assert!(prompt.contains(
            r#"Did the model provide a real solution to the prompted request/problem/task?"#
        ));
        assert!(prompt.contains("Do not include speed"));
        assert!(prompt.contains("95-100: complete production-quality solution"));
    }

    #[test]
    fn judge_prompt_requires_scores_for_all_present_runners() {
        let cwd = tempfile::tempdir().expect("tempdir");
        let mut runs = BTreeMap::new();
        for runner in ["spark-harness", "codex-cli", "opencode"] {
            runs.insert(runner.to_string(), comparison_row(runner, "repo-survey"));
        }

        let prompt = judge_prompt(cwd.path(), "repo-survey", &runs).expect("build prompt");

        assert!(prompt.contains("Return exactly one scores entry for every runner"));
        assert!(prompt.contains(r#""runner": "opencode""#));
        assert!(prompt.contains("codex-cli|opencode|spark-harness|tie|inconclusive"));
    }

    #[test]
    fn matched_scenario_pairs_accept_grouped_runner_variants() {
        let rows = vec![
            comparison_row("spark-harness/high", "repo-survey"),
            comparison_row("codex-cli/low", "repo-survey"),
            comparison_row("opencode/low", "repo-survey"),
            comparison_row("spark-harness/high", "technical-essay"),
        ];

        let matched = matched_scenario_pairs(&rows);

        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].0, "repo-survey");
        assert!(matched[0].1.contains_key("spark-harness/high"));
        assert!(matched[0].1.contains_key("codex-cli/low"));
    }

    #[test]
    fn parse_judge_response_clamps_scores() {
        let parsed = parse_judge_response(
            "precise-patch",
            r#"```json
{"scenario":"x","scores":[{"runner":"spark-harness","solution_score":120,"process_score":-4,"confidence":75,"notes":"ok"}],"verdict":"spark-harness","rationale":"ok"}
```"#,
        )
        .expect("parse");

        assert_eq!(parsed.scenario, "precise-patch");
        assert_eq!(parsed.scores[0].solution_score, 100.0);
        assert_eq!(parsed.scores[0].process_score, 0.0);
    }

    #[test]
    fn judge_evidence_redacts_request_and_turn_counters() {
        let mut value = json!({
            "requests": {"count": 7},
            "tool_only_turns": {"max_consecutive": 4},
            "profile_scenario_call_expectations": {
                "extra_turns_after_satisfied": 2,
                "extra_calls_after_satisfied": 3
            },
            "duration_ms": 123,
            "nested": [{"request_duration_ms": 456, "tool_calls": 8}]
        });

        redact_non_comparable_protocol_units(&mut value);

        let text = serde_json::to_string(&value).expect("json");
        assert!(!text.contains("requests"));
        assert!(!text.contains("turn"));
        assert!(!text.contains("request_duration_ms"));
        assert!(text.contains("extra_calls_after_satisfied"));
        assert!(text.contains("duration_ms"));
        assert!(text.contains("tool_calls"));
    }

    fn comparison_row(runner: &str, scenario: &str) -> ComparisonRunRow {
        ComparisonRunRow {
            runner: runner.to_string(),
            suite: "real-world".to_string(),
            scenario: scenario.to_string(),
            model: "model".to_string(),
            completion_score: 100.0,
            quality_score: 90.0,
            process_score: 90.0,
            success: true,
            validation_exit_code: Some(0),
            validation_timed_out: false,
            duration_ms: 1_000,
            tool_or_item_calls: 1,
            input_tokens: 1_000,
            output_tokens: 100,
            source_files: 1,
            source_bytes: 100,
            failure_points: String::new(),
            source: ".".to_string(),
        }
    }
}
