use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};

use crate::{
    DEFAULT_COMPACT_AFTER_TOOL_ONLY_TURNS, DEFAULT_MODEL, DEFAULT_SCENARIO_TARGET_TOKENS, tools,
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

#[derive(Debug, Clone, Copy, ValueEnum)]
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
    /// Repo-local skill mention task that exercises automatic skill compile/load.
    SkillUse,
    /// Open-ended SteamNetworkLib repo explanation that stresses redundant read/search behavior.
    SteamNetworkLibSurvey,
    /// Open-ended S1API repo explanation that stresses broad API surface surveying.
    #[value(name = "s1api-survey", alias = "s1-api-survey")]
    S1ApiSurvey,
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

impl ProfileScenarioKind {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::RepoSurvey => "repo-survey",
            Self::NaturalCompaction => "natural-compaction",
            Self::CompactionPressure => "compaction-pressure",
            Self::FileEdit => "file-edit",
            Self::FileOps => "file-ops",
            Self::ToolRecovery => "tool-recovery",
            Self::SkillUse => "skill-use",
            Self::SteamNetworkLibSurvey => "steamnetworklib-survey",
            Self::S1ApiSurvey => "s1api-survey",
        }
    }
}
