pub(crate) mod codex_cli;
pub(crate) mod infrastructure;
pub(crate) mod judge;
pub(crate) mod opencode;
pub(crate) mod results;
pub(crate) mod workspace;

use crate::cli::ProfileScenarioKind;

pub(crate) fn expected_scenario_artifacts(
    scenario: ProfileScenarioKind,
) -> &'static [&'static str] {
    match scenario {
        ProfileScenarioKind::FileEdit => &[".spark-scenarios/file-edit/summary.txt"],
        ProfileScenarioKind::FileOps => &[".spark-scenarios/file-ops/final/report.md"],
        ProfileScenarioKind::ShellRecovery => &[".spark-scenarios/shell-recovery/summary.txt"],
        ProfileScenarioKind::PrecisePatch => &[".spark-scenarios/precise-patch/src/status_map.ts"],
        ProfileScenarioKind::MultiFilePatch => &[
            ".spark-scenarios/multi-file-patch/src/routes.ts",
            ".spark-scenarios/multi-file-patch/src/navigation.ts",
            ".spark-scenarios/multi-file-patch/docs/routes.md",
        ],
        ProfileScenarioKind::ReactCalculatorScaffold => &[
            ".spark-scenarios/react-calculator/package.json",
            ".spark-scenarios/react-calculator/index.html",
            ".spark-scenarios/react-calculator/src/main.tsx",
            ".spark-scenarios/react-calculator/src/App.tsx",
            ".spark-scenarios/react-calculator/src/App.test.tsx",
            ".spark-scenarios/react-calculator/src/styles.css",
        ],
        ProfileScenarioKind::RustLogAnalyzerScaffold => &[
            ".spark-scenarios/rust-log-analyzer/Cargo.toml",
            ".spark-scenarios/rust-log-analyzer/src/lib.rs",
            ".spark-scenarios/rust-log-analyzer/src/main.rs",
        ],
        ProfileScenarioKind::RustNotesTuiScaffold => &[
            ".spark-scenarios/rust-notes-tui/Cargo.toml",
            ".spark-scenarios/rust-notes-tui/src/lib.rs",
            ".spark-scenarios/rust-notes-tui/src/main.rs",
        ],
        ProfileScenarioKind::GithubIssueBugfix => {
            &[".spark-scenarios/github-issue-bugfix/src/quote.ts"]
        }
        ProfileScenarioKind::RustFailingTestBugfix => {
            &[".spark-scenarios/rust-failing-test-bugfix/src/lib.rs"]
        }
        ProfileScenarioKind::TypeScriptReducerBugfix => {
            &[".spark-scenarios/typescript-reducer-bugfix/src/cart.ts"]
        }
        ProfileScenarioKind::MergeConflictResolution => {
            &[".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts"]
        }
        ProfileScenarioKind::GithubIssueTriage => {
            &[".spark-scenarios/github-issue-triage/triage.md"]
        }
        ProfileScenarioKind::CiFailureTriage => {
            &[".spark-scenarios/ci-failure-triage/ci-triage.md"]
        }
        ProfileScenarioKind::PullRequestReview => {
            &[".spark-scenarios/pull-request-review/review.md"]
        }
        ProfileScenarioKind::DependencyUpgradeTriage => {
            &[".spark-scenarios/dependency-upgrade-triage/upgrade-triage.md"]
        }
        ProfileScenarioKind::TechnicalEssay => &[".spark-scenarios/technical-essay/essay.md"],
        ProfileScenarioKind::ConfigMigration => &[
            ".spark-scenarios/config-migration/config/app.json",
            ".spark-scenarios/config-migration/src/config.ts",
            ".spark-scenarios/config-migration/docs/config.md",
        ],
        ProfileScenarioKind::OpsReport => &[
            ".spark-scenarios/ops-report/metrics.json",
            ".spark-scenarios/ops-report/report.md",
        ],
        ProfileScenarioKind::MultiModuleBugfix => &[
            ".spark-scenarios/multi-module-bugfix/src/invoice.ts",
            ".spark-scenarios/multi-module-bugfix/src/total.ts",
        ],
        ProfileScenarioKind::StatefulReconciliationBugfix => &[
            ".spark-scenarios/stateful-reconciliation-bugfix/src/normalize.ts",
            ".spark-scenarios/stateful-reconciliation-bugfix/src/project.ts",
        ],
        ProfileScenarioKind::TerminalRepair => {
            &[".spark-scenarios/terminal-repair/config/settings.json"]
        }
        ProfileScenarioKind::MultiHopAnalysis => &[
            ".spark-scenarios/multi-hop-analysis/answer.json",
            ".spark-scenarios/multi-hop-analysis/answer.md",
        ],
        ProfileScenarioKind::PolicySupportAgent => {
            &[".spark-scenarios/policy-support-agent/resolution.json"]
        }
        _ => &[],
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::ProfileBenchmarkSuiteKind;

    #[test]
    fn expected_artifacts_cover_real_world_output_scenarios() {
        for scenario in [
            ProfileScenarioKind::PrecisePatch,
            ProfileScenarioKind::MultiFilePatch,
            ProfileScenarioKind::GithubIssueBugfix,
            ProfileScenarioKind::RustFailingTestBugfix,
            ProfileScenarioKind::TypeScriptReducerBugfix,
            ProfileScenarioKind::MergeConflictResolution,
            ProfileScenarioKind::GithubIssueTriage,
            ProfileScenarioKind::CiFailureTriage,
            ProfileScenarioKind::PullRequestReview,
            ProfileScenarioKind::DependencyUpgradeTriage,
            ProfileScenarioKind::TechnicalEssay,
            ProfileScenarioKind::ConfigMigration,
            ProfileScenarioKind::OpsReport,
            ProfileScenarioKind::MultiModuleBugfix,
            ProfileScenarioKind::StatefulReconciliationBugfix,
            ProfileScenarioKind::TerminalRepair,
            ProfileScenarioKind::MultiHopAnalysis,
            ProfileScenarioKind::PolicySupportAgent,
            ProfileScenarioKind::ReactCalculatorScaffold,
            ProfileScenarioKind::RustLogAnalyzerScaffold,
            ProfileScenarioKind::RustNotesTuiScaffold,
        ] {
            assert!(
                ProfileBenchmarkSuiteKind::RealWorld
                    .scenarios()
                    .contains(&scenario),
                "{scenario:?} should stay in real-world if artifact-scored here"
            );
            assert!(
                !expected_scenario_artifacts(scenario).is_empty(),
                "{scenario:?} should have expected artifacts"
            );
        }
        assert!(expected_scenario_artifacts(ProfileScenarioKind::RepoSurvey).is_empty());
    }
}
