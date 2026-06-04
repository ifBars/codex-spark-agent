use std::path::Path;

use anyhow::Result;
use serde_json::{Value, json};

use crate::{
    APPROX_CHARS_PER_TOKEN, MAX_SCENARIO_REPEAT, MAX_SCENARIO_TARGET_TOKENS,
    cli::ProfileScenarioKind,
};

pub(crate) fn prepare_profile_scenario(cwd: &Path, scenario: ProfileScenarioKind) -> Result<()> {
    let Some(name) = (match scenario {
        ProfileScenarioKind::FileEdit => Some("file-edit"),
        ProfileScenarioKind::FileOps => Some("file-ops"),
        ProfileScenarioKind::ToolRecovery => Some("tool-recovery"),
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
        ProfileScenarioKind::ToolRecovery => {
            vec![vec!["fs.read"], vec!["fs.stat"], vec!["fs.write"]]
        }
        ProfileScenarioKind::SkillUse => vec![vec!["fs.read"], vec!["fs.search"]],
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
                "tool": "fs.stat",
                "path": ".spark-scenarios/tool-recovery/source/note.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/tool-recovery/source/note.md",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/tool-recovery/recovery-summary.txt",
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
