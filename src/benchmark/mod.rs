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
        ProfileScenarioKind::FileEdit => &["summary.txt"],
        ProfileScenarioKind::FileOps => &["final/report.md"],
        ProfileScenarioKind::ShellRecovery => &["summary.txt"],
        ProfileScenarioKind::PrecisePatch => &["src/status_map.ts"],
        ProfileScenarioKind::MultiFilePatch => {
            &["src/routes.ts", "src/navigation.ts", "docs/routes.md"]
        }
        ProfileScenarioKind::ManifestContractWrite => &[
            "generated/release-manifest.json",
            "generated/release-notes.md",
        ],
        ProfileScenarioKind::ScopedPolicyPatch => &["src/rate_limit.ts"],
        ProfileScenarioKind::ReactCalculatorScaffold => &[
            "package.json",
            "index.html",
            "src/main.tsx",
            "src/App.tsx",
            "src/App.test.tsx",
            "src/styles.css",
        ],
        ProfileScenarioKind::RustLogAnalyzerScaffold => {
            &["Cargo.toml", "src/lib.rs", "src/main.rs"]
        }
        ProfileScenarioKind::RustNotesTuiScaffold => &["Cargo.toml", "src/lib.rs", "src/main.rs"],
        ProfileScenarioKind::GithubIssueBugfix => &["src/quote.ts"],
        ProfileScenarioKind::RustFailingTestBugfix => &["src/lib.rs"],
        ProfileScenarioKind::TypeScriptReducerBugfix => &["src/cart.ts"],
        ProfileScenarioKind::MergeConflictResolution => &["src/featureFlags.ts"],
        ProfileScenarioKind::GithubIssueTriage => &["triage.md"],
        ProfileScenarioKind::CiFailureTriage => &["ci-triage.md"],
        ProfileScenarioKind::PullRequestReview => &["review.md"],
        ProfileScenarioKind::DependencyUpgradeTriage => &["upgrade-triage.md"],
        ProfileScenarioKind::TechnicalEssay => &["essay.md"],
        ProfileScenarioKind::ConfigMigration => {
            &["config/app.json", "src/config.ts", "docs/config.md"]
        }
        ProfileScenarioKind::OpsReport => &["metrics.json", "report.md"],
        ProfileScenarioKind::InventoryRebalancePlan => &["plan.json", "memo.md"],
        ProfileScenarioKind::ExperimentRolloutAudit => &["audit.json", "memo.md"],
        ProfileScenarioKind::MultiModuleBugfix => &["src/invoice.ts", "src/total.ts"],
        ProfileScenarioKind::StatefulReconciliationBugfix => {
            &["src/normalize.ts", "src/project.ts"]
        }
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => {
            &["src/store.ts", "src/cache.ts", "src/evaluate.ts"]
        }
        ProfileScenarioKind::FrontierRuleTransfer => &["src/solver.ts"],
        ProfileScenarioKind::TerminalRepair => &["config/settings.json"],
        ProfileScenarioKind::MultiHopAnalysis => &["answer.json", "answer.md"],
        ProfileScenarioKind::PolicySupportAgent => &["resolution.json"],
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
            ProfileScenarioKind::ManifestContractWrite,
            ProfileScenarioKind::ScopedPolicyPatch,
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
            ProfileScenarioKind::InventoryRebalancePlan,
            ProfileScenarioKind::ExperimentRolloutAudit,
            ProfileScenarioKind::MultiModuleBugfix,
            ProfileScenarioKind::StatefulReconciliationBugfix,
            ProfileScenarioKind::FeatureRolloutConsistencyBugfix,
            ProfileScenarioKind::FrontierRuleTransfer,
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
