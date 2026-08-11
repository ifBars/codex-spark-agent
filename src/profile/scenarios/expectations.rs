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
        ProfileScenarioKind::ManifestContractWrite => {
            vec![vec!["fs.read"], vec!["fs.write"], vec!["fs.read"]]
        }
        ProfileScenarioKind::ScopedPolicyPatch => {
            vec![
                vec!["fs.read"],
                vec!["fs.edit", "fs.replace"],
                vec!["fs.search"],
            ]
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
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix
        | ProfileScenarioKind::FrontierRuleTransfer => {
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
                "path": "notes.md",
            }),
            json!({
                "tool": "fs.write",
                "path": "summary.txt",
            }),
        ],
        ProfileScenarioKind::FileOps => vec![
            json!({
                "tool": "fs.write",
                "path": "drafts/report-draft.md",
            }),
            json!({
                "tool": "fs.rename",
                "from": "drafts/report-draft.md",
                "to": "final/report.md",
            }),
            json!({
                "tool": "fs.stat",
                "path": "final/report.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "final/report.md",
            }),
            json!({
                "tool": "fs.search",
                "path": ".",
            }),
        ],
        ProfileScenarioKind::ToolRecovery => vec![
            json!({
                "tool": "fs.read",
                "path": "source/missing-note.md",
                "ok": false,
            }),
            json!({
                "tool": "fs.read",
                "path": "source/note.md",
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
                "path": "summary.txt",
            }),
        ],
        ProfileScenarioKind::PrecisePatch => vec![
            json!({
                "tool": "fs.read",
                "path": "tests/status_map.spec.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/status_map.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace"],
                "path": "src/status_map.ts",
            }),
            json!({
                "tool": "fs.search",
                "path": "src",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/status_map.ts",
            }),
        ],
        ProfileScenarioKind::MultiFilePatch => vec![
            json!({
                "tool": "fs.read",
                "path": "src/routes.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/navigation.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "docs/routes.md",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/routes.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/navigation.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "docs/routes.md",
            }),
            json!({
                "tool": "fs.search",
                "path": ".",
            }),
            json!({
                "tool": "fs.search",
                "path": ".",
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
        ProfileScenarioKind::ManifestContractWrite => vec![
            json!({ "tool": "fs.read", "path": "brief.md" }),
            json!({ "tool": "fs.read", "path": "data/releases.json" }),
            json!({ "tool": "fs.write", "path": "generated/release-manifest.json" }),
            json!({ "tool": "fs.write", "path": "generated/release-notes.md" }),
            json!({ "tool": "fs.read", "path": "generated/release-manifest.json" }),
            json!({ "tool": "fs.read", "path": "generated/release-notes.md" }),
        ],
        ProfileScenarioKind::ScopedPolicyPatch => vec![
            json!({ "tool": "fs.read", "path": "tests/rate_limit.spec.md" }),
            json!({ "tool": "fs.read", "path": "src/rate_limit.ts" }),
            json!({ "tools": ["fs.edit", "fs.replace"], "path": "src/rate_limit.ts" }),
            json!({ "tool": "fs.search", "path": "src" }),
            json!({ "tool": "fs.read", "path": "src/rate_limit.ts" }),
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
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.write",
                "path": "package.json",
            }),
            json!({
                "tool": "fs.write",
                "path": "index.html",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/main.tsx",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/App.tsx",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/App.test.tsx",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/styles.css",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::RustLogAnalyzerScaffold => vec![
            json!({
                "tool": "fs.read",
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "sample.log",
            }),
            json!({
                "tool": "fs.write",
                "path": "Cargo.toml",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/lib.rs",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/main.rs",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "cargo test",
            }),
        ],
        ProfileScenarioKind::RustNotesTuiScaffold => vec![
            json!({
                "tool": "fs.read",
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.write",
                "path": "Cargo.toml",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/lib.rs",
            }),
            json!({
                "tool": "fs.write",
                "path": "src/main.rs",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "cargo test",
            }),
        ],
        ProfileScenarioKind::GithubIssueBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/quote.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/quote.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/quote.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::RustFailingTestBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/lib.rs",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/retry_scheduler.rs",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/lib.rs",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "cargo test",
            }),
        ],
        ProfileScenarioKind::TypeScriptReducerBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/cart.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/cart.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/cart.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::MergeConflictResolution => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/featureFlags.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/featureFlags.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/featureFlags.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
            json!({
                "tools": ["fs.search", "fs.read"],
                "path": "src/featureFlags.ts",
            }),
        ],
        ProfileScenarioKind::GithubIssueTriage => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/cachePolicy.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "logs/warehouse-import.log",
            }),
            json!({
                "tool": "fs.write",
                "path": "triage.md",
            }),
        ],
        ProfileScenarioKind::CiFailureTriage => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": ".github/workflows/frontend.yml",
            }),
            json!({
                "tool": "fs.read",
                "path": "logs/frontend-tests.log",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/discount.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/discount.test.ts",
            }),
            json!({
                "tool": "fs.write",
                "path": "ci-triage.md",
            }),
        ],
        ProfileScenarioKind::PullRequestReview => vec![
            json!({
                "tool": "fs.read",
                "path": "pr.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "diff.patch",
            }),
            json!({
                "tool": "fs.read",
                "path": "diff-extra.patch",
            }),
            json!({
                "tool": "fs.read",
                "path": "diff-concurrency.patch",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/audit.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/batchOrders.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/checkout.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/invites.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/orders.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/payments.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/reportCache.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/reportQuery.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/rollout.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/runtimeSettings.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/sessionExpiry.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/useSocketMessages.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/webhookDelivery.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/audit.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/batchOrders.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/checkout.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/invites.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/orders.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/payments.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/reportCache.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/reportQuery.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/rollout.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/runtimeSettings.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/sessionExpiry.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/useSocketMessages.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/webhookDelivery.test.ts",
            }),
            json!({
                "tool": "fs.write",
                "path": "review.json",
            }),
            json!({
                "tool": "fs.write",
                "path": "review.md",
            }),
        ],
        ProfileScenarioKind::DependencyUpgradeTriage => vec![
            json!({
                "tool": "fs.read",
                "path": "upgrade.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "package.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "bun.lock",
            }),
            json!({
                "tool": "fs.read",
                "path": "docs/time-utils-2.0.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/billingWindow.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/billingWeek.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/billingWindow.test.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/billingWeek.test.ts",
            }),
            json!({
                "tool": "fs.write",
                "path": "upgrade-triage.md",
            }),
        ],
        ProfileScenarioKind::TechnicalEssay => vec![
            json!({
                "tool": "fs.read",
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.write",
                "path": "essay.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "essay.md",
            }),
        ],
        ProfileScenarioKind::ConfigMigration => vec![
            json!({
                "tool": "fs.read",
                "path": "migration.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "config/app.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/config.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "docs/config.md",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "config/app.json",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/config.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "docs/config.md",
            }),
            json!({
                "tools": ["cmd.exec", "fs.search"],
            }),
        ],
        ProfileScenarioKind::OpsReport => vec![
            json!({
                "tool": "fs.read",
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/tickets.csv",
            }),
            json!({
                "tool": "fs.write",
                "path": "metrics.json",
            }),
            json!({
                "tool": "fs.write",
                "path": "report.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "metrics.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "report.md",
            }),
        ],
        ProfileScenarioKind::InventoryRebalancePlan => vec![
            json!({
                "tool": "fs.read",
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/products.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/warehouses.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/transfer_options.csv",
            }),
            json!({
                "tool": "cmd.exec",
            }),
            json!({
                "tool": "fs.write",
                "path": "plan.json",
            }),
            json!({
                "tool": "fs.write",
                "path": "memo.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "plan.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "memo.md",
            }),
        ],
        ProfileScenarioKind::ExperimentRolloutAudit => vec![
            json!({
                "tool": "fs.read",
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/assignments.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/exclusions.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/events.csv",
            }),
            json!({
                "tool": "cmd.exec",
            }),
            json!({
                "tool": "fs.write",
                "path": "audit.json",
            }),
            json!({
                "tool": "fs.write",
                "path": "memo.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "audit.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "memo.md",
            }),
        ],
        ProfileScenarioKind::MultiModuleBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/invoice.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/total.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/invoice.test.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/invoice.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/total.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::StatefulReconciliationBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "docs/invariants.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "logs/incident.log",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/normalize.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/project.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/normalize.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/project.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test",
            }),
        ],
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": "issue.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "docs/invariants.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "logs/incident.log",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/store.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/cache.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/evaluate.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test tests/rollout.test.ts",
            }),
        ],
        ProfileScenarioKind::FrontierRuleTransfer => vec![
            json!({
                "tool": "fs.read",
                "path": "task.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "examples.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/types.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/solver.ts",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "src/solver.ts",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun test tests/public.test.ts",
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
                "path": "config/settings.json",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "config/settings.json",
            }),
            json!({
                "tool": "cmd.exec",
                "command": "bun run start",
            }),
        ],
        ProfileScenarioKind::MultiHopAnalysis => vec![
            json!({
                "tool": "fs.read",
                "path": "question.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/orders.csv",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/refunds.csv",
            }),
            json!({
                "tool": "fs.write",
                "path": "answer.json",
            }),
            json!({
                "tool": "fs.write",
                "path": "answer.md",
            }),
        ],
        ProfileScenarioKind::PolicySupportAgent => vec![
            json!({
                "tool": "fs.read",
                "path": "brief.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "cases/order_5591.json",
            }),
            json!({
                "tool": "fs.write",
                "path": "resolution.json",
            }),
            json!({
                "tools": ["fs.edit", "fs.replace", "fs.write"],
                "path": "resolution.json",
            }),
        ],
    }
}

pub(crate) fn profile_scenario_optional_tool_calls(scenario: ProfileScenarioKind) -> Vec<Value> {
    match scenario {
        ProfileScenarioKind::GithubIssueBugfix => vec![json!({
            "tool": "fs.read",
            "path": "src/quote.ts",
        })],
        ProfileScenarioKind::RustFailingTestBugfix => vec![json!({
            "tool": "fs.read",
            "path": "src/lib.rs",
        })],
        ProfileScenarioKind::TypeScriptReducerBugfix => vec![json!({
            "tool": "fs.read",
            "path": "src/cart.ts",
        })],
        ProfileScenarioKind::ConfigMigration => vec![
            json!({
                "tool": "fs.search",
                "path": ".",
            }),
            json!({
                "tool": "fs.search",
                "path": ".",
            }),
            json!({
                "tool": "fs.read",
                "path": "config/app.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/config.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "docs/config.md",
            }),
        ],
        ProfileScenarioKind::MultiFilePatch => vec![
            json!({
                "tool": "fs.read",
                "path": "src/routes.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "src/navigation.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "docs/routes.md",
            }),
        ],
        ProfileScenarioKind::MultiModuleBugfix => vec![json!({
            "tool": "fs.read",
            "path": "src/tax.ts",
        })],
        ProfileScenarioKind::StatefulReconciliationBugfix => vec![
            json!({
                "tool": "fs.read",
                "path": "src/types.ts",
            }),
            json!({
                "tool": "fs.read",
                "path": "tests/projection.test.ts",
            }),
        ],
        ProfileScenarioKind::TerminalRepair => vec![
            json!({
                "tool": "fs.read",
                "path": "src/index.js",
            }),
            json!({
                "tool": "fs.read",
                "path": "data/report.csv",
            }),
        ],
        ProfileScenarioKind::MultiHopAnalysis => vec![
            json!({
                "tool": "fs.read",
                "path": "answer.json",
            }),
            json!({
                "tool": "fs.read",
                "path": "answer.md",
            }),
        ],
        ProfileScenarioKind::PolicySupportAgent => vec![
            json!({
                "tool": "fs.read",
                "path": "policy.md",
            }),
            json!({
                "tool": "fs.read",
                "path": "resolution.json",
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
