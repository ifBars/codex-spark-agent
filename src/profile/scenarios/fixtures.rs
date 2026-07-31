use std::path::Path;

use anyhow::Result;

use crate::cli::ProfileScenarioKind;

pub(crate) fn prepare_profile_scenario(cwd: &Path, scenario: ProfileScenarioKind) -> Result<()> {
    let Some(name) = profile_scenario_fixture_name(scenario) else {
        return Ok(());
    };
    prepare_profile_scenario_at(&cwd.join(".spark-scenarios").join(name), scenario)
}

pub(crate) fn profile_scenario_cwd(
    cwd: &Path,
    scenario: ProfileScenarioKind,
) -> std::path::PathBuf {
    profile_scenario_fixture_name(scenario)
        .map(|name| cwd.join(".spark-scenarios").join(name))
        .unwrap_or_else(|| cwd.to_path_buf())
}

pub(crate) fn prepare_benchmark_scenario(cwd: &Path, scenario: ProfileScenarioKind) -> Result<()> {
    if profile_scenario_fixture_name(scenario).is_none() {
        return Ok(());
    }
    prepare_profile_scenario_at(cwd, scenario)
}

fn profile_scenario_fixture_name(scenario: ProfileScenarioKind) -> Option<&'static str> {
    match scenario {
        ProfileScenarioKind::FileEdit => Some("file-edit"),
        ProfileScenarioKind::FileOps => Some("file-ops"),
        ProfileScenarioKind::ToolRecovery => Some("tool-recovery"),
        ProfileScenarioKind::ShellRecovery => Some("shell-recovery"),
        ProfileScenarioKind::PrecisePatch => Some("precise-patch"),
        ProfileScenarioKind::MultiFilePatch => Some("multi-file-patch"),
        ProfileScenarioKind::ManifestContractWrite => Some("manifest-contract-write"),
        ProfileScenarioKind::ScopedPolicyPatch => Some("scoped-policy-patch"),
        ProfileScenarioKind::ReactCalculatorScaffold => Some("react-calculator"),
        ProfileScenarioKind::RustLogAnalyzerScaffold => Some("rust-log-analyzer"),
        ProfileScenarioKind::RustNotesTuiScaffold => Some("rust-notes-tui"),
        ProfileScenarioKind::GithubIssueBugfix => Some("github-issue-bugfix"),
        ProfileScenarioKind::RustFailingTestBugfix => Some("rust-failing-test-bugfix"),
        ProfileScenarioKind::TypeScriptReducerBugfix => Some("typescript-reducer-bugfix"),
        ProfileScenarioKind::MergeConflictResolution => Some("merge-conflict-resolution"),
        ProfileScenarioKind::GithubIssueTriage => Some("github-issue-triage"),
        ProfileScenarioKind::CiFailureTriage => Some("ci-failure-triage"),
        ProfileScenarioKind::PullRequestReview => Some("pull-request-review"),
        ProfileScenarioKind::DependencyUpgradeTriage => Some("dependency-upgrade-triage"),
        ProfileScenarioKind::TechnicalEssay => Some("technical-essay"),
        ProfileScenarioKind::ConfigMigration => Some("config-migration"),
        ProfileScenarioKind::OpsReport => Some("ops-report"),
        ProfileScenarioKind::InventoryRebalancePlan => Some("inventory-rebalance-plan"),
        ProfileScenarioKind::ExperimentRolloutAudit => Some("experiment-rollout-audit"),
        ProfileScenarioKind::MultiModuleBugfix => Some("multi-module-bugfix"),
        ProfileScenarioKind::StatefulReconciliationBugfix => Some("stateful-reconciliation-bugfix"),
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => {
            Some("feature-rollout-consistency-bugfix")
        }
        ProfileScenarioKind::FrontierRuleTransfer => Some("frontier-rule-transfer"),
        ProfileScenarioKind::TerminalRepair => Some("terminal-repair"),
        ProfileScenarioKind::MultiHopAnalysis => Some("multi-hop-analysis"),
        ProfileScenarioKind::PolicySupportAgent => Some("policy-support-agent"),
        _ => None,
    }
}

fn prepare_profile_scenario_at(dir: &Path, scenario: ProfileScenarioKind) -> Result<()> {
    if dir.exists() {
        std::fs::remove_dir_all(dir)
            .map_err(|error| anyhow::anyhow!("failed to reset {}: {error}", dir.display()))?;
    }
    std::fs::create_dir_all(dir)
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
        ProfileScenarioKind::ShellRecovery => {
            std::fs::create_dir_all(dir.join("data"))
                .map_err(|error| anyhow::anyhow!("failed to create data fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tools"))
                .map_err(|error| anyhow::anyhow!("failed to create tools fixture: {error}"))?;
            std::fs::write(
                dir.join("data").join("events.csv"),
                "service,status\napi,ok\npayments,failed\npayments,failed\nsearch,ok\npayments,ok\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture events.csv: {error}"))?;
            std::fs::write(
                dir.join("tools").join("analyze-events.ps1"),
                "param([string]$Path)\n\
                 $ErrorActionPreference = 'Stop'\n\
                 $rows = Import-Csv -LiteralPath $Path\n\
                 $failed = @($rows | Where-Object { $_.status -eq 'failed' })\n\
                 $top = $rows | Group-Object service | Sort-Object Count -Descending | Select-Object -First 1\n\
                 \"total=$($rows.Count)\"\n\
                 \"failed=$($failed.Count)\"\n\
                 \"top_service=$($top.Name)\"\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture analyze-events.ps1: {error}"))?;
        }
        ProfileScenarioKind::PrecisePatch => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("src").join("status_map.ts"),
                "export function labelForStatus(status: string): string {\n\
                   switch (status) {\n\
                     case 'ready':\n\
                       return 'Ready';\n\
                     case 'queued':\n\
                       return 'Unknown';\n\
                     case 'failed':\n\
                       return 'Failed';\n\
                     default:\n\
                       return 'Unknown';\n\
                   }\n\
                 }\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture status_map.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("status_map.spec.md"),
                "# Status Map Spec\n\n`queued` must render as `Queued`, while the default branch must still render as `Unknown`.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture status_map.spec.md: {error}"))?;
        }
        ProfileScenarioKind::MultiFilePatch => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("docs"))
                .map_err(|error| anyhow::anyhow!("failed to create docs fixture: {error}"))?;
            std::fs::write(
                dir.join("src").join("routes.ts"),
                "export const routes = [\n\
                   { id: 'home', path: '/' },\n\
                   { id: 'settings', path: '/settings' },\n\
                 ];\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture routes.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("navigation.ts"),
                "export const navigationItems = [\n\
                   { label: 'Home', routeId: 'home' },\n\
                   { label: 'Settings', routeId: 'settings' },\n\
                 ];\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture navigation.ts: {error}"))?;
            std::fs::write(
                dir.join("docs").join("routes.md"),
                "# Routes\n\n- `/` home dashboard\n- `/settings` settings page\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture routes.md: {error}"))?;
        }
        ProfileScenarioKind::ManifestContractWrite => {
            std::fs::create_dir_all(dir.join("data"))
                .map_err(|error| anyhow::anyhow!("failed to create data fixture: {error}"))?;
            std::fs::write(
                dir.join("brief.md"),
                "# Release artifact contract\n\nCreate the two files under `generated/` from the approved release in `data/releases.json`. The candidate release is rejected and must not appear in either output. Preserve artifact order and use only the approved values.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("data").join("releases.json"),
                "[\n  {\n    \"status\": \"rejected\",\n    \"channel\": \"stable\",\n    \"version\": \"1.4.1-rc.1\",\n    \"previousVersion\": \"1.4.0\",\n    \"sha256\": \"deadbeef\",\n    \"artifacts\": [\"spark-1.4.1-rc.1.zip\"]\n  },\n  {\n    \"status\": \"approved\",\n    \"channel\": \"stable\",\n    \"version\": \"1.4.0\",\n    \"previousVersion\": \"1.3.9\",\n    \"sha256\": \"9c2f8a1d\",\n    \"artifacts\": [\"spark-1.4.0-windows-x64.zip\", \"spark-1.4.0-checksums.txt\"]\n  }\n]\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture releases.json: {error}"))?;
        }
        ProfileScenarioKind::ScopedPolicyPatch => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("src").join("rate_limit.ts"),
                "export type Account = { active: boolean; retriesToday: number };\n\nexport function canRetryPayment(account: Account): boolean {\n  if (account.retriesToday >= 3) return false;\n  return true;\n}\n\nexport function isRetryLimitExceeded(account: Account): boolean {\n  if (!account.active) return true;\n  return account.retriesToday >= 3;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture rate_limit.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("rate_limit.spec.md"),
                "# Retry policy\n\nInactive accounts cannot start payment retries, even with zero prior attempts. `isRetryLimitExceeded` reports true for inactive accounts and is already correct; do not change it. Active accounts can retry below three attempts and cannot retry at three or more.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture rate_limit.spec.md: {error}"))?;
        }
        ProfileScenarioKind::ReactCalculatorScaffold => {
            std::fs::write(
                dir.join("brief.md"),
                "# React Calculator Brief\n\nBuild a small React + TypeScript calculator app in this folder. It should support digits, decimal input, clear, backspace, the four basic operators, equals, keyboard input, and a visible calculation history. Use bun for JavaScript package management and keep all generated app files inside this ignored fixture folder. The validation commands are `bun test` plus a harness-owned Playwright browser smoke check that runs the app through Vite, screenshots it, and clicks 1 + 2 = after your run finishes. Include a browser-runnable Vite entrypoint such as index.html and package setup, but do not install Playwright, launch browsers, or start a long-lived dev server yourself. Either keep tests compatible with Bun's default test runtime or add the package/config setup required for DOM-based React tests before using React Testing Library.\n",
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
        ProfileScenarioKind::RustNotesTuiScaffold => {
            std::fs::write(
                dir.join("brief.md"),
                "# Rust Notes TUI Brief\n\nCreate a small Rust CLI project in this folder named `notevim`. It should feel like a vim-style notes tool while staying scriptable enough for automated validation. Keep Cargo output in this project's default target/ directory; do not set CARGO_TARGET_DIR.\n\nRequired behavior:\n- Store notes in a plain text file selected with `--store <path>`.\n- Support `add <title> <body...>` and print the generated note id.\n- Support `list` and show each note id and title.\n- Support `search <query>` across note titles and bodies.\n- Support `export <path>` and write all notes as Markdown.\n- Support `help-keys` and include vim-style keys: `j`, `k`, `/`, `i`, `Esc`, and `:w`.\n- Include focused tests for note parsing/storage/search/export behavior.\n\nYou may use only the Rust standard library unless adding a dependency clearly reduces complexity.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("validate-notes.ps1"),
                "$ErrorActionPreference = 'Stop'\n\
                 cargo test\n\
                 $store = Join-Path $PWD 'notes.db'\n\
                 $export = Join-Path $PWD 'export.md'\n\
                 if (Test-Path -LiteralPath $store) { Remove-Item -LiteralPath $store -Force }\n\
                 if (Test-Path -LiteralPath $export) { Remove-Item -LiteralPath $export -Force }\n\
                 $addOut = cargo run --quiet -- --store $store add 'Inbox plan' 'First body with alpha marker'\n\
                 if ($LASTEXITCODE -ne 0) { throw 'add command failed' }\n\
                 if (($addOut -join \"`n\") -notmatch 'note|id|[0-9]') { throw 'add output did not mention a note id' }\n\
                 cargo run --quiet -- --store $store add 'Project Vim Mode' 'Use j and k to browse beta marker'\n\
                 if ($LASTEXITCODE -ne 0) { throw 'second add command failed' }\n\
                 $listOut = cargo run --quiet -- --store $store list\n\
                 if (($listOut -join \"`n\") -notmatch 'Inbox plan') { throw 'list missing first title' }\n\
                 if (($listOut -join \"`n\") -notmatch 'Project Vim Mode') { throw 'list missing second title' }\n\
                 $searchOut = cargo run --quiet -- --store $store search beta\n\
                 if (($searchOut -join \"`n\") -notmatch 'Project Vim Mode') { throw 'search missing matching note' }\n\
                 if (($searchOut -join \"`n\") -match 'Inbox plan') { throw 'search returned unrelated note' }\n\
                 $keysOut = cargo run --quiet -- help-keys\n\
                 foreach ($term in @('j','k','/','i','Esc',':w')) { if (($keysOut -join \"`n\") -notlike \"*$term*\") { throw \"help-keys missing $term\" } }\n\
                 cargo run --quiet -- --store $store export $export\n\
                 if ($LASTEXITCODE -ne 0) { throw 'export command failed' }\n\
                 $markdown = Get-Content -LiteralPath $export -Raw\n\
                 foreach ($term in @('# Inbox plan','# Project Vim Mode','alpha marker','beta marker')) { if ($markdown -notlike \"*$term*\") { throw \"export missing $term\" } }\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture validate-notes.ps1: {error}"))?;
        }
        ProfileScenarioKind::GithubIssueBugfix => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Issue #417: annual quotes are undercharged\n\nCustomers on annual billing are receiving quotes that are far too low. A pro customer with base monthly price 10 and 2 seats should pay 192 for the year: 10 * 2 * 12 with a 20% annual discount. Monthly customers and enterprise seat discounts should keep working. Fix the bug with the smallest reasonable code change and do not rewrite the tests.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"type\": \"module\",\n  \"scripts\": {\n    \"test\": \"bun test\"\n  }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("src").join("quote.ts"),
                "export type Customer = {\n  plan: 'free' | 'pro' | 'enterprise';\n  seats: number;\n  annual: boolean;\n};\n\nexport function quoteTotal(baseMonthly: number, customer: Customer): number {\n  let subtotal = baseMonthly * customer.seats;\n  if (customer.plan === 'enterprise') {\n    subtotal *= 0.85;\n  }\n  if (customer.annual) {\n    subtotal *= 0.8;\n  }\n  return Math.round(subtotal * 100) / 100;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture quote.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("quote.test.ts"),
                "import { expect, test } from 'bun:test';\nimport { quoteTotal } from '../src/quote';\n\ntest('monthly pro quote multiplies seats by monthly price', () => {\n  expect(quoteTotal(20, { plan: 'pro', seats: 3, annual: false })).toBe(60);\n});\n\ntest('enterprise quote keeps the seat discount', () => {\n  expect(quoteTotal(100, { plan: 'enterprise', seats: 10, annual: false })).toBe(850);\n});\n\ntest('annual quote annualizes before applying the annual discount', () => {\n  expect(quoteTotal(10, { plan: 'pro', seats: 2, annual: true })).toBe(192);\n});\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture quote.test.ts: {error}"))?;
        }
        ProfileScenarioKind::RustFailingTestBugfix => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Issue #733: retry scheduler runs low-priority work first\n\nThe retry scheduler should return runnable jobs with higher priority values first. It should also drop exhausted jobs and jobs whose id is blank or only whitespace. A recent incident retried low-priority webhooks before urgent billing repairs because the queue order was inverted. Fix the production code with the smallest reasonable Rust change and keep the public API intact.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("Cargo.toml"),
                "[package]\nname = \"spark-retry-scheduler-fixture\"\nversion = \"0.1.0\"\nedition = \"2021\"\n\n[lib]\npath = \"src/lib.rs\"\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture Cargo.toml: {error}"))?;
            std::fs::write(
                dir.join("src").join("lib.rs"),
                "#[derive(Clone, Debug, Eq, PartialEq)]\npub struct RetryJob {\n    pub id: String,\n    pub priority: u8,\n    pub attempts: u8,\n}\n\npub fn runnable_jobs(mut jobs: Vec<RetryJob>) -> Vec<RetryJob> {\n    jobs.retain(|job| !job.id.is_empty() && job.attempts < 3);\n    jobs.sort_by_key(|job| job.priority);\n    jobs\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture lib.rs: {error}"))?;
            std::fs::write(
                dir.join("tests").join("retry_scheduler.rs"),
                "use spark_retry_scheduler_fixture::{runnable_jobs, RetryJob};\n\nfn job(id: &str, priority: u8, attempts: u8) -> RetryJob {\n    RetryJob {\n        id: id.to_string(),\n        priority,\n        attempts,\n    }\n}\n\n#[test]\nfn returns_highest_priority_jobs_first() {\n    let jobs = runnable_jobs(vec![job(\"low\", 1, 0), job(\"urgent\", 9, 0), job(\"normal\", 5, 0)]);\n    let ids = jobs.into_iter().map(|job| job.id).collect::<Vec<_>>();\n    assert_eq!(ids, [\"urgent\", \"normal\", \"low\"]);\n}\n\n#[test]\nfn filters_exhausted_and_blank_jobs() {\n    let jobs = runnable_jobs(vec![job(\"ready\", 4, 2), job(\"done\", 9, 3), job(\"   \", 8, 0), job(\"\", 7, 0)]);\n    assert_eq!(jobs, vec![job(\"ready\", 4, 2)]);\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture retry_scheduler.rs: {error}"))?;
        }
        ProfileScenarioKind::TypeScriptReducerBugfix => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Issue #812: cart restore can charge removed items\n\nRestored carts can contain inactive historical lines. `cartSubtotalCents` must ignore inactive lines. Also, setting a SKU quantity to zero or a negative value should remove that line from the cart instead of leaving a zero-quantity item around. Fix the reducer with the smallest reasonable production change and keep the exported types/functions intact.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"type\": \"module\",\n  \"scripts\": {\n    \"test\": \"bun test\"\n  }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("src").join("cart.ts"),
                "export type CartItem = {\n  sku: string;\n  quantity: number;\n  unitPriceCents: number;\n  active: boolean;\n};\n\nexport type CartState = {\n  items: CartItem[];\n  couponCode?: string;\n};\n\nexport type CartAction =\n  | { type: 'add'; item: CartItem }\n  | { type: 'setQuantity'; sku: string; quantity: number }\n  | { type: 'remove'; sku: string };\n\nexport function reduceCart(state: CartState, action: CartAction): CartState {\n  switch (action.type) {\n    case 'add': {\n      const existing = state.items.find((item) => item.sku === action.item.sku);\n      if (!existing) {\n        return { ...state, items: [...state.items, action.item] };\n      }\n      return {\n        ...state,\n        items: state.items.map((item) =>\n          item.sku === action.item.sku\n            ? { ...item, quantity: item.quantity + action.item.quantity, active: true }\n            : item,\n        ),\n      };\n    }\n    case 'setQuantity':\n      return {\n        ...state,\n        items: state.items.map((item) =>\n          item.sku === action.sku ? { ...item, quantity: action.quantity } : item,\n        ),\n      };\n    case 'remove':\n      return { ...state, items: state.items.filter((item) => item.sku !== action.sku) };\n  }\n}\n\nexport function cartSubtotalCents(state: CartState): number {\n  return state.items.reduce((total, item) => total + item.quantity * item.unitPriceCents, 0);\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture cart.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("cart.test.ts"),
                "import { expect, test } from 'bun:test';\nimport { cartSubtotalCents, reduceCart, type CartState } from '../src/cart';\n\nconst baseState: CartState = {\n  couponCode: 'WELCOME',\n  items: [\n    { sku: 'active-a', quantity: 2, unitPriceCents: 500, active: true },\n    { sku: 'old-b', quantity: 9, unitPriceCents: 999, active: false },\n  ],\n};\n\ntest('subtotal ignores inactive restored lines', () => {\n  expect(cartSubtotalCents(baseState)).toBe(1000);\n});\n\ntest('setting quantity to zero removes the cart line', () => {\n  const next = reduceCart(baseState, { type: 'setQuantity', sku: 'active-a', quantity: 0 });\n  expect(next.items.map((item) => item.sku)).toEqual(['old-b']);\n});\n\ntest('setting quantity negative also removes the cart line', () => {\n  const next = reduceCart(baseState, { type: 'setQuantity', sku: 'active-a', quantity: -3 });\n  expect(next.items.some((item) => item.sku === 'active-a')).toBe(false);\n});\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture cart.test.ts: {error}"))?;
        }
        ProfileScenarioKind::MergeConflictResolution => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Merge Conflict: preserve both feature flag rollouts\n\nThe feature flag file was left with conflict markers after merging the enterprise dashboard rollout with the EU data-residency rollout. Resolve the conflict in `src/featureFlags.ts` without changing the public function shape. Keep the dashboard-v2 flag for enterprise accounts and beta tenants, and also keep the data-residency flag for EU accounts. Run `bun test` when done.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"type\": \"module\",\n  \"scripts\": {\n    \"test\": \"bun test\"\n  }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("src").join("featureFlags.ts"),
                "export type Account = {\n  plan: 'free' | 'team' | 'enterprise';\n  tenant: string;\n  region: 'us' | 'eu';\n};\n\nexport function enabledFlags(account: Account): string[] {\n  const flags = ['core'];\n<<<<<<< HEAD\n  if (account.plan === 'enterprise' || account.tenant.startsWith('beta-')) {\n    flags.push('dashboard-v2');\n  }\n=======\n  if (account.region === 'eu') {\n    flags.push('data-residency');\n  }\n>>>>>>> region-rollout\n  return flags;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture featureFlags.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("featureFlags.test.ts"),
                "import { expect, test } from 'bun:test';\nimport { enabledFlags } from '../src/featureFlags';\n\ntest('enterprise accounts keep dashboard-v2', () => {\n  expect(enabledFlags({ plan: 'enterprise', tenant: 'acme', region: 'us' })).toContain('dashboard-v2');\n});\n\ntest('beta tenants keep dashboard-v2', () => {\n  expect(enabledFlags({ plan: 'team', tenant: 'beta-acme', region: 'us' })).toContain('dashboard-v2');\n});\n\ntest('eu accounts keep data residency', () => {\n  expect(enabledFlags({ plan: 'team', tenant: 'acme', region: 'eu' })).toContain('data-residency');\n});\n\ntest('eu enterprise accounts receive both branch flags', () => {\n  const flags = enabledFlags({ plan: 'enterprise', tenant: 'acme', region: 'eu' });\n  expect(flags).toContain('dashboard-v2');\n  expect(flags).toContain('data-residency');\n});\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture featureFlags.test.ts: {error}"))?;
        }
        ProfileScenarioKind::GithubIssueTriage => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("logs"))
                .map_err(|error| anyhow::anyhow!("failed to create logs fixture: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Issue #612: /api/items sometimes serves stale inventory\n\nSupport reports that after a warehouse import completes, `/api/items` can keep returning old quantities for several minutes. Please triage the issue and write `triage.md` with the likely root cause, evidence, reproduction steps, and a fix plan. Do not edit source code in this task.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("src").join("cachePolicy.ts"),
                "export function cacheHeader(route: string): string {\n  if (route.startsWith('/api/items')) {\n    return 'Cache-Control: public, max-age=300, stale-while-revalidate=30';\n  }\n  return 'Cache-Control: no-store';\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture cachePolicy.ts: {error}"))?;
            std::fs::write(
                dir.join("logs").join("warehouse-import.log"),
                "10:00:01 import complete sku=A-100 quantity=12\n10:00:04 GET /api/items cache=HIT age=287 quantity=9\n10:04:48 GET /api/items cache=HIT age=295 quantity=9\n10:05:03 GET /api/items cache=MISS age=0 quantity=12\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture warehouse-import.log: {error}"))?;
        }
        ProfileScenarioKind::CiFailureTriage => {
            std::fs::create_dir_all(dir.join(".github").join("workflows"))
                .map_err(|error| anyhow::anyhow!("failed to create workflow fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("logs"))
                .map_err(|error| anyhow::anyhow!("failed to create logs fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# CI Failure: checkout discount regression\n\nThe `frontend-tests` GitHub Actions job fails after adding the SAVE20 campaign. Triage the failure from the local workflow, CI log, production helper, and test file. Do not modify source files; write a focused diagnosis with the failing command, failing assertion, likely root cause, and minimal fix plan.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join(".github").join("workflows").join("frontend.yml"),
                "name: frontend-tests\n\non: [pull_request]\n\njobs:\n  frontend-tests:\n    runs-on: ubuntu-latest\n    steps:\n      - uses: actions/checkout@v4\n      - uses: oven-sh/setup-bun@v1\n      - run: bun install --frozen-lockfile\n      - run: bun test\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture frontend.yml: {error}"))?;
            std::fs::write(
                dir.join("logs").join("frontend-tests.log"),
                "[frontend-tests] run bun test\n\ntests/discount.test.ts:\n  ✓ keeps full price without a discount code\n  ✓ applies SAVE10 campaign\n  ✗ applies SAVE20 campaign to checkout totals\n\n  expect(received).toBe(expected)\n\n  Expected: 80\n  Received: 100\n\n  at tests/discount.test.ts:14:41\n\n1 failed, 2 passed\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture frontend-tests.log: {error}"))?;
            std::fs::write(
                dir.join("src").join("discount.ts"),
                "export function applyDiscount(total: number, discountCode?: string): number {\n  const code = discountCode?.trim().toUpperCase();\n  if (!code) return total;\n  if (code === 'SAVE10') return Math.round(total * 0.9 * 100) / 100;\n  return total;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture discount.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("discount.test.ts"),
                "import { expect, test } from 'bun:test';\nimport { applyDiscount } from '../src/discount';\n\ntest('keeps full price without a discount code', () => {\n  expect(applyDiscount(100)).toBe(100);\n});\n\ntest('applies SAVE10 campaign', () => {\n  expect(applyDiscount(100, 'SAVE10')).toBe(90);\n});\n\ntest('applies SAVE20 campaign to checkout totals', () => {\n  expect(applyDiscount(100, 'SAVE20')).toBe(80);\n});\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture discount.test.ts: {error}"))?;
        }
        ProfileScenarioKind::PullRequestReview => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("pr.md"),
                "# PR #184: Add admin comp discount\n\nReview this checkout discount PR. Product rules: only users with role exactly `admin` may receive a full internal comp discount. `read-only-admin` users can inspect orders but must never create discounts. Do not edit source files; write `review.md` with any blocking finding, evidence, and a minimal test/fix recommendation.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture pr.md: {error}"))?;
            std::fs::write(
                dir.join("diff.patch"),
                "diff --git a/src/checkout.ts b/src/checkout.ts\nindex 2db51a1..8d7ef22 100644\n--- a/src/checkout.ts\n+++ b/src/checkout.ts\n@@\n export function discountFor(user: User, cart: Cart): number {\n+  const normalizedRole = user.role.trim().toLowerCase();\n+  if (normalizedRole.includes('admin')) {\n+    return cart.subtotalCents;\n+  }\n   if (cart.couponCode === 'SAVE10') {\n     return Math.round(cart.subtotalCents * 0.1);\n   }\ndiff --git a/tests/checkout.test.ts b/tests/checkout.test.ts\nindex aa0b21c..f12ca39 100644\n--- a/tests/checkout.test.ts\n+++ b/tests/checkout.test.ts\n@@\n test('admin users can comp internal carts', () => {\n   expect(discountFor({ id: 'u-1', role: 'admin' }, { subtotalCents: 5000 })).toBe(5000);\n });\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture diff.patch: {error}"))?;
            std::fs::write(
                dir.join("src").join("checkout.ts"),
                "export type UserRole = 'customer' | 'support' | 'admin' | 'read-only-admin';\n\nexport type User = {\n  id: string;\n  role: UserRole;\n};\n\nexport type Cart = {\n  subtotalCents: number;\n  couponCode?: string;\n};\n\nexport function discountFor(user: User, cart: Cart): number {\n  const normalizedRole = user.role.trim().toLowerCase();\n  if (normalizedRole.includes('admin')) {\n    return cart.subtotalCents;\n  }\n  if (cart.couponCode === 'SAVE10') {\n    return Math.round(cart.subtotalCents * 0.1);\n  }\n  return 0;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture checkout.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("checkout.test.ts"),
                "import { expect, test } from 'bun:test';\nimport { discountFor } from '../src/checkout';\n\ntest('SAVE10 applies a ten percent discount', () => {\n  expect(discountFor({ id: 'u-1', role: 'customer' }, { subtotalCents: 5000, couponCode: 'SAVE10' })).toBe(500);\n});\n\ntest('admin users can comp internal carts', () => {\n  expect(discountFor({ id: 'u-2', role: 'admin' }, { subtotalCents: 5000 })).toBe(5000);\n});\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture checkout.test.ts: {error}"))?;
        }
        ProfileScenarioKind::DependencyUpgradeTriage => {
            std::fs::create_dir_all(dir.join("docs"))
                .map_err(|error| anyhow::anyhow!("failed to create docs fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("upgrade.md"),
                "# Renovate PR: @acme/time-utils 1.4.2 -> 2.0.0\n\nTriage this dependency upgrade before merge. Billing cutoffs must remain based on UTC dates, because local-time cutoffs can bill customers in the wrong month near midnight. Do not edit source files; write `upgrade-triage.md` with the changed package, migration risk, affected code, test gap, and minimal fix plan.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture upgrade.md: {error}"))?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"name\": \"billing-cutoff-service\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"dependencies\": {\n    \"@acme/time-utils\": \"2.0.0\"\n  },\n  \"devDependencies\": {\n    \"bun-types\": \"latest\"\n  }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("bun.lock"),
                "@acme/time-utils@2.0.0:\n  version \"2.0.0\"\n  integrity \"sha512-fixture\"\n\n@acme/time-utils@1.4.2:\n  version \"1.4.2\"\n  integrity \"sha512-previous\"\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture bun.lock: {error}"))?;
            std::fs::write(
                dir.join("docs").join("time-utils-2.0.md"),
                "# @acme/time-utils 2.0 Migration\n\n`parseBusinessDate(input)` now interprets date-only strings in the process local timezone by default. Version 1.x interpreted date-only strings as UTC. To preserve UTC behavior, call `parseBusinessDate(input, { zone: 'utc' })`.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture time-utils-2.0.md: {error}"))?;
            std::fs::write(
                dir.join("src").join("billingWindow.ts"),
                "import { parseBusinessDate } from '@acme/time-utils';\n\nexport function billingCutoffIso(input: string): string {\n  const cutoff = parseBusinessDate(input);\n  return cutoff.toISOString().slice(0, 10);\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture billingWindow.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("billingWindow.test.ts"),
                "import { expect, test } from 'bun:test';\nimport { billingCutoffIso } from '../src/billingWindow';\n\ntest('formats a billing cutoff date', () => {\n  expect(billingCutoffIso('2026-03-31')).toBe('2026-03-31');\n});\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture billingWindow.test.ts: {error}"))?;
        }
        ProfileScenarioKind::TechnicalEssay => {
            std::fs::create_dir_all(dir.join("sources"))
                .map_err(|error| anyhow::anyhow!("failed to create sources fixture: {error}"))?;
            std::fs::write(
                dir.join("brief.md"),
                "# Essay Brief\n\nWrite `essay.md`: a polished 450-750 word technical essay for engineering managers titled `Operational Visibility Is a Product Feature`. Ground every concrete claim in the provided source notes. Use short section headings and cite the notes inline as [S1], [S2], and [S3]. Do not browse the web.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("sources").join("S1-observability.md"),
                "# S1\n\nTeams that expose queue depth, freshness, and error budgets to operators resolve customer incidents faster because support can distinguish user error from platform degradation before escalating.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture S1: {error}"))?;
            std::fs::write(
                dir.join("sources").join("S2-product.md"),
                "# S2\n\nDashboards are most useful when they sit beside the workflow they describe. A separate status page helps executives, but embedded operational context helps the person taking the next action.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture S2: {error}"))?;
            std::fs::write(
                dir.join("sources").join("S3-cost.md"),
                "# S3\n\nThe cost of visibility work should be justified by avoided support loops, reduced mean time to repair, and safer rollout decisions rather than by vanity chart coverage.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture S3: {error}"))?;
        }
        ProfileScenarioKind::ConfigMigration => {
            std::fs::create_dir_all(dir.join("config"))
                .map_err(|error| anyhow::anyhow!("failed to create config fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("docs"))
                .map_err(|error| anyhow::anyhow!("failed to create docs fixture: {error}"))?;
            std::fs::write(
                dir.join("config").join("app.json"),
                "{\n  \"schemaVersion\": 1,\n  \"authMode\": \"password\",\n  \"retry\": {\n    \"retries\": 3,\n    \"backoffMs\": 250\n  }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture app.json: {error}"))?;
            std::fs::write(
                dir.join("src").join("config.ts"),
                "export type AppConfig = {\n  schemaVersion: 1;\n  authMode: 'password' | 'oauth';\n  retry: { retries: number; backoffMs: number };\n};\n\nexport function describeConfig(config: AppConfig): string {\n  return `${config.authMode} auth with ${config.retry.retries} retries`;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture config.ts: {error}"))?;
            std::fs::write(
                dir.join("docs").join("config.md"),
                "# Config\n\nVersion 1 uses `authMode` and `retry.retries` / `retry.backoffMs`.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture config.md: {error}"))?;
            std::fs::write(
                dir.join("migration.md"),
                "# Migration Request\n\nMigrate this fixture to schema version 2. Replace `authMode` with `authentication.method`, replace `retry.retries` with `retry.maxAttempts`, keep `retry.backoffMs`, update the TypeScript type/description helper, and update docs/config.md. Preserve the password authentication method and the same retry values.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture migration.md: {error}"))?;
        }
        ProfileScenarioKind::OpsReport => {
            std::fs::create_dir_all(dir.join("data"))
                .map_err(|error| anyhow::anyhow!("failed to create data fixture: {error}"))?;
            std::fs::write(
                dir.join("brief.md"),
                "# Ops Report Brief\n\nAnalyze `data/tickets.csv`. Treat the first CSV line as the header, not a ticket. Write `report.md` with the operational readout and `metrics.json` with exactly these numeric keys: `totalTickets`, `openTickets`, `p1Open`, and `averageOpenMinutes`. Count ticket rows only for `totalTickets`; round `averageOpenMinutes` for open tickets to one decimal place. Rank the highest-risk team by open P1 ticket count, then by oldest open P1 age as the tie-breaker. Do not count P2 tickets as P1 tickets. Mention the highest-risk team in the report and explain why.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("data").join("tickets.csv"),
                "id,team,severity,status,minutes\n1,api,P1,open,42\n2,api,P2,closed,30\n3,billing,P1,open,95\n4,billing,P1,closed,120\n5,search,P3,open,15\n6,api,P2,open,60\n7,billing,P2,open,45\n8,search,P1,closed,80\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture tickets.csv: {error}"))?;
        }
        ProfileScenarioKind::InventoryRebalancePlan => {
            std::fs::create_dir_all(dir.join("data"))
                .map_err(|error| anyhow::anyhow!("failed to create data fixture: {error}"))?;
            std::fs::write(
                dir.join("brief.md"),
                "# Inventory Rebalance Brief\n\nChoose transfer options for two planning cases: the base budget is 325 and the contingency budget is 250. Write `plan.json` with exactly the top-level keys `basePlan`, `contingencyPlan`, and `incrementalNetBenefit`, plus `memo.md` with a concise recommendation and tradeoff explanation. Each plan must contain exactly `budget`, `selectedOptionIds`, `totalUnits`, `totalCost`, `grossAvoidedPenalty`, `netBenefit`, and `remainingBudget`. Keep option ids sorted. Use every rule in `policy.md` and all three CSV files. Do not edit the inputs. Use a short script or command to enumerate all feasible subsets rather than relying on a greedy guess.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("policy.md"),
                "# Rebalance Policy\n\n- Every row in `transfer_options.csv` is an all-or-nothing transfer option.\n- An option is eligible only when `lead_days` is at most 3.\n- For each SKU and origin, selected units cannot exceed `on_hand - forecast_14d - safety_stock` from `warehouses.csv`.\n- For each SKU and destination, selected units cannot exceed `forecast_14d + safety_stock - on_hand`.\n- Total cost is `units * variable_cost_per_unit + fixed_cost` for every selected option and cannot exceed the case budget.\n- Gross avoided penalty is `units * stockout_penalty_per_unit` from `products.csv`.\n- Net benefit is gross avoided penalty minus total cost.\n- Optimize each budget independently for maximum net benefit. Break a tie by lower total cost, then by the lexicographically smaller comma-joined sorted option-id list.\n- `incrementalNetBenefit` is base-plan net benefit minus contingency-plan net benefit.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture policy.md: {error}"))?;
            std::fs::write(
                dir.join("data").join("products.csv"),
                "sku,stockout_penalty_per_unit\nAtlas,42\nBolt,30\nCipher,55\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture products.csv: {error}"))?;
            std::fs::write(
                dir.join("data").join("warehouses.csv"),
                "warehouse,sku,on_hand,forecast_14d,safety_stock\nNORTH,Atlas,110,60,20\nWEST,Atlas,45,60,10\nEAST,Atlas,35,50,10\nSOUTH,Atlas,75,50,10\nNORTH,Bolt,80,50,10\nWEST,Bolt,25,35,10\nEAST,Bolt,20,30,10\nSOUTH,Bolt,70,45,10\nNORTH,Cipher,55,30,10\nWEST,Cipher,10,20,5\nEAST,Cipher,15,25,5\nSOUTH,Cipher,45,25,10\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture warehouses.csv: {error}"))?;
            std::fs::write(
                dir.join("data").join("transfer_options.csv"),
                "option_id,sku,origin,destination,units,variable_cost_per_unit,fixed_cost,lead_days\nT01,Atlas,NORTH,WEST,15,4,20,2\nT02,Atlas,NORTH,EAST,18,3,35,3\nT03,Atlas,SOUTH,WEST,12,2,30,2\nT04,Atlas,SOUTH,EAST,15,5,10,1\nT05,Atlas,NORTH,WEST,20,2,70,1\nT06,Bolt,NORTH,WEST,12,2,12,2\nT07,Bolt,NORTH,EAST,15,3,8,3\nT08,Bolt,SOUTH,WEST,15,1,25,1\nT09,Bolt,SOUTH,EAST,10,4,5,2\nT10,Cipher,NORTH,WEST,10,4,15,3\nT11,Cipher,NORTH,EAST,12,2,40,2\nT12,Cipher,SOUTH,WEST,10,3,10,1\nT13,Cipher,SOUTH,EAST,8,5,5,2\nT14,Atlas,NORTH,EAST,10,1,15,5\n",
            )
            .map_err(|error| {
                anyhow::anyhow!("failed to write fixture transfer_options.csv: {error}")
            })?;
        }
        ProfileScenarioKind::ExperimentRolloutAudit => {
            std::fs::create_dir_all(dir.join("data"))
                .map_err(|error| anyhow::anyhow!("failed to create data fixture: {error}"))?;
            std::fs::write(
                dir.join("brief.md"),
                "# Experiment Rollout Audit\n\nAudit the `control` and `treatment` variants using `policy.md` and all three CSV files. Write `audit.json` and `memo.md`. The JSON must contain exactly the top-level keys `control`, `dataQuality`, `decision`, `treatment`, and `uplifts`. Each variant must contain exactly `conversionRatePct`, `converters`, `eligibleUsers`, `grossRevenueCents`, `netRevenueCents`, `netRevenuePerEligibleCents`, `orders`, `refundCents`, `refundRatePct`, and `refundedOrders`. `uplifts` must contain exactly `conversionUpliftPercentagePoints`, `netRevenuePerEligibleUpliftPct`, `refundRateDeltaPercentagePoints`, and `relativeConversionUpliftPct`. `dataQuality` must contain exactly `assignmentRows`, `conflictedUsers`, `duplicateAssignmentRows`, `duplicateEventRows`, `duplicateOrderEvents`, `eventRows`, `excludedUsers`, `orphanEvents`, and `outOfWindowCheckouts`. Use numeric percentages, not fractions. `decision` must be `launch` or `hold`. Do not edit the inputs. Use a short Bun or PowerShell audit script rather than hand-counting rows.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("policy.md"),
                "# Experiment Policy\n\n- Collapse exact duplicate assignment rows. `duplicateAssignmentRows` counts only the removed rows.\n- A user assigned to more than one distinct variant is conflicted and excluded. Count the user once in `conflictedUsers`.\n- Remove users listed in `exclusions.csv`; `excludedUsers` counts distinct listed users that otherwise have a non-conflicted assignment.\n- The remaining canonical assignments are the denominators for each variant.\n- Collapse exact duplicate event rows by `event_id`; `duplicateEventRows` counts removed rows.\n- An event whose user has no remaining eligible assignment is an orphan event, including events for conflicted, excluded, or unknown users.\n- A checkout is attributed when it occurs at or after assignment and strictly before 72 hours after assignment. Count eligible-user checkouts outside that window in `outOfWindowCheckouts`.\n- Deduplicate attributed checkouts by `order_id`, keeping the earliest event. Count later attributed checkout events for an already-counted order in `duplicateOrderEvents`.\n- A user converts once when they have at least one attributed unique order. `orders` counts attributed unique orders. Gross revenue is the sum of their checkout `amount_cents`.\n- A refund counts only when it references an attributed order and occurs by `2026-07-08T00:00:00Z`. Count refunded orders once and subtract the first matching refund amount from net revenue.\n- Conversion rate is converters divided by eligible users. Refund rate is refunded orders divided by attributed orders. Net revenue per eligible user is net revenue divided by eligible users. Report rates as percentages rounded to two decimals and revenue-per-eligible cents rounded to the nearest whole cent.\n- Uplifts are treatment minus control. Relative conversion uplift and net-revenue-per-eligible uplift divide that difference by the control value and are percentages rounded to two decimals.\n- Recommend `launch` only when both variants have at least 10 eligible users, relative conversion uplift is at least 20%, net-revenue-per-eligible uplift is at least 5%, and treatment refund rate is no more than 3 percentage points above control. Otherwise recommend `hold`.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture policy.md: {error}"))?;
            std::fs::write(
                dir.join("data").join("assignments.csv"),
                "user_id,variant,assigned_at\nC01,control,2026-07-01T00:00:00Z\nC02,control,2026-07-01T00:00:00Z\nC03,control,2026-07-01T00:00:00Z\nC04,control,2026-07-01T00:00:00Z\nC05,control,2026-07-01T00:00:00Z\nC06,control,2026-07-01T00:00:00Z\nC07,control,2026-07-01T00:00:00Z\nC08,control,2026-07-01T00:00:00Z\nC09,control,2026-07-01T00:00:00Z\nC10,control,2026-07-01T00:00:00Z\nT01,treatment,2026-07-01T00:00:00Z\nT02,treatment,2026-07-01T00:00:00Z\nT03,treatment,2026-07-01T00:00:00Z\nT04,treatment,2026-07-01T00:00:00Z\nT05,treatment,2026-07-01T00:00:00Z\nT06,treatment,2026-07-01T00:00:00Z\nT07,treatment,2026-07-01T00:00:00Z\nT08,treatment,2026-07-01T00:00:00Z\nT09,treatment,2026-07-01T00:00:00Z\nT10,treatment,2026-07-01T00:00:00Z\nC03,control,2026-07-01T00:00:00Z\nX01,control,2026-07-01T00:00:00Z\nX01,treatment,2026-07-01T01:00:00Z\nE01,control,2026-07-01T00:00:00Z\nB01,treatment,2026-07-01T00:00:00Z\n",
            )
            .map_err(|error| {
                anyhow::anyhow!("failed to write fixture assignments.csv: {error}")
            })?;
            std::fs::write(
                dir.join("data").join("exclusions.csv"),
                "user_id,reason\nE01,employee\nB01,automation\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture exclusions.csv: {error}"))?;
            std::fs::write(
                dir.join("data").join("events.csv"),
                "event_id,user_id,event_type,occurred_at,order_id,amount_cents\nEC01,C01,checkout,2026-07-01T01:00:00Z,OC01,12000\nEC02,C02,checkout,2026-07-01T02:00:00Z,OC02,8000\nRC02,C02,refund,2026-07-02T00:00:00Z,OC02,8000\nEC03,C03,checkout,2026-07-01T03:00:00Z,OC03,5000\nEC03,C03,checkout,2026-07-01T03:00:00Z,OC03,5000\nEC05,C05,checkout,2026-07-04T00:00:00Z,OC05,9000\nEC06A,C06,checkout,2026-07-01T05:00:00Z,OC06,15000\nEC06B,C06,checkout,2026-07-01T06:00:00Z,OC06,15000\nEC08,C08,checkout,2026-07-02T00:00:00Z,OC08,7000\nRC09,C09,refund,2026-07-02T01:00:00Z,UNKNOWN,5000\nET01,T01,checkout,2026-07-01T01:00:00Z,OT01,10000\nET02,T02,checkout,2026-07-01T02:00:00Z,OT02,9000\nET03A,T03,checkout,2026-07-01T03:00:00Z,OT03A,6000\nET03B,T03,checkout,2026-07-01T04:00:00Z,OT03B,4000\nET04,T04,checkout,2026-07-01T05:00:00Z,OT04,11000\nRT04,T04,refund,2026-07-02T00:00:00Z,OT04,11000\nET05,T05,checkout,2026-07-01T06:00:00Z,OT05,7000\nET06,T06,checkout,2026-07-04T00:00:00Z,OT06,8000\nET08,T08,checkout,2026-07-01T08:00:00Z,OT08,13000\nRT08,T08,refund,2026-07-03T00:00:00Z,OT08,13000\nET09,T09,checkout,2026-07-01T09:00:00Z,OT09,5000\nEX01,X01,checkout,2026-07-01T02:00:00Z,OX01,3000\nEE01,E01,checkout,2026-07-01T02:00:00Z,OE01,3000\nEB01,B01,checkout,2026-07-01T02:00:00Z,OB01,3000\nEZ99,Z99,checkout,2026-07-01T02:00:00Z,OZ99,3000\nVC04,C04,view,2026-07-01T02:00:00Z,,0\nST07,T07,session_start,2026-07-01T02:00:00Z,,0\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture events.csv: {error}"))?;
        }
        ProfileScenarioKind::MultiModuleBugfix => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Issue #418: invoice totals drift by cents on fractional pricing\n\nFinance flagged that invoices with fractional unit prices are off by a few cents, and that fixed discount codes reduce the post-tax amount instead of the taxable subtotal.\n\nExpected behavior:\n- Line precision is preserved until the very end; only the final invoice total is rounded to whole cents.\n- Discounts reduce the taxable subtotal before tax is computed.\n\nFix the production code with the smallest reasonable change and keep the exported functions and types intact. Run `bun test` to confirm.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"type\": \"module\",\n  \"scripts\": {\n    \"test\": \"bun test\"\n  }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("src").join("invoice.ts"),
                "export type OrderItem = {\n  sku: string;\n  quantity: number;\n  unitPriceCents: number;\n};\n\nexport type InvoiceLine = {\n  sku: string;\n  amountCents: number;\n};\n\nexport function buildInvoiceLines(items: OrderItem[]): InvoiceLine[] {\n  return items.map((item) => ({\n    sku: item.sku,\n    amountCents: Math.round(item.unitPriceCents * item.quantity),\n  }));\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture invoice.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("tax.ts"),
                "export function taxCentsFor(amountCents: number, rateBps: number): number {\n  return (amountCents * rateBps) / 10_000;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture tax.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("total.ts"),
                "import type { InvoiceLine } from './invoice';\nimport { taxCentsFor } from './tax';\n\nexport function invoiceTotalCents(\n  lines: InvoiceLine[],\n  discountCents: number,\n  taxRateBps: number,\n): number {\n  const subtotalCents = lines.reduce((sum, line) => sum + line.amountCents, 0);\n  return subtotalCents + taxCentsFor(subtotalCents, taxRateBps) - discountCents;\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture total.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("invoice.test.ts"),
                "import { expect, test } from 'bun:test';\nimport { buildInvoiceLines } from '../src/invoice';\nimport { invoiceTotalCents } from '../src/total';\n\ntest('rounds only the final invoice total', () => {\n  const lines = buildInvoiceLines([\n    { sku: 'a', quantity: 1, unitPriceCents: 20.5 },\n    { sku: 'b', quantity: 1, unitPriceCents: 20.5 },\n  ]);\n  expect(invoiceTotalCents(lines, 0, 0)).toBe(41);\n});\n\ntest('applies discount before tax', () => {\n  const lines = buildInvoiceLines([{ sku: 'a', quantity: 1, unitPriceCents: 1000 }]);\n  expect(invoiceTotalCents(lines, 100, 1000)).toBe(990);\n});\n\ntest('keeps fractional line precision through tax and discount', () => {\n  const lines = buildInvoiceLines([\n    { sku: 'a', quantity: 3, unitPriceCents: 333.34 },\n    { sku: 'b', quantity: 1, unitPriceCents: 10.01 },\n  ]);\n  expect(invoiceTotalCents(lines, 100, 1000)).toBe(1001);\n});\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture invoice.test.ts: {error}"))?;
        }
        ProfileScenarioKind::TerminalRepair => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("config"))
                .map_err(|error| anyhow::anyhow!("failed to create config fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("data"))
                .map_err(|error| anyhow::anyhow!("failed to create data fixture: {error}"))?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"type\": \"module\",\n  \"scripts\": {\n    \"start\": \"bun run src/index.js\"\n  }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("src").join("index.js"),
                "import fs from 'node:fs';\n\nconst settings = JSON.parse(fs.readFileSync('config/settings.json', 'utf8'));\nconst raw = fs.readFileSync(settings.dataPath, 'utf8');\nconst lines = raw.trim().split('\\n');\nconst rows = lines.slice(1).map((line) => line.split(','));\n\nconst counts = new Map();\nfor (const [team] of rows) {\n  counts.set(team, (counts.get(team) ?? 0) + 1);\n}\n\nlet top = '';\nlet topCount = -1;\nfor (const [team, count] of counts) {\n  if (count > topCount) {\n    top = team;\n    topCount = count;\n  }\n}\n\nconsole.log(`REPORT OK rows=${rows.length} top=${top}`);\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture index.js: {error}"))?;
            std::fs::write(
                dir.join("config").join("settings.json"),
                "{\n  \"dataPath\": \"data/summary.json\",\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture settings.json: {error}"))?;
            std::fs::write(
                dir.join("data").join("report.csv"),
                "team,status\napi,ok\nbilling,failed\nbilling,ok\napi,failed\napi,ok\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture report.csv: {error}"))?;
        }
        ProfileScenarioKind::MultiHopAnalysis => {
            std::fs::create_dir_all(dir.join("data"))
                .map_err(|error| anyhow::anyhow!("failed to create data fixture: {error}"))?;
            std::fs::write(
                dir.join("question.md"),
                "# Question\n\nWhat is the Q1 net revenue for product Atlas in region EMEA?\n\nWrite `answer.json` with exactly the keys `product`, `region`, and `netRevenue` (a number), and `answer.md` with a short explanation that names the included order ids. Follow `policy.md` for the net revenue rules.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture question.md: {error}"))?;
            std::fs::write(
                dir.join("policy.md"),
                "# Net Revenue Policy\n\n- Net revenue counts orders with status `shipped` only; `cancelled` and `returned` orders are excluded entirely.\n- Subtract refund amounts attached to included orders; refunds attached to excluded orders are ignored.\n- Report the final amount rounded to two decimals.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture policy.md: {error}"))?;
            std::fs::write(
                dir.join("data").join("orders.csv"),
                "order_id,product,region,units,unit_price,status\nA1,Atlas,EMEA,2,50.00,shipped\nA2,Atlas,EMEA,1,50.00,cancelled\nA3,Atlas,AMER,3,50.00,shipped\nA4,Atlas,EMEA,4,25.00,shipped\nA5,Bolt,EMEA,5,40.00,shipped\nA6,Atlas,EMEA,1,80.00,returned\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture orders.csv: {error}"))?;
            std::fs::write(
                dir.join("data").join("refunds.csv"),
                "order_id,refund_amount\nA4,20.00\nA6,60.00\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture refunds.csv: {error}"))?;
        }
        ProfileScenarioKind::StatefulReconciliationBugfix => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests"))
                .map_err(|error| anyhow::anyhow!("failed to create tests fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests").join(".harness")).map_err(|error| {
                anyhow::anyhow!("failed to create harness tests fixture: {error}")
            })?;
            std::fs::create_dir_all(dir.join("docs"))
                .map_err(|error| anyhow::anyhow!("failed to create docs fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("logs"))
                .map_err(|error| anyhow::anyhow!("failed to create logs fixture: {error}"))?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"name\": \"stateful-reconciliation-fixture\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": { \"test\": \"bun test\" }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Reservation projection incident\n\nAfter a consumer failover, the reservation dashboard mixed quantities between orders, replayed an older delivery of an event, and showed negative reserved units. A shipment was also followed by a late reserve event for the same order line.\n\nUse the incident log and projection invariants to repair the production implementation. The existing tests cover only part of the incident. Keep the public types stable and do not change the evidence, documentation, or tests.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("docs").join("invariants.md"),
                "# Projection invariants\n\n1. An event id is idempotent. When transport redelivers the same id, the delivery with the latest `receivedAt` is authoritative.\n2. Authoritative events are applied by `occurredAt`, then `sequence`, then `eventId` for a deterministic tie break.\n3. State is isolated by the pair `(orderId, sku)`.\n4. Non-finite or non-positive quantities have no effect.\n5. A release cannot reduce reserved quantity below zero.\n6. Ship consumes at most the currently reserved quantity and records only the amount consumed. A successful ship is terminal for that order line, so later events have no effect.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture invariants.md: {error}"))?;
            std::fs::write(
                dir.join("logs").join("incident.log"),
                "09:14:02 duplicate event=e-17 received=09:13:58 sequence=7\n09:14:03 duplicate event=e-17 received=09:14:01 sequence=8\n09:14:05 projection order=o-42 sku=atlas reserved=-2\n09:14:06 projection order=o-77 sku=atlas unexpectedly_changed=true\n09:14:08 late event=e-22 kind=reserve order=o-42 sku=atlas after_ship=e-21\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture incident.log: {error}"))?;
            std::fs::write(
                dir.join("src").join("types.ts"),
                "export type ReservationEvent = {\n  eventId: string;\n  orderId: string;\n  sku: string;\n  kind: \"reserve\" | \"release\" | \"ship\";\n  quantity: number;\n  occurredAt: string;\n  receivedAt: string;\n  sequence: number;\n};\n\nexport type ReservationState = {\n  orderId: string;\n  sku: string;\n  reserved: number;\n  shipped: number;\n  terminal: boolean;\n};\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture types.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("normalize.ts"),
                "import type { ReservationEvent } from \"./types\";\n\nexport function normalizeEvents(events: ReservationEvent[]): ReservationEvent[] {\n  const unique = new Map<string, ReservationEvent>();\n  for (const event of events) {\n    if (!unique.has(event.eventId)) unique.set(event.eventId, event);\n  }\n  return [...unique.values()].sort((left, right) => left.sequence - right.sequence);\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture normalize.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("project.ts"),
                "import { normalizeEvents } from \"./normalize\";\nimport type { ReservationEvent, ReservationState } from \"./types\";\n\nexport function projectReservations(events: ReservationEvent[]): ReservationState[] {\n  const states = new Map<string, ReservationState>();\n  for (const event of normalizeEvents(events)) {\n    const key = event.sku;\n    const state = states.get(key) ?? {\n      orderId: event.orderId,\n      sku: event.sku,\n      reserved: 0,\n      shipped: 0,\n      terminal: false,\n    };\n    if (event.kind === \"reserve\") state.reserved += event.quantity;\n    if (event.kind === \"release\") state.reserved -= event.quantity;\n    if (event.kind === \"ship\") {\n      state.reserved -= event.quantity;\n      state.shipped += event.quantity;\n    }\n    states.set(key, state);\n  }\n  return [...states.values()];\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture project.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("projection.test.ts"),
                r#"import { describe, expect, test } from "bun:test";
import { normalizeEvents } from "../src/normalize";
import { projectReservations } from "../src/project";
import type { ReservationEvent } from "../src/types";

const event = (overrides: Partial<ReservationEvent>): ReservationEvent => ({
  eventId: "e-1", orderId: "o-1", sku: "atlas", kind: "reserve", quantity: 1,
  occurredAt: "2026-07-26T09:00:00Z", receivedAt: "2026-07-26T09:00:01Z",
  sequence: 1, ...overrides,
});

describe("reservation reconciliation", () => {
  test("isolates equal skus belonging to different orders", () => {
    const states = projectReservations([
      event({ eventId: "e-1", orderId: "o-1", quantity: 3 }),
      event({ eventId: "e-2", orderId: "o-2", quantity: 7 }),
    ]);
    expect(states).toHaveLength(2);
    expect(states.find(item => item.orderId === "o-1")?.reserved).toBe(3);
    expect(states.find(item => item.orderId === "o-2")?.reserved).toBe(7);
  });

  test("clamps release to the available reservation", () => {
    const released = projectReservations([
      event({ eventId: "e-1", quantity: 2 }),
      event({ eventId: "e-2", kind: "release", quantity: 5, sequence: 2 }),
    ])[0];
    expect(released.reserved).toBe(0);
  });
});
"#,
            )
            .map_err(|error| {
                anyhow::anyhow!("failed to write fixture projection.test.ts: {error}")
            })?;
            std::fs::write(
                dir.join("tests")
                    .join(".harness")
                    .join("projection.validation.ts"),
                r#"import { describe, expect, test } from "bun:test";
import { normalizeEvents } from "../../src/normalize";
import { projectReservations } from "../../src/project";
import type { ReservationEvent } from "../../src/types";

const selectedCheck = process.env.SPARK_VALIDATION_CHECK;
const check = (name: string, title: string, body: () => void) => {
  if (!selectedCheck || selectedCheck === name) test(title, body);
};

const event = (overrides: Partial<ReservationEvent>): ReservationEvent => ({
  eventId: "e-1", orderId: "o-1", sku: "atlas", kind: "reserve", quantity: 1,
  occurredAt: "2026-07-26T09:00:00Z", receivedAt: "2026-07-26T09:00:01Z",
  sequence: 1, ...overrides,
});

describe("harness reconciliation invariants", () => {
  check("duplicate-timezone", "chooses the chronologically latest duplicate across timezone offsets", () => {
    const latest = event({ eventId: "e-17", quantity: 5, receivedAt: "2026-07-26T09:00:00Z" });
    const earlier = event({ eventId: "e-17", quantity: 2, receivedAt: "2026-07-26T10:30:00+02:00" });
    expect(normalizeEvents([latest, earlier])).toEqual([latest]);
  });

  check("event-order", "orders equivalent offset timestamps by instant, then sequence and id", () => {
    const later = event({ eventId: "e-3", occurredAt: "2026-07-26T10:15:00+01:00", sequence: 1 });
    const tieB = event({ eventId: "e-2", occurredAt: "2026-07-26T09:00:00Z", sequence: 2 });
    const tieA = event({ eventId: "e-1", occurredAt: "2026-07-26T10:00:00+01:00", sequence: 2 });
    expect(normalizeEvents([later, tieB, tieA]).map(item => item.eventId))
      .toEqual(["e-1", "e-2", "e-3"]);
  });

  check("terminal-shipment", "records only consumed shipment and ignores later events", () => {
    const state = projectReservations([
      event({ eventId: "e-1", quantity: 4 }),
      event({ eventId: "e-2", kind: "ship", quantity: 2, sequence: 2 }),
      event({ eventId: "e-3", kind: "reserve", quantity: 9, sequence: 3 }),
    ])[0];
    expect(state).toMatchObject({ reserved: 2, shipped: 2, terminal: true });
  });

  check("empty-shipment", "does not close an empty line when a ship consumes nothing", () => {
    const state = projectReservations([
      event({ eventId: "e-1", kind: "ship", quantity: 2 }),
      event({ eventId: "e-2", kind: "reserve", quantity: 3, sequence: 2 }),
    ])[0];
    expect(state).toMatchObject({ reserved: 3, shipped: 0, terminal: false });
  });

  check("invalid-quantity", "invalid quantities do not create projection rows", () => {
    expect(projectReservations([
      event({ eventId: "e-1", quantity: Number.NaN }),
      event({ eventId: "e-2", orderId: "o-2", quantity: 0 }),
    ])).toEqual([]);
  });
});
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write harness validation: {error}"))?;
        }
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests").join(".harness")).map_err(|error| {
                anyhow::anyhow!("failed to create harness tests fixture: {error}")
            })?;
            std::fs::create_dir_all(dir.join("docs"))
                .map_err(|error| anyhow::anyhow!("failed to create docs fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("logs"))
                .map_err(|error| anyhow::anyhow!("failed to create logs fixture: {error}"))?;

            std::fs::write(
                dir.join("package.json"),
                "{\n  \"name\": \"feature-rollout-consistency-fixture\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": { \"test\": \"bun test tests/rollout.test.ts\" }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("issue.md"),
                "# Cross-tenant feature rollout incident\n\nA control-plane retry and a cache collision exposed the `checkout-v2` flag to the wrong tenant. During the same incident, an older config revision replaced a newer emergency deny list, and changing a user's email moved them into a different rollout cohort.\n\nRepair the production implementation using the supplied invariants and incident evidence. The visible tests cover only the first symptom. Preserve the public types and service API, and do not change tests, documentation, or evidence.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture issue.md: {error}"))?;
            std::fs::write(
                dir.join("docs").join("invariants.md"),
                "# Rollout invariants\n\n1. Flag configs are isolated by the pair `(tenantId, flagKey)`.\n2. Only a strictly higher revision may replace the current config. Equal or stale revisions are ignored.\n3. A missing config, disabled flag, or tenant mismatch always denies access.\n4. The deny list takes precedence over the allow list. An explicitly allowed subject bypasses percentage rollout only when not denied.\n5. Percentage rollout is clamped to 0 through 100 and uses the stable tuple `(tenantId, flagKey, subjectId)`. Mutable profile fields such as email must not affect cohort assignment.\n6. Cached decisions are isolated by tenant, flag key, config revision, and subject id. A new revision must never reuse a prior revision's decision.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture invariants.md: {error}"))?;
            std::fs::write(
                dir.join("logs").join("incident.log"),
                "14:02:11 tenant=acme flag=checkout-v2 revision=42 decision=allow subject=u-17\n14:02:12 tenant=globex flag=checkout-v2 revision=7 cache_hit=true subject=u-17 unexpected=true\n14:03:05 tenant=acme flag=checkout-v2 incoming_revision=41 current_revision=42 replaced=true\n14:03:08 tenant=acme flag=checkout-v2 subject=u-19 allow_list=true deny_list=true decision=allow\n14:04:31 tenant=acme flag=checkout-v2 subject=u-23 email_changed=true cohort_changed=true\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture incident.log: {error}"))?;
            std::fs::write(
                dir.join("src").join("types.ts"),
                r#"export type FlagConfig = {
  tenantId: string;
  flagKey: string;
  revision: number;
  enabled: boolean;
  rolloutPercent: number;
  allowSubjects: string[];
  denySubjects: string[];
};

export type Subject = {
  tenantId: string;
  subjectId: string;
  email: string;
};

export type DecisionReason =
  | "missing"
  | "disabled"
  | "tenant_mismatch"
  | "denied"
  | "allowed"
  | "rollout"
  | "outside_rollout";

export type Decision = {
  allowed: boolean;
  reason: DecisionReason;
  bucket: number | null;
};
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture types.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("hash.ts"),
                r#"export function stableBucket(input: string): number {
  let hash = 2166136261;
  for (let index = 0; index < input.length; index += 1) {
    hash ^= input.charCodeAt(index);
    hash = Math.imul(hash, 16777619);
  }
  return (hash >>> 0) % 100;
}
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture hash.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("store.ts"),
                r#"import type { FlagConfig } from "./types";

export class FlagConfigStore {
  private readonly configs = new Map<string, FlagConfig>();

  upsert(config: FlagConfig): boolean {
    this.configs.set(config.flagKey, config);
    return true;
  }

  get(_tenantId: string, flagKey: string): FlagConfig | undefined {
    return this.configs.get(flagKey);
  }
}
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture store.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("cache.ts"),
                r#"import type { Decision, FlagConfig, Subject } from "./types";

export class DecisionCache {
  private readonly decisions = new Map<string, Decision>();

  get(config: FlagConfig, subject: Subject): Decision | undefined {
    return this.decisions.get(`${config.flagKey}:${subject.subjectId}`);
  }

  set(config: FlagConfig, subject: Subject, decision: Decision): void {
    this.decisions.set(`${config.flagKey}:${subject.subjectId}`, decision);
  }
}
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture cache.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("evaluate.ts"),
                r#"import { stableBucket } from "./hash";
import type { Decision, FlagConfig, Subject } from "./types";

export function evaluate(config: FlagConfig, subject: Subject): Decision {
  if (!config.enabled) return { allowed: false, reason: "disabled", bucket: null };
  if (config.allowSubjects.includes(subject.subjectId)) {
    return { allowed: true, reason: "allowed", bucket: null };
  }
  if (config.denySubjects.includes(subject.subjectId)) {
    return { allowed: false, reason: "denied", bucket: null };
  }

  const bucket = stableBucket(`${config.flagKey}:${subject.email}`);
  return bucket < config.rolloutPercent
    ? { allowed: true, reason: "rollout", bucket }
    : { allowed: false, reason: "outside_rollout", bucket };
}
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture evaluate.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("service.ts"),
                r#"import { DecisionCache } from "./cache";
import { evaluate } from "./evaluate";
import { FlagConfigStore } from "./store";
import type { Decision, FlagConfig, Subject } from "./types";

export class RolloutService {
  constructor(
    private readonly store = new FlagConfigStore(),
    private readonly cache = new DecisionCache(),
  ) {}

  upsert(config: FlagConfig): boolean {
    return this.store.upsert(config);
  }

  decide(subject: Subject, flagKey: string): Decision {
    const config = this.store.get(subject.tenantId, flagKey);
    if (!config) return { allowed: false, reason: "missing", bucket: null };

    const cached = this.cache.get(config, subject);
    if (cached) return cached;

    const decision = evaluate(config, subject);
    this.cache.set(config, subject, decision);
    return decision;
  }
}
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture service.ts: {error}"))?;
            std::fs::write(
                dir.join("tests").join("rollout.test.ts"),
                r#"import { describe, expect, test } from "bun:test";
import { evaluate } from "../src/evaluate";
import { FlagConfigStore } from "../src/store";
import type { FlagConfig, Subject } from "../src/types";

const config = (overrides: Partial<FlagConfig> = {}): FlagConfig => ({
  tenantId: "acme",
  flagKey: "checkout-v2",
  revision: 1,
  enabled: true,
  rolloutPercent: 0,
  allowSubjects: [],
  denySubjects: [],
  ...overrides,
});

const subject = (overrides: Partial<Subject> = {}): Subject => ({
  tenantId: "acme",
  subjectId: "u-1",
  email: "before@example.test",
  ...overrides,
});

describe("feature rollout", () => {
  test("stores equal flag keys independently for each tenant", () => {
    const store = new FlagConfigStore();
    store.upsert(config({ tenantId: "acme", revision: 4 }));
    store.upsert(config({ tenantId: "globex", revision: 9 }));
    expect(store.get("acme", "checkout-v2")?.revision).toBe(4);
    expect(store.get("globex", "checkout-v2")?.revision).toBe(9);
  });

  test("disabled flags deny access and explicit allows can opt in", () => {
    expect(evaluate(config({ enabled: false }), subject()).allowed).toBe(false);
    expect(evaluate(config({ allowSubjects: ["u-1"] }), subject())).toMatchObject({
      allowed: true,
      reason: "allowed",
    });
  });
});
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture rollout.test.ts: {error}"))?;
            std::fs::write(
                dir.join("tests")
                    .join(".harness")
                    .join("rollout.validation.ts"),
                r#"import { describe, expect, test } from "bun:test";
import { DecisionCache } from "../../src/cache";
import { evaluate } from "../../src/evaluate";
import { RolloutService } from "../../src/service";
import { FlagConfigStore } from "../../src/store";
import type { Decision, FlagConfig, Subject } from "../../src/types";

const selectedCheck = process.env.SPARK_VALIDATION_CHECK;
const check = (name: string, title: string, body: () => void) => {
  if (!selectedCheck || selectedCheck === name) test(title, body);
};

const config = (overrides: Partial<FlagConfig> = {}): FlagConfig => ({
  tenantId: "acme",
  flagKey: "checkout-v2",
  revision: 1,
  enabled: true,
  rolloutPercent: 0,
  allowSubjects: [],
  denySubjects: [],
  ...overrides,
});

const subject = (overrides: Partial<Subject> = {}): Subject => ({
  tenantId: "acme",
  subjectId: "u-1",
  email: "before@example.test",
  ...overrides,
});

describe("harness rollout invariants", () => {
  check("tenant-store", "isolates equal flag keys across tenants", () => {
    const store = new FlagConfigStore();
    const acme = config({ tenantId: "acme", revision: 4 });
    const globex = config({ tenantId: "globex", revision: 9 });
    store.upsert(acme);
    store.upsert(globex);
    expect(store.get("acme", "checkout-v2")).toEqual(acme);
    expect(store.get("globex", "checkout-v2")).toEqual(globex);
  });

  check("monotonic-revision", "ignores stale and equal config revisions", () => {
    const store = new FlagConfigStore();
    const current = config({ revision: 8, rolloutPercent: 65 });
    expect(store.upsert(current)).toBe(true);
    expect(store.upsert(config({ revision: 7, rolloutPercent: 0 }))).toBe(false);
    expect(store.upsert(config({ revision: 8, rolloutPercent: 100 }))).toBe(false);
    expect(store.get("acme", "checkout-v2")).toEqual(current);
  });

  check("decision-precedence", "enforces tenant, disabled, deny, then allow precedence", () => {
    const both = config({ allowSubjects: ["u-1"], denySubjects: ["u-1"] });
    expect(evaluate(both, subject())).toMatchObject({ allowed: false, reason: "denied" });
    expect(evaluate(config({ enabled: false, allowSubjects: ["u-1"] }), subject()))
      .toMatchObject({ allowed: false, reason: "disabled" });
    expect(evaluate(config({ allowSubjects: ["u-1"] }), subject({ tenantId: "globex" })))
      .toMatchObject({ allowed: false, reason: "tenant_mismatch" });
  });

  check("stable-rollout", "uses stable identity and clamps percentage bounds", () => {
    const partial = config({ rolloutPercent: 37 });
    const before = evaluate(partial, subject({ email: "before@example.test" }));
    const after = evaluate(partial, subject({ email: "after@example.test" }));
    expect(after).toEqual(before);
    expect(evaluate(config({ rolloutPercent: 150 }), subject()).allowed).toBe(true);
    expect(evaluate(config({ rolloutPercent: -20 }), subject()).allowed).toBe(false);
  });

  check("cache-isolation", "isolates cached decisions by tenant and revision", () => {
    const cache = new DecisionCache();
    const actor = subject();
    const allow: Decision = { allowed: true, reason: "allowed", bucket: null };
    const deny: Decision = { allowed: false, reason: "denied", bucket: null };
    const outside: Decision = { allowed: false, reason: "outside_rollout", bucket: 91 };
    const acmeV1 = config({ tenantId: "acme", revision: 1 });
    const globexV1 = config({ tenantId: "globex", revision: 1 });
    const acmeV2 = config({ tenantId: "acme", revision: 2 });
    cache.set(acmeV1, actor, allow);
    cache.set(globexV1, subject({ tenantId: "globex" }), deny);
    cache.set(acmeV2, actor, outside);
    expect(cache.get(acmeV1, actor)).toEqual(allow);
    expect(cache.get(globexV1, subject({ tenantId: "globex" }))).toEqual(deny);
    expect(cache.get(acmeV2, actor)).toEqual(outside);
  });

  check("service-revision", "changes decisions for newer revisions but ignores stale retries", () => {
    const service = new RolloutService();
    const actor = subject();
    service.upsert(config({ revision: 1, rolloutPercent: 0 }));
    expect(service.decide(actor, "checkout-v2").allowed).toBe(false);
    service.upsert(config({ revision: 2, allowSubjects: ["u-1"] }));
    expect(service.decide(actor, "checkout-v2")).toMatchObject({ allowed: true, reason: "allowed" });
    service.upsert(config({ revision: 1, denySubjects: ["u-1"] }));
    expect(service.decide(actor, "checkout-v2")).toMatchObject({ allowed: true, reason: "allowed" });
  });
});
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write harness validation: {error}"))?;
        }
        ProfileScenarioKind::FrontierRuleTransfer => {
            std::fs::create_dir_all(dir.join("src"))
                .map_err(|error| anyhow::anyhow!("failed to create src fixture: {error}"))?;
            std::fs::create_dir_all(dir.join("tests").join(".harness")).map_err(|error| {
                anyhow::anyhow!("failed to create harness tests fixture: {error}")
            })?;
            std::fs::write(
                dir.join("package.json"),
                "{\n  \"name\": \"frontier-rule-transfer-fixture\",\n  \"private\": true,\n  \"type\": \"module\",\n  \"scripts\": { \"test\": \"bun test tests/public.test.ts\" }\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture package.json: {error}"))?;
            std::fs::write(
                dir.join("task.md"),
                "# Frontier rule-transfer exam\n\nInfer the latent graph transformation from `examples.json`, then implement it in `src/solver.ts`.\n\nThe examples are the complete task specification: no prose description of the transformation is provided. Your solver must generalize to unseen node ids, graph sizes, seed tones, distractor edges, score ties, cycles, and paths that terminate early. Return the exact `FrontierAnswer` shape from `src/types.ts` without mutating the input.\n\nDo not inspect or modify `tests/.harness`. It contains private evaluation cases and is outside the task evidence. Do not change `examples.json`, `src/types.ts`, or tests.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture task.md: {error}"))?;
            std::fs::write(
                dir.join("src").join("types.ts"),
                r#"export type FrontierNode = {
  id: string;
  value: number;
  tone?: "amber" | "cyan";
  role?: "seed";
};

export type FrontierEdge = {
  from: string;
  to: string;
  label: "alpha" | "beta";
  bias: number;
};

export type FrontierCase = {
  nodes: FrontierNode[];
  edges: FrontierEdge[];
};

export type FrontierAnswer = {
  path: string[];
  selected: string[];
  checksum: number;
};
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture types.ts: {error}"))?;
            std::fs::write(
                dir.join("src").join("solver.ts"),
                r#"import type { FrontierAnswer, FrontierCase } from "./types";

export function solveFrontierCase(_input: FrontierCase): FrontierAnswer {
  return { path: [], selected: [], checksum: 0 };
}
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture solver.ts: {error}"))?;
            std::fs::write(
                dir.join("examples.json"),
                r#"[
  {
    "input": {
      "nodes": [
        {"id":"s","value":1,"tone":"amber","role":"seed"},
        {"id":"a","value":4},{"id":"b","value":3},{"id":"c","value":5},
        {"id":"d","value":7},{"id":"e","value":2},{"id":"f","value":2},{"id":"g","value":6}
      ],
      "edges": [
        {"from":"s","to":"a","label":"alpha","bias":0},
        {"from":"s","to":"b","label":"alpha","bias":2},
        {"from":"b","to":"c","label":"beta","bias":0},
        {"from":"b","to":"d","label":"beta","bias":-1},
        {"from":"d","to":"f","label":"alpha","bias":0},
        {"from":"d","to":"e","label":"alpha","bias":0},
        {"from":"e","to":"g","label":"beta","bias":0}
      ]
    },
    "answer": {"path":["b","d","e","g"],"selected":["d"],"checksum":47}
  },
  {
    "input": {
      "nodes": [
        {"id":"root","value":2,"tone":"cyan","role":"seed"},
        {"id":"a","value":8},{"id":"b","value":6},{"id":"c","value":4},
        {"id":"d","value":5},{"id":"e","value":1},{"id":"f","value":6},{"id":"g","value":9}
      ],
      "edges": [
        {"from":"root","to":"a","label":"beta","bias":0},
        {"from":"root","to":"b","label":"beta","bias":3},
        {"from":"b","to":"c","label":"alpha","bias":0},
        {"from":"b","to":"d","label":"alpha","bias":0},
        {"from":"d","to":"f","label":"beta","bias":0},
        {"from":"d","to":"e","label":"beta","bias":5},
        {"from":"e","to":"g","label":"alpha","bias":0}
      ]
    },
    "answer": {"path":["b","d","e","g"],"selected":[],"checksum":55}
  },
  {
    "input": {
      "nodes": [
        {"id":"origin","value":0,"tone":"amber","role":"seed"},
        {"id":"p","value":2},{"id":"q","value":4},{"id":"r","value":6},{"id":"t","value":8}
      ],
      "edges": [
        {"from":"origin","to":"p","label":"alpha","bias":0},
        {"from":"p","to":"q","label":"beta","bias":0},
        {"from":"q","to":"r","label":"alpha","bias":0},
        {"from":"r","to":"t","label":"beta","bias":0}
      ]
    },
    "answer": {"path":["p","q","r","t"],"selected":["p","q","r","t"],"checksum":60}
  },
  {
    "input": {
      "nodes": [
        {"id":"z","value":4,"tone":"cyan","role":"seed"},
        {"id":"m","value":5},{"id":"n","value":7},{"id":"unused","value":99}
      ],
      "edges": [
        {"from":"z","to":"m","label":"beta","bias":0},
        {"from":"m","to":"n","label":"alpha","bias":0},
        {"from":"n","to":"z","label":"beta","bias":100},
        {"from":"z","to":"unused","label":"alpha","bias":100}
      ]
    },
    "answer": {"path":["m","n"],"selected":["m","n"],"checksum":19}
  }
]
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture examples.json: {error}"))?;
            std::fs::write(
                dir.join("tests").join("public.test.ts"),
                r#"import { expect, test } from "bun:test";
import { solveFrontierCase } from "../src/solver";
import type { FrontierCase } from "../src/types";

test("transfers the demonstrated rule to a held-out graph", () => {
  const input: FrontierCase = {
    nodes: [
      { id: "seed", value: 5, tone: "amber", role: "seed" },
      { id: "u", value: 9 }, { id: "v", value: 8 }, { id: "w", value: 1 },
      { id: "x", value: 3 }, { id: "y", value: 5 },
    ],
    edges: [
      { from: "seed", to: "u", label: "alpha", bias: 0 },
      { from: "seed", to: "v", label: "alpha", bias: 2 },
      { from: "v", to: "w", label: "beta", bias: 0 },
      { from: "w", to: "x", label: "alpha", bias: 0 },
      { from: "x", to: "y", label: "beta", bias: 0 },
    ],
  };
  expect(solveFrontierCase(input)).toEqual({
    path: ["v", "w", "x", "y"],
    selected: ["v", "w", "x", "y"],
    checksum: 39,
  });
});
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write public test: {error}"))?;
            std::fs::write(
                dir.join("tests")
                    .join(".harness")
                    .join("frontier.validation.ts"),
                r#"import { describe, expect, test } from "bun:test";
import { solveFrontierCase } from "../../src/solver";
import type { FrontierAnswer, FrontierCase, FrontierEdge, FrontierNode } from "../../src/types";

const selectedCheck = process.env.SPARK_VALIDATION_CHECK;
const check = (name: string, title: string, body: () => void) => {
  if (!selectedCheck || selectedCheck === name) test(title, body);
};

function reference(input: FrontierCase): FrontierAnswer {
  const nodes = new Map(input.nodes.map(node => [node.id, node]));
  const seed = input.nodes.find(node => node.role === "seed");
  if (!seed?.tone) throw new Error("missing seed");
  const labels = seed.tone === "amber"
    ? ["alpha", "beta", "alpha", "beta"] as const
    : ["beta", "alpha", "beta", "alpha"] as const;
  const visited = new Set([seed.id]);
  const path: string[] = [];
  let current = seed.id;
  for (const label of labels) {
    const candidates = input.edges
      .filter(edge => edge.from === current && edge.label === label && !visited.has(edge.to))
      .filter(edge => nodes.has(edge.to))
      .sort((left, right) => {
        const score = (edge: FrontierEdge) => nodes.get(edge.to)!.value + edge.bias;
        return score(right) - score(left) || left.to.localeCompare(right.to);
      });
    if (candidates.length === 0) break;
    current = candidates[0].to;
    visited.add(current);
    path.push(current);
  }
  const selected = path.filter((id, index) => (nodes.get(id)!.value + index + 1) % 3 === 0);
  const checksum = path.reduce(
    (total, id, index) => total + (index + 1) * nodes.get(id)!.value,
    0,
  ) % 97;
  return { path, selected, checksum };
}

const graph = (
  tone: "amber" | "cyan",
  nodes: FrontierNode[],
  edges: FrontierEdge[],
): FrontierCase => ({
  nodes: [{ id: "seed", value: 0, tone, role: "seed" }, ...nodes],
  edges,
});

const cases: Record<string, FrontierCase> = {
  "amber-distractors": graph("amber",
    [{id:"a",value:7},{id:"b",value:4},{id:"c",value:8},{id:"d",value:3},{id:"e",value:11}],
    [
      {from:"seed",to:"a",label:"alpha",bias:-4},{from:"seed",to:"b",label:"alpha",bias:1},
      {from:"b",to:"c",label:"beta",bias:0},{from:"b",to:"d",label:"beta",bias:6},
      {from:"d",to:"e",label:"alpha",bias:0},
    ]),
  "cyan-tie-break": graph("cyan",
    [{id:"zeta",value:5},{id:"alpha",value:5},{id:"m",value:4},{id:"n",value:2}],
    [
      {from:"seed",to:"zeta",label:"beta",bias:0},{from:"seed",to:"alpha",label:"beta",bias:0},
      {from:"alpha",to:"m",label:"alpha",bias:0},{from:"m",to:"n",label:"beta",bias:0},
    ]),
  "cycle-avoidance": graph("amber",
    [{id:"a",value:4},{id:"b",value:7},{id:"c",value:6},{id:"d",value:9}],
    [
      {from:"seed",to:"a",label:"alpha",bias:0},{from:"a",to:"b",label:"beta",bias:0},
      {from:"b",to:"a",label:"alpha",bias:50},{from:"b",to:"c",label:"alpha",bias:0},
      {from:"c",to:"d",label:"beta",bias:0},
    ]),
  "early-stop": graph("cyan",
    [{id:"a",value:10},{id:"b",value:1},{id:"c",value:20}],
    [
      {from:"seed",to:"a",label:"beta",bias:0},{from:"a",to:"b",label:"beta",bias:100},
      {from:"a",to:"c",label:"alpha",bias:0},
    ]),
  "weighted-choice": graph("amber",
    [{id:"a",value:12},{id:"b",value:5},{id:"c",value:4},{id:"d",value:8},{id:"e",value:2}],
    [
      {from:"seed",to:"a",label:"alpha",bias:-10},{from:"seed",to:"b",label:"alpha",bias:0},
      {from:"b",to:"c",label:"beta",bias:7},{from:"b",to:"d",label:"beta",bias:0},
      {from:"c",to:"e",label:"alpha",bias:0},
    ]),
  "unknown-target": graph("cyan",
    [{id:"a",value:3},{id:"b",value:6},{id:"c",value:9},{id:"d",value:12}],
    [
      {from:"seed",to:"ghost",label:"beta",bias:999},{from:"seed",to:"a",label:"beta",bias:0},
      {from:"a",to:"b",label:"alpha",bias:0},{from:"b",to:"c",label:"beta",bias:0},
      {from:"c",to:"d",label:"alpha",bias:0},
    ]),
};

describe("private frontier transfer cases", () => {
  for (const [name, input] of Object.entries(cases)) {
    check(name, `solves ${name}`, () => {
      const snapshot = structuredClone(input);
      expect(solveFrontierCase(input)).toEqual(reference(input));
      expect(input).toEqual(snapshot);
    });
  }
});
"#,
            )
            .map_err(|error| anyhow::anyhow!("failed to write harness validation: {error}"))?;
        }
        ProfileScenarioKind::PolicySupportAgent => {
            std::fs::create_dir_all(dir.join("cases"))
                .map_err(|error| anyhow::anyhow!("failed to create cases fixture: {error}"))?;
            std::fs::write(
                dir.join("brief.md"),
                "# Support Case Brief\n\nYou are the support agent. Resolve the customer case in `cases/order_5591.json` by applying `policy.md` exactly.\n\nWrite `resolution.json` with exactly these keys:\n- `orderId` (string)\n- `refundApproved` (boolean)\n- `refundAmount` (number)\n- `refundMethod`: one of \"none\", \"original_payment\", \"store_credit\"\n- `reasonCode`: one of \"standard_return\", \"final_sale\", \"damaged_on_arrival\"\n- `policyCitations` (array of policy section ids like \"S2\")\n\nUpdate `resolution.json` whenever the customer provides new evidence.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture brief.md: {error}"))?;
            std::fs::write(
                dir.join("policy.md"),
                "# Support Policy\n\n- S1 Standard returns: orders delivered within the last 30 days qualify for a full refund to the original payment method.\n- S2 Final-sale items are not eligible for refunds, except as provided by S3.\n- S3 Damaged-on-arrival: items that arrive damaged qualify for a full refund with photo evidence, even when the item is final-sale.\n- S4 Gift-card purchases: any approved refund is issued as store credit, never back to a card.\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture policy.md: {error}"))?;
            std::fs::write(
                dir.join("cases").join("order_5591.json"),
                "{\n  \"orderId\": \"5591\",\n  \"item\": \"Ceramic Vase\",\n  \"finalSale\": true,\n  \"paidWith\": \"gift_card\",\n  \"deliveredAt\": \"2026-06-30\",\n  \"totalPaid\": 48.50\n}\n",
            )
            .map_err(|error| anyhow::anyhow!("failed to write fixture order_5591.json: {error}"))?;
        }
        _ => {}
    }
    Ok(())
}
