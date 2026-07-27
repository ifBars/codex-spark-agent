use serde_json::{Value, json};

use crate::cli::ProfileScenarioKind;

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
        ProfileScenarioKind::ShellRecovery => {
            vec![
                vec!["cmd.exec"],
                vec!["fs.list", "fs.search"],
                vec!["fs.read"],
            ]
        }
        ProfileScenarioKind::PrecisePatch => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace"],
                vec!["fs.search"],
            ]
        }
        ProfileScenarioKind::MultiFilePatch => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace", "fs.write"],
                vec!["fs.search"],
            ]
        }
        ProfileScenarioKind::SkillUse => vec![vec!["fs.read"], vec!["fs.search"]],
        ProfileScenarioKind::SteamNetworkLibSurvey => {
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        }
        ProfileScenarioKind::S1ApiSurvey => vec![vec!["fs.list"], vec!["fs.read"]],
        ProfileScenarioKind::RepoArchitectureSurvey => {
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        }
        ProfileScenarioKind::BenchmarkDesignSurvey => vec![vec!["fs.read"], vec!["fs.search"]],
        ProfileScenarioKind::AssetRipperExploration
        | ProfileScenarioKind::FiveMExploration
        | ProfileScenarioKind::Cpp2IlExploration
        | ProfileScenarioKind::Il2CppInteropExploration => {
            vec![vec!["fs.list"], vec!["fs.read"], vec!["fs.search"]]
        }
        ProfileScenarioKind::ReactCalculatorScaffold => {
            vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
        }
        ProfileScenarioKind::RustLogAnalyzerScaffold => {
            vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
        }
        ProfileScenarioKind::RustNotesTuiScaffold => {
            vec![vec!["fs.read"], vec!["fs.write"], vec!["cmd.exec"]]
        }
        ProfileScenarioKind::GithubIssueBugfix => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace"],
                vec!["cmd.exec"],
            ]
        }
        ProfileScenarioKind::RustFailingTestBugfix => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace"],
                vec!["cmd.exec"],
            ]
        }
        ProfileScenarioKind::TypeScriptReducerBugfix => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace"],
                vec!["cmd.exec"],
            ]
        }
        ProfileScenarioKind::MergeConflictResolution => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace"],
                vec!["cmd.exec"],
                vec!["fs.search", "fs.read"],
            ]
        }
        ProfileScenarioKind::GithubIssueTriage => vec![vec!["fs.read"], vec!["fs.write"]],
        ProfileScenarioKind::CiFailureTriage => vec![vec!["fs.read"], vec!["fs.write"]],
        ProfileScenarioKind::PullRequestReview => vec![vec!["fs.read"], vec!["fs.write"]],
        ProfileScenarioKind::DependencyUpgradeTriage => vec![vec!["fs.read"], vec!["fs.write"]],
        ProfileScenarioKind::TechnicalEssay => vec![vec!["fs.read"], vec!["fs.write"]],
        ProfileScenarioKind::ConfigMigration => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace", "fs.write"],
                vec!["cmd.exec", "fs.search"],
            ]
        }
        ProfileScenarioKind::OpsReport => vec![vec!["fs.read"], vec!["fs.write"]],
        ProfileScenarioKind::InventoryRebalancePlan => {
            vec![vec!["fs.read"], vec!["cmd.exec"], vec!["fs.write"]]
        }
        ProfileScenarioKind::ExperimentRolloutAudit => {
            vec![vec!["fs.read"], vec!["cmd.exec"], vec!["fs.write"]]
        }
        ProfileScenarioKind::MultiModuleBugfix => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace"],
                vec!["cmd.exec"],
            ]
        }
        ProfileScenarioKind::StatefulReconciliationBugfix => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace", "fs.write"],
                vec!["cmd.exec"],
            ]
        }
        ProfileScenarioKind::TerminalRepair => {
            vec![
                vec!["cmd.exec"],
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace", "fs.write"],
            ]
        }
        ProfileScenarioKind::MultiHopAnalysis => vec![vec!["fs.read"], vec!["fs.write"]],
        ProfileScenarioKind::PolicySupportAgent => {
            vec![vec!["fs.read"], vec!["fs.write", "fs.replace", "fs.edit"]]
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
                "ok": false,
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/tool-recovery/source/note.md",
            }),
        ],
        ProfileScenarioKind::ShellRecovery => vec![
            json!({
                "tool": "cmd.exec",
                "ok": false,
            }),
            json!({
                "tool": "cmd.exec",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/shell-recovery/summary.txt",
            }),
        ],
        ProfileScenarioKind::PrecisePatch => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/precise-patch/tests/status_map.spec.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/precise-patch/src/status_map.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace"],
                "path": ".spark-scenarios/precise-patch/src/status_map.ts",
            }),
            json!({
                "tool": "fs.search",
                "path": ".spark-scenarios/precise-patch/src",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/precise-patch/src/status_map.ts",
            }),
        ],
        ProfileScenarioKind::MultiFilePatch => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-file-patch/src/routes.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-file-patch/src/navigation.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-file-patch/docs/routes.md",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/multi-file-patch/src/routes.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/multi-file-patch/src/navigation.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/multi-file-patch/docs/routes.md",
            }),
            json!({
                "tool": "fs.search",
                "path": ".spark-scenarios/multi-file-patch",
            }),
            json!({
                "tool": "fs.search",
                "path": ".spark-scenarios/multi-file-patch",
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
                "path": "src/profile/scenarios/prompts.rs",
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
        ProfileScenarioKind::AssetRipperExploration => vec![
            json!({
                "tool": "fs.read",
                "path": "ExportedProject/ProjectSettings/ProjectSettings.asset",
            }),
            json!({
                "tool": "fs.list",
                "path": "ExportedProject/Assets",
                "recursive": false,
            }),
            json!({
                "tool": "fs.read",
                "path": "ExportedProject/Assets/Scripts/Assembly-CSharp/ScheduleOne/Product/ProductManager.cs",
            }),
            json!({
                "tool": "fs.search",
                "path": "ExportedProject/Assets",
            }),
        ],
        ProfileScenarioKind::FiveMExploration => vec![
            json!({
                "tool": "fs.read",
                "path": "README.md",
            }),
            json!({
                "tool": "fs.list",
                "path": "code/components",
                "recursive": false,
            }),
            json!({
                "tool": "fs.search",
                "path": "code",
            }),
            json!({
                "tool": "fs.search",
                "path": "code/components",
            }),
        ],
        ProfileScenarioKind::Cpp2IlExploration => vec![
            json!({
                "tool": "fs.read",
                "path": "README.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "Cpp2IL/Program.cs",
            }),
            json!({
                "tool": "fs.read",
                "path": "LibCpp2IL/LibCpp2IlMain.cs",
            }),
            json!({
                "tool": "fs.search",
                "path": "Cpp2IL.Core",
            }),
        ],
        ProfileScenarioKind::Il2CppInteropExploration => vec![
            json!({
                "tool": "fs.read",
                "path": "README.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "Il2CppInterop.CLI/Program.cs",
            }),
            json!({
                "tool": "fs.search",
                "path": "Il2CppInterop.Generator",
            }),
            json!({
                "tool": "fs.search",
                "path": "Il2CppInterop.Runtime",
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
                "command": "bun test",
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
                "command": "cargo test",
            }),
        ],
        ProfileScenarioKind::RustNotesTuiScaffold => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/rust-notes-tui/brief.md",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/rust-notes-tui/Cargo.toml",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/rust-notes-tui/src/lib.rs",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/rust-notes-tui/src/main.rs",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "cargo test",
            }),
        ],
        ProfileScenarioKind::GithubIssueBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-bugfix/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-bugfix/src/quote.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-bugfix/tests/quote.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/github-issue-bugfix/src/quote.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::RustFailingTestBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/rust-failing-test-bugfix/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/rust-failing-test-bugfix/tests/retry_scheduler.rs",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "cargo test",
            }),
        ],
        ProfileScenarioKind::TypeScriptReducerBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/typescript-reducer-bugfix/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/typescript-reducer-bugfix/src/cart.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/typescript-reducer-bugfix/tests/cart.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/typescript-reducer-bugfix/src/cart.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::MergeConflictResolution => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/merge-conflict-resolution/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/merge-conflict-resolution/tests/featureFlags.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
            json!({
                "tools": ["fs.search", "fs.read"],
                "path": ".spark-scenarios/merge-conflict-resolution/src/featureFlags.ts",
            }),
        ],
        ProfileScenarioKind::GithubIssueTriage => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-triage/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-triage/src/cachePolicy.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/github-issue-triage/logs/warehouse-import.log",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/github-issue-triage/triage.md",
            }),
        ],
        ProfileScenarioKind::CiFailureTriage => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ci-failure-triage/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ci-failure-triage/.github/workflows/frontend.yml",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ci-failure-triage/logs/frontend-tests.log",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ci-failure-triage/src/discount.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ci-failure-triage/tests/discount.test.ts",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/ci-failure-triage/ci-triage.md",
            }),
        ],
        ProfileScenarioKind::PullRequestReview => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/pull-request-review/pr.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/pull-request-review/diff.patch",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/pull-request-review/src/checkout.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/pull-request-review/tests/checkout.test.ts",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/pull-request-review/review.md",
            }),
        ],
        ProfileScenarioKind::DependencyUpgradeTriage => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/dependency-upgrade-triage/upgrade.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/dependency-upgrade-triage/package.json",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/dependency-upgrade-triage/bun.lock",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/dependency-upgrade-triage/docs/time-utils-2.0.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/dependency-upgrade-triage/src/billingWindow.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/dependency-upgrade-triage/tests/billingWindow.test.ts",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/dependency-upgrade-triage/upgrade-triage.md",
            }),
        ],
        ProfileScenarioKind::TechnicalEssay => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/technical-essay/brief.md",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/technical-essay/essay.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/technical-essay/essay.md",
            }),
        ],
        ProfileScenarioKind::ConfigMigration => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/config-migration/migration.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/config-migration/config/app.json",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/config-migration/src/config.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/config-migration/docs/config.md",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/config-migration/config/app.json",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/config-migration/src/config.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/config-migration/docs/config.md",
            }),
            json!({
                "tools": ["cmd.exec", "fs.search"],
            }),
        ],
        ProfileScenarioKind::OpsReport => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ops-report/brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ops-report/data/tickets.csv",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/ops-report/metrics.json",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/ops-report/report.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ops-report/metrics.json",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/ops-report/report.md",
            }),
        ],
        ProfileScenarioKind::InventoryRebalancePlan => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/inventory-rebalance-plan/brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/inventory-rebalance-plan/policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/inventory-rebalance-plan/data/products.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/inventory-rebalance-plan/data/warehouses.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/inventory-rebalance-plan/data/transfer_options.csv",
            }),
            json!({
                "tool": "cmd.exec",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/inventory-rebalance-plan/plan.json",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/inventory-rebalance-plan/memo.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/inventory-rebalance-plan/plan.json",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/inventory-rebalance-plan/memo.md",
            }),
        ],
        ProfileScenarioKind::ExperimentRolloutAudit => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/experiment-rollout-audit/brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/experiment-rollout-audit/policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/experiment-rollout-audit/data/assignments.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/experiment-rollout-audit/data/exclusions.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/experiment-rollout-audit/data/events.csv",
            }),
            json!({
                "tool": "cmd.exec",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/experiment-rollout-audit/audit.json",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/experiment-rollout-audit/memo.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/experiment-rollout-audit/audit.json",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/experiment-rollout-audit/memo.md",
            }),
        ],
        ProfileScenarioKind::MultiModuleBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-module-bugfix/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-module-bugfix/src/invoice.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-module-bugfix/src/total.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-module-bugfix/tests/invoice.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/multi-module-bugfix/src/invoice.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/multi-module-bugfix/src/total.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::StatefulReconciliationBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/docs/invariants.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/logs/incident.log",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/src/normalize.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/src/project.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/src/normalize.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/src/project.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::TerminalRepair => vec![
            json!({
                "tool": "cmd.exec",
                "command": "bun run start",
                "ok": false,
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/terminal-repair/config/settings.json",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/terminal-repair/config/settings.json",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun run start",
            }),
        ],
        ProfileScenarioKind::MultiHopAnalysis => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-hop-analysis/question.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-hop-analysis/policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-hop-analysis/data/orders.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-hop-analysis/data/refunds.csv",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/multi-hop-analysis/answer.json",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/multi-hop-analysis/answer.md",
            }),
        ],
        ProfileScenarioKind::PolicySupportAgent => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/policy-support-agent/brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/policy-support-agent/policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/policy-support-agent/cases/order_5591.json",
            }),
            json!({
                "tool": "fs.write",
                "path": ".spark-scenarios/policy-support-agent/resolution.json",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": ".spark-scenarios/policy-support-agent/resolution.json",
            }),
        ],
    }
}

pub(crate) fn profile_scenario_optional_tool_calls(scenario: ProfileScenarioKind) -> Vec<Value> {
    match scenario {
        ProfileScenarioKind::GithubIssueBugfix => vec![json!({
            "tool": "fs.read",
            "path": ".spark-scenarios/github-issue-bugfix/src/quote.ts",
        })],
        ProfileScenarioKind::RustFailingTestBugfix => vec![json!({
            "tool": "fs.read",
            "path": ".spark-scenarios/rust-failing-test-bugfix/src/lib.rs",
        })],
        ProfileScenarioKind::TypeScriptReducerBugfix => vec![json!({
            "tool": "fs.read",
            "path": ".spark-scenarios/typescript-reducer-bugfix/src/cart.ts",
        })],
        ProfileScenarioKind::ConfigMigration => vec![
            json!({
                "tool": "fs.search",
                "path": ".spark-scenarios/config-migration",
            }),
            json!({
                "tool": "fs.search",
                "path": ".spark-scenarios/config-migration",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/config-migration/config/app.json",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/config-migration/src/config.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/config-migration/docs/config.md",
            }),
        ],
        ProfileScenarioKind::MultiFilePatch => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-file-patch/src/routes.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-file-patch/src/navigation.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-file-patch/docs/routes.md",
            }),
        ],
        ProfileScenarioKind::MultiModuleBugfix => vec![json!({
            "tool": "fs.read",
            "path": ".spark-scenarios/multi-module-bugfix/src/tax.ts",
        })],
        ProfileScenarioKind::StatefulReconciliationBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/src/types.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/stateful-reconciliation-bugfix/tests/projection.test.ts",
            }),
        ],
        ProfileScenarioKind::TerminalRepair => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/terminal-repair/src/index.js",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/terminal-repair/data/report.csv",
            }),
        ],
        ProfileScenarioKind::MultiHopAnalysis => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-hop-analysis/answer.json",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/multi-hop-analysis/answer.md",
            }),
        ],
        ProfileScenarioKind::PolicySupportAgent => vec![
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/policy-support-agent/policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".spark-scenarios/policy-support-agent/resolution.json",
            }),
        ],
        _ => Vec::new(),
    }
}

pub(crate) fn profile_scenario_expected_skills(scenario: ProfileScenarioKind) -> Vec<&'static str> {
    match scenario {
        ProfileScenarioKind::SkillUse => vec!["rust-patterns"],
        _ => vec![],
    }
}
