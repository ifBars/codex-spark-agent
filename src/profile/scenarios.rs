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
        ProfileScenarioKind::ShellRecovery => Some("shell-recovery"),
        ProfileScenarioKind::PrecisePatch => Some("precise-patch"),
        ProfileScenarioKind::MultiFilePatch => Some("multi-file-patch"),
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
             3. Search src for tool and compaction surfaces using narrow search terms.\n\
             4. Do not recursively list src, do not search from the repository root, and do not read more than four src files unless a search result is ambiguous.\n\
             5. Finish with a concise harness-risk summary and one next profiling recommendation."
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
             2. Recover by using fs.read on .spark-scenarios/tool-recovery/source/note.md.\n\
             3. Verify it contains: Spark recovery path verified.\n\
             Finish with what failed, how you recovered, whether verification passed, and whether the harness observation made the correction clear."
                .to_string(),
        ]),
        ProfileScenarioKind::ShellRecovery => Ok(vec![
            "Profile scenario: shell-recovery.\n\
             Work only under .spark-scenarios/shell-recovery.\n\
             This scenario answers whether the harness can run shell commands, inspect stdout/stderr, and recover from an expected terminal error.\n\
             Required actions:\n\
             1. Use cmd.exec from .spark-scenarios/shell-recovery to run this intentionally wrong command: .\\scripts\\analyze-events.ps1 .\\data\\events.csv\n\
             2. Inspect the failure output, then use native tools to locate the corrected script path.\n\
             3. Use cmd.exec from .spark-scenarios/shell-recovery to run the corrected script and write its output to summary.txt.\n\
             4. Use fs.read on .spark-scenarios/shell-recovery/summary.txt and verify it contains total=5, failed=2, and top_service=payments.\n\
             Finish with the failed command, the corrected command, validation result, and whether the shell observation made recovery clear."
                .to_string(),
        ]),
        ProfileScenarioKind::PrecisePatch => Ok(vec![
            "Profile scenario: precise-patch.\n\
             Work only under .spark-scenarios/precise-patch.\n\
             This scenario answers whether the harness can make a precise code patch without over-editing unrelated branches.\n\
             Required actions:\n\
             1. Read .spark-scenarios/precise-patch/tests/status_map.spec.md.\n\
             2. Read .spark-scenarios/precise-patch/src/status_map.ts.\n\
             3. Use fs.edit or fs.replace to change only the queued branch so queued returns Queued.\n\
             4. The exact line return 'Unknown'; appears in more than one branch; do not replace that bare line globally. Either use line-scoped fs.edit on the queued branch, or use fs.replace with both case 'queued' and the return line in old and new so the branch label is preserved.\n\
             5. Use fs.search under .spark-scenarios/precise-patch/src for return 'Unknown'; and confirm the default branch still returns Unknown.\n\
             6. Use fs.read on .spark-scenarios/precise-patch/src/status_map.ts to verify the final contents.\n\
             Finish with the exact file changed, validation result, and whether any unrelated code was left untouched."
                .to_string(),
        ]),
        ProfileScenarioKind::MultiFilePatch => Ok(vec![
            "Profile scenario: multi-file-patch.\n\
             Work only under .spark-scenarios/multi-file-patch.\n\
             This scenario answers whether the harness can coordinate a small feature across multiple files correctly.\n\
             Required actions:\n\
             1. Read .spark-scenarios/multi-file-patch/src/routes.ts.\n\
             2. Read .spark-scenarios/multi-file-patch/src/navigation.ts.\n\
             3. Read .spark-scenarios/multi-file-patch/docs/routes.md.\n\
             4. Add a reports route with id reports and path /reports to routes.ts.\n\
             5. Add a Reports navigation item targeting routeId reports to navigation.ts.\n\
             6. Document /reports in docs/routes.md.\n\
             7. Use fs.search under .spark-scenarios/multi-file-patch for reports and /reports to verify all three files were updated.\n\
             Finish with the files changed, validation result, and whether the updates stayed consistent across code and docs."
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
             1. Use fs.read on src/profile/scenarios.rs.\n\
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
             The finished app will be checked by bun test and a harness-owned Playwright browser smoke check after your run finishes, so it must be runnable through Vite in a real browser.\n\
             Required actions:\n\
             1. Use fs.read on .spark-scenarios/react-calculator/brief.md.\n\
             2. Use fs.write to create .spark-scenarios/react-calculator/package.json.\n\
             3. Use fs.write to create .spark-scenarios/react-calculator/index.html.\n\
             4. Use fs.write to create .spark-scenarios/react-calculator/src/main.tsx.\n\
             5. Use fs.write to create .spark-scenarios/react-calculator/src/App.tsx.\n\
             6. Use fs.write to create .spark-scenarios/react-calculator/src/App.test.tsx.\n\
             7. Use fs.write to create .spark-scenarios/react-calculator/src/styles.css.\n\
             8. Use cmd.exec from .spark-scenarios/react-calculator to run bun test when possible; if tests need a DOM, configure it before using DOM-based test helpers.\n\
             9. Do not install Playwright, launch browsers, or start a long-lived Vite dev server yourself; the harness will run that browser smoke check externally.\n\
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
             7. Use cmd.exec from .spark-scenarios/rust-log-analyzer to run the CLI against sample.log when possible and verify it reports INFO/WARN/ERROR counts plus top error code E42.\n\
             Finish with the CLI behavior, test result, and any harness behavior that made project scaffolding easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::RustNotesTuiScaffold => Ok(vec![
            "Profile scenario: rust-notes-tui-scaffold.\n\
             Build a brand new Rust CLI project only under .spark-scenarios/rust-notes-tui.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             Required actions:\n\
             1. Use fs.read on .spark-scenarios/rust-notes-tui/brief.md.\n\
             2. Use fs.write to create .spark-scenarios/rust-notes-tui/Cargo.toml.\n\
             3. Use fs.write to create .spark-scenarios/rust-notes-tui/src/lib.rs.\n\
             4. Use fs.write to create .spark-scenarios/rust-notes-tui/src/main.rs.\n\
             5. Include focused tests for note parsing, storage, search, and export behavior.\n\
             6. Use cmd.exec from .spark-scenarios/rust-notes-tui to run cargo test.\n\
             7. Do not manually run the full add/list/search/export/help-keys smoke path; the harness will run validate-notes.ps1 after your run completes.\n\
             Finish with the CLI behavior, test result, and any harness behavior that made project scaffolding easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::GithubIssueBugfix => Ok(vec![
            "Profile scenario: github-issue-bugfix.\n\
             Work only under .spark-scenarios/github-issue-bugfix.\n\
             Treat issue.md like a GitHub issue assigned to you.\n\
             Required actions:\n\
             1. Read .spark-scenarios/github-issue-bugfix/issue.md.\n\
             2. Read .spark-scenarios/github-issue-bugfix/src/quote.ts.\n\
             3. Read .spark-scenarios/github-issue-bugfix/tests/quote.test.ts.\n\
             4. Patch the production code with the smallest reasonable change so annual quotes annualize before discounting.\n\
             5. Run bun test from .spark-scenarios/github-issue-bugfix after the patch. If you ran it before patching and it failed, run it again after patching and only finalize after the post-patch run passes.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::RustFailingTestBugfix => Ok(vec![
            "Profile scenario: rust-failing-test-bugfix.\n\
             Work only under .spark-scenarios/rust-failing-test-bugfix.\n\
             Treat issue.md like a Rust bug report assigned to you.\n\
             Required actions:\n\
             1. Read .spark-scenarios/rust-failing-test-bugfix/issue.md.\n\
             2. Read .spark-scenarios/rust-failing-test-bugfix/src/lib.rs.\n\
             3. Read .spark-scenarios/rust-failing-test-bugfix/tests/retry_scheduler.rs.\n\
             4. Patch production code with the smallest reasonable change so runnable jobs are filtered and ordered correctly.\n\
             5. Run cargo test from .spark-scenarios/rust-failing-test-bugfix after the patch. If you ran it before patching and it failed, run it again after patching and only finalize after the post-patch run passes. Do not set CARGO_TARGET_DIR.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::TypeScriptReducerBugfix => Ok(vec![
            "Profile scenario: typescript-reducer-bugfix.\n\
             Work only under .spark-scenarios/typescript-reducer-bugfix.\n\
             Treat issue.md like a TypeScript bug report assigned to you.\n\
             Required actions:\n\
             1. Read .spark-scenarios/typescript-reducer-bugfix/issue.md.\n\
             2. Read .spark-scenarios/typescript-reducer-bugfix/src/cart.ts.\n\
             3. Read .spark-scenarios/typescript-reducer-bugfix/tests/cart.test.ts.\n\
             4. Patch production code with the smallest reasonable change so inactive lines are ignored and non-positive quantities remove the line.\n\
             5. Run bun test from .spark-scenarios/typescript-reducer-bugfix after the patch. If you ran it before patching and it failed, run it again after patching and only finalize after the post-patch run passes.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::MergeConflictResolution => Ok(vec![
            "Profile scenario: merge-conflict-resolution.\n\
             Work only under .spark-scenarios/merge-conflict-resolution.\n\
             Treat issue.md like a merge conflict assigned to you.\n\
             Required actions:\n\
             1. Read .spark-scenarios/merge-conflict-resolution/issue.md.\n\
             2. Read .spark-scenarios/merge-conflict-resolution/src/featureFlags.ts and .spark-scenarios/merge-conflict-resolution/tests/featureFlags.test.ts.\n\
             3. Resolve the conflict markers in src/featureFlags.ts while preserving both dashboard-v2 and data-residency behavior.\n\
             4. Run bun test from .spark-scenarios/merge-conflict-resolution.\n\
             5. Read src/featureFlags.ts to verify no <<<<<<<, =======, or >>>>>>> markers remain.\n\
             Finish with the conflict resolution summary, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::GithubIssueTriage => Ok(vec![
            "Profile scenario: github-issue-triage.\n\
             Work only under .spark-scenarios/github-issue-triage.\n\
             Treat issue.md like a GitHub issue you are triaging, not fixing.\n\
             Required actions:\n\
             1. Read .spark-scenarios/github-issue-triage/issue.md.\n\
             2. Read .spark-scenarios/github-issue-triage/src/cachePolicy.ts.\n\
             3. Read .spark-scenarios/github-issue-triage/logs/warehouse-import.log.\n\
             4. Write .spark-scenarios/github-issue-triage/triage.md with likely root cause, evidence, reproduction steps, and fix plan.\n\
             5. Read triage.md to verify it names /api/items, src/cachePolicy.ts, Cache-Control, max-age=300, and stale-while-revalidate=30.\n\
             Finish with a concise triage summary and confidence level."
                .to_string(),
        ]),
        ProfileScenarioKind::CiFailureTriage => Ok(vec![
            "Profile scenario: ci-failure-triage.\n\
             Work only under .spark-scenarios/ci-failure-triage.\n\
             Triage the failing CI run and write a grounded diagnosis; do not modify source files.\n\
             Required actions:\n\
             1. Read .spark-scenarios/ci-failure-triage/issue.md.\n\
             2. Read .spark-scenarios/ci-failure-triage/.github/workflows/frontend.yml.\n\
             3. Read .spark-scenarios/ci-failure-triage/logs/frontend-tests.log.\n\
             4. Read .spark-scenarios/ci-failure-triage/src/discount.ts and .spark-scenarios/ci-failure-triage/tests/discount.test.ts.\n\
             5. Write .spark-scenarios/ci-failure-triage/ci-triage.md with the failing command, failing test/assertion, likely root cause, and minimal fix plan.\n\
             6. Read ci-triage.md to verify it names bun test, SAVE20, applyDiscount, src/discount.ts, and tests/discount.test.ts.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string(),
        ]),
        ProfileScenarioKind::PullRequestReview => Ok(vec![
            "Profile scenario: pull-request-review.\n\
             Work only under .spark-scenarios/pull-request-review.\n\
             Review the PR like a code reviewer; do not modify source files.\n\
             Required actions:\n\
             1. Read .spark-scenarios/pull-request-review/pr.md.\n\
             2. Read .spark-scenarios/pull-request-review/diff.patch.\n\
             3. Read .spark-scenarios/pull-request-review/src/checkout.ts and .spark-scenarios/pull-request-review/tests/checkout.test.ts.\n\
             4. Write .spark-scenarios/pull-request-review/review.md with severity, blocking finding, evidence, and a minimal test/fix recommendation.\n\
             5. Read review.md to verify it names read-only-admin, includes('admin'), discountFor, src/checkout.ts, and tests/checkout.test.ts.\n\
             Finish with the review path and whether source files were left unchanged."
                .to_string(),
        ]),
        ProfileScenarioKind::DependencyUpgradeTriage => Ok(vec![
            "Profile scenario: dependency-upgrade-triage.\n\
             Work only under .spark-scenarios/dependency-upgrade-triage.\n\
             Triage the dependency upgrade like a maintainer; do not modify source files.\n\
             Required actions:\n\
             1. Read .spark-scenarios/dependency-upgrade-triage/upgrade.md.\n\
             2. Read .spark-scenarios/dependency-upgrade-triage/package.json and .spark-scenarios/dependency-upgrade-triage/bun.lock.\n\
             3. Read .spark-scenarios/dependency-upgrade-triage/docs/time-utils-2.0.md.\n\
             4. Read .spark-scenarios/dependency-upgrade-triage/src/billingWindow.ts and .spark-scenarios/dependency-upgrade-triage/tests/billingWindow.test.ts.\n\
             5. Write .spark-scenarios/dependency-upgrade-triage/upgrade-triage.md with the changed package, migration risk, affected code, test gap, and minimal fix plan.\n\
             6. Read upgrade-triage.md to verify it names @acme/time-utils, 2.0.0, parseBusinessDate, zone: 'utc', src/billingWindow.ts, and tests/billingWindow.test.ts.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string(),
        ]),
        ProfileScenarioKind::TechnicalEssay => Ok(vec![
            "Profile scenario: technical-essay.\n\
             Work only under .spark-scenarios/technical-essay.\n\
             Write a sourced essay from the provided local notes; do not browse the web.\n\
             Required actions:\n\
             1. Read .spark-scenarios/technical-essay/brief.md.\n\
             2. Read all files under .spark-scenarios/technical-essay/sources.\n\
             3. Write .spark-scenarios/technical-essay/essay.md following the brief.\n\
             4. Read essay.md and verify it includes the title, section headings, and citations [S1], [S2], and [S3]. Use fs.read total_words for the word-count estimate; do not use cmd.exec just to count words.\n\
             Finish with the essay path, word-count estimate, and whether all citations are present."
                .to_string(),
        ]),
        ProfileScenarioKind::ConfigMigration => Ok(vec![
            "Profile scenario: config-migration.\n\
             Work only under .spark-scenarios/config-migration.\n\
             Required actions:\n\
             1. Read .spark-scenarios/config-migration/migration.md.\n\
             2. Read .spark-scenarios/config-migration/config/app.json.\n\
             3. Read .spark-scenarios/config-migration/src/config.ts.\n\
             4. Read .spark-scenarios/config-migration/docs/config.md.\n\
             5. Update all three files for schema version 2 using the new authentication/method object and maxAttempts retry field. Do not keep the old key names in rewritten docs or code.\n\
             6. Before finalizing, make an actual cmd.exec or fs.search tool call to validate the JSON is parseable and verify stale authMode/retry.retries references are gone from changed files. If this validation finds a stale reference and you edit again, rerun validation after the final edit before answering. If using cmd.exec from the scenario directory, use paths like config/app.json, src/config.ts, and docs/config.md rather than prefixing .spark-scenarios/config-migration again. If using PowerShell Select-String, do not combine these terms into one -SimpleMatch alternation; check each stale term separately or use a real regex match. Do not replace this tool call with a prose claim in the final answer.\n\
             Finish with files changed, validation result, and any migration risks."
                .to_string(),
        ]),
        ProfileScenarioKind::OpsReport => Ok(vec![
            "Profile scenario: ops-report.\n\
             Work only under .spark-scenarios/ops-report.\n\
             Required actions:\n\
             1. Read .spark-scenarios/ops-report/brief.md.\n\
             2. Read .spark-scenarios/ops-report/data/tickets.csv.\n\
             3. Compute total ticket rows excluding the CSV header, open tickets, open P1 tickets, and average minutes for open tickets.\n\
             4. Write .spark-scenarios/ops-report/metrics.json with totalTickets, openTickets, p1Open, and averageOpenMinutes.\n\
             5. Write .spark-scenarios/ops-report/report.md with a concise operational readout and the highest-risk team. Rank highest risk by open P1 count, then oldest open P1 age; do not count P2 tickets as P1 tickets.\n\
             6. Read both outputs to verify the numbers and narrative.\n\
             Finish with the computed metrics and risk summary."
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
             Keep this survey lean: complete the required evidence path, inspect only the top search hits needed to ground the answer, then stop.\n\
             1. List the repository root.\n\
             2. Read Cargo.toml and README.md.\n\
             3. Search src for tool surfaces with a narrow query such as \"builtin_tools\" or \"ToolDescriptor\".\n\
             4. Search src for compaction surfaces with a narrow query such as \"responses_compact\" or \"maybe_compact\".\n\
             5. Do not recursively list src, do not search from the repository root, and do not read more than four src files unless a search result is ambiguous.\n\
             6. Finish with a concise harness-risk summary and one next profiling recommendation."
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
             1. Read src/profile/scenarios.rs.\n\
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
             The finished app will be checked by bun test and a harness-owned Playwright browser smoke check after your run finishes, so it must be runnable through Vite in a real browser.\n\
             Do not install Playwright, launch browsers, or start a long-lived Vite dev server yourself; the harness owns that browser smoke check.\n\
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
             9. Ensure the Vite entrypoint is browser-runnable so the harness smoke check can click 1 + 2 = and observe 3.\n\
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
             7. Run the CLI against sample.log when possible and verify it reports INFO/WARN/ERROR counts plus top error code E42.\n\
             Finish with the CLI behavior, test result, and any agent behavior that made project scaffolding easier or harder."
                .to_string()
        }
        ProfileScenarioKind::RustNotesTuiScaffold => {
            "Benchmark scenario: rust-notes-tui-scaffold.\n\
             Build a brand new Rust CLI project only under .spark-scenarios/rust-notes-tui.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             This is a scoped fixture task: start with the listed brief and do not survey unrelated repository files unless a concrete blocker requires it.\n\
             The app should feel like a vim-style notes tool while remaining scriptable for validation.\n\
             On Windows, run validation commands separately rather than chaining them with &&.\n\
             Required actions:\n\
             1. Read .spark-scenarios/rust-notes-tui/brief.md.\n\
             2. Create .spark-scenarios/rust-notes-tui/Cargo.toml.\n\
             3. Create .spark-scenarios/rust-notes-tui/src/lib.rs.\n\
             4. Create .spark-scenarios/rust-notes-tui/src/main.rs.\n\
             5. Implement `--store <path> add <title> <body...>`, `list`, `search <query>`, `export <path>`, and `help-keys`.\n\
             6. Include focused tests for note parsing/storage/search/export behavior.\n\
             7. Run cargo test for the nested project.\n\
             8. Do not manually run the full add/list/search/export/help-keys smoke path; the harness will run .spark-scenarios/rust-notes-tui/validate-notes.ps1 after your run completes.\n\
             Finish with the CLI behavior, test result, and any agent behavior that made project scaffolding easier or harder."
                .to_string()
        }
        ProfileScenarioKind::GithubIssueBugfix => {
            "Benchmark scenario: github-issue-bugfix.\n\
             Work only under .spark-scenarios/github-issue-bugfix.\n\
             Treat issue.md like a GitHub issue assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read .spark-scenarios/github-issue-bugfix/issue.md.\n\
             2. Inspect the production code and tests under .spark-scenarios/github-issue-bugfix.\n\
             3. Patch production code with the smallest reasonable change so annual quotes annualize before discounting.\n\
             4. Run bun test from .spark-scenarios/github-issue-bugfix.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::RustFailingTestBugfix => {
            "Benchmark scenario: rust-failing-test-bugfix.\n\
             Work only under .spark-scenarios/rust-failing-test-bugfix.\n\
             Treat issue.md like a Rust bug report assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             Required actions:\n\
             1. Read .spark-scenarios/rust-failing-test-bugfix/issue.md.\n\
             2. Inspect the production code and tests under .spark-scenarios/rust-failing-test-bugfix.\n\
             3. Patch production code with the smallest reasonable change so runnable jobs filter blank ids and sort higher priority values first.\n\
             4. Run cargo test from .spark-scenarios/rust-failing-test-bugfix.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::TypeScriptReducerBugfix => {
            "Benchmark scenario: typescript-reducer-bugfix.\n\
             Work only under .spark-scenarios/typescript-reducer-bugfix.\n\
             Treat issue.md like a TypeScript bug report assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Use bun for JavaScript package management and validation.\n\
             Required actions:\n\
             1. Read .spark-scenarios/typescript-reducer-bugfix/issue.md.\n\
             2. Inspect the production code and tests under .spark-scenarios/typescript-reducer-bugfix.\n\
             3. Patch production code with the smallest reasonable change so inactive lines are ignored by subtotal and non-positive quantities remove the line.\n\
             4. Run bun test from .spark-scenarios/typescript-reducer-bugfix.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::MergeConflictResolution => {
            "Benchmark scenario: merge-conflict-resolution.\n\
             Work only under .spark-scenarios/merge-conflict-resolution.\n\
             Treat issue.md like a merge conflict assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Use bun for JavaScript validation.\n\
             Required actions:\n\
             1. Read issue.md, src/featureFlags.ts, and tests/featureFlags.test.ts.\n\
             2. Resolve the conflict markers in src/featureFlags.ts with the smallest reasonable edit.\n\
             3. Preserve dashboard-v2 for enterprise accounts and beta tenants, and preserve data-residency for EU accounts.\n\
             4. Run bun test from .spark-scenarios/merge-conflict-resolution.\n\
             5. Verify src/featureFlags.ts no longer contains <<<<<<<, =======, or >>>>>>>.\n\
             Finish with the conflict resolution summary, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::GithubIssueTriage => {
            "Benchmark scenario: github-issue-triage.\n\
             Work only under .spark-scenarios/github-issue-triage.\n\
             Treat issue.md like a GitHub issue you are triaging, not fixing. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read .spark-scenarios/github-issue-triage/issue.md.\n\
             2. Inspect the local source and log evidence under .spark-scenarios/github-issue-triage.\n\
             3. Write .spark-scenarios/github-issue-triage/triage.md with likely root cause, evidence, reproduction steps, and fix plan.\n\
             4. Verify triage.md names /api/items, src/cachePolicy.ts, Cache-Control, max-age=300, and stale-while-revalidate=30.\n\
             Finish with a concise triage summary and confidence level."
                .to_string()
        }
        ProfileScenarioKind::CiFailureTriage => {
            "Benchmark scenario: ci-failure-triage.\n\
             Work only under .spark-scenarios/ci-failure-triage.\n\
             Triage the failing CI run and write a grounded diagnosis; do not modify source files or inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read issue.md, .github/workflows/frontend.yml, logs/frontend-tests.log, src/discount.ts, and tests/discount.test.ts.\n\
             2. Write ci-triage.md with the failing command, failing test/assertion, likely root cause, and minimal fix plan.\n\
             3. Identify the SAVE20 path in applyDiscount as the likely production gap.\n\
             4. Verify ci-triage.md names bun test, SAVE20, applyDiscount, src/discount.ts, and tests/discount.test.ts.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string()
        }
        ProfileScenarioKind::PullRequestReview => {
            "Benchmark scenario: pull-request-review.\n\
             Work only under .spark-scenarios/pull-request-review.\n\
             Review the PR like a code reviewer; do not modify source files or inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read pr.md, diff.patch, src/checkout.ts, and tests/checkout.test.ts.\n\
             2. Write review.md with severity, blocking finding, evidence, and a minimal test/fix recommendation.\n\
             3. Identify that role.includes('admin') lets read-only-admin users receive a full comp discount even though the product rule allows only role exactly admin.\n\
             4. Verify review.md names read-only-admin, includes('admin'), discountFor, src/checkout.ts, and tests/checkout.test.ts.\n\
             Finish with the review path and whether source files were left unchanged."
                .to_string()
        }
        ProfileScenarioKind::DependencyUpgradeTriage => {
            "Benchmark scenario: dependency-upgrade-triage.\n\
             Work only under .spark-scenarios/dependency-upgrade-triage.\n\
             Triage the dependency upgrade like a maintainer; do not modify source files or inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read upgrade.md, package.json, bun.lock, docs/time-utils-2.0.md, src/billingWindow.ts, and tests/billingWindow.test.ts.\n\
             2. Write upgrade-triage.md with the changed package, migration risk, affected code, test gap, and minimal fix plan.\n\
             3. Identify that @acme/time-utils 2.0.0 changed parseBusinessDate date-only defaults from UTC to local time, so src/billingWindow.ts should pass { zone: 'utc' } to preserve billing cutoff behavior.\n\
             4. Verify upgrade-triage.md names @acme/time-utils, 2.0.0, parseBusinessDate, zone: 'utc', src/billingWindow.ts, and tests/billingWindow.test.ts.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string()
        }
        ProfileScenarioKind::TechnicalEssay => {
            "Benchmark scenario: technical-essay.\n\
             Work only under .spark-scenarios/technical-essay.\n\
             Write a sourced essay from the provided local notes; do not browse the web and do not inspect unrelated repository files.\n\
             Required actions:\n\
             1. Read .spark-scenarios/technical-essay/brief.md.\n\
             2. Read all local source notes under .spark-scenarios/technical-essay/sources.\n\
             3. Write .spark-scenarios/technical-essay/essay.md following the brief.\n\
             4. Verify essay.md includes the title, section headings, and citations [S1], [S2], and [S3].\n\
             Finish with the essay path, word-count estimate, and whether all citations are present."
                .to_string()
        }
        ProfileScenarioKind::ConfigMigration => {
            "Benchmark scenario: config-migration.\n\
             Work only under .spark-scenarios/config-migration.\n\
             Required actions:\n\
             1. Read .spark-scenarios/config-migration/migration.md plus the JSON, TypeScript, and docs files in that fixture.\n\
             2. Update config/app.json, src/config.ts, and docs/config.md for schema version 2 using the new authentication/method object and maxAttempts retry field. Do not keep the old key names in rewritten docs or code.\n\
             3. Before finalizing, make an actual cmd.exec or fs.search tool call to validate the JSON is parseable and verify stale authMode/retry.retries references are gone. If this validation finds a stale reference and you edit again, rerun validation after the final edit before answering. If using cmd.exec from the scenario directory, use paths like config/app.json, src/config.ts, and docs/config.md rather than prefixing .spark-scenarios/config-migration again. If using PowerShell Select-String, do not combine these terms into one -SimpleMatch alternation; check each stale term separately or use a real regex match. Do not replace this tool call with a prose claim in the final answer.\n\
             Finish with files changed, validation result, and any migration risks."
                .to_string()
        }
        ProfileScenarioKind::OpsReport => {
            "Benchmark scenario: ops-report.\n\
             Work only under .spark-scenarios/ops-report.\n\
             Analyze data/tickets.csv and produce both machine-checkable metrics and a concise narrative. Treat the first CSV line as the header, not a ticket.\n\
             Required actions:\n\
             1. Read .spark-scenarios/ops-report/brief.md.\n\
             2. Read .spark-scenarios/ops-report/data/tickets.csv.\n\
             3. Write .spark-scenarios/ops-report/metrics.json with totalTickets, openTickets, p1Open, and averageOpenMinutes.\n\
             4. Write .spark-scenarios/ops-report/report.md with the operational readout and highest-risk team. Rank highest risk by open P1 count, then oldest open P1 age; do not count P2 tickets as P1 tickets.\n\
             5. Verify both outputs before finishing.\n\
             Finish with the computed metrics and risk summary."
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
        ProfileScenarioKind::RustNotesTuiScaffold => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/rust-notes-tui",
            program: "powershell",
            args: &["-NoProfile", "-File", "validate-notes.ps1"],
        }),
        ProfileScenarioKind::GithubIssueBugfix => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/github-issue-bugfix",
            program: "bun",
            args: &["test"],
        }),
        ProfileScenarioKind::RustFailingTestBugfix => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/rust-failing-test-bugfix",
            program: "cargo",
            args: &["test"],
        }),
        ProfileScenarioKind::TypeScriptReducerBugfix => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/typescript-reducer-bugfix",
            program: "bun",
            args: &["test"],
        }),
        ProfileScenarioKind::MergeConflictResolution => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/merge-conflict-resolution",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $source = Get-Content -LiteralPath 'src/featureFlags.ts' -Raw; foreach ($marker in @('<<<<<<<','=======','>>>>>>>')) { if ($source -like \"*$marker*\") { throw \"unresolved conflict marker $marker\" } }; foreach ($term in @('dashboard-v2','data-residency','startsWith(''beta-'')','region === ''eu''')) { if ($source -notlike \"*$term*\") { throw \"missing $term\" } }; bun test",
            ],
        }),
        ProfileScenarioKind::GithubIssueTriage => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/github-issue-triage",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'triage.md' -Raw; foreach ($term in @('/api/items','Cache-Control','max-age=300','stale-while-revalidate=30','src/cachePolicy.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }",
            ],
        }),
        ProfileScenarioKind::CiFailureTriage => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/ci-failure-triage",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'ci-triage.md' -Raw; foreach ($term in @('bun test','SAVE20','applyDiscount','src/discount.ts','tests/discount.test.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch '(?i)(Expected:\\s*80|expected\\s+80)') { throw 'missing expected 80 assertion evidence' }; if ($content -notmatch '(?i)(Received:\\s*100|received\\s+100)') { throw 'missing received 100 assertion evidence' }",
            ],
        }),
        ProfileScenarioKind::PullRequestReview => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/pull-request-review",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'review.md' -Raw; foreach ($term in @('read-only-admin','discountFor','src/checkout.ts','tests/checkout.test.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch \"includes\\s*\\(\\s*[''`\"]admin[''`\"]\\s*\\)\") { throw 'missing includes admin evidence' }; if ($content -notmatch '(?i)(blocking|must fix|p1|p0)') { throw 'missing blocking severity' }; if ($content -notmatch '(?i)(exactly\\s+admin|role\\s+exactly\\s+admin|===\\s*[''`\"]admin[''`\"]|==\\s*[''`\"]admin[''`\"]|strict equality)') { throw 'missing exact admin fix recommendation' }",
            ],
        }),
        ProfileScenarioKind::DependencyUpgradeTriage => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/dependency-upgrade-triage",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'upgrade-triage.md' -Raw; foreach ($term in @('@acme/time-utils','2.0.0','parseBusinessDate','src/billingWindow.ts','tests/billingWindow.test.ts')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; if ($content -notmatch '(?i)\\bUTC\\b') { throw 'missing UTC risk' }; if ($content -notmatch '(?i)\\blocal\\b') { throw 'missing local timezone change' }; if ($content -notmatch \"zone\\s*:\\s*[''`\"]utc[''`\"]\") { throw 'missing zone utc fix' }; if ($content -notmatch '(?i)(test gap|missing test|add.*test|regression test)') { throw 'missing test gap recommendation' }",
            ],
        }),
        ProfileScenarioKind::TechnicalEssay => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/technical-essay",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'essay.md' -Raw; foreach ($term in @('Operational Visibility Is a Product Feature','[S1]','[S2]','[S3]')) { if ($content -notlike \"*$term*\") { throw \"missing $term\" } }; $words = @($content -split '\\s+' | Where-Object { $_ }); if ($words.Count -lt 350) { throw \"essay too short: $($words.Count) words\" }; $headings = @($content -split \"`r?`n\" | Where-Object { $_ -like '## *' }); if ($headings.Count -lt 2) { throw 'missing section headings' }",
            ],
        }),
        ProfileScenarioKind::ConfigMigration => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/config-migration",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $json = Get-Content -LiteralPath 'config/app.json' -Raw | ConvertFrom-Json; if ($json.schemaVersion -ne 2) { throw 'schemaVersion not 2' }; if ($json.authentication.method -ne 'password') { throw 'authentication.method not preserved' }; if ($json.retry.maxAttempts -ne 3) { throw 'retry.maxAttempts not migrated' }; if ($json.retry.backoffMs -ne 250) { throw 'retry.backoffMs not preserved' }; $all = (Get-Content -LiteralPath 'src/config.ts' -Raw) + (Get-Content -LiteralPath 'docs/config.md' -Raw) + (Get-Content -LiteralPath 'config/app.json' -Raw); foreach ($term in @('authMode','retries: number','retry.retries')) { if ($all -like \"*$term*\") { throw \"stale term $term\" } }; foreach ($term in @('authentication','method','maxAttempts','schemaVersion: 2')) { if ($all -notlike \"*$term*\") { throw \"missing $term\" } }",
            ],
        }),
        ProfileScenarioKind::OpsReport => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/ops-report",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $metrics = Get-Content -LiteralPath 'metrics.json' -Raw | ConvertFrom-Json; if ($metrics.totalTickets -ne 8) { throw 'totalTickets must be 8' }; if ($metrics.openTickets -ne 5) { throw 'openTickets must be 5' }; if ($metrics.p1Open -ne 2) { throw 'p1Open must be 2' }; if ([math]::Abs([double]$metrics.averageOpenMinutes - 51.4) -gt 0.01) { throw 'averageOpenMinutes must be 51.4' }; $report = Get-Content -LiteralPath 'report.md' -Raw; $plain = (($report -replace '[*`#_]', '') -replace '\\s+', ' ').Trim(); if ($plain -notmatch '(?i)(highest-risk team\\s*(:|-|is)?\\s*billing|billing\\s+(is\\s+)?(the\\s+)?highest-risk team|billing\\s+team\\s+is\\s+highest\\s+risk)') { throw 'report must identify billing as highest-risk team' }; if ($plain -match '(?i)(highest-risk team\\s*(:|-|is)?\\s*api|api\\s+(is\\s+)?(the\\s+)?highest-risk team|api\\s+team\\s+is\\s+highest\\s+risk)') { throw 'report incorrectly identifies api as highest-risk team' }; if ($plain -notmatch '95') { throw 'report must explain billing risk with the 95 minute open P1 age' }",
            ],
        }),
        ProfileScenarioKind::ShellRecovery => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/shell-recovery",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $summary = Get-Content -LiteralPath 'summary.txt' -Raw; if ($summary -notmatch 'total=5') { throw 'missing total=5' }; if ($summary -notmatch 'failed=2') { throw 'missing failed=2' }; if ($summary -notmatch 'top_service=payments') { throw 'missing top_service=payments' }",
            ],
        }),
        ProfileScenarioKind::PrecisePatch => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/precise-patch",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $content = Get-Content -LiteralPath 'src/status_map.ts' -Raw; if ($content -notmatch \"case 'queued':[\\s\\S]*return 'Queued';\") { throw 'queued branch was not patched' }; if ($content -notmatch \"default:[\\s\\S]*return 'Unknown';\") { throw 'default branch changed' }; if (($content | Select-String \"return 'Queued';\" -AllMatches).Matches.Count -ne 1) { throw 'queued label was over-applied' }",
            ],
        }),
        ProfileScenarioKind::MultiFilePatch => Some(ProfileScenarioValidationCommand {
            workdir: ".spark-scenarios/multi-file-patch",
            program: "powershell",
            args: &[
                "-NoProfile",
                "-Command",
                "$ErrorActionPreference='Stop'; $routes = Get-Content -LiteralPath 'src/routes.ts' -Raw; $nav = Get-Content -LiteralPath 'src/navigation.ts' -Raw; $docs = Get-Content -LiteralPath 'docs/routes.md' -Raw; if ($routes -notmatch \"id: 'reports'\" -or $routes -notmatch \"path: '/reports'\") { throw 'routes.ts missing reports route' }; if ($nav -notmatch \"label: 'Reports'\" -or $nav -notmatch \"routeId: 'reports'\") { throw 'navigation.ts missing Reports item' }; if ($docs -notmatch '/reports') { throw 'docs missing /reports' }",
            ],
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
                "path": "src/profile/scenarios.rs",
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
        _ => Vec::new(),
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
