//! Local, privacy-preserving usage aggregation for Codex JSONL session histories.
//!
//! The report deliberately contains aggregates and diagnostics only. Session files can contain
//! prompts, tools, paths, and other private content, none of which is retained in this schema.

use std::{
    collections::{BTreeMap, HashMap, HashSet},
    ffi::OsStr,
    fs::{self, File},
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use anyhow::Result;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

const SCHEMA_VERSION: &str = "spark.usage_history.v1";
const HISTORY_KIND: &str = "local_codex_session_history";

#[derive(Debug, Clone)]
pub(crate) struct HistoryOptions {
    pub(crate) codex_home: Option<PathBuf>,
    pub(crate) since_days: Option<u64>,
    pub(crate) max_files: usize,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UsageHistoryReport {
    pub(crate) schema_version: &'static str,
    pub(crate) kind: &'static str,
    pub(crate) generated_at_unix_seconds: u64,
    pub(crate) source: HistorySource,
    pub(crate) scope: HistoryScope,
    pub(crate) scan: ScanDiagnostics,
    pub(crate) aggregate: UsageAggregate,
    pub(crate) by_day: Vec<UsageBreakdown>,
    pub(crate) by_model: Vec<UsageBreakdown>,
    pub(crate) pricing: PricingAvailability,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HistorySource {
    pub(crate) kind: &'static str,
    pub(crate) network: bool,
    pub(crate) codex_home_source: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct HistoryScope {
    pub(crate) since_days: Option<u64>,
    pub(crate) max_files: usize,
}

#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct ScanDiagnostics {
    pub(crate) files_discovered: u64,
    pub(crate) files_scanned: u64,
    pub(crate) files_truncated: bool,
    pub(crate) files_unreadable: u64,
    pub(crate) malformed_lines: u64,
    pub(crate) sessions_without_metadata: u64,
    pub(crate) duplicate_session_files: u64,
    pub(crate) fork_replayed_observations_skipped: u64,
    pub(crate) fork_observations_without_cumulative_evidence: u64,
    pub(crate) cumulative_fallback_observations: u64,
    pub(crate) counter_resets: u64,
    pub(crate) partial_observations: u64,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UsageAggregate {
    pub(crate) observations: u64,
    pub(crate) reporting_coverage: ReportingCoverage,
    pub(crate) metrics: UsageMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct ReportingCoverage {
    pub(crate) observations_with_any_usage: u64,
    pub(crate) complete: bool,
    pub(crate) availability: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UsageBreakdown {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) day: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) model: Option<String>,
    pub(crate) observations: u64,
    pub(crate) reporting_coverage: ReportingCoverage,
    pub(crate) metrics: UsageMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct UsageMetrics {
    pub(crate) input_tokens: MetricTotal,
    pub(crate) cached_input_tokens: MetricTotal,
    pub(crate) cache_write_input_tokens: MetricTotal,
    pub(crate) uncached_input_tokens: MetricTotal,
    pub(crate) output_tokens: MetricTotal,
    pub(crate) reasoning_output_tokens: MetricTotal,
    pub(crate) total_tokens: MetricTotal,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct MetricTotal {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) total: Option<u64>,
    pub(crate) reported_observations: u64,
    pub(crate) observations: u64,
    pub(crate) complete: bool,
    pub(crate) availability: &'static str,
}

#[derive(Debug, Clone, Serialize)]
pub(crate) struct PricingAvailability {
    pub(crate) availability: &'static str,
    pub(crate) model: Option<String>,
    pub(crate) reason: &'static str,
}

#[derive(Debug, Clone, Default)]
struct TokenUsage {
    input_tokens: Option<u64>,
    cached_input_tokens: Option<u64>,
    cache_write_input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    reasoning_output_tokens: Option<u64>,
    total_tokens: Option<u64>,
}

impl TokenUsage {
    fn any(&self) -> bool {
        self.input_tokens.is_some()
            || self.cached_input_tokens.is_some()
            || self.cache_write_input_tokens.is_some()
            || self.output_tokens.is_some()
            || self.reasoning_output_tokens.is_some()
            || self.total_tokens.is_some()
    }

    fn uncached_input_tokens(&self) -> Option<u64> {
        let input = self.input_tokens?;
        let cached = self.cached_input_tokens?.min(input);
        let remaining = input.saturating_sub(cached);
        let cache_write = self.cache_write_input_tokens?.min(remaining);
        Some(remaining.saturating_sub(cache_write))
    }

    fn cumulative_key(&self) -> Option<String> {
        // A total-token counter is the stable proof that an event is a replay, rather than a
        // coincidentally equal per-turn usage value in a forked conversation.
        self.total_tokens.map(|total| {
            format!(
                "{total}:{:?}:{:?}:{:?}:{:?}:{:?}",
                self.input_tokens,
                self.cached_input_tokens,
                self.cache_write_input_tokens,
                self.output_tokens,
                self.reasoning_output_tokens
            )
        })
    }

    fn delta_from(&self, previous: Option<&TokenUsage>, diagnostics: &mut ScanDiagnostics) -> Self {
        fn delta(
            current: Option<u64>,
            previous: Option<u64>,
            diagnostics: &mut ScanDiagnostics,
        ) -> Option<u64> {
            match (current, previous) {
                (Some(current), Some(previous)) if current >= previous => Some(current - previous),
                (Some(current), Some(_)) => {
                    diagnostics.counter_resets += 1;
                    Some(current)
                }
                (Some(current), None) => Some(current),
                (None, _) => None,
            }
        }

        Self {
            input_tokens: delta(
                self.input_tokens,
                previous.and_then(|value| value.input_tokens),
                diagnostics,
            ),
            cached_input_tokens: delta(
                self.cached_input_tokens,
                previous.and_then(|value| value.cached_input_tokens),
                diagnostics,
            ),
            cache_write_input_tokens: delta(
                self.cache_write_input_tokens,
                previous.and_then(|value| value.cache_write_input_tokens),
                diagnostics,
            ),
            output_tokens: delta(
                self.output_tokens,
                previous.and_then(|value| value.output_tokens),
                diagnostics,
            ),
            reasoning_output_tokens: delta(
                self.reasoning_output_tokens,
                previous.and_then(|value| value.reasoning_output_tokens),
                diagnostics,
            ),
            total_tokens: delta(
                self.total_tokens,
                previous.and_then(|value| value.total_tokens),
                diagnostics,
            ),
        }
    }
}

#[derive(Debug, Clone)]
struct Observation {
    day: String,
    model: String,
    usage: TokenUsage,
    cumulative_key: Option<String>,
    used_cumulative_fallback: bool,
}

#[derive(Debug, Clone)]
struct SessionHistory {
    session_id: String,
    forked_from_id: Option<String>,
    observations: Vec<Observation>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct HistoryFile {
    path: PathBuf,
    modified_unix_seconds: u64,
}

#[derive(Debug, Default, Clone)]
struct UsageAccumulator {
    observations: u64,
    input_tokens: AccumulatedMetric,
    cached_input_tokens: AccumulatedMetric,
    cache_write_input_tokens: AccumulatedMetric,
    uncached_input_tokens: AccumulatedMetric,
    output_tokens: AccumulatedMetric,
    reasoning_output_tokens: AccumulatedMetric,
    total_tokens: AccumulatedMetric,
}

#[derive(Debug, Default, Clone)]
struct AccumulatedMetric {
    total: u64,
    reported_observations: u64,
}

impl UsageAccumulator {
    fn record(&mut self, usage: &TokenUsage) {
        self.observations += 1;
        self.input_tokens.record(usage.input_tokens);
        self.cached_input_tokens.record(usage.cached_input_tokens);
        self.cache_write_input_tokens
            .record(usage.cache_write_input_tokens);
        self.uncached_input_tokens
            .record(usage.uncached_input_tokens());
        self.output_tokens.record(usage.output_tokens);
        self.reasoning_output_tokens
            .record(usage.reasoning_output_tokens);
        self.total_tokens.record(usage.total_tokens);
    }

    fn finish(&self) -> (ReportingCoverage, UsageMetrics) {
        let covered = [
            self.input_tokens.reported_observations,
            self.cached_input_tokens.reported_observations,
            self.cache_write_input_tokens.reported_observations,
            self.output_tokens.reported_observations,
            self.total_tokens.reported_observations,
        ]
        .into_iter()
        .max()
        .unwrap_or(0);
        let complete = self.observations > 0 && covered == self.observations;
        (
            ReportingCoverage {
                observations_with_any_usage: covered,
                complete,
                availability: availability(covered, self.observations),
            },
            UsageMetrics {
                input_tokens: self.input_tokens.finish(self.observations),
                cached_input_tokens: self.cached_input_tokens.finish(self.observations),
                cache_write_input_tokens: self.cache_write_input_tokens.finish(self.observations),
                uncached_input_tokens: self.uncached_input_tokens.finish(self.observations),
                output_tokens: self.output_tokens.finish(self.observations),
                reasoning_output_tokens: self.reasoning_output_tokens.finish(self.observations),
                total_tokens: self.total_tokens.finish(self.observations),
            },
        )
    }
}

impl AccumulatedMetric {
    fn record(&mut self, value: Option<u64>) {
        if let Some(value) = value {
            self.total = self.total.saturating_add(value);
            self.reported_observations += 1;
        }
    }

    fn finish(&self, observations: u64) -> MetricTotal {
        MetricTotal {
            total: (self.reported_observations > 0).then_some(self.total),
            reported_observations: self.reported_observations,
            observations,
            complete: observations > 0 && self.reported_observations == observations,
            availability: availability(self.reported_observations, observations),
        }
    }
}

fn availability(reported: u64, observations: u64) -> &'static str {
    if reported == 0 {
        "unavailable"
    } else if reported == observations && observations > 0 {
        "reported"
    } else {
        "partial"
    }
}

pub(crate) fn scan_history(options: HistoryOptions) -> Result<UsageHistoryReport> {
    let (codex_home, codex_home_source) = resolve_codex_home(options.codex_home);
    let mut diagnostics = ScanDiagnostics::default();
    let mut files = discover_history_files(&codex_home, &mut diagnostics);
    sort_history_files_newest_first(&mut files);
    if files.len() > options.max_files {
        files.truncate(options.max_files);
        diagnostics.files_truncated = true;
    }

    let mut histories = Vec::new();
    for (index, file) in files.iter().enumerate() {
        diagnostics.files_scanned += 1;
        match File::open(&file.path) {
            Ok(file) => {
                let (history, had_io_error) =
                    parse_session(BufReader::new(file), index, &mut diagnostics);
                if had_io_error {
                    diagnostics.files_unreadable += 1;
                }
                if let Some(history) = history {
                    histories.push(history);
                }
            }
            Err(_) => diagnostics.files_unreadable += 1,
        }
    }

    let histories = select_unique_sessions(histories, &mut diagnostics);
    let roots = lineage_roots(&histories);
    let since_day = options.since_days.map(cutoff_day);
    let mut aggregate = UsageAccumulator::default();
    let mut days: BTreeMap<String, UsageAccumulator> = BTreeMap::new();
    let mut models: BTreeMap<String, UsageAccumulator> = BTreeMap::new();
    let mut replayed_cumulative = HashSet::new();

    for history in histories {
        let root = roots
            .get(&history.session_id)
            .cloned()
            .unwrap_or_else(|| history.session_id.clone());
        let forked = history.forked_from_id.is_some();
        for observation in history.observations {
            if since_day
                .as_ref()
                .is_some_and(|cutoff| &observation.day < cutoff)
            {
                continue;
            }
            if observation.used_cumulative_fallback {
                diagnostics.cumulative_fallback_observations += 1;
            }
            if forked {
                if let Some(cumulative_key) = &observation.cumulative_key {
                    let key = format!("{root}:{cumulative_key}");
                    if !replayed_cumulative.insert(key) {
                        diagnostics.fork_replayed_observations_skipped += 1;
                        continue;
                    }
                } else {
                    diagnostics.fork_observations_without_cumulative_evidence += 1;
                }
            } else if let Some(cumulative_key) = &observation.cumulative_key {
                replayed_cumulative.insert(format!("{root}:{cumulative_key}"));
            }

            if !all_metric_fields_present(&observation.usage) {
                diagnostics.partial_observations += 1;
            }
            aggregate.record(&observation.usage);
            days.entry(observation.day.clone())
                .or_default()
                .record(&observation.usage);
            models
                .entry(observation.model.clone())
                .or_default()
                .record(&observation.usage);
        }
    }

    let (coverage, metrics) = aggregate.finish();
    Ok(UsageHistoryReport {
        schema_version: SCHEMA_VERSION,
        kind: HISTORY_KIND,
        generated_at_unix_seconds: unix_now(),
        source: HistorySource {
            kind: "codex_local_jsonl",
            network: false,
            codex_home_source,
        },
        scope: HistoryScope {
            since_days: options.since_days,
            max_files: options.max_files,
        },
        scan: diagnostics,
        aggregate: UsageAggregate {
            observations: aggregate.observations,
            reporting_coverage: coverage,
            metrics,
        },
        by_day: days
            .into_iter()
            .map(|(day, accumulator)| breakdown(Some(day), None, accumulator))
            .collect(),
        by_model: models
            .into_iter()
            .map(|(model, accumulator)| breakdown(None, Some(model), accumulator))
            .collect(),
        pricing: PricingAvailability {
            availability: "unavailable",
            model: None,
            reason: "local Codex session histories report token counters, not authoritative prices or subscription charges",
        },
    })
}

fn breakdown(
    day: Option<String>,
    model: Option<String>,
    accumulator: UsageAccumulator,
) -> UsageBreakdown {
    let (reporting_coverage, metrics) = accumulator.finish();
    UsageBreakdown {
        day,
        model,
        observations: accumulator.observations,
        reporting_coverage,
        metrics,
    }
}

fn resolve_codex_home(explicit: Option<PathBuf>) -> (PathBuf, &'static str) {
    if let Some(home) = explicit {
        return (home, "explicit");
    }
    if let Some(home) = std::env::var_os("CODEX_HOME") {
        return (PathBuf::from(home), "environment");
    }
    let default = directories::BaseDirs::new()
        .map(|dirs| dirs.home_dir().join(".codex"))
        .unwrap_or_else(|| PathBuf::from(".codex"));
    (default, "default")
}

fn discover_history_files(
    codex_home: &Path,
    diagnostics: &mut ScanDiagnostics,
) -> Vec<HistoryFile> {
    let mut files = Vec::new();
    for root in [
        codex_home.join("sessions"),
        codex_home.join("archived_sessions"),
    ] {
        collect_jsonl_files(&root, &mut files);
    }
    diagnostics.files_discovered = files.len() as u64;
    files
}

fn collect_jsonl_files(root: &Path, files: &mut Vec<HistoryFile>) {
    let Ok(entries) = fs::read_dir(root) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_jsonl_files(&path, files);
        } else if path.extension() == Some(OsStr::new("jsonl")) {
            let modified_unix_seconds = entry
                .metadata()
                .ok()
                .and_then(|metadata| metadata.modified().ok())
                .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                .map(|duration| duration.as_secs())
                .unwrap_or(0);
            files.push(HistoryFile {
                path,
                modified_unix_seconds,
            });
        }
    }
}

fn sort_history_files_newest_first(files: &mut [HistoryFile]) {
    files.sort_by(|left, right| {
        right
            .modified_unix_seconds
            .cmp(&left.modified_unix_seconds)
            .then_with(|| left.path.cmp(&right.path))
    });
}

fn parse_session<R: BufRead>(
    reader: R,
    index: usize,
    diagnostics: &mut ScanDiagnostics,
) -> (Option<SessionHistory>, bool) {
    let mut session_id = None;
    let mut forked_from_id = None;
    let mut current_model = None;
    let mut previous_cumulative = None;
    let mut observations = Vec::new();

    let mut digest = Sha256::new();
    let mut had_io_error = false;
    for line in reader.lines() {
        let line = match line {
            Ok(line) => line,
            Err(_) => {
                had_io_error = true;
                break;
            }
        };
        digest.update(line.as_bytes());
        digest.update(b"\n");
        let Ok(value) = serde_json::from_str::<Value>(&line) else {
            diagnostics.malformed_lines += 1;
            continue;
        };
        let Some(kind) = value.get("type").and_then(Value::as_str) else {
            continue;
        };
        let payload = value.get("payload").unwrap_or(&Value::Null);
        match kind {
            "session_meta" => {
                session_id = payload
                    .get("session_id")
                    .or_else(|| payload.get("id"))
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                forked_from_id = payload
                    .get("forked_from_id")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
                current_model = payload
                    .get("model")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
            "turn_context" => {
                current_model = payload
                    .get("model")
                    .and_then(Value::as_str)
                    .map(ToOwned::to_owned);
            }
            "event_msg" if payload.get("type").and_then(Value::as_str) == Some("token_count") => {
                let info = payload.get("info").unwrap_or(&Value::Null);
                let last = usage_from_value(info.get("last_token_usage"));
                let cumulative = usage_from_value(info.get("total_token_usage"));
                let (usage, used_cumulative_fallback) = if last.any() {
                    (last, false)
                } else if cumulative.any() {
                    let delta = cumulative.delta_from(previous_cumulative.as_ref(), diagnostics);
                    (delta, true)
                } else {
                    continue;
                };
                if cumulative.any() {
                    previous_cumulative = Some(cumulative.clone());
                }
                let Some(day) = value
                    .get("timestamp")
                    .and_then(Value::as_str)
                    .and_then(day_from_timestamp)
                else {
                    continue;
                };
                observations.push(Observation {
                    day,
                    model: current_model
                        .clone()
                        .unwrap_or_else(|| "unknown".to_string()),
                    usage,
                    cumulative_key: cumulative.cumulative_key(),
                    used_cumulative_fallback,
                });
            }
            _ => {}
        }
    }

    if session_id.is_none() {
        diagnostics.sessions_without_metadata += 1;
    }

    let history = if let Some(session_id) = session_id {
        Some(SessionHistory {
            session_id,
            forked_from_id,
            observations,
        })
    } else {
        // A session without metadata cannot be safely merged with another file. Preserve the
        // contribution under a non-user-facing opaque identity rather than inventing a path.
        (!observations.is_empty()).then(|| {
            let digest = digest.finalize();
            SessionHistory {
                session_id: format!("unattributed-{index}-{:x}", digest),
                forked_from_id: None,
                observations,
            }
        })
    };

    (history, had_io_error)
}

fn usage_from_value(value: Option<&Value>) -> TokenUsage {
    let Some(value) = value.and_then(Value::as_object) else {
        return TokenUsage::default();
    };
    TokenUsage {
        input_tokens: value.get("input_tokens").and_then(Value::as_u64),
        cached_input_tokens: value.get("cached_input_tokens").and_then(Value::as_u64),
        cache_write_input_tokens: value
            .get("cache_write_input_tokens")
            .and_then(Value::as_u64),
        output_tokens: value.get("output_tokens").and_then(Value::as_u64),
        reasoning_output_tokens: value.get("reasoning_output_tokens").and_then(Value::as_u64),
        total_tokens: value.get("total_tokens").and_then(Value::as_u64),
    }
}

fn select_unique_sessions(
    histories: Vec<SessionHistory>,
    diagnostics: &mut ScanDiagnostics,
) -> Vec<SessionHistory> {
    let mut selected: HashMap<String, SessionHistory> = HashMap::new();
    for history in histories {
        match selected.get_mut(&history.session_id) {
            Some(existing) => {
                diagnostics.duplicate_session_files += 1;
                if session_weight(&history) > session_weight(existing) {
                    *existing = history;
                }
            }
            None => {
                selected.insert(history.session_id.clone(), history);
            }
        }
    }
    let mut selected: Vec<_> = selected.into_values().collect();
    // Process roots before forks so replayed cumulative observations are attributed to the
    // original lineage when both histories are available, independent of filename ordering.
    selected.sort_by(|left, right| {
        left.forked_from_id
            .is_some()
            .cmp(&right.forked_from_id.is_some())
            .then_with(|| left.session_id.cmp(&right.session_id))
    });
    selected
}

fn session_weight(history: &SessionHistory) -> (usize, u64) {
    let total = history
        .observations
        .iter()
        .filter_map(|observation| observation.usage.total_tokens)
        .fold(0u64, u64::saturating_add);
    (history.observations.len(), total)
}

fn lineage_roots(histories: &[SessionHistory]) -> HashMap<String, String> {
    let parents: HashMap<_, _> = histories
        .iter()
        .filter_map(|history| {
            history
                .forked_from_id
                .as_ref()
                .map(|parent| (history.session_id.clone(), parent.clone()))
        })
        .collect();
    histories
        .iter()
        .map(|history| {
            let mut root = history.session_id.clone();
            let mut visited = HashSet::new();
            while let Some(parent) = parents.get(&root) {
                if !visited.insert(root.clone()) {
                    break;
                }
                root = parent.clone();
            }
            (history.session_id.clone(), root)
        })
        .collect()
}

fn all_metric_fields_present(usage: &TokenUsage) -> bool {
    usage.input_tokens.is_some()
        && usage.cached_input_tokens.is_some()
        && usage.cache_write_input_tokens.is_some()
        && usage.output_tokens.is_some()
        && usage.reasoning_output_tokens.is_some()
        && usage.total_tokens.is_some()
}

fn day_from_timestamp(timestamp: &str) -> Option<String> {
    let day = timestamp.get(..10)?;
    (day.as_bytes().get(4) == Some(&b'-') && day.as_bytes().get(7) == Some(&b'-'))
        .then(|| day.to_string())
}

fn cutoff_day(since_days: u64) -> String {
    let days_since_epoch = unix_now() / 86_400;
    let cutoff = days_since_epoch.saturating_sub(since_days.saturating_sub(1));
    civil_from_days(cutoff as i64)
}

// Gregorian conversion adapted from the public-domain civil-calendar algorithm. It only feeds a
// display/filter date, never an input timestamp parser.
fn civil_from_days(days_since_epoch: i64) -> String {
    let z = days_since_epoch + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    format!("{year:04}-{month:02}-{day:02}")
}

fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

pub(crate) fn render_human(report: &UsageHistoryReport) -> String {
    let metrics = &report.aggregate.metrics;
    let mut lines = vec![
        "Local Codex usage history (no network)".to_string(),
        format!("  Observations: {}", report.aggregate.observations),
        format!(
            "  Files scanned: {} of {}",
            report.scan.files_scanned, report.scan.files_discovered
        ),
        format!("  Scan complete: {}", !report.scan.files_truncated),
        format_metric("Input tokens", &metrics.input_tokens),
        format_metric("Cached input tokens", &metrics.cached_input_tokens),
        format_metric(
            "Cache-write input tokens",
            &metrics.cache_write_input_tokens,
        ),
        format_metric("Uncached input tokens", &metrics.uncached_input_tokens),
        format_metric("Output tokens", &metrics.output_tokens),
        format_metric(
            "Reasoning output tokens (subset of output)",
            &metrics.reasoning_output_tokens,
        ),
        format_metric("Total tokens", &metrics.total_tokens),
        "  Pricing: unavailable (local history is not a billing source)".to_string(),
    ];
    if report.scan.files_truncated {
        lines.push("  Note: --max-files limited this scan; totals are incomplete.".to_string());
    }
    lines.join("\n") + "\n"
}

fn format_metric(name: &str, metric: &MetricTotal) -> String {
    let total = metric
        .total
        .map(|value| value.to_string())
        .unwrap_or_else(|| "unavailable".to_string());
    format!(
        "  {name}: {total} ({}/{}, {})",
        metric.reported_observations, metric.observations, metric.availability
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::{self, Read};
    use tempfile::TempDir;

    struct ReadThenError {
        bytes: Vec<u8>,
        position: usize,
    }

    impl Read for ReadThenError {
        fn read(&mut self, buffer: &mut [u8]) -> io::Result<usize> {
            if self.position >= self.bytes.len() {
                return Err(io::Error::other("simulated history read failure"));
            }
            let count = (self.bytes.len() - self.position).min(buffer.len()).min(7);
            buffer[..count].copy_from_slice(&self.bytes[self.position..self.position + count]);
            self.position += count;
            Ok(count)
        }
    }

    fn options(home: &Path) -> HistoryOptions {
        HistoryOptions {
            codex_home: Some(home.to_path_buf()),
            since_days: None,
            max_files: 100,
        }
    }

    fn write_session(home: &Path, directory: &str, file: &str, lines: &[String]) {
        let path = home.join(directory);
        fs::create_dir_all(&path).expect("fixture directory");
        fs::write(path.join(file), lines.join("\n")).expect("fixture file");
    }

    fn meta(id: &str, forked_from: Option<&str>) -> String {
        let fork = forked_from
            .map(|value| format!(",\"forked_from_id\":\"{value}\""))
            .unwrap_or_default();
        format!(
            "{{\"timestamp\":\"2026-08-01T00:00:00Z\",\"type\":\"session_meta\",\"payload\":{{\"session_id\":\"{id}\"{fork}}}}}"
        )
    }

    fn context(model: &str) -> String {
        format!(
            "{{\"timestamp\":\"2026-08-01T00:00:01Z\",\"type\":\"turn_context\",\"payload\":{{\"model\":\"{model}\"}}}}"
        )
    }

    fn token(timestamp: &str, last: Option<&str>, total: Option<&str>) -> String {
        let last = last
            .map(|value| format!("\"last_token_usage\":{value}"))
            .unwrap_or_default();
        let total = total
            .map(|value| format!("\"total_token_usage\":{value}"))
            .unwrap_or_default();
        let separator = (!last.is_empty() && !total.is_empty())
            .then_some(",")
            .unwrap_or("");
        format!(
            "{{\"timestamp\":\"{timestamp}\",\"type\":\"event_msg\",\"payload\":{{\"type\":\"token_count\",\"info\":{{{last}{separator}{total}}}}}}}"
        )
    }

    const FULL: &str = "{\"input_tokens\":100,\"cached_input_tokens\":20,\"cache_write_input_tokens\":10,\"output_tokens\":30,\"reasoning_output_tokens\":12,\"total_tokens\":130}";

    #[test]
    fn deduplicates_live_and_archived_session_copies_without_exposing_paths() {
        let temp = TempDir::new().expect("temp");
        let lines = vec![
            meta("same", None),
            context("gpt-test"),
            token("2026-08-01T01:00:00Z", Some(FULL), Some(FULL)),
        ];
        write_session(temp.path(), "sessions/2026/08/01", "live.jsonl", &lines);
        write_session(temp.path(), "archived_sessions", "copy.jsonl", &lines);

        let value =
            serde_json::to_value(scan_history(options(temp.path())).expect("scan")).expect("json");
        assert_eq!(value["aggregate"]["observations"], 1);
        assert_eq!(value["aggregate"]["metrics"]["total_tokens"]["total"], 130);
        assert_eq!(value["scan"]["duplicate_session_files"], 1);
        let text = value.to_string();
        assert!(!text.contains("live.jsonl"));
        assert!(!text.contains("archived_sessions"));
    }

    #[test]
    fn removes_fork_replay_when_cumulative_evidence_matches() {
        let temp = TempDir::new().expect("temp");
        write_session(
            temp.path(),
            "sessions",
            "parent.jsonl",
            &[
                meta("parent", None),
                context("gpt-test"),
                token("2026-08-01T01:00:00Z", Some(FULL), Some(FULL)),
            ],
        );
        write_session(
            temp.path(),
            "sessions",
            "fork.jsonl",
            &[
                meta("fork", Some("parent")),
                context("gpt-test"),
                token("2026-08-01T01:00:00Z", Some(FULL), Some(FULL)),
                token(
                    "2026-08-01T02:00:00Z",
                    Some(
                        "{\"input_tokens\":50,\"cached_input_tokens\":5,\"cache_write_input_tokens\":0,\"output_tokens\":10,\"reasoning_output_tokens\":2,\"total_tokens\":60}",
                    ),
                    Some(
                        "{\"input_tokens\":150,\"cached_input_tokens\":25,\"cache_write_input_tokens\":10,\"output_tokens\":40,\"reasoning_output_tokens\":14,\"total_tokens\":190}",
                    ),
                ),
            ],
        );

        let report = scan_history(options(temp.path())).expect("scan");
        assert_eq!(report.aggregate.observations, 2);
        assert_eq!(report.aggregate.metrics.total_tokens.total, Some(190));
        assert_eq!(report.scan.fork_replayed_observations_skipped, 1);
    }

    #[test]
    fn uses_non_negative_cumulative_deltas_and_reports_counter_resets() {
        let temp = TempDir::new().expect("temp");
        write_session(
            temp.path(),
            "sessions",
            "cumulative.jsonl",
            &[
                meta("cumulative", None),
                context("gpt-test"),
                token(
                    "2026-08-01T01:00:00Z",
                    None,
                    Some("{\"input_tokens\":10,\"total_tokens\":10}"),
                ),
                token(
                    "2026-08-01T02:00:00Z",
                    None,
                    Some("{\"input_tokens\":25,\"total_tokens\":30}"),
                ),
                token(
                    "2026-08-01T03:00:00Z",
                    None,
                    Some("{\"input_tokens\":4,\"total_tokens\":5}"),
                ),
            ],
        );
        let report = scan_history(options(temp.path())).expect("scan");
        assert_eq!(report.aggregate.metrics.input_tokens.total, Some(29));
        assert_eq!(report.aggregate.metrics.total_tokens.total, Some(35));
        assert_eq!(report.scan.cumulative_fallback_observations, 3);
        assert_eq!(report.scan.counter_resets, 2);
    }

    #[test]
    fn preserves_partial_coverage_and_reasoning_is_not_added_to_output() {
        let temp = TempDir::new().expect("temp");
        write_session(
            temp.path(),
            "sessions",
            "partial.jsonl",
            &[
                meta("partial", None),
                context("gpt-a"),
                token("2026-08-01T01:00:00Z", Some(FULL), Some(FULL)),
                token(
                    "2026-08-02T01:00:00Z",
                    Some(
                        "{\"input_tokens\":20,\"output_tokens\":7,\"reasoning_output_tokens\":6,\"total_tokens\":27}",
                    ),
                    None,
                ),
            ],
        );
        let report = scan_history(options(temp.path())).expect("scan");
        assert_eq!(report.aggregate.metrics.output_tokens.total, Some(37));
        assert_eq!(
            report.aggregate.metrics.reasoning_output_tokens.total,
            Some(18)
        );
        assert_eq!(
            report.aggregate.metrics.cached_input_tokens.availability,
            "partial"
        );
        assert_eq!(
            report.aggregate.metrics.uncached_input_tokens.total,
            Some(70)
        );
        assert_eq!(report.by_day.len(), 2);
        assert_eq!(report.by_model[0].model.as_deref(), Some("gpt-a"));
    }

    #[test]
    fn max_files_marks_report_incomplete_and_since_days_filters_using_calendar_day() {
        let temp = TempDir::new().expect("temp");
        write_session(
            temp.path(),
            "sessions",
            "a.jsonl",
            &[
                meta("a", None),
                context("model-a"),
                token("2020-01-01T01:00:00Z", Some(FULL), Some(FULL)),
            ],
        );
        write_session(
            temp.path(),
            "sessions",
            "b.jsonl",
            &[
                meta("b", None),
                context("model-b"),
                token("2026-08-01T01:00:00Z", Some(FULL), Some(FULL)),
            ],
        );
        let mut bounded = options(temp.path());
        bounded.max_files = 1;
        let report = scan_history(bounded).expect("scan");
        assert!(report.scan.files_truncated);
        assert_eq!(report.scan.files_scanned, 1);

        let mut recent = options(temp.path());
        recent.since_days = Some(1);
        let report = scan_history(recent).expect("scan");
        // The fixture date is deliberately only included if it is current at test time; old data
        // must be absent regardless of the machine's day.
        assert!(
            report
                .by_day
                .iter()
                .all(|entry| entry.day.as_deref() != Some("2020-01-01"))
        );
    }

    #[test]
    fn civil_day_conversion_matches_unix_epoch() {
        assert_eq!(civil_from_days(0), "1970-01-01");
        assert_eq!(civil_from_days(20_000), "2024-10-04");
    }

    #[test]
    fn bounded_scans_select_newest_files_with_a_deterministic_path_tiebreak() {
        let mut files = vec![
            HistoryFile {
                path: PathBuf::from("sessions/older.jsonl"),
                modified_unix_seconds: 10,
            },
            HistoryFile {
                path: PathBuf::from("sessions/z-same-time.jsonl"),
                modified_unix_seconds: 20,
            },
            HistoryFile {
                path: PathBuf::from("sessions/a-same-time.jsonl"),
                modified_unix_seconds: 20,
            },
        ];

        sort_history_files_newest_first(&mut files);
        let selected: Vec<_> = files.into_iter().take(2).map(|file| file.path).collect();
        assert_eq!(
            selected,
            vec![
                PathBuf::from("sessions/a-same-time.jsonl"),
                PathBuf::from("sessions/z-same-time.jsonl"),
            ]
        );
    }

    #[test]
    fn streaming_parser_keeps_completed_observations_and_reports_later_io_failure() {
        let contents = [
            meta("partial-read", None),
            context("gpt-test"),
            token("2026-08-01T01:00:00Z", Some(FULL), Some(FULL)),
        ]
        .join("\n")
            + "\n";
        let reader = ReadThenError {
            bytes: contents.into_bytes(),
            position: 0,
        };
        let mut diagnostics = ScanDiagnostics::default();
        let (history, had_io_error) = parse_session(BufReader::new(reader), 0, &mut diagnostics);

        assert!(had_io_error);
        assert_eq!(
            history
                .expect("completed observations survive")
                .observations
                .len(),
            1
        );
    }
}
