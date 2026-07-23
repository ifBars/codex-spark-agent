use std::path::Path;

use anyhow::Result;

use crate::cli::ProfileScenarioKind;

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
        ProfileScenarioKind::MultiModuleBugfix => Some("multi-module-bugfix"),
        ProfileScenarioKind::TerminalRepair => Some("terminal-repair"),
        ProfileScenarioKind::MultiHopAnalysis => Some("multi-hop-analysis"),
        ProfileScenarioKind::PolicySupportAgent => Some("policy-support-agent"),
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
