use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::{
    DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS, DEFAULT_MODEL, DEFAULT_SCENARIO_TARGET_TOKENS,
    benchmark::judge::DEFAULT_JUDGE_MODEL, tools,
};

#[derive(Debug, Parser)]
#[command(name = "spark")]
#[command(about = "A small GPT-5.3 Codex Spark agent harness")]
#[command(version)]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
}

/// Parse on a dedicated stack so the growing clap command tree remains usable from the Windows main thread.
pub(crate) fn parse_with_stack() -> anyhow::Result<Cli> {
    std::thread::Builder::new()
        .name("spark-cli-parse".to_string())
        .stack_size(8 * 1024 * 1024)
        .spawn(Cli::parse)
        .map_err(|error| anyhow::anyhow!("failed to start CLI parser: {error}"))?
        .join()
        .map_err(|_| anyhow::anyhow!("CLI parser thread panicked"))
}

#[derive(Debug, Subcommand)]
pub(crate) enum Command {
    /// Sign in with ChatGPT/Codex OAuth and save tokens locally.
    Login {
        /// Do not open the browser automatically.
        #[arg(long)]
        no_browser: bool,
        /// Use Codex device-code auth instead of local browser callback.
        #[arg(long)]
        device: bool,
    },
    /// Run first-run setup: device login, local directories, sessions, and optional skill migration.
    Setup {
        /// Use defaults without terminal prompts.
        #[arg(long)]
        non_interactive: bool,
        /// Do not run device-code login.
        #[arg(long)]
        skip_login: bool,
        /// Do not offer repo-local skill migration.
        #[arg(long)]
        skip_skill_migration: bool,
        /// Migrate skills from this directory instead of prompting for a discovered source.
        #[arg(long)]
        skill_source: Option<PathBuf>,
        /// Workspace root that receives migrated .agents/skills.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Register the Spark MCP explorer and install the native Codex explorer bridge.
        #[arg(long)]
        codex: bool,
        /// Replace an existing spark_harness MCP registration or explorer agent, preserving a backup.
        #[arg(long, requires = "codex")]
        force_codex: bool,
    },
    /// Show saved auth status.
    AuthStatus,
    /// Show source-reported ChatGPT Codex quota and credits (no token or dollar estimates).
    Usage {
        /// Print normalized account usage JSON.
        #[arg(long)]
        json: bool,
        /// Scan local Codex session history instead of calling the account-quota service.
        #[arg(long)]
        history: bool,
        /// Codex home to scan. Defaults to CODEX_HOME, then ~/.codex.
        #[arg(long, requires = "history")]
        codex_home: Option<PathBuf>,
        /// Include observations whose event day is within this many calendar days.
        #[arg(long, requires = "history", value_parser = clap::value_parser!(u64).range(1..))]
        since_days: Option<u64>,
        /// Bound the number of local history files scanned; a bounded scan is reported as incomplete.
        #[arg(long, requires = "history", default_value_t = 10_000, value_parser = parse_positive_usize)]
        max_files: usize,
        /// Write the stable normalized history JSON to this file for Spark Bench ingestion.
        #[arg(long, requires = "history")]
        output: Option<PathBuf>,
    },
    /// Send one instruction to the Spark agent loop.
    Chat {
        prompt: Vec<String>,
        /// Read the prompt from a file instead of command-line args.
        #[arg(long)]
        prompt_file: Option<PathBuf>,
        /// Workspace root for filesystem and command tools.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Model slug to use.
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
        /// Reasoning effort to request from the model.
        #[arg(long, value_parser = ["low", "medium", "high", "xhigh"], default_value = crate::client::DEFAULT_SPARK_AGENT_REASONING_EFFORT)]
        reasoning_effort: String,
        /// Additional system/developer instructions appended to Spark's built-in harness prompt.
        #[arg(long)]
        system_prompt: Option<String>,
        /// Durable objective to store before this chat run. With no prompt, Spark runs bounded goal checkpoints.
        #[arg(long)]
        goal: Option<String>,
        /// Number of goal checkpoints to run when --goal is provided.
        #[arg(long, default_value_t = 3)]
        goal_checkpoints: usize,
        /// Tool access mode. ask is read-only; work allows edits and command execution.
        #[arg(long, value_enum, default_value_t = RunMode::Work)]
        mode: RunMode,
        /// Save raw request/response JSON under .spark-runs/.
        #[arg(long)]
        trace: bool,
        /// Print a compact profiling summary after each completed prompt.
        #[arg(long)]
        profile: bool,
        /// Named session to resume/save in the SQLite session store. Interactive chat uses a workspace session when omitted.
        #[arg(long)]
        session: Option<String>,
        /// Load a compiled skill into the conversation before the prompt.
        #[arg(long = "skill")]
        skills: Vec<String>,
        /// Start the named session from an empty history, replacing any saved state after the next save.
        #[arg(long)]
        new_session: bool,
        /// Override the default compaction threshold (90% of the model context window), in JSON characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Override the default compaction threshold (90% of the model context window), in estimated tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Optional tool-only-turn compaction trigger. Disabled by default; use 0 to disable.
        #[arg(long, default_value_t = DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS)]
        compact_after_tool_only_turns: usize,
        /// Override the model-context input guard, in JSON characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Override the model-context input guard, in estimated tokens.
        #[arg(long)]
        max_input_tokens: Option<usize>,
    },
    /// Print available built-in tools as JSON.
    Tools,
    /// Serve the Spark repository explorer over MCP stdio for native Codex.
    McpServer,
    /// Serve the persistent Spark Desktop protocol over newline-delimited JSON stdin/stdout.
    DesktopServer {
        /// Use standard input and output for the desktop protocol.
        #[arg(long, required = true)]
        stdio: bool,
    },
    /// Run one trusted-host automation request from stdin and return one JSON response.
    Automation {
        /// Read the versioned automation request from stdin and write the response to stdout.
        #[arg(long, required = true)]
        stdio: bool,
    },
    /// Produce a compact, read-only local repository brief.
    Brief {
        /// Concrete repository question to answer.
        question: String,
        /// Workspace root for local filesystem inspection.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Repository-relative file or directory to inspect first. May be repeated.
        #[arg(long = "path")]
        paths: Vec<String>,
        /// Output format.
        #[arg(long, value_enum, default_value_t = RepoBriefFormat::Text)]
        format: RepoBriefFormat,
        /// Reasoning effort to request from the model.
        #[arg(long, value_parser = ["low", "medium", "high", "xhigh"], default_value = "medium")]
        reasoning_effort: String,
        /// Save the run trace under the workspace's .spark-runs directory.
        #[arg(long)]
        trace: bool,
        /// Stop the standalone request after this many seconds.
        #[arg(long, value_parser = clap::value_parser!(u64).range(1..=3_600), default_value_t = crate::repo_brief::DEFAULT_TIMEOUT_SECONDS)]
        timeout_seconds: u64,
    },
    /// List saved chat sessions.
    Sessions,
    /// List or refresh built-in and repo-local Spark skill cache.
    Skills {
        /// Rebuild cached summaries from built-in and .agents/skills sources.
        #[arg(long)]
        refresh: bool,
    },
    /// List or expand reusable prompt commands from .agents/commands, .spark/commands, and .claude/commands.
    Commands {
        /// Workspace root containing command directories.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Print discovered commands as JSON.
        #[arg(long)]
        json: bool,
        /// Command name to expand. Omit to list commands.
        name: Option<String>,
        /// Arguments injected into the command prompt.
        args: Vec<String>,
    },
    /// Browse available TUI spinner sets in a live terminal preview.
    SpinnerPreview,
    /// List saved trace runs under .spark-runs/.
    Traces {
        /// Maximum number of trace directories to print.
        #[arg(long, default_value_t = 20)]
        limit: usize,
        /// Print one compact profile row per trace.
        #[arg(long)]
        summary: bool,
        /// Only include traces for a profile scenario name.
        #[arg(long)]
        scenario: Option<String>,
        /// Only include traces that contain this diagnostic kind. Repeat to require multiple kinds.
        #[arg(long = "diagnostic")]
        diagnostics: Vec<String>,
        /// Print an aggregate row for matching trace summaries.
        #[arg(long)]
        aggregate: bool,
        /// Sort matching analyzed traces by a profiling metric.
        #[arg(long, value_enum, default_value_t = TraceSort::Newest)]
        sort: TraceSort,
        /// Only include traces whose max tool-only streak is at least this many turns.
        #[arg(long)]
        min_tool_only_streak: Option<u64>,
        /// Only include traces whose post-satisfaction overrun is at least this many turns.
        #[arg(long)]
        min_overrun_turns: Option<u64>,
        /// Only include traces whose post-satisfaction context growth is at least this many chars.
        #[arg(long)]
        min_overrun_context_chars: Option<u64>,
        /// Only include traces whose post-compaction request regrowth is at least this many chars.
        #[arg(long)]
        min_compaction_regrowth_chars: Option<u64>,
        /// Print matching analyzed traces as one JSON object.
        #[arg(long)]
        json: bool,
        /// Print matching analyzed traces as one JSON object per line.
        #[arg(long)]
        jsonl: bool,
    },
    /// Inspect local trace retention or safely purge old trace runs.
    TraceRetention {
        /// Workspace root containing .spark-runs/.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Only consider trace runs older than this many days.
        #[arg(long, default_value_t = 30, value_parser = clap::value_parser!(u64).range(1..))]
        older_than_days: u64,
        /// Prepare a purge. This remains a dry run until --confirm is also passed.
        #[arg(long)]
        purge: bool,
        /// Explicitly delete the exact eligible trace directories. Requires --purge.
        #[arg(long)]
        confirm: bool,
    },
    /// Summarize a .spark-runs/run-* trace for repeated tool calls and compaction behavior.
    AnalyzeTrace {
        /// Trace directory to analyze. Defaults to the latest .spark-runs/run-* directory.
        dir: Option<PathBuf>,
        /// Analyze the latest .spark-runs/run-* directory.
        #[arg(long)]
        latest: bool,
        /// Print a compact human-readable timeline instead of full JSON.
        #[arg(long)]
        timeline: bool,
    },
    /// Run a repeatable Spark profiling scenario through the real agent loop.
    ProfileScenario {
        /// Scenario to run.
        #[arg(value_enum)]
        scenario: ProfileScenarioKind,
        /// Workspace root for filesystem and command tools.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Model slug to use.
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
        /// Reasoning effort to request from the model.
        #[arg(long, value_enum, default_value_t = BenchmarkReasoningEffort::Medium)]
        reasoning_effort: BenchmarkReasoningEffort,
        /// Target prompt size for long-context scenarios, in approximate tokens.
        #[arg(long, default_value_t = DEFAULT_SCENARIO_TARGET_TOKENS)]
        target_tokens: usize,
        /// Run the scenario this many times and aggregate the traces from this invocation.
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        /// Disable trace files for this scenario.
        #[arg(long)]
        no_trace: bool,
        /// Disable printed profile JSON for this scenario.
        #[arg(long)]
        no_profile: bool,
        /// Override the default compaction threshold (90% of the model context window), in JSON characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Override the default compaction threshold (90% of the model context window), in estimated tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Optional tool-only-turn compaction trigger. Disabled by default; use 0 to disable.
        #[arg(long, default_value_t = DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS)]
        compact_after_tool_only_turns: usize,
        /// Override the model-context input guard, in JSON characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Override the model-context input guard, in estimated tokens.
        #[arg(long)]
        max_input_tokens: Option<usize>,
    },
    /// Run a benchmark suite made from repeatable profiling scenarios.
    ProfileBenchmark {
        /// Benchmark suite to run.
        #[arg(value_enum)]
        suite: ProfileBenchmarkSuiteKind,
        /// Workspace root for filesystem and command tools.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Model slug to use.
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
        /// Reasoning effort to request from the model.
        #[arg(long, value_enum, default_value_t = BenchmarkReasoningEffort::Medium)]
        reasoning_effort: BenchmarkReasoningEffort,
        /// Target prompt size for long-context scenarios, in approximate tokens.
        #[arg(long, default_value_t = DEFAULT_SCENARIO_TARGET_TOKENS)]
        target_tokens: usize,
        /// Run each scenario in the suite this many times.
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        /// Run only these scenarios from the selected suite. Repeat to select multiple scenarios.
        #[arg(long = "scenario", value_enum)]
        scenarios: Vec<ProfileScenarioKind>,
        /// Disable trace files for this benchmark run.
        #[arg(long)]
        no_trace: bool,
        /// Disable printed profile JSON for this benchmark run.
        #[arg(long)]
        no_profile: bool,
        /// Override the default compaction threshold (90% of the model context window), in JSON characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Override the default compaction threshold (90% of the model context window), in estimated tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Optional tool-only-turn compaction trigger. Disabled by default; use 0 to disable.
        #[arg(long, default_value_t = DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS)]
        compact_after_tool_only_turns: usize,
        /// Override the model-context input guard, in JSON characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Override the model-context input guard, in estimated tokens.
        #[arg(long)]
        max_input_tokens: Option<usize>,
    },
    /// Generate benchmark result files and charts from saved profile-benchmark traces.
    ProfileBenchmarkReport {
        /// Benchmark suite to report.
        #[arg(long, value_enum, default_value_t = ProfileBenchmarkSuiteKind::RealWorld)]
        suite: ProfileBenchmarkSuiteKind,
        /// Workspace root containing .spark-runs/.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Maximum recent trace directories to scan.
        #[arg(long, default_value_t = 200)]
        limit: usize,
        /// Include every matching run instead of only the newest run per scenario.
        #[arg(long)]
        all_runs: bool,
        /// Report exact profile-benchmark run manifest(s) instead of scanning for latest traces.
        #[arg(long = "run-manifest")]
        run_manifests: Vec<PathBuf>,
        /// Directory where JSON, CSV, and HTML report files are written.
        #[arg(long, default_value = ".spark-profile/benchmarks")]
        output_dir: PathBuf,
    },
    /// Run a benchmark suite through Codex CLI for comparison with this harness.
    CodexCliBenchmark {
        /// Benchmark suite to run.
        #[arg(value_enum)]
        suite: ProfileBenchmarkSuiteKind,
        /// Workspace root for Codex CLI.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Codex CLI executable.
        #[arg(long, default_value = "codex")]
        codex_bin: PathBuf,
        /// Model slug to pass to Codex CLI.
        #[arg(long, default_value = DEFAULT_MODEL)]
        model: String,
        /// Reasoning effort to pass to Codex CLI.
        #[arg(long, value_enum, default_value_t = BenchmarkReasoningEffort::Medium)]
        reasoning_effort: BenchmarkReasoningEffort,
        /// Run each scenario in the suite this many times.
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        /// Run only these scenarios from the selected suite. Repeat to select multiple scenarios.
        #[arg(long = "scenario", value_enum)]
        scenarios: Vec<ProfileScenarioKind>,
        /// Kill a Codex CLI scenario attempt after this many seconds.
        #[arg(long, default_value_t = 900)]
        timeout_seconds: u64,
        /// Pass --ignore-user-config to Codex CLI for a more controlled run.
        #[arg(long)]
        ignore_user_config: bool,
        /// Run Codex CLI with a generated CODEX_HOME containing only copied auth.
        #[arg(long)]
        isolated_codex_home: bool,
        /// Directory where Codex CLI benchmark outputs are written.
        #[arg(long, default_value = ".spark-profile/codex-cli")]
        output_dir: PathBuf,
    },
    /// Run a benchmark suite through opencode for comparison with this harness.
    OpencodeBenchmark {
        /// Benchmark suite to run.
        #[arg(value_enum)]
        suite: ProfileBenchmarkSuiteKind,
        /// Workspace root for opencode.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// opencode executable.
        #[arg(long, default_value = "opencode")]
        opencode_bin: PathBuf,
        /// Model slug to pass to opencode, in provider/model form. Omit to use opencode's configured default.
        #[arg(long)]
        model: Option<String>,
        /// Reasoning effort to pass to opencode as its model variant.
        #[arg(long, value_enum, default_value_t = BenchmarkReasoningEffort::Medium)]
        reasoning_effort: BenchmarkReasoningEffort,
        /// Run each scenario in the suite this many times.
        #[arg(long, default_value_t = 1)]
        repeat: usize,
        /// Run only these scenarios from the selected suite. Repeat to select multiple scenarios.
        #[arg(long = "scenario", value_enum)]
        scenarios: Vec<ProfileScenarioKind>,
        /// Kill an opencode scenario attempt after this many seconds.
        #[arg(long, default_value_t = 900)]
        timeout_seconds: u64,
        /// Run opencode without external plugins.
        #[arg(long)]
        pure: bool,
        /// Directory where opencode benchmark outputs are written.
        #[arg(long, default_value = ".spark-profile/opencode")]
        output_dir: PathBuf,
    },
    /// Compare saved harness benchmark traces with external benchmark reports.
    BenchmarkCompare {
        /// Benchmark suite to compare.
        #[arg(long, value_enum, default_value_t = ProfileBenchmarkSuiteKind::RealWorld)]
        suite: ProfileBenchmarkSuiteKind,
        /// Workspace root containing .spark-runs/.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Maximum recent harness trace directories to scan.
        #[arg(long, default_value_t = 200)]
        limit: usize,
        /// Include every matching harness run instead of only the newest run per scenario.
        #[arg(long)]
        all_runs: bool,
        /// Optional Spark harness run manifest or saved benchmark report JSON. Repeat to merge inputs.
        #[arg(long)]
        harness_report: Vec<PathBuf>,
        /// Labeled Spark harness report for paired before/after comparisons, as LABEL=REPORT.
        /// Repeat with at least two labels. The first label is the default baseline when Codex CLI
        /// is absent.
        #[arg(long, value_name = "LABEL=REPORT")]
        harness_variant: Vec<String>,
        /// Runner to normalize as the 100-point resource baseline. Accepts a complete runner id or
        /// a harness-variant label. Defaults to Codex CLI when present, otherwise the first variant.
        #[arg(long)]
        baseline_runner: Option<String>,
        /// Optional Codex CLI benchmark JSON report(s) to compare against. Repeat to merge reports.
        /// Supply at least one benchmark or usage-history report input overall.
        #[arg(long)]
        codex_cli_report: Vec<PathBuf>,
        /// Optional opencode benchmark JSON report(s) to include in the comparison. Repeat to merge reports.
        #[arg(long)]
        opencode_report: Vec<PathBuf>,
        /// Optional Spark usage history JSON report(s) to include as usage-comparative evidence.
        #[arg(long)]
        usage_history_report: Vec<PathBuf>,
        /// Optional LLM judge JSON report to fold into solution/process scoring.
        #[arg(long)]
        llm_judge_report: Option<PathBuf>,
        /// Split runner labels by recorded reasoning effort, e.g. spark-harness/high.
        #[arg(long)]
        group_by_reasoning: bool,
        /// Split runner labels by recorded model, e.g. codex-cli/gpt-5.6-luna.
        #[arg(long)]
        group_by_model: bool,
        /// Exclude task attempts that did not pass benchmark validation before averaging.
        #[arg(long)]
        successful_only: bool,
        /// Exit nonzero after writing artifacts when inputs or provider skips make the headline directional.
        #[arg(long)]
        fail_on_directional_comparison: bool,
        /// Directory where comparison JSON, CSV, and HTML files are written.
        #[arg(long, default_value = ".spark-profile/benchmarks")]
        output_dir: PathBuf,
    },
    /// Use Spark as an LLM judge over an existing benchmark comparison report.
    BenchmarkJudge {
        /// Benchmark comparison JSON report to review.
        #[arg(long)]
        comparison_report: PathBuf,
        /// Workspace root used to resolve relative run evidence paths.
        #[arg(long, default_value = ".")]
        cwd: PathBuf,
        /// Model slug to use for the judge pass.
        #[arg(long, default_value = DEFAULT_JUDGE_MODEL)]
        model: String,
        /// Codex CLI executable used when the selected judge model is a Codex-only model.
        #[arg(long, default_value = "codex")]
        codex_bin: PathBuf,
        /// Reasoning effort for the judge model.
        #[arg(long, value_enum, default_value_t = JudgeReasoningEffort::Medium)]
        reasoning_effort: JudgeReasoningEffort,
        /// Kill a Codex CLI judge invocation after this many seconds.
        #[arg(long, default_value_t = 900)]
        timeout_seconds: u64,
        /// Directory where judge JSON is written.
        #[arg(long, default_value = ".spark-profile/benchmarks")]
        output_dir: PathBuf,
        /// Maximum matched scenarios to judge. Omit to judge every matched scenario.
        #[arg(long)]
        limit: Option<usize>,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RunMode {
    /// Read-only filesystem inspection tools only.
    Ask,
    /// Full workspace tools, including edits and command execution.
    Work,
}

fn parse_positive_usize(value: &str) -> Result<usize, String> {
    let parsed = value
        .parse::<usize>()
        .map_err(|_| "must be a positive whole number".to_string())?;
    if parsed == 0 {
        Err("must be at least 1".to_string())
    } else {
        Ok(parsed)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum RepoBriefFormat {
    Text,
    Json,
}

impl std::fmt::Display for RepoBriefFormat {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Text => "text",
            Self::Json => "json",
        })
    }
}

impl From<RunMode> for tools::AgentMode {
    fn from(value: RunMode) -> Self {
        match value {
            RunMode::Ask => Self::Ask,
            RunMode::Work => Self::Work,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum ProfileScenarioKind {
    /// Small repo survey that usually exercises read/list/search without edits.
    RepoSurvey,
    /// Multi-turn conversation that crosses auto-compaction pressure naturally.
    NaturalCompaction,
    /// Long prompt that crosses compaction pressure while staying below 128k tokens.
    CompactionPressure,
    /// Scratch-file coding task that exercises read, edit, write, and verification tools.
    FileEdit,
    /// Scratch-file workflow that exercises write, rename, search, and verification tools.
    FileOps,
    /// Scratch-file task that intentionally exercises native tool failure and recovery.
    ToolRecovery,
    /// Shell task that intentionally exercises command failure, stdout/stderr inspection, and recovery.
    ShellRecovery,
    /// Small code edit task that checks precise patching without unrelated rewrites.
    PrecisePatch,
    /// Coordinated multi-file edit task that checks consistency across code and docs.
    MultiFilePatch,
    /// Exact multi-artifact write task that checks schema, source-data, and prose consistency.
    ManifestContractWrite,
    /// Scoped code patch task that checks a targeted policy change against lookalike branches.
    ScopedPolicyPatch,
    /// Repo-local skill mention task that exercises automatic skill compile/load.
    SkillUse,
    /// Open-ended SteamNetworkLib repo explanation that stresses redundant read/search behavior.
    SteamNetworkLibSurvey,
    /// Open-ended S1API repo explanation that stresses broad API surface surveying.
    #[value(name = "s1api-survey", alias = "s1-api-survey")]
    S1ApiSurvey,
    /// Open-ended architecture survey of this Spark harness repo.
    RepoArchitectureSurvey,
    /// Benchmark-design survey that asks Spark to inspect and extend the scenario taxonomy.
    BenchmarkDesignSurvey,
    /// Four-turn read-only exploration of a Schedule I AssetRipper export.
    AssetRipperExploration,
    /// Four-turn read-only exploration of the Cfx.re/FiveM codebase.
    #[value(name = "fivem-exploration", alias = "five-m-exploration")]
    FiveMExploration,
    /// Four-turn read-only exploration of the Cpp2IL codebase.
    #[value(name = "cpp2il-exploration", alias = "cpp2-il-exploration")]
    Cpp2IlExploration,
    /// Four-turn read-only exploration of the Il2CppInterop codebase.
    #[value(
        name = "il2cpp-interop-exploration",
        alias = "il2-cpp-interop-exploration"
    )]
    Il2CppInteropExploration,
    /// Repo-local React + TypeScript calculator app scaffold in an ignored fixture folder.
    ReactCalculatorScaffold,
    /// Repo-local Rust log analyzer CLI scaffold in an ignored fixture folder.
    RustLogAnalyzerScaffold,
    /// Repo-local Rust modal notes CLI scaffold in an ignored fixture folder.
    RustNotesTuiScaffold,
    /// GitHub-style bugfix task with issue context, code, tests, and validation.
    GithubIssueBugfix,
    /// Rust failing-test bugfix task with objective Cargo validation.
    RustFailingTestBugfix,
    /// TypeScript reducer bugfix task with objective Bun validation.
    #[value(
        name = "typescript-reducer-bugfix",
        alias = "type-script-reducer-bugfix"
    )]
    TypeScriptReducerBugfix,
    /// Merge conflict resolution task with objective Bun validation.
    MergeConflictResolution,
    /// GitHub-style issue triage task that writes a grounded investigation note.
    GithubIssueTriage,
    /// CI failure triage task that diagnoses failing logs against source and tests.
    CiFailureTriage,
    /// Pull request review task that reports a regression from diff, source, and tests.
    PullRequestReview,
    /// Dependency upgrade triage task that reviews package, migration, source, and tests.
    DependencyUpgradeTriage,
    /// Sourced essay task that checks long-form writing from provided materials.
    TechnicalEssay,
    /// Config migration task that coordinates JSON, TypeScript, and docs.
    ConfigMigration,
    /// Operational data report task with computed metrics and narrative summary.
    OpsReport,
    /// Constrained inventory-rebalancing optimization with base and contingency budgets.
    InventoryRebalancePlan,
    /// Experiment rollout audit with dirty assignments, event attribution, and guardrails.
    ExperimentRolloutAudit,
    /// SWE-bench-style bugfix spanning multiple TypeScript modules with failing Bun tests.
    MultiModuleBugfix,
    /// Incident-driven state reconciliation bugfix with ambiguous evidence and cross-module invariants.
    StatefulReconciliationBugfix,
    /// Multi-tenant feature rollout bugfix spanning storage, evaluation, and revision-aware caching.
    FeatureRolloutConsistencyBugfix,
    /// Demonstration-only symbolic rule transfer with executable hidden-case validation.
    FrontierRuleTransfer,
    /// Terminal-Bench-style repair task that fixes a broken service through the terminal.
    TerminalRepair,
    /// GAIA-style multi-hop analysis that joins policy, orders, and refunds into an exact answer.
    MultiHopAnalysis,
    /// tau-bench-style two-turn support case that applies a multi-rule policy consistently.
    PolicySupportAgent,
}

#[derive(Debug, Clone, Copy, ValueEnum)]
pub(crate) enum ProfileBenchmarkSuiteKind {
    /// Existing smoke and native-tool scenarios.
    Core,
    /// Survey/exploration scenarios that stress repo understanding.
    Survey,
    /// New-project scaffolding scenarios in ignored fixture folders.
    Scaffolding,
    /// Precise and coordinated code-edit scenarios.
    Editing,
    /// Difficulty-focused compound tasks intended to preserve reasoning-level headroom.
    Reasoning,
    /// Real-world implementation, repair, migration, and project-scaffolding tasks.
    Coding,
    /// Quantitative reasoning over operational data, policies, and exact computed outputs.
    Quantitative,
    /// Evidence synthesis, investigation, review, and multi-source reasoning tasks.
    Analysis,
    /// Terminal repair, incident diagnosis, configuration, and operational reporting tasks.
    Operations,
    /// Grounded long-form, review, support, and configuration-writing tasks.
    Writing,
    /// Experimental unsaturated tasks targeting difficult novel-rule and long-horizon transfer.
    Frontier,
    /// Mixed real-world suite for broad Spark profiling.
    RealWorld,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum TraceSort {
    /// Newest run directory first.
    Newest,
    /// Highest post-satisfaction context growth first.
    OverrunContext,
    /// Highest post-satisfaction extra turn count first.
    OverrunTurns,
    /// Highest tool-only turn streak first.
    ToolOnlyStreak,
    /// Highest post-compaction request context regrowth first.
    CompactionRegrowth,
    /// Highest estimated request token count first.
    Context,
    /// Highest Spark request latency first.
    RequestMs,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum BenchmarkReasoningEffort {
    Minimal,
    Low,
    Medium,
    High,
}

impl BenchmarkReasoningEffort {
    pub(crate) fn wire_value(self) -> &'static str {
        match self {
            Self::Minimal => "minimal",
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub(crate) enum JudgeReasoningEffort {
    Medium,
    High,
    Xhigh,
}

impl JudgeReasoningEffort {
    pub(crate) fn wire_value(self) -> &'static str {
        match self {
            Self::Medium => "medium",
            Self::High => "high",
            Self::Xhigh => "xhigh",
        }
    }
}

impl ProfileScenarioKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::RepoSurvey => "repo-survey",
            Self::NaturalCompaction => "natural-compaction",
            Self::CompactionPressure => "compaction-pressure",
            Self::FileEdit => "file-edit",
            Self::FileOps => "file-ops",
            Self::ToolRecovery => "tool-recovery",
            Self::ShellRecovery => "shell-recovery",
            Self::PrecisePatch => "precise-patch",
            Self::MultiFilePatch => "multi-file-patch",
            Self::ManifestContractWrite => "manifest-contract-write",
            Self::ScopedPolicyPatch => "scoped-policy-patch",
            Self::SkillUse => "skill-use",
            Self::SteamNetworkLibSurvey => "steamnetworklib-survey",
            Self::S1ApiSurvey => "s1api-survey",
            Self::RepoArchitectureSurvey => "repo-architecture-survey",
            Self::BenchmarkDesignSurvey => "benchmark-design-survey",
            Self::AssetRipperExploration => "asset-ripper-exploration",
            Self::FiveMExploration => "fivem-exploration",
            Self::Cpp2IlExploration => "cpp2il-exploration",
            Self::Il2CppInteropExploration => "il2cpp-interop-exploration",
            Self::ReactCalculatorScaffold => "react-calculator-scaffold",
            Self::RustLogAnalyzerScaffold => "rust-log-analyzer-scaffold",
            Self::RustNotesTuiScaffold => "rust-notes-tui-scaffold",
            Self::GithubIssueBugfix => "github-issue-bugfix",
            Self::RustFailingTestBugfix => "rust-failing-test-bugfix",
            Self::TypeScriptReducerBugfix => "typescript-reducer-bugfix",
            Self::MergeConflictResolution => "merge-conflict-resolution",
            Self::GithubIssueTriage => "github-issue-triage",
            Self::CiFailureTriage => "ci-failure-triage",
            Self::PullRequestReview => "pull-request-review",
            Self::DependencyUpgradeTriage => "dependency-upgrade-triage",
            Self::TechnicalEssay => "technical-essay",
            Self::ConfigMigration => "config-migration",
            Self::OpsReport => "ops-report",
            Self::InventoryRebalancePlan => "inventory-rebalance-plan",
            Self::ExperimentRolloutAudit => "experiment-rollout-audit",
            Self::MultiModuleBugfix => "multi-module-bugfix",
            Self::StatefulReconciliationBugfix => "stateful-reconciliation-bugfix",
            Self::FeatureRolloutConsistencyBugfix => "feature-rollout-consistency-bugfix",
            Self::FrontierRuleTransfer => "frontier-rule-transfer",
            Self::TerminalRepair => "terminal-repair",
            Self::MultiHopAnalysis => "multi-hop-analysis",
            Self::PolicySupportAgent => "policy-support-agent",
        }
    }
}

impl ProfileBenchmarkSuiteKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Core => "core",
            Self::Survey => "survey",
            Self::Scaffolding => "scaffolding",
            Self::Editing => "editing",
            Self::Reasoning => "reasoning",
            Self::Coding => "coding",
            Self::Quantitative => "quantitative",
            Self::Analysis => "analysis",
            Self::Operations => "operations",
            Self::Writing => "writing",
            Self::Frontier => "frontier",
            Self::RealWorld => "real-world",
        }
    }

    pub(crate) fn scenarios(self) -> &'static [ProfileScenarioKind] {
        match self {
            Self::Core => &[
                ProfileScenarioKind::RepoSurvey,
                ProfileScenarioKind::FileEdit,
                ProfileScenarioKind::FileOps,
                ProfileScenarioKind::ToolRecovery,
                ProfileScenarioKind::ShellRecovery,
                ProfileScenarioKind::SkillUse,
            ],
            Self::Survey => &[
                ProfileScenarioKind::RepoSurvey,
                ProfileScenarioKind::RepoArchitectureSurvey,
                ProfileScenarioKind::BenchmarkDesignSurvey,
                ProfileScenarioKind::AssetRipperExploration,
                ProfileScenarioKind::FiveMExploration,
                ProfileScenarioKind::Cpp2IlExploration,
                ProfileScenarioKind::Il2CppInteropExploration,
                ProfileScenarioKind::GithubIssueTriage,
                ProfileScenarioKind::CiFailureTriage,
                ProfileScenarioKind::PullRequestReview,
                ProfileScenarioKind::DependencyUpgradeTriage,
                ProfileScenarioKind::TechnicalEssay,
                ProfileScenarioKind::SteamNetworkLibSurvey,
                ProfileScenarioKind::S1ApiSurvey,
            ],
            Self::Scaffolding => &[
                ProfileScenarioKind::ReactCalculatorScaffold,
                ProfileScenarioKind::RustLogAnalyzerScaffold,
                ProfileScenarioKind::RustNotesTuiScaffold,
            ],
            Self::Editing => &[
                ProfileScenarioKind::PrecisePatch,
                ProfileScenarioKind::MultiFilePatch,
                ProfileScenarioKind::ManifestContractWrite,
                ProfileScenarioKind::ScopedPolicyPatch,
                ProfileScenarioKind::GithubIssueBugfix,
                ProfileScenarioKind::RustFailingTestBugfix,
                ProfileScenarioKind::TypeScriptReducerBugfix,
                ProfileScenarioKind::MergeConflictResolution,
                ProfileScenarioKind::ConfigMigration,
                ProfileScenarioKind::MultiModuleBugfix,
                ProfileScenarioKind::StatefulReconciliationBugfix,
                ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
            ],
            Self::Reasoning => &[
                ProfileScenarioKind::TechnicalEssay,
                ProfileScenarioKind::ConfigMigration,
                ProfileScenarioKind::OpsReport,
                ProfileScenarioKind::InventoryRebalancePlan,
                ProfileScenarioKind::ExperimentRolloutAudit,
                ProfileScenarioKind::MultiModuleBugfix,
                ProfileScenarioKind::TerminalRepair,
                ProfileScenarioKind::MultiHopAnalysis,
                ProfileScenarioKind::PolicySupportAgent,
                ProfileScenarioKind::RustNotesTuiScaffold,
                ProfileScenarioKind::StatefulReconciliationBugfix,
                ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
                ProfileScenarioKind::FrontierRuleTransfer,
            ],
            Self::Coding => &[
                ProfileScenarioKind::MultiFilePatch,
                ProfileScenarioKind::ManifestContractWrite,
                ProfileScenarioKind::ScopedPolicyPatch,
                ProfileScenarioKind::GithubIssueBugfix,
                ProfileScenarioKind::RustFailingTestBugfix,
                ProfileScenarioKind::TypeScriptReducerBugfix,
                ProfileScenarioKind::MergeConflictResolution,
                ProfileScenarioKind::ConfigMigration,
                ProfileScenarioKind::ReactCalculatorScaffold,
                ProfileScenarioKind::RustLogAnalyzerScaffold,
                ProfileScenarioKind::RustNotesTuiScaffold,
                ProfileScenarioKind::MultiModuleBugfix,
                ProfileScenarioKind::StatefulReconciliationBugfix,
                ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
            ],
            Self::Quantitative => &[
                ProfileScenarioKind::OpsReport,
                ProfileScenarioKind::MultiHopAnalysis,
                ProfileScenarioKind::InventoryRebalancePlan,
                ProfileScenarioKind::ExperimentRolloutAudit,
            ],
            Self::Analysis => &[
                ProfileScenarioKind::GithubIssueTriage,
                ProfileScenarioKind::CiFailureTriage,
                ProfileScenarioKind::PullRequestReview,
                ProfileScenarioKind::DependencyUpgradeTriage,
                ProfileScenarioKind::TechnicalEssay,
                ProfileScenarioKind::MultiHopAnalysis,
                ProfileScenarioKind::ExperimentRolloutAudit,
                ProfileScenarioKind::PolicySupportAgent,
            ],
            Self::Operations => &[
                ProfileScenarioKind::ShellRecovery,
                ProfileScenarioKind::ToolRecovery,
                ProfileScenarioKind::CiFailureTriage,
                ProfileScenarioKind::DependencyUpgradeTriage,
                ProfileScenarioKind::ConfigMigration,
                ProfileScenarioKind::OpsReport,
                ProfileScenarioKind::InventoryRebalancePlan,
                ProfileScenarioKind::ExperimentRolloutAudit,
                ProfileScenarioKind::TerminalRepair,
            ],
            Self::Writing => &[
                ProfileScenarioKind::GithubIssueTriage,
                ProfileScenarioKind::CiFailureTriage,
                ProfileScenarioKind::PullRequestReview,
                ProfileScenarioKind::DependencyUpgradeTriage,
                ProfileScenarioKind::TechnicalEssay,
                ProfileScenarioKind::ConfigMigration,
                ProfileScenarioKind::PolicySupportAgent,
            ],
            Self::Frontier => &[
                ProfileScenarioKind::FrontierRuleTransfer,
                ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
            ],
            Self::RealWorld => &[
                ProfileScenarioKind::RepoSurvey,
                ProfileScenarioKind::RepoArchitectureSurvey,
                ProfileScenarioKind::BenchmarkDesignSurvey,
                ProfileScenarioKind::GithubIssueTriage,
                ProfileScenarioKind::CiFailureTriage,
                ProfileScenarioKind::PullRequestReview,
                ProfileScenarioKind::DependencyUpgradeTriage,
                ProfileScenarioKind::TechnicalEssay,
                ProfileScenarioKind::ShellRecovery,
                ProfileScenarioKind::PrecisePatch,
                ProfileScenarioKind::MultiFilePatch,
                ProfileScenarioKind::ManifestContractWrite,
                ProfileScenarioKind::ScopedPolicyPatch,
                ProfileScenarioKind::GithubIssueBugfix,
                ProfileScenarioKind::RustFailingTestBugfix,
                ProfileScenarioKind::TypeScriptReducerBugfix,
                ProfileScenarioKind::MergeConflictResolution,
                ProfileScenarioKind::ConfigMigration,
                ProfileScenarioKind::ReactCalculatorScaffold,
                ProfileScenarioKind::RustLogAnalyzerScaffold,
                ProfileScenarioKind::RustNotesTuiScaffold,
                ProfileScenarioKind::OpsReport,
                ProfileScenarioKind::InventoryRebalancePlan,
                ProfileScenarioKind::ExperimentRolloutAudit,
                ProfileScenarioKind::ToolRecovery,
                ProfileScenarioKind::MultiModuleBugfix,
                ProfileScenarioKind::StatefulReconciliationBugfix,
                ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
                ProfileScenarioKind::FrontierRuleTransfer,
                ProfileScenarioKind::TerminalRepair,
                ProfileScenarioKind::MultiHopAnalysis,
                ProfileScenarioKind::PolicySupportAgent,
            ],
        }
    }
}

#[cfg(test)]
mod benchmark_suite_tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn category_suites_are_nonempty_real_world_subsets() {
        let real_world = ProfileBenchmarkSuiteKind::RealWorld.scenarios();
        for suite in [
            ProfileBenchmarkSuiteKind::Coding,
            ProfileBenchmarkSuiteKind::Quantitative,
            ProfileBenchmarkSuiteKind::Analysis,
            ProfileBenchmarkSuiteKind::Operations,
            ProfileBenchmarkSuiteKind::Writing,
            ProfileBenchmarkSuiteKind::Frontier,
        ] {
            assert!(
                !suite.scenarios().is_empty(),
                "{} must not be empty",
                suite.name()
            );
            for scenario in suite.scenarios() {
                assert!(
                    real_world.contains(scenario),
                    "{} contains non-real-world scenario {}",
                    suite.name(),
                    scenario.name()
                );
            }
        }
    }

    #[test]
    fn published_reasoning_sweep_uses_reasoning_suite_scenarios() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("web/src/data/reasoning-sweep.json");
        let text = std::fs::read_to_string(path).expect("read published reasoning sweep");
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("reasoning sweep should be valid JSON");
        let dataset = &value["dataset"];
        let reasoning = ProfileBenchmarkSuiteKind::Reasoning
            .scenarios()
            .iter()
            .map(|scenario| scenario.name())
            .collect::<HashSet<_>>();
        for attempt in dataset["attempts"].as_array().expect("attempt rows") {
            let scenario = attempt["scenario"].as_str().expect("scenario name");
            assert!(
                reasoning.contains(scenario),
                "{scenario} is not in reasoning"
            );
        }
    }

    #[test]
    fn published_reasoning_sweep_has_a_complete_balanced_matrix() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("web/src/data/reasoning-sweep.json");
        let text = std::fs::read_to_string(path).expect("read published reasoning sweep");
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("reasoning sweep should be valid JSON");
        let dataset = &value["dataset"];
        let scenarios = dataset["scenarioCount"].as_u64().expect("scenario count");
        let repeats = dataset["expectedRepeats"].as_u64().expect("repeat count");
        let attempt_rows = dataset["attempts"].as_array().expect("attempt rows");

        assert_eq!(attempt_rows.len() as u64, scenarios * 6);
        assert_eq!(
            dataset["totalAttempts"].as_u64(),
            Some(scenarios * repeats * 6)
        );
        assert_eq!(dataset["rows"].as_array().map(Vec::len), Some(6));
    }

    #[test]
    fn published_reasoning_sweep_keeps_quality_and_failure_counts_separate() {
        let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("web/src/data/reasoning-sweep.json");
        let text = std::fs::read_to_string(path).expect("read published reasoning sweep");
        let value: serde_json::Value =
            serde_json::from_str(&text).expect("reasoning sweep should be valid JSON");
        let dataset = &value["dataset"];
        let attempts = dataset["attempts"].as_array().expect("attempt rows");
        let failed = attempts
            .iter()
            .map(|row| row["failed"].as_u64().expect("failed count"))
            .sum::<u64>();

        assert_eq!(dataset["failedAttempts"].as_u64(), Some(failed));
        assert!(attempts.iter().all(|row| row["quality"].as_f64().is_some()));
        assert_eq!(dataset["providerExclusions"].as_u64(), Some(0));
    }
}
