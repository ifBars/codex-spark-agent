use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::{
    DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS, DEFAULT_MODEL, DEFAULT_SCENARIO_TARGET_TOKENS,
    benchmark_judge::DEFAULT_JUDGE_MODEL, tools,
};

#[derive(Debug, Parser)]
#[command(name = "spark")]
#[command(about = "A small GPT-5.3 Codex Spark agent harness")]
pub(crate) struct Cli {
    #[command(subcommand)]
    pub(crate) command: Command,
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
    /// Show saved auth status.
    AuthStatus,
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
        /// Tool access mode. ask is read-only; work allows edits and command execution.
        #[arg(long, value_enum, default_value_t = RunMode::Work)]
        mode: RunMode,
        /// Maximum agent/tool turns. Omit to let Spark run until it completes.
        #[arg(long)]
        max_turns: Option<usize>,
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
        /// Compact older tool outputs once request JSON exceeds this many characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Compact older tool outputs once estimated input exceeds this many tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Force compaction after this many consecutive tool-only turns. Use 0 to disable.
        #[arg(long, default_value_t = DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS)]
        compact_after_tool_only_turns: usize,
        /// Refuse to send request JSON above this many characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Refuse to send a request once estimated input exceeds this many tokens.
        #[arg(long)]
        max_input_tokens: Option<usize>,
    },
    /// Print available built-in tools as JSON.
    Tools,
    /// List saved chat sessions.
    Sessions,
    /// List or refresh repo-local Spark skill cache.
    Skills {
        /// Rebuild cached summaries from .agents/skills.
        #[arg(long)]
        refresh: bool,
    },
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
        /// Maximum agent/tool turns. Omit to let Spark run until it completes.
        #[arg(long)]
        max_turns: Option<usize>,
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
        /// Compact older context once request JSON exceeds this many characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Compact older context once estimated input exceeds this many tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Force compaction after this many consecutive tool-only turns. Use 0 to disable.
        #[arg(long, default_value_t = DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS)]
        compact_after_tool_only_turns: usize,
        /// Refuse to send request JSON above this many characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Refuse to send a request once estimated input exceeds this many tokens.
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
        /// Maximum agent/tool turns per scenario prompt. Omit to let Spark run until it completes.
        #[arg(long)]
        max_turns: Option<usize>,
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
        /// Compact older context once request JSON exceeds this many characters.
        #[arg(long)]
        compact_after_chars: Option<usize>,
        /// Compact older context once estimated input exceeds this many tokens.
        #[arg(long)]
        compact_after_tokens: Option<usize>,
        /// Force compaction after this many consecutive tool-only turns. Use 0 to disable.
        #[arg(long, default_value_t = DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS)]
        compact_after_tool_only_turns: usize,
        /// Refuse to send request JSON above this many characters.
        #[arg(long)]
        max_input_chars: Option<usize>,
        /// Refuse to send a request once estimated input exceeds this many tokens.
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
        /// Codex CLI benchmark JSON report(s) to compare against. Repeat to merge reports.
        #[arg(long, required = true)]
        codex_cli_report: Vec<PathBuf>,
        /// Optional opencode benchmark JSON report(s) to include in the comparison. Repeat to merge reports.
        #[arg(long)]
        opencode_report: Vec<PathBuf>,
        /// Optional LLM judge JSON report to fold into solution/process scoring.
        #[arg(long)]
        llm_judge_report: Option<PathBuf>,
        /// Split runner labels by recorded reasoning effort, e.g. spark-harness/high.
        #[arg(long)]
        group_by_reasoning: bool,
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
        /// Reasoning effort for the judge model.
        #[arg(long, value_enum, default_value_t = JudgeReasoningEffort::High)]
        reasoning_effort: JudgeReasoningEffort,
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
    /// GitHub-style issue triage task that writes a grounded investigation note.
    GithubIssueTriage,
    /// Sourced essay task that checks long-form writing from provided materials.
    TechnicalEssay,
    /// Config migration task that coordinates JSON, TypeScript, and docs.
    ConfigMigration,
    /// Operational data report task with computed metrics and narrative summary.
    OpsReport,
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
    High,
    Xhigh,
}

impl JudgeReasoningEffort {
    pub(crate) fn wire_value(self) -> &'static str {
        match self {
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
            Self::SkillUse => "skill-use",
            Self::SteamNetworkLibSurvey => "steamnetworklib-survey",
            Self::S1ApiSurvey => "s1api-survey",
            Self::RepoArchitectureSurvey => "repo-architecture-survey",
            Self::BenchmarkDesignSurvey => "benchmark-design-survey",
            Self::ReactCalculatorScaffold => "react-calculator-scaffold",
            Self::RustLogAnalyzerScaffold => "rust-log-analyzer-scaffold",
            Self::RustNotesTuiScaffold => "rust-notes-tui-scaffold",
            Self::GithubIssueBugfix => "github-issue-bugfix",
            Self::RustFailingTestBugfix => "rust-failing-test-bugfix",
            Self::TypeScriptReducerBugfix => "typescript-reducer-bugfix",
            Self::GithubIssueTriage => "github-issue-triage",
            Self::TechnicalEssay => "technical-essay",
            Self::ConfigMigration => "config-migration",
            Self::OpsReport => "ops-report",
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
                ProfileScenarioKind::GithubIssueTriage,
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
                ProfileScenarioKind::GithubIssueBugfix,
                ProfileScenarioKind::RustFailingTestBugfix,
                ProfileScenarioKind::TypeScriptReducerBugfix,
                ProfileScenarioKind::ConfigMigration,
            ],
            Self::RealWorld => &[
                ProfileScenarioKind::RepoSurvey,
                ProfileScenarioKind::RepoArchitectureSurvey,
                ProfileScenarioKind::BenchmarkDesignSurvey,
                ProfileScenarioKind::GithubIssueTriage,
                ProfileScenarioKind::TechnicalEssay,
                ProfileScenarioKind::ShellRecovery,
                ProfileScenarioKind::PrecisePatch,
                ProfileScenarioKind::MultiFilePatch,
                ProfileScenarioKind::GithubIssueBugfix,
                ProfileScenarioKind::RustFailingTestBugfix,
                ProfileScenarioKind::TypeScriptReducerBugfix,
                ProfileScenarioKind::ConfigMigration,
                ProfileScenarioKind::ReactCalculatorScaffold,
                ProfileScenarioKind::RustLogAnalyzerScaffold,
                ProfileScenarioKind::RustNotesTuiScaffold,
                ProfileScenarioKind::OpsReport,
                ProfileScenarioKind::ToolRecovery,
            ],
        }
    }
}
