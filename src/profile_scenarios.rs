use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use crate::{
    APPROX_CHARS_PER_TOKEN, MAX_SCENARIO_REPEAT, MAX_SCENARIO_TARGET_TOKENS,
    cli::ProfileScenarioKind,
};

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProfileScenarioValidationCommand {
    pub(crate) workdir: &'static str,
    pub(crate) program: &'static str,
    pub(crate) args: &'static [&'static str],
}

pub(crate) fn prepare_profile_scenario(cwd: &Path, scenario: ProfileScenarioKind) -> Result<()> {
    let Some(name) = (match scenario {
        ProfileScenarioKind::FileEdit => Some("file-edit"),
        ProfileScenarioKind::FileOps => Some("file-ops"),
        ProfileScenarioKind::ToolRecovery => Some("tool-recovery"),
        ProfileScenarioKind::ReactCalculatorScaffold => Some("react-calculator"),
        ProfileScenarioKind::RustLogAnalyzerScaffold => Some("rust-log-analyzer"),
        _ => None,
    }) else {
        return Ok(());
    };

    let dir = cwd.join(".spark-scenarios").join(name);
    if dir.exists() {
        std::fs::remove_dir_all(&dir)
            .map_err(|error| anyhow::anyhow!("failed to reset {}: {error}", dir.display()))?;
    }
    std::fs::create_dir_all(&dir)
        .map_err(|error| anyhow::anyhow!("failed to create {}: {error}", dir.display()))?;
    match scenario {
        ProfileScenarioKind::FileEdit => {
            std::fs::write(
                dir.join("notes.md"),
                "# Spark File Edit Fixture\n\n- status: draft\n- owner: spark\n\nTODO: replace this line with a concise final note.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture notes.md: {error}"))?;
            std::fs::write(
                dir.join("config.toml"),
                "name = \"spark-fixture\"\nmode = \"draft\"\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture config.toml: {error}"))?;
        }
        ProfileScenarioKind::FileOps => {
            std::fs::create_dir_all(dir.join("drafts"))
                .map_err(|error| anyhow::anyhow!("failed to create drafts fixture: {error}"))?;
            std::fs::write(
                dir.join("manifest.txt"),
                "file-ops fixture\nexpected_final=final/report.md\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture manifest.txt: {error}"))?;
        }
        ProfileScenarioKind::ToolRecovery => {
            std::fs::create_dir_all(dir.join("source"))
                .map_err(|error| anyhow::anyhow!("failed to create source fixture: {error}"))?;
            std::fs::write(
                dir.join("source").join("note.md"),
                "# Recovery Fixture\n\nSpark recovery path verified.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture source/note.md: {error}"))?;
        }
        ProfileScenarioKind::ReactCalculatorScaffold => {
            std::fs::write(
                dir.join("brief.md"),
                "# React Calculator Brief\n\nBuild a small React + TypeScript calculator app in this folder. It should support digits, decimal input, clear, backspace, the four basic operators, equals, keyboard input, and a visible calculation history. Use bun for JavaScript package management and keep all generated app files inside this ignored fixture folder. The validation commands are `bun test` plus a Playwright browser smoke check that runs the app through Vite, screenshots it, and clicks 1 + 2 =. Include a browser-runnable Vite entrypoint such as index.html and package setup. Either keep tests compatible with Bun's default test runtime or add the package/config setup required for DOM-based React tests before using React Testing Library.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
        }
        ProfileScenarioKind::RustLogAnalyzerScaffold => {
            std::fs::write(
                dir.join("brief.md"),
                "# Rust Log Analyzer Brief\n\nCreate a small Rust CLI project in this folder that reads a log file path, counts INFO/WARN/ERROR lines, reports the top error code when present, and has focused unit tests for the parser. Keep Cargo output in this project's default target/ directory; do not set CARGO_TARGET_DIR.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("sample.log"),
                "2026-06-03T10:00:00Z INFO boot complete\n2026-06-03T10:01:00Z WARN queue lag=42\n2026-06-03T10:02:00Z ERROR code=E42 payment failed\n2026-06-03T10:03:00Z ERROR code=E42 retry failed\n2026-06-03T10:04:00Z ERROR code=E7 cache miss\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture sample.log: {error}"))?;
        }
        _ => {}
    }
    Ok(())
}

pub(crate) fn profile_scenario_prompts(
    scenario: ProfileScenarioKind,
    target_tokens: usize,
) -> Result<Vec<String>> {
    if target_tokens == 0 {
        anyhow::bail!("--target-tokens must be greater than 0");
    }
    if target_tokens > MAX_SCENARIO_TARGET_TOKENS {
        anyhow::bail!(
            "--target-tokens must be <= {MAX_SCENARIO_TARGET_TOKENS} so the prompt stays below Spark's 128k context window with JSON overhead"
        );
    }

    match scenario {
        ProfileScenarioKind::RepoSurvey => Ok(vec![
            "Profile scenario: repo-survey.\n\
             Inspect this repository like a coding agent. Use targeted native tools, not broad command output.\n\
             1. List the repository root.\n\
             2. Read Cargo.toml and README.md with bounded windows.\n\
             3. Search src for tool and compaction surfaces.\n\
             4. Finish with a concise harness-risk summary and one next profiling recommendation."
                .to_string(),
        ]),
        ProfileScenarioKind::FileEdit => Ok(vec![
            "Profile scenario: file-edit.\n\
             Work only under .spark-scenarios/file-edit.\n\
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.read on .spark-scenarios/file-edit/notes.md.\n\
             2. Use fs.edit or fs.replace on .spark-scenarios/file-edit/notes.md to replace the TODO line with: Final note: Spark edited this fixture with native tools.\n\
             3. Use fs.write on .spark-scenarios/file-edit/summary.txt with a one-line summary of what changed.\n\
             4. Use fs.read on both changed files to verify the final contents.\n\
             Finish with the tools used, whether verification passed, and any harness behavior that made the task easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::FileOps => Ok(vec![
            "Profile scenario: file-ops.\n\
             Work only under .spark-scenarios/file-ops.\n\
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.write on .spark-scenarios/file-ops/drafts/report-draft.md with a short markdown report containing the exact phrase: Spark rename path verified.\n\
             2. Use fs.rename to move .spark-scenarios/file-ops/drafts/report-draft.md to .spark-scenarios/file-ops/final/report.md.\n\
             3. Use fs.stat on .spark-scenarios/file-ops/final/report.md to verify the final path exists before reading it.\n\
             4. Use fs.read on .spark-scenarios/file-ops/final/report.md to verify the final contents.\n\
             5. Use fs.search under .spark-scenarios/file-ops for Spark rename path verified.\n\
             Finish with the native tools used, whether verification passed, and any harness behavior that made the workflow easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::ToolRecovery => Ok(vec![
            "Profile scenario: tool-recovery.\n\
             Work only under .spark-scenarios/tool-recovery.\n\
             Use native file tools, not cmd.exec.\n\
             Required actions:\n\
             1. First use fs.read on .spark-scenarios/tool-recovery/source/missing-note.md. This path is intentionally missing; do not skip this failing probe.\n\
             2. Use fs.stat on .spark-scenarios/tool-recovery/source/note.md to verify the corrected path exists after the failed observation.\n\
             3. Use fs.read on .spark-scenarios/tool-recovery/source/note.md to verify it contains: Spark recovery path verified.\n\
             4. Use fs.write on .spark-scenarios/tool-recovery/recovery-summary.txt with one line naming whether native-tool recovery succeeded.\n\
             Finish with the native tools used, whether recovery passed, and whether the harness observation made the correction clear."
                .to_string(),
        ]),
        ProfileScenarioKind::SkillUse => Ok(vec![
            "Profile scenario: skill-use.\n\
             Load and apply @rust-patterns before answering.\n\
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.read on src/main.rs with a bounded window.\n\
             2. Use fs.search under src for load_skill_mentions.\n\
             Finish with two concise Rust harness risks or cleanup opportunities, and mention whether the loaded skill guidance affected your review."
                .to_string(),
        ]),
        ProfileScenarioKind::SteamNetworkLibSurvey => Ok(vec![
            "Profile scenario: steamnetworklib-survey.\n\
             Answer this like a natural repo-understanding chat, grounded in repository sources:\n\
             What is SteamNetworkLib, what does it do, and how does it work?\n\
             Use targeted native tools to inspect the repo. Start from the root shape and key docs, then inspect implementation files only where needed.\n\
             Finish with a concise explanation of the library's purpose, main subsystems, and request/data flow.\n\
             Also mention one thing the harness made easier or harder while gathering evidence."
                .to_string(),
        ]),
        ProfileScenarioKind::S1ApiSurvey => Ok(vec![
            "Profile scenario: s1api-survey.\n\
             Answer this like a natural repo-understanding chat, grounded in repository sources:\n\
             What is S1API, what does it do, and how does it work?\n\
             Use targeted native tools to inspect the repo. Start from the root shape and key docs such as index.md, then inspect the entrypoint and representative API areas only where needed.\n\
             Avoid trying to read the entire generated api/_site tree; use bounded reads and narrow searches.\n\
             Finish with a concise explanation of the API's purpose, main subsystems, and mod/runtime flow.\n\
             Also mention one thing the harness made easier or harder while gathering evidence."
                .to_string(),
        ]),
        ProfileScenarioKind::RepoArchitectureSurvey => Ok(vec![
            "Profile scenario: repo-architecture-survey.\n\
             Answer like a coding agent being asked to understand this Spark harness repo before changing it.\n\
             Use targeted native tools and bounded reads.\n\
             Required evidence path:\n\
             1. Use fs.list on . with recursive=false.\n\
             2. Use fs.read on AGENTS.md.\n\
             3. Use fs.read on README.md.\n\
             4. Use fs.search under src for ProfileScenarioKind.\n\
             5. Use fs.search under src for AgentRunner.\n\
             Finish with a concise architecture map, the scenario/profiler flow, and two likely failure surfaces to profile next."
                .to_string(),
        ]),
        ProfileScenarioKind::BenchmarkDesignSurvey => Ok(vec![
            "Profile scenario: benchmark-design-survey.\n\
             Survey the existing profiling scenarios and propose benchmark coverage gaps grounded in this repo.\n\
             Use targeted native tools and bounded reads; do not run benchmarks.\n\
             Required evidence path:\n\
             1. Use fs.read on src/profile_scenarios.rs.\n\
             2. Use fs.read on src/profiler/analyze/expectations.rs.\n\
             3. Use fs.search under README.md for profile-scenario.\n\
             4. Use fs.search under src for expected_tool_calls.\n\
             Finish with a prioritized benchmark plan containing three concrete new task prompts, expected evidence signals, and which existing scenarios they should be compared against."
                .to_string(),
        ]),
        ProfileScenarioKind::ReactCalculatorScaffold => Ok(vec![
            "Profile scenario: react-calculator-scaffold.\n\
             Build a brand new React + TypeScript calculator app only under .spark-scenarios/react-calculator.\n\
             Use bun for JavaScript package management. Do not create files outside this ignored fixture folder.\n\
             The finished app will be checked by bun test and a Playwright browser smoke check, so it must be runnable through Vite in a real browser.\n\
             Required actions:\n\
             1. Use fs.read on .spark-scenarios/react-calculator/brief.md.\n\
             2. Use fs.write to create .spark-scenarios/react-calculator/package.json.\n\
             3. Use fs.write to create .spark-scenarios/react-calculator/index.html.\n\
             4. Use fs.write to create .spark-scenarios/react-calculator/src/main.tsx.\n\
             5. Use fs.write to create .spark-scenarios/react-calculator/src/App.tsx.\n\
             6. Use fs.write to create .spark-scenarios/react-calculator/src/App.test.tsx.\n\
             7. Use fs.write to create .spark-scenarios/react-calculator/src/styles.css.\n\
             8. Use cmd.exec from .spark-scenarios/react-calculator to run bun test when possible; if tests need a DOM, configure it before using DOM-based test helpers.\n\
             Finish with the app files created, validation result, and any harness behavior that made project scaffolding easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::RustLogAnalyzerScaffold => Ok(vec![
            "Profile scenario: rust-log-analyzer-scaffold.\n\
             Build a brand new Rust CLI project only under .spark-scenarios/rust-log-analyzer.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             Required actions:\n\
             1. Use fs.read on .spark-scenarios/rust-log-analyzer/brief.md.\n\
             2. Use fs.read on .spark-scenarios/rust-log-analyzer/sample.log.\n\
             3. Use fs.write to create .spark-scenarios/rust-log-analyzer/Cargo.toml.\n\
             4. Use fs.write to create .spark-scenarios/rust-log-analyzer/src/lib.rs.\n\
             5. Use fs.write to create .spark-scenarios/rust-log-analyzer/src/main.rs.\n\
             6. Use cmd.exec from .spark-scenarios/rust-log-analyzer to run cargo test.\n\
             Finish with the CLI behavior, test result, and any harness behavior that made project scaffolding easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::NaturalCompaction => natural_compaction_scenario_prompts(target_tokens),
        ProfileScenarioKind::CompactionPressure => {
            let target_chars = target_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
            let mut prompt = String::from(
                "Profile scenario: compaction-pressure.\n\
                 This prompt intentionally creates long-context pressure below Spark's 128k context window.\n\
                 Let the harness compact automatically if its threshold is crossed.\n\
                 Do not restate the synthetic payload. After any compaction, use fs.list on src with recursive=false, then answer with:\n\
                 - whether the task remained understandable,\n\
                 - which tool you used,\n\
                 - any missing information caused by compaction,\n\
                 - one concrete harness change that would make this scenario more reliable.\n\n\
                 Synthetic payload follows. Preserve the high-level instruction above; payload rows are intentionally repetitive profiling filler.\n",
            );
            let mut row = 0usize;
            while prompt.len() < target_chars {
                row += 1;
                prompt.push_str(&format!(
                    "row {row:05}: spark compaction profiling filler; keep task intent, discard repetition, prefer native tools over shell floods, report uncertainty plainly.\n"
                ));
            }
            Ok(vec![prompt])
        }
    }
}

pub(crate) fn benchmark_profile_prompts(
    scenario: ProfileScenarioKind,
    target_tokens: usize,
) -> Result<Vec<String>> {
    match scenario {
        ProfileScenarioKind::NaturalCompaction | ProfileScenarioKind::CompactionPressure => {
            profile_scenario_prompts(scenario, target_tokens)
        }
        _ => Ok(vec![benchmark_task_prompt(scenario)]),
    }
}

pub(crate) fn benchmark_task_prompt(scenario: ProfileScenarioKind) -> String {
    match scenario {
        ProfileScenarioKind::RepoSurvey => {
            "Benchmark scenario: repo-survey.\n\
             Inspect this repository like a coding agent. Use bounded file reads and targeted searches rather than broad output.\n\
             1. List the repository root.\n\
             2. Read Cargo.toml and README.md.\n\
             3. Search src for tool and compaction surfaces.\n\
             4. Finish with a concise harness-risk summary and one next profiling recommendation."
                .to_string()
        }
        ProfileScenarioKind::RepoArchitectureSurvey => {
            "Benchmark scenario: repo-architecture-survey.\n\
             Understand this Spark harness repo before changing it.\n\
             Required evidence path:\n\
             1. Inspect the repository root.\n\
             2. Read AGENTS.md.\n\
             3. Read README.md.\n\
             4. Search src for ProfileScenarioKind.\n\
             5. Search src for AgentRunner.\n\
             Finish with a concise architecture map, the scenario/profiler flow, and two likely failure surfaces to profile next."
                .to_string()
        }
        ProfileScenarioKind::BenchmarkDesignSurvey => {
            "Benchmark scenario: benchmark-design-survey.\n\
             Survey the existing profiling scenarios and propose benchmark coverage gaps grounded in this repo.\n\
             Do not run benchmarks.\n\
             Required evidence path:\n\
             1. Read src/profile_scenarios.rs.\n\
             2. Read src/profiler/analyze/expectations.rs.\n\
             3. Search README.md for profile-scenario.\n\
             4. Search src for expected_tool_calls.\n\
             Finish with a prioritized benchmark plan containing three concrete new task prompts, expected evidence signals, and which existing scenarios they should be compared against."
                .to_string()
        }
        ProfileScenarioKind::ReactCalculatorScaffold => {
            "Benchmark scenario: react-calculator-scaffold.\n\
             Build a brand new React + TypeScript calculator app only under .spark-scenarios/react-calculator.\n\
             Use bun for JavaScript package management. Do not create files outside this ignored fixture folder.\n\
             This is a scoped fixture task: start with the listed brief and do not survey unrelated repository files unless a concrete blocker requires it.\n\
             The finished app will be checked by bun test and a Playwright browser smoke check, so it must be runnable through Vite in a real browser.\n\
             On Windows, run validation commands separately rather than chaining them with &&.\n\
             Required actions:\n\
             1. Read .spark-scenarios/react-calculator/brief.md.\n\
             2. Create .spark-scenarios/react-calculator/package.json.\n\
             3. Create .spark-scenarios/react-calculator/index.html.\n\
             4. Create .spark-scenarios/react-calculator/src/main.tsx.\n\
             5. Create .spark-scenarios/react-calculator/src/App.tsx.\n\
             6. Create .spark-scenarios/react-calculator/src/App.test.tsx.\n\
             7. Create .spark-scenarios/react-calculator/src/styles.css.\n\
             8. Run bun test if possible; if tests need a DOM, configure it before using DOM-based test helpers.\n\
             Finish with the app files created, validation result, and any agent behavior that made project scaffolding easier or harder."
                .to_string()
        }
        ProfileScenarioKind::RustLogAnalyzerScaffold => {
            "Benchmark scenario: rust-log-analyzer-scaffold.\n\
             Build a brand new Rust CLI project only under .spark-scenarios/rust-log-analyzer.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             This is a scoped fixture task: start with the listed brief/sample log and do not survey unrelated repository files unless a concrete blocker requires it.\n\
             On Windows, run validation commands separately rather than chaining them with &&.\n\
             Required actions:\n\
             1. Read .spark-scenarios/rust-log-analyzer/brief.md.\n\
             2. Read .spark-scenarios/rust-log-analyzer/sample.log.\n\
             3. Create .spark-scenarios/rust-log-analyzer/Cargo.toml.\n\
             4. Create .spark-scenarios/rust-log-analyzer/src/lib.rs.\n\
             5. Create .spark-scenarios/rust-log-analyzer/src/main.rs.\n\
             6. Run cargo test for the nested project.\n\
             Finish with the CLI behavior, test result, and any agent behavior that made project scaffolding easier or harder."
                .to_string()
        }
        ProfileScenarioKind::ToolRecovery => {
            "Benchmark scenario: tool-recovery.\n\
             Work only under .spark-scenarios/tool-recovery.\n\
             Required actions:\n\
             1. First attempt to read .spark-scenarios/tool-recovery/source/missing-note.md. This path is intentionally missing; do not skip this failing probe.\n\
             2. Recover by checking .spark-scenarios/tool-recovery/source/note.md.\n\
             3. Verify it contains: Spark recovery path verified.\n\
             Finish with what failed, how you recovered, and whether verification passed."
                .to_string()
        }
        other => profile_scenario_prompts(other, 45_000)
            .ok()
            .and_then(|prompts| prompts.into_iter().next())
            .unwrap_or_else(|| format!("Benchmark scenario: {}", other.name())),
    }
}

pub(crate) fn natural_compaction_scenario_prompts(target_tokens: usize) -> Result<Vec<String>> {
    let turn_count = 3usize;
    let target_chars = target_tokens.saturating_mul(APPROX_CHARS_PER_TOKEN);
    let target_chars_per_turn = target_chars.div_ceil(turn_count);
    let mut prompts = Vec::with_capacity(turn_count);

    for turn in 1..=turn_count {
        let mut prompt = format!(
            "Profile scenario: natural-compaction turn {turn}/{turn_count}.\n\
             This is a scripted multi-turn chat profiling run. Treat each message as normal conversation history and do not restate the filler rows.\n"
        );
        match turn {
            1 => prompt.push_str(
                "Answer with one sentence confirming you are tracking the harness context pressure.\n",
            ),
            2 => prompt.push_str(
                "Answer with one sentence naming one risk signal you would watch in the trace.\n",
            ),
            _ => prompt.push_str(
                "After the harness has a chance to compact naturally, use fs.list on src with recursive=false, then answer with whether compaction preserved the task intent and whether any required information was missing.\n",
            ),
        }

        let mut row = 0usize;
        while prompt.len() < target_chars_per_turn {
            row += 1;
            prompt.push_str(&format!(
                "turn {turn} row {row:05}: conversational long-context filler; retain the current turn objective, discard repetition, prefer native tools after compaction, and report uncertainty plainly.\n"
            ));
        }
        prompts.push(prompt);
    }

    Ok(prompts)
}

pub(crate) fn codex_cli_benchmark_prompt(scenario: ProfileScenarioKind) -> String {
    benchmark_task_prompt(scenario)
}

pub(crate) fn profile_scenario_validation_command(
    scenario: ProfileScenarioKind,
) -> Option<ProfileScenarioValidationCommand> {
    match scenario {
        ProfileScenarioKind::ReactCalculatorScaffold => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/react-calculator",
            program: "bun",
            args: &["test"],
        }),
        ProfileScenarioKind::RustLogAnalyzerScaffold => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/rust-log-analyzer",
            program: "cargo",
            args: &["test"],
        }),
        _ => None,
    }
}

pub(crate) fn profile_scenario_expected_tool_groups(
    scenario: ProfileScenarioKind,
) -> Vec<Vec<&'static str>> {
    match scenario {
        ProfileScenarioKind::RepoSurvey => {
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        }
        ProfileScenarioKind::NaturalCompaction | ProfileScenarioKind::CompactionPressure => {
            vec![vec!["fs.list"]]
        }
        ProfileScenarioKind::FileEdit => vec![
            vec!["fs.read"],
            vec!["fs.edit", "fs.replace"],
            vec!["fs.write"],
        ],
        ProfileScenarioKind::FileOps => {
            vec![
                vec!["fs.write"],
                vec!["fs.rename"],
                vec!["fs.stat"],
                vec!["fs.read"],
                vec!["fs.search"],
            ]
        }
        ProfileScenarioKind::ToolRecovery => vec![vec!["fs.read"]],
        ProfileScenarioKind::SkillUse => vec![vec!["fs.read"], vec!["fs.search"]],
        ProfileScenarioKind::SteamNetworkLibSurvey => {
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        }
        ProfileScenarioKind::S1ApiSurvey => vec![vec!["fs.list"], vec!["fs.read"]],
        ProfileScenarioKind::RepoArchitectureSurvey => {
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        }
        ProfileScenarioKind::BenchmarkDesignSurvey => vec![vec!["fs.read"], vec!["fs.search"]],
        ProfileScenarioKind::ReactCalculatorScaffold => {
            vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
        }
        ProfileScenarioKind::RustLogAnalyzerScaffold => {
            vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
        }
    }
}

pub(crate) fn profile_scenario_expected_tool_calls(scenario: ProfileScenarioKind) -> Vec<Value> {
    match scenario {
        ProfileScenarioKind::RepoSurvey => vec![],
        ProfileScenarioKind::NaturalCompaction | ProfileScenarioKind::CompactionPressure => {
            vec![json!({
                "tool": "fs.list",
                "path": "src",
                "recursive": false,
            })]
        }
        ProfileScenarioKind::FileEdit => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/file-edit/notes.md",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/file-edit/summary.txt",
            }),
        ],
        ProfileScenarioKind::FileOps => vec![
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/file-ops/drafts/report-draft.md",
            }),
            json!({
                "tool": "fs.rename",
                "from": ".spark-scenarios/file-ops/drafts/report-draft.md",
                "to": ".spark-scenarios/file-ops/final/report.md",
            }),
            json!({
                "tool": "fs.stat",
                "path": ".spark-scenarios/file-ops/final/report.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/file-ops/final/report.md",
            }),
            json!({
                "tool": "fs.search",
                "path": ".spark-scenarios/file-ops",
            }),
        ],
        ProfileScenarioKind::ToolRecovery => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/tool-recovery/source/missing-note.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/tool-recovery/source/note.md",
            }),
        ],
        ProfileScenarioKind::SkillUse => vec![
            json!({
                "tool": "fs.read",
                "path": "src/main.rs",
            }),
            json!({
                "tool": "fs.search",
                "path": "src",
            }),
        ],
        ProfileScenarioKind::SteamNetworkLibSurvey => vec![
            json!({
                "tool": "fs.list",
                "path": ".",
            }),
            json!({
                "tool": "fs.read",
                "path": "README.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "SteamNetworkClient.cs",
            }),
            json!({
                "tool": "fs.search",
            }),
        ],
        ProfileScenarioKind::S1ApiSurvey => vec![
            json!({
                "tool": "fs.list",
                "path": ".",
            }),
            json!({
                "tool": "fs.read",
                "path": "index.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "S1API.cs",
            }),
        ],
        ProfileScenarioKind::RepoArchitectureSurvey => vec![
            json!({
                "tool": "fs.list",
                "path": ".",
                "recursive": false,
            }),
            json!({
                "tool": "fs.read",
                "path": "AGENTS.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "README.md",
            }),
            json!({
                "tool": "fs.search",
                "path": "src",
            }),
        ],
        ProfileScenarioKind::BenchmarkDesignSurvey => vec![
            json!({
                "tool": "fs.read",
                "path": "src/profile_scenarios.rs",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/profiler/analyze/expectations.rs",
            }),
            json!({
                "tool": "fs.search",
                "path": "README.md",
            }),
            json!({
                "tool": "fs.search",
                "path": "src",
            }),
        ],
        ProfileScenarioKind::ReactCalculatorScaffold => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/react-calculator/brief.md",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/react-calculator/package.json",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/react-calculator/index.html",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/react-calculator/src/main.tsx",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/react-calculator/src/App.tsx",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/react-calculator/src/App.test.tsx",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/react-calculator/src/styles.css",
            }),
            json!({
                "tool": "cmd.exec",
            }),
        ],
        ProfileScenarioKind::RustLogAnalyzerScaffold => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/rust-log-analyzer/brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/rust-log-analyzer/sample.log",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/rust-log-analyzer/Cargo.toml",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/rust-log-analyzer/src/lib.rs",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/rust-log-analyzer/src/main.rs",
            }),
            json!({
                "tool": "cmd.exec",
            }),
        ],
    }
}

pub(crate) fn profile_scenario_expected_skills(scenario: ProfileScenarioKind) -> Vec<&'static str> {
    match scenario {
        ProfileScenarioKind::SkillUse => vec!["rust-patterns"],
        _ => vec![],
    }
}

pub(crate) fn validate_scenario_repeat(repeat: usize) -> Result<()> {
    if repeat == 0 {
        anyhow::bail!("--repeat must be greater than 0");
    }
    if repeat > MAX_SCENARIO_REPEAT {
        anyhow::bail!("--repeat must be <= {MAX_SCENARIO_REPEAT}");
    }
    Ok(())
}
