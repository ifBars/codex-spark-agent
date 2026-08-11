use anyhow::Result;

use crate::{
    APPROX_CHARS_PER_TOKEN, DEFAULT_COMPACT_AFTER_CHARS, MAX_SCENARIO_TARGET_TOKENS,
    cli::ProfileScenarioKind,
};

fn exploration_scenario_prompts(scenario: ProfileScenarioKind) -> Option<Vec<String>> {
    let (name, codebase, tasks) = match scenario {
        ProfileScenarioKind::AssetRipperExploration => (
            "asset-ripper-exploration",
            "the Schedule I AssetRipper export",
            [
                "Orient yourself in the exported Unity project. Inspect ProjectSettings.asset, the top-level Assets layout, and the Assembly-CSharp script tree. Build a compact evidence ledger that distinguishes project metadata, recovered scripts, and serialized assets.",
                "Trace ScheduleOne.Product.ProductManager from its declaration into its networking, persistence, and product-registration collaborators. Follow targeted symbols into the smallest useful set of files, and record the concrete call/data path plus path evidence.",
                "Challenge the trace by finding serialized asset or prefab evidence connected to one product-facing type or field. Explain what the YAML/script link does prove, what it does not prove about runtime behavior, and any ambiguity introduced by exported or recovered artifacts.",
                "Synthesize the findings for an engineer new to this export. Explain the project shape, the ProductManager-centered runtime/data flow, and the boundary between confirmed evidence and inference. Cite at least six specific paths spanning settings, scripts, and serialized assets; include residual unknowns and the next two highest-value checks.",
            ],
        ),
        ProfileScenarioKind::FiveMExploration => (
            "fivem-exploration",
            "the Cfx.re/FiveM codebase",
            [
                "Orient yourself in the repository. Read README.md, inspect code and code/components without recursively dumping them, and identify how product targets, shared infrastructure, components, ext, and vendor content are separated. Return a compact evidence ledger.",
                "Trace one concrete FXServer startup or server-instance path from an entry/registration seam into the components that own resources, networking, or scripting. Prefer symbol searches and bounded reads; record the call/ownership chain with path evidence.",
                "Challenge that architecture map by tracing one client/server or native/scripting boundary and the build or component-registration mechanism that connects it. Separate confirmed links from naming-based inference and note platform/product variants.",
                "Synthesize an explanation for a new contributor: repository topology, component composition, the selected server flow, the selected boundary, and where an extension or debugging change would belong. Cite at least six specific paths from distinct areas, distinguish facts/inferences/unknowns, and name the next two checks.",
            ],
        ),
        ProfileScenarioKind::Cpp2IlExploration => (
            "cpp2il-exploration",
            "the Cpp2IL codebase",
            [
                "Orient yourself in the solution. Read README.md and Cpp2IL.slnx, inspect the CLI, Cpp2IL.Core, LibCpp2IL, plugins, and tests, and summarize the responsibility of each layer in a compact evidence ledger.",
                "Trace the input pipeline from Cpp2IL/Program.cs path resolution through core/plugin orchestration to LibCpp2IL context initialization. Follow concrete symbols and record inputs, ownership transitions, and path evidence.",
                "Trace how binary plus metadata information becomes recovered type/method analysis and reaches an output or plugin seam. Identify assumptions, version/platform branches, and failure surfaces; verify claims against implementation rather than README text alone.",
                "Synthesize an explanation for a tool integrator: architecture, end-to-end analysis flow, extension points, major failure boundaries, and where Cpp2IL stops relative to downstream interop generation. Cite at least six specific paths across CLI, core, library, plugin, and test areas; distinguish facts/inferences/unknowns and name two next checks.",
            ],
        ),
        ProfileScenarioKind::Il2CppInteropExploration => (
            "il2cpp-interop-exploration",
            "the Il2CppInterop codebase",
            [
                "Orient yourself in the solution. Read README.md and Il2CppInterop.sln, inspect CLI, Generator, Runtime, Common, HarmonySupport, and documentation, and summarize their responsibilities in a compact evidence ledger.",
                "Trace the generate command from Il2CppInterop.CLI/Program.cs through generator options/runners to emitted interop assemblies. Follow concrete symbols, identify inputs and outputs, and record the path evidence.",
                "Trace one managed-wrapper-to-native-IL2CPP runtime path, including object or method invocation machinery and one integration seam such as class injection or Harmony support. Verify the boundary in code and documentation, and record uncertainty.",
                "Synthesize an explanation for a plugin/tool author: how Cpp2IL output is consumed, generator versus runtime responsibilities, the selected invocation path, optional integration layers, and likely failure boundaries. Cite at least six specific paths across CLI, generator, runtime, and docs; distinguish facts/inferences/unknowns and name two next checks.",
            ],
        ),
        _ => return None,
    };

    Some(
        tasks
            .into_iter()
            .enumerate()
            .map(|(index, task)| {
                format!(
                    "Benchmark scenario: {name}, task subset {}/4.\n\
                     Explore {codebase} through the single read-only reference root supplied in the environment context.\n\
                     This is strictly read-only: use only fs.list, fs.read, fs.search, and fs.stat. Do not call cmd.exec and do not write, edit, rename, or delete anything.\n\
                     Keep reads bounded and searches targeted; do not recursively dump the codebase.\n\
                     {task}",
                    index + 1
                )
            })
            .collect(),
    )
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

    if let Some(prompts) = exploration_scenario_prompts(scenario) {
        return Ok(prompts);
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
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.read on notes.md.\n\
             2. Use fs.edit or fs.replace on notes.md to replace the TODO line with: Final note: Spark edited this fixture with native tools.\n\
             3. Use fs.write on summary.txt with a one-line summary of what changed.\n\
             4. Use fs.read on both changed files to verify the final contents.\n\
             Finish with the tools used, whether verification passed, and any harness behavior that made the task easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::FileOps => Ok(vec![
            "Profile scenario: file-ops.\n\
             Use native file tools, not cmd.exec, unless verification cannot be done otherwise.\n\
             Required actions:\n\
             1. Use fs.write on drafts/report-draft.md with a short markdown report containing the exact phrase: Spark rename path verified.\n\
             2. Use fs.rename to move drafts/report-draft.md to final/report.md.\n\
             3. Use fs.stat on final/report.md to verify the final path exists before reading it.\n\
             4. Use fs.read on final/report.md to verify the final contents.\n\
             5. Use fs.search in cwd for Spark rename path verified.\n\
             Finish with the native tools used, whether verification passed, and any harness behavior that made the workflow easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::ToolRecovery => Ok(vec![
            "Profile scenario: tool-recovery.\n\
             Use native file tools, not cmd.exec.\n\
             Required actions:\n\
             1. First use fs.read on source/missing-note.md. This path is intentionally missing; do not skip this failing probe.\n\
             2. Recover by using fs.read on source/note.md.\n\
             3. Verify it contains: Spark recovery path verified.\n\
             Finish with what failed, how you recovered, whether verification passed, and whether the harness observation made the correction clear."
                .to_string(),
        ]),
        ProfileScenarioKind::ShellRecovery => Ok(vec![
            "Profile scenario: shell-recovery.\n\
             This scenario answers whether the harness can run shell commands, inspect stdout/stderr, and recover from an expected terminal error.\n\
             Required actions:\n\
             1. Use cmd.exec from cwd to run this intentionally wrong command: .\\scripts\\analyze-events.ps1 .\\data\\events.csv\n\
             2. Inspect the failure output, then use native tools to locate the corrected script path.\n\
             3. Use cmd.exec from cwd to run the corrected script and write its output to summary.txt.\n\
             4. Use fs.read on summary.txt and verify it contains total=5, failed=2, and top_service=payments.\n\
             Finish with the failed command, the corrected command, validation result, and whether the shell observation made recovery clear."
                .to_string(),
        ]),
        ProfileScenarioKind::PrecisePatch => Ok(vec![
            "Profile scenario: precise-patch.\n\
             This scenario answers whether the harness can make a precise code patch without over-editing unrelated branches.\n\
             Required actions:\n\
             1. Read tests/status_map.spec.md.\n\
             2. Read src/status_map.ts.\n\
             3. Use fs.edit or fs.replace to change only the queued branch so queued returns Queued.\n\
             4. The exact line return 'Unknown'; appears in more than one branch; do not replace that bare line globally. Either use line-scoped fs.edit on the queued branch, or use fs.replace with both case 'queued' and the return line in old and new so the branch label is preserved.\n\
             5. Use fs.search under src for return 'Unknown'; and confirm the default branch still returns Unknown.\n\
             6. Use fs.read on src/status_map.ts to verify the final contents.\n\
             Finish with the exact file changed, validation result, and whether any unrelated code was left untouched."
                .to_string(),
        ]),
        ProfileScenarioKind::MultiFilePatch => Ok(vec![
            "Profile scenario: multi-file-patch.\n\
             This scenario answers whether the harness can coordinate a small feature across multiple files correctly.\n\
             Required actions:\n\
             1. Read src/routes.ts.\n\
             2. Read src/navigation.ts.\n\
             3. Read docs/routes.md.\n\
             4. Add a reports route with id reports and path /reports to routes.ts.\n\
             5. Add a Reports navigation item targeting routeId reports to navigation.ts.\n\
             6. Document /reports in docs/routes.md.\n\
             7. Use fs.search in cwd for reports and /reports to verify all three files were updated.\n\
             Finish with the files changed, validation result, and whether the updates stayed consistent across code and docs."
                .to_string(),
        ]),
        ProfileScenarioKind::ManifestContractWrite => Ok(vec![
            "Profile scenario: manifest-contract-write.\n\
             This scenario tests whether the harness can write two exact, mutually consistent release artifacts from supplied source data.\n\
             Use native file tools only; do not call cmd.exec.\n\
             Required actions:\n\
             1. Read brief.md and data/releases.json.\n\
             2. Use fs.write to create generated/release-manifest.json. It must be valid JSON with exactly these top-level keys: channel, currentVersion, previousVersion, artifacts, checksum. The values must come from the approved release in releases.json; artifacts must retain source order; checksum is the approved release's sha256.\n\
             3. Use fs.write to create generated/release-notes.md. It must have the heading `# Spark 1.4.0`, list both artifact filenames as bullets, and state `SHA-256: 9c2f8a1d`. Do not mention the rejected candidate release.\n\
             4. Use fs.read on both generated files to verify exact values, JSON shape, and that the notes agree with the manifest.\n\
             Finish with the files written and validation result."
                .to_string(),
        ]),
        ProfileScenarioKind::ScopedPolicyPatch => Ok(vec![
            "Profile scenario: scoped-policy-patch.\n\
             This scenario tests whether the harness can patch the requested policy branch without changing a lookalike safety branch.\n\
             Use native file tools only; do not call cmd.exec.\n\
             Required actions:\n\
             1. Read tests/rate_limit.spec.md and src/rate_limit.ts.\n\
             2. Change only `canRetryPayment` so that an inactive account returns false before any retry-count check. Preserve `isRetryLimitExceeded` exactly; it intentionally contains a similar inactive-account branch with the opposite outcome.\n\
             3. Do not globally replace `return true;`, do not rename exports, and do not modify tests.\n\
             4. Use fs.search under src for `account.active` and use fs.read on src/rate_limit.ts to verify that only canRetryPayment changed.\n\
             Finish with the exact file changed and validation result."
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
              1. Use fs.read on src/profile/scenarios/prompts.rs.\n\
              2. Use fs.read on src/profiler/analyze/expectations.rs.\n\
             3. Use fs.search under README.md for profile-scenario.\n\
             4. Use fs.search under src for expected_tool_calls.\n\
             Finish with a prioritized benchmark plan containing three concrete new task prompts, expected evidence signals, and which existing scenarios they should be compared against."
                .to_string(),
        ]),
        ProfileScenarioKind::ReactCalculatorScaffold => Ok(vec![
            "Profile scenario: react-calculator-scaffold.\n\
             Build a brand new React + TypeScript calculator app only in cwd.\n\
             Use bun for JavaScript package management. Do not create files outside this ignored fixture folder.\n\
             The finished app will be checked by bun test and a harness-owned Playwright browser smoke check after your run finishes, so it must be runnable through Vite in a real browser.\n\
             Required actions:\n\
             1. Use fs.read on brief.md.\n\
             2. Use fs.write to create package.json.\n\
             3. Use fs.write to create index.html.\n\
             4. Use fs.write to create src/main.tsx.\n\
             5. Use fs.write to create src/App.tsx.\n\
             6. Use fs.write to create src/App.test.tsx.\n\
             7. Use fs.write to create src/styles.css.\n\
             8. Use cmd.exec from cwd to run bun test when possible; if tests need a DOM, configure it before using DOM-based test helpers.\n\
             9. Do not install Playwright, launch browsers, or start a long-lived Vite dev server yourself; the harness will run that browser smoke check externally.\n\
             Finish with the app files created, validation result, and any harness behavior that made project scaffolding easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::RustLogAnalyzerScaffold => Ok(vec![
            "Profile scenario: rust-log-analyzer-scaffold.\n\
             Build a brand new Rust CLI project only in cwd.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             Required actions:\n\
             1. Use fs.read on brief.md.\n\
             2. Use fs.read on sample.log.\n\
             3. Use fs.write to create Cargo.toml.\n\
             4. Use fs.write to create src/lib.rs.\n\
             5. Use fs.write to create src/main.rs.\n\
             6. Use cmd.exec from cwd to run cargo test.\n\
             7. Use cmd.exec from cwd to run the CLI against sample.log when possible and verify it reports INFO/WARN/ERROR counts plus top error code E42.\n\
             Finish with the CLI behavior, test result, and any harness behavior that made project scaffolding easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::RustNotesTuiScaffold => Ok(vec![
            "Profile scenario: rust-notes-tui-scaffold.\n\
             Build a brand new Rust CLI project only in cwd.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             Required actions:\n\
             1. Use fs.read on brief.md.\n\
             2. Use fs.write to create Cargo.toml.\n\
             3. Use fs.write to create src/lib.rs.\n\
             4. Use fs.write to create src/main.rs.\n\
             5. Include focused tests for note parsing, storage, search, and export behavior.\n\
             6. Use cmd.exec from cwd to run cargo test.\n\
             7. Do not manually run the full add/list/search/export/help-keys smoke path; the harness will run validate-notes.ps1 after your run completes.\n\
             Finish with the CLI behavior, test result, and any harness behavior that made project scaffolding easier or harder."
                .to_string(),
        ]),
        ProfileScenarioKind::GithubIssueBugfix => Ok(vec![
            "Profile scenario: github-issue-bugfix.\n\
             Treat issue.md like a GitHub issue assigned to you.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/quote.ts.\n\
             3. Read tests/quote.test.ts.\n\
             4. Patch the production code with the smallest reasonable change so annual quotes annualize before discounting.\n\
             5. Run bun test from cwd after the patch. If you ran it before patching and it failed, run it again after patching and only finalize after the post-patch run passes.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::RustFailingTestBugfix => Ok(vec![
            "Profile scenario: rust-failing-test-bugfix.\n\
             Treat issue.md like a Rust bug report assigned to you.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/lib.rs.\n\
             3. Read tests/retry_scheduler.rs.\n\
             4. Patch production code with the smallest reasonable change so runnable jobs are filtered and ordered correctly.\n\
             5. Run cargo test from cwd after the patch. If you ran it before patching and it failed, run it again after patching and only finalize after the post-patch run passes. Do not set CARGO_TARGET_DIR.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::TypeScriptReducerBugfix => Ok(vec![
            "Profile scenario: typescript-reducer-bugfix.\n\
             Treat issue.md like a TypeScript bug report assigned to you.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/cart.ts.\n\
             3. Read tests/cart.test.ts.\n\
             4. Patch production code with the smallest reasonable change so inactive lines are ignored and non-positive quantities remove the line.\n\
             5. Run bun test from cwd after the patch. If you ran it before patching and it failed, run it again after patching and only finalize after the post-patch run passes.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::MergeConflictResolution => Ok(vec![
            "Profile scenario: merge-conflict-resolution.\n\
             Treat issue.md like a merge conflict assigned to you.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/featureFlags.ts and tests/featureFlags.test.ts.\n\
             3. Resolve the conflict markers in src/featureFlags.ts while preserving both dashboard-v2 and data-residency behavior.\n\
             4. Run bun test from cwd.\n\
             5. Read src/featureFlags.ts to verify no <<<<<<<, =======, or >>>>>>> markers remain.\n\
             Finish with the conflict resolution summary, changed file, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::GithubIssueTriage => Ok(vec![
            "Profile scenario: github-issue-triage.\n\
             Treat issue.md like a GitHub issue you are triaging, not fixing.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/cachePolicy.ts.\n\
             3. Read logs/warehouse-import.log.\n\
             4. Write triage.md with likely root cause, evidence, reproduction steps, and fix plan.\n\
             5. Read triage.md to verify it names /api/items, src/cachePolicy.ts, Cache-Control, max-age=300, and stale-while-revalidate=30.\n\
             Finish with a concise triage summary and confidence level."
                .to_string(),
        ]),
        ProfileScenarioKind::CiFailureTriage => Ok(vec![
            "Profile scenario: ci-failure-triage.\n\
             Triage the failing CI run and write a grounded diagnosis; do not modify source files.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read .github/workflows/frontend.yml.\n\
             3. Read logs/frontend-tests.log.\n\
             4. Read src/discount.ts and tests/discount.test.ts.\n\
             5. Write ci-triage.md with the failing command, failing test/assertion, Expected 80 / Received 100 evidence, likely root cause, and minimal fix plan.\n\
             6. Read ci-triage.md to verify it names bun test, SAVE20, applyDiscount, Expected 80, Received 100, src/discount.ts, and tests/discount.test.ts.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string(),
        ]),
        ProfileScenarioKind::PullRequestReview => Ok(vec![
            "Profile scenario: pull-request-review.\n\
             Review the PR like a code reviewer; do not modify source files.\n\
             Required actions:\n\
             1. Read pr.md.\n\
             2. Read diff.patch, diff-extra.patch, and diff-concurrency.patch.\n\
             3. Read every changed source and test file named in the diff.\n\
             4. Write review.json as a JSON array with one object per finding. Each object must contain exactly source, severity, evidence, impact, fix, and test as non-empty strings. Use the changed source path in source and the existing test path in test.\n\
             5. Write review.md as the human-readable version of the same findings, ordered by severity.\n\
             6. Read both outputs once to verify that every finding is grounded in a changed line and that the JSON is valid.\n\
             Finish with the review path and whether source files were left unchanged."
                .to_string(),
        ]),
        ProfileScenarioKind::DependencyUpgradeTriage => Ok(vec![
            "Profile scenario: dependency-upgrade-triage.\n\
             Triage the dependency upgrade like a maintainer; do not modify source files.\n\
             Required actions:\n\
             1. Read upgrade.md.\n\
             2. Read package.json and bun.lock.\n\
             3. Read docs/time-utils-2.0.md.\n\
             4. Read src/billingWindow.ts, src/billingWeek.ts, tests/billingWindow.test.ts, and tests/billingWeek.test.ts.\n\
             5. Write upgrade-triage.md with the changed package, any behavior-changing migration risk, affected code, test gap, and minimal fix plan.\n\
             6. Read upgrade-triage.md once to verify every conclusion is grounded in the supplied package, migration, source, or test evidence.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string(),
        ]),
        ProfileScenarioKind::TechnicalEssay => Ok(vec![
            "Profile scenario: technical-essay.\n\
             Write a sourced essay from the provided local notes; do not browse the web.\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Read all files under sources.\n\
             3. Write essay.md following the brief.\n\
             4. Read essay.md and verify it includes the title, section headings, and citations [S1], [S2], and [S3]. Use fs.read total_words for the word-count estimate; do not use cmd.exec just to count words.\n\
             Finish with the essay path, word-count estimate, and whether all citations are present."
                .to_string(),
        ]),
        ProfileScenarioKind::ConfigMigration => Ok(vec![
            "Profile scenario: config-migration.\n\
             Required actions:\n\
             1. Read migration.md.\n\
             2. Read config/app.json.\n\
             3. Read src/config.ts.\n\
             4. Read docs/config.md.\n\
             5. Update all three files for schema version 2 using the new authentication/method object and maxAttempts retry field. Do not keep the old key names in rewritten docs or code.\n\
             6. Before finalizing, make an actual cmd.exec or fs.search tool call to validate the JSON is parseable and verify stale authMode/retry.retries references are gone from changed files. If this validation finds a stale reference and you edit again, rerun validation after the final edit before answering. If using cmd.exec from the scenario directory, use paths like config/app.json, src/config.ts, and docs/config.md rather than prefixing the scenario directory again. If using PowerShell Select-String, do not combine these terms into one -SimpleMatch alternation; check each stale term separately or use a real regex match. Do not replace this tool call with a prose claim in the final answer.\n\
             Finish with files changed, validation result, and any migration risks."
                .to_string(),
        ]),
        ProfileScenarioKind::OpsReport => Ok(vec![
            "Profile scenario: ops-report.\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Read data/tickets.csv.\n\
             3. Compute total ticket rows excluding the CSV header, open tickets, open P1 tickets, and average minutes for open tickets.\n\
             4. Write metrics.json with totalTickets, openTickets, p1Open, and averageOpenMinutes.\n\
             5. Write report.md with a concise operational readout and the highest-risk team. Rank highest risk by open P1 count, then oldest open P1 age; do not count P2 tickets as P1 tickets.\n\
             6. Read both outputs to verify the numbers and narrative.\n\
              Finish with the computed metrics and risk summary."
                .to_string(),
        ]),
        ProfileScenarioKind::InventoryRebalancePlan => Ok(vec![
            "Profile scenario: inventory-rebalance-plan.\n\
             Build an optimal inventory-transfer recommendation for both requested budgets.\n\
             Required actions:\n\
             1. Read brief.md and policy.md.\n\
             2. Read data/products.csv, data/warehouses.csv, and data/transfer_options.csv.\n\
             3. Use a short Bun or PowerShell script from the benchmark workspace root to enumerate every feasible option subset under each budget; do not use a greedy shortcut or Bash-only heredoc syntax.\n\
             4. Write plan.json with the exact schema from the brief and sorted option ids.\n\
             5. Write memo.md explaining the base-versus-contingency tradeoff, why T14 is ineligible, and how the selected plans respect budget, origin-surplus, and destination-deficit limits.\n\
             6. Re-read both fully qualified output paths and verify every total before finishing.\n\
             Finish with both selected option sets, their net benefits, and the incremental net benefit."
                .to_string(),
        ]),
        ProfileScenarioKind::ExperimentRolloutAudit => Ok(vec![
            "Profile scenario: experiment-rollout-audit.\n\
             Produce a defensible experiment rollout audit from dirty assignment and event data.\n\
             Required actions:\n\
             1. Read brief.md and policy.md.\n\
             2. Read data/assignments.csv, data/exclusions.csv, and data/events.csv.\n\
             3. Use a short Bun or PowerShell script from the benchmark workspace root to canonicalize assignments and events, attribute orders, apply refunds, and print the calculated values; do not hand-count the CSV, use Bash-only heredoc syntax, or have the script write the final artifacts.\n\
             4. Use fs.write to write audit.json with the exact schema and rounded values requested in the brief.\n\
             5. Use fs.write to write memo.md explaining every launch gate, the decisive guardrail, and the important data-quality exclusions.\n\
             6. After both final writes, re-read both fully qualified outputs and verify denominators, order deduplication, refund attribution, and the final decision.\n\
             Finish with the decision and the four uplift/guardrail values."
                .to_string(),
        ]),
        ProfileScenarioKind::MultiModuleBugfix => Ok(vec![
            "Profile scenario: multi-module-bugfix.\n\
             Treat issue.md like a TypeScript bug report assigned to you.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/invoice.ts and src/total.ts.\n\
             3. Read tests/invoice.test.ts.\n\
             4. Patch production code with the smallest reasonable change so fractional line precision is preserved until the final total and discounts apply before tax. Keep src/tax.ts unchanged; it is already correct.\n\
             5. Run bun test from cwd after the patch. If you ran it before patching and it failed, run it again after patching and only finalize after the post-patch run passes.\n\
             Finish with the root cause in each module, changed files, test result, and whether the patch stayed scoped."
                .to_string(),
        ]),
        ProfileScenarioKind::StatefulReconciliationBugfix => Ok(vec![
            "Profile scenario: stateful-reconciliation-bugfix.\n\
             Treat issue.md as a production incident assigned to you. Diagnose the behavior from the supplied evidence before changing code.\n\
             Use bun for validation. Do not change src/types.ts, tests, documentation, or incident evidence.\n\
             Required actions:\n\
             1. Read issue.md, docs/invariants.md, and logs/incident.log.\n\
             2. Inspect src/normalize.ts, src/project.ts, src/types.ts, and tests/projection.test.ts.\n\
             3. Repair the production implementation so all documented invariants hold, including edge cases not represented by the visible incident lines.\n\
             4. Run bun test after the final edit.\n\
             Finish with the inferred root causes, changed production files, and validation result."
                .to_string(),
        ]),
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => Ok(vec![
            "Profile scenario: feature-rollout-consistency-bugfix.\n\
             Treat issue.md as a production incident. Use bun for validation.\n\
             Do not change src/types.ts, src/hash.ts, src/service.ts, tests, docs, or evidence.\n\
             Required actions:\n\
             1. Read issue.md, docs/invariants.md, and logs/incident.log.\n\
             2. Inspect src/store.ts, src/cache.ts, src/evaluate.ts, src/service.ts, src/types.ts, and tests/rollout.test.ts.\n\
             3. Repair only src/store.ts, src/cache.ts, and src/evaluate.ts so all documented invariants hold together.\n\
             4. Run bun test tests/rollout.test.ts after the final edit.\n\
             Finish with the root causes, changed production files, and validation result."
                .to_string(),
        ]),
        ProfileScenarioKind::FrontierRuleTransfer => Ok(vec![
            "Profile scenario: frontier-rule-transfer.\n\
             This is a novel-rule transfer exam. The demonstrations are the complete specification.\n\
             Do not inspect or modify tests/.harness, examples.json, src/types.ts, or tests.\n\
             Required actions:\n\
             1. Read task.md, examples.json, src/types.ts, src/solver.ts, and tests/public.test.ts.\n\
             2. Infer one coherent transformation that explains every demonstration, including path choice, termination, selection, and checksum behavior.\n\
             3. Implement the general transformation only in src/solver.ts without mutating inputs or hardcoding example ids.\n\
             4. Run bun test tests/public.test.ts after the final edit.\n\
             Finish with a concise statement of the inferred rule and the validation result."
                .to_string(),
        ]),
        ProfileScenarioKind::TerminalRepair => Ok(vec![
            "Profile scenario: terminal-repair.\n\
             The reporting service fails to start. Diagnose it through the terminal and repair the configuration.\n\
             Required actions:\n\
             1. Run bun run start from cwd to capture the current failure.\n\
             2. Read the config or code path the error points at.\n\
             3. Repair the configuration with the smallest change so the service starts. Do not modify src/index.js or data/report.csv.\n\
             4. Run bun run start again and confirm it prints REPORT OK with the computed row count and top team.\n\
             Finish with each breakage you found, the fix applied, and the final output."
                .to_string(),
        ]),
        ProfileScenarioKind::MultiHopAnalysis => Ok(vec![
            "Profile scenario: multi-hop-analysis.\n\
             Required actions:\n\
             1. Read question.md.\n\
             2. Read policy.md.\n\
             3. Read data/orders.csv and data/refunds.csv.\n\
             4. Compute the Q1 net revenue for product Atlas in region EMEA by joining the policy rules with both data files.\n\
             5. Write answer.json with exactly the keys product, region, and netRevenue.\n\
             6. Write answer.md with a short explanation naming the included order ids.\n\
             Finish with the computed net revenue and the orders included."
                .to_string(),
        ]),
        ProfileScenarioKind::PolicySupportAgent => Ok(vec![
            "Profile scenario: policy-support-agent turn 1/2.\n\
             You are the support agent. The customer writes: \"Hi! I'd like a full refund on order 5591, back to my card please. I know it's final sale, but it was delivered on June 30 so it's still within 30 days.\"\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Read policy.md.\n\
             3. Read cases/order_5591.json.\n\
             4. Write resolution.json following the brief's schema, applying the policy exactly to what the customer has claimed so far.\n\
             Finish with a short customer-facing reply explaining the decision.".to_string(),
            "Profile scenario: policy-support-agent turn 2/2.\n\
             The customer replies: \"It actually arrived cracked - I have photos of the damage right here.\"\n\
             Treat this as photo evidence of damage on arrival.\n\
             Required actions:\n\
             1. Update resolution.json so it reflects the full policy applied to all evidence from both turns.\n\
             Finish with a short customer-facing reply explaining the updated decision."
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
        ProfileScenarioKind::AssetRipperExploration
        | ProfileScenarioKind::FiveMExploration
        | ProfileScenarioKind::Cpp2IlExploration
        | ProfileScenarioKind::Il2CppInteropExploration => {
            unreachable!("exploration prompts are returned before the main scenario match")
        }
    }
}

pub(crate) fn benchmark_profile_prompts(
    scenario: ProfileScenarioKind,
    target_tokens: usize,
) -> Result<Vec<String>> {
    match scenario {
        ProfileScenarioKind::NaturalCompaction
        | ProfileScenarioKind::CompactionPressure
        | ProfileScenarioKind::PolicySupportAgent
        | ProfileScenarioKind::AssetRipperExploration
        | ProfileScenarioKind::FiveMExploration
        | ProfileScenarioKind::Cpp2IlExploration
        | ProfileScenarioKind::Il2CppInteropExploration => {
            profile_scenario_prompts(scenario, target_tokens)
        }
        _ => Ok(vec![benchmark_task_prompt(scenario)]),
    }
}

pub(crate) fn benchmark_task_prompt(scenario: ProfileScenarioKind) -> String {
    if let Some(prompts) = exploration_scenario_prompts(scenario) {
        return prompts
            .join("\n\nContinue in the same read-only session with the next task subset.\n\n");
    }

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
             Build a brand new React + TypeScript calculator app only in cwd.\n\
             Use bun for JavaScript package management. Do not create files outside this ignored fixture folder.\n\
             This is a scoped fixture task: start with the listed brief and do not survey unrelated repository files unless a concrete blocker requires it.\n\
             The finished app will be checked by bun test and a harness-owned Playwright browser smoke check after your run finishes, so it must be runnable through Vite in a real browser.\n\
             Do not install Playwright, launch browsers, or start a long-lived Vite dev server yourself; the harness owns that browser smoke check.\n\
             On Windows, run validation commands separately rather than chaining them with &&.\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Create package.json.\n\
             3. Create index.html.\n\
             4. Create src/main.tsx.\n\
             5. Create src/App.tsx.\n\
             6. Create src/App.test.tsx.\n\
             7. Create src/styles.css.\n\
             8. Run bun test if possible; if tests need a DOM, configure it before using DOM-based test helpers.\n\
             9. Ensure the Vite entrypoint is browser-runnable so the harness smoke check can click 1 + 2 = and observe 3.\n\
             Finish with the app files created, validation result, and any agent behavior that made project scaffolding easier or harder."
                .to_string()
        }
        ProfileScenarioKind::RustLogAnalyzerScaffold => {
            "Benchmark scenario: rust-log-analyzer-scaffold.\n\
             Build a brand new Rust CLI project only in cwd.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             This is a scoped fixture task: start with the listed brief/sample log and do not survey unrelated repository files unless a concrete blocker requires it.\n\
             Do not list the scenario directory; the required paths below are the complete initial evidence set.\n\
             On Windows, run validation commands separately rather than chaining them with &&.\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Read sample.log.\n\
             3. Create Cargo.toml.\n\
             4. Create src/lib.rs.\n\
             5. Create src/main.rs.\n\
             6. Run cargo test for the nested project.\n\
             7. Do not run cargo run manually; the harness will run the CLI sample-log smoke check after your run completes and verify INFO/WARN/ERROR counts plus top error code E42.\n\
             Finish with the CLI behavior, test result, and any agent behavior that made project scaffolding easier or harder."
                .to_string()
        }
        ProfileScenarioKind::RustNotesTuiScaffold => {
            "Benchmark scenario: rust-notes-tui-scaffold.\n\
             Build a brand new Rust CLI project only in cwd.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             This is a scoped fixture task: start with the listed brief and do not survey unrelated repository files unless a concrete blocker requires it.\n\
             The app should feel like a vim-style notes tool while remaining scriptable for validation.\n\
             On Windows, run validation commands separately rather than chaining them with &&.\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Create Cargo.toml.\n\
             3. Create src/lib.rs.\n\
             4. Create src/main.rs.\n\
             5. Implement `--store <path> add <title> <body...>`, `list`, `search <query>`, `export <path>`, and `help-keys`.\n\
             6. Include focused tests for note parsing/storage/search/export behavior.\n\
             7. Run cargo test for the nested project.\n\
             8. Do not manually run the full add/list/search/export/help-keys smoke path; the harness will run validate-notes.ps1 after your run completes.\n\
             Finish with the CLI behavior, test result, and any agent behavior that made project scaffolding easier or harder."
                .to_string()
        }
        ProfileScenarioKind::GithubIssueBugfix => {
            "Benchmark scenario: github-issue-bugfix.\n\
             Treat issue.md like a GitHub issue assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Do not list the scenario directory; the paths above are the complete evidence set: issue.md, src/quote.ts, and tests/quote.test.ts.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/quote.ts and tests/quote.test.ts.\n\
             3. Patch production code with the smallest reasonable change so annual quotes annualize before discounting.\n\
             4. Run bun test from cwd.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::RustFailingTestBugfix => {
            "Benchmark scenario: rust-failing-test-bugfix.\n\
             Treat issue.md like a Rust bug report assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Do not set CARGO_TARGET_DIR; use Cargo's default target/ directory for this nested project.\n\
             Do not list the scenario directory; the paths above are the complete evidence set: issue.md, src/lib.rs, and tests/retry_scheduler.rs.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/lib.rs and tests/retry_scheduler.rs.\n\
             3. Patch production code with the smallest reasonable change so runnable jobs filter blank ids and sort higher priority values first.\n\
             4. Run cargo test from cwd.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::TypeScriptReducerBugfix => {
            "Benchmark scenario: typescript-reducer-bugfix.\n\
             Treat issue.md like a TypeScript bug report assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Use bun for JavaScript package management and validation.\n\
             Do not list the scenario directory; the paths above are the complete evidence set: issue.md, src/cart.ts, and tests/cart.test.ts.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/cart.ts and tests/cart.test.ts.\n\
             3. Patch production code with the smallest reasonable change so inactive lines are ignored by subtotal and non-positive quantities remove the line.\n\
             4. Run bun test from cwd.\n\
             Finish with the root cause, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::MergeConflictResolution => {
            "Benchmark scenario: merge-conflict-resolution.\n\
             Treat issue.md like a merge conflict assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Use bun for JavaScript validation.\n\
             Required actions:\n\
             1. Read issue.md, src/featureFlags.ts, and tests/featureFlags.test.ts.\n\
             2. Resolve the conflict markers in src/featureFlags.ts with the smallest reasonable edit.\n\
             3. Preserve dashboard-v2 for enterprise accounts and beta tenants, and preserve data-residency for EU accounts.\n\
             4. Run bun test from cwd.\n\
             5. Verify src/featureFlags.ts no longer contains <<<<<<<, =======, or >>>>>>>.\n\
             Finish with the conflict resolution summary, changed file, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::GithubIssueTriage => {
            "Benchmark scenario: github-issue-triage.\n\
             Treat issue.md like a GitHub issue you are triaging, not fixing. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Inspect the local source and log evidence in cwd.\n\
             3. Write triage.md with likely root cause, evidence, reproduction steps, and fix plan.\n\
             4. Verify triage.md names /api/items, src/cachePolicy.ts, Cache-Control, max-age=300, and stale-while-revalidate=30.\n\
             Finish with a concise triage summary and confidence level."
                .to_string()
        }
        ProfileScenarioKind::CiFailureTriage => {
            "Benchmark scenario: ci-failure-triage.\n\
             Triage the failing CI run and write a grounded diagnosis; do not modify source files or inspect unrelated repository files unless a concrete blocker requires it.\n\
             Do not list the scenario directory; the paths below are the complete evidence set.\n\
             Required actions:\n\
             1. Read issue.md, .github/workflows/frontend.yml, logs/frontend-tests.log, src/discount.ts, and tests/discount.test.ts.\n\
             2. Write ci-triage.md with the failing command, failing test/assertion, Expected 80 / Received 100 evidence, likely root cause, and minimal fix plan.\n\
             3. Identify the SAVE20 path in applyDiscount as the likely production gap.\n\
             4. Do not re-read ci-triage.md solely to verify terms; the harness validates those required terms after your run.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string()
        }
        ProfileScenarioKind::PullRequestReview => {
            "Benchmark scenario: pull-request-review.\n\
             Review the PR like a code reviewer; do not modify source files or inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read pr.md, diff.patch, diff-extra.patch, and diff-concurrency.patch, then read every changed source and test file named by those diffs.\n\
             2. Write review.json as a JSON array with one object per finding. Each object must contain exactly source, severity, evidence, impact, fix, and test as non-empty strings. Use the changed source path in source and the existing test path in test.\n\
             3. Write review.md as the human-readable version of the same findings, ordered by severity.\n\
             4. Do not add style-only observations or restate the PR; prioritize defects that can change production behavior.\n\
             5. Re-read both outputs once to confirm the JSON is valid and each finding is tied to a changed line.\n\
             Finish with the review path and whether source files were left unchanged."
                .to_string()
        }
        ProfileScenarioKind::DependencyUpgradeTriage => {
            "Benchmark scenario: dependency-upgrade-triage.\n\
             Triage the dependency upgrade like a maintainer; do not modify source files or inspect unrelated repository files unless a concrete blocker requires it.\n\
             Required actions:\n\
             1. Read upgrade.md, package.json, bun.lock, docs/time-utils-2.0.md, src/billingWindow.ts, src/billingWeek.ts, tests/billingWindow.test.ts, and tests/billingWeek.test.ts.\n\
             2. Write upgrade-triage.md with the changed package, any behavior-changing migration risk, affected code, test gap, and minimal fix plan.\n\
             3. Ground every conclusion in the supplied upgrade request, lockfile, migration documentation, source, or tests.\n\
             4. Re-read upgrade-triage.md once to confirm the identified risk, mitigation, and regression coverage are evidence-backed.\n\
             Finish with the triage path and whether source files were left unchanged."
                .to_string()
        }
        ProfileScenarioKind::TechnicalEssay => {
            "Benchmark scenario: technical-essay.\n\
             Write a sourced essay from the provided local notes; do not browse the web and do not inspect unrelated repository files.\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Read all local source notes under sources.\n\
             3. Write essay.md following the brief.\n\
             4. Verify essay.md includes the title, section headings, and citations [S1], [S2], and [S3].\n\
             Finish with the essay path, word-count estimate, and whether all citations are present."
                .to_string()
        }
        ProfileScenarioKind::ConfigMigration => {
            "Benchmark scenario: config-migration.\n\
             Required actions:\n\
             1. Read migration.md plus the JSON, TypeScript, and docs files in that fixture.\n\
             2. Update config/app.json, src/config.ts, and docs/config.md for schema version 2 using the new authentication/method object and maxAttempts retry field. Do not keep the old key names in rewritten docs or code.\n\
             3. Before finalizing, make an actual cmd.exec or fs.search tool call to validate the JSON is parseable and verify stale authMode/retry.retries references are gone. If this validation finds a stale reference and you edit again, rerun validation after the final edit before answering. If using cmd.exec from the scenario directory, use paths like config/app.json, src/config.ts, and docs/config.md rather than prefixing the scenario directory again. If using PowerShell Select-String, do not combine these terms into one -SimpleMatch alternation; check each stale term separately or use a real regex match. Do not replace this tool call with a prose claim in the final answer.\n\
             Finish with files changed, validation result, and any migration risks."
                .to_string()
        }
        ProfileScenarioKind::OpsReport => {
            "Benchmark scenario: ops-report.\n\
             Analyze data/tickets.csv and produce both machine-checkable metrics and a concise narrative. Treat the first CSV line as the header, not a ticket.\n\
             Required actions:\n\
             1. Read brief.md.\n\
             2. Read data/tickets.csv.\n\
             3. Write metrics.json with totalTickets, openTickets, p1Open, and averageOpenMinutes.\n\
             4. Write report.md with the operational readout and highest-risk team. Rank highest risk by open P1 count, then oldest open P1 age; do not count P2 tickets as P1 tickets.\n\
             5. Verify both outputs before finishing.\n\
              Finish with the computed metrics and risk summary."
                .to_string()
        }
        ProfileScenarioKind::InventoryRebalancePlan => {
            "Benchmark scenario: inventory-rebalance-plan.\n\
             Produce an optimal inventory-transfer recommendation for the base and contingency budgets. This is a scoped fixture task; do not inspect unrelated repository files or edit the input files.\n\
             Required actions:\n\
             1. Read brief.md and policy.md.\n\
             2. Read data/products.csv, data/warehouses.csv, and data/transfer_options.csv.\n\
             3. Use a short Bun or PowerShell script from the benchmark workspace root to enumerate every feasible all-or-nothing option subset under each budget. Enforce lead-time eligibility, origin surplus, destination deficit, and total cost before applying the policy tie-breakers; do not use a greedy shortcut or Bash-only heredoc syntax.\n\
             4. Write plan.json with the exact schema from the brief, sorted option ids, and independently optimized base and contingency plans.\n\
             5. Write memo.md explaining the budget tradeoff, the binding constraints, and why T14 is ineligible.\n\
             6. Re-read both fully qualified output paths and verify every total once before finishing.\n\
             Finish with both option sets, total cost, net benefit, and incremental net benefit."
                .to_string()
        }
        ProfileScenarioKind::ExperimentRolloutAudit => {
            "Benchmark scenario: experiment-rollout-audit.\n\
             Audit the treatment rollout using the supplied policy and dirty assignment/event data. This is a scoped fixture task; do not inspect unrelated repository files or edit inputs.\n\
             Required actions:\n\
             1. Read brief.md and policy.md.\n\
             2. Read data/assignments.csv, data/exclusions.csv, and data/events.csv.\n\
             3. Use a short Bun or PowerShell script from the benchmark workspace root to deduplicate rows, resolve assignment eligibility, attribute 72-hour orders, apply refunds, and print the requested metrics. Do not hand-count rows, use Bash-only heredoc syntax, or have the script write the final artifacts.\n\
             4. Use fs.write to write audit.json with the exact schema and rounding from the brief.\n\
             5. Use fs.write to write memo.md explaining which launch gates pass, which fail, the decisive guardrail, and the important data-quality exclusions.\n\
             6. After both final writes, re-read both fully qualified outputs and verify every denominator, order/refund join, uplift, and the final decision once before finishing.\n\
             Finish with the decision, conversion uplift, revenue-per-eligible uplift, and refund-rate delta."
                .to_string()
        }
        ProfileScenarioKind::MultiModuleBugfix => {
            "Benchmark scenario: multi-module-bugfix.\n\
             Treat issue.md like a TypeScript bug report assigned to you. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Use bun for JavaScript package management and validation.\n\
             Do not list the scenario directory; the paths above are the complete evidence set: issue.md, src/invoice.ts, src/total.ts, and tests/invoice.test.ts.\n\
             Required actions:\n\
             1. Read issue.md.\n\
             2. Read src/invoice.ts and src/total.ts.\n\
             3. Read tests/invoice.test.ts.\n\
             4. Patch production code with the smallest reasonable change so fractional line precision is preserved until the final total and discounts apply before tax. Keep src/tax.ts unchanged; it is already correct.\n\
             5. Run bun test from cwd.\n\
             Finish with the root cause in each module, changed files, test result, and whether the patch stayed scoped."
                .to_string()
        }
        ProfileScenarioKind::StatefulReconciliationBugfix => {
            "Benchmark scenario: stateful-reconciliation-bugfix.\n\
             Investigate and repair the reservation projection incident. This is a scoped fixture task; do not inspect unrelated repository files.\n\
             Use bun for validation. Keep public types, tests, docs, and evidence unchanged.\n\
             Required actions:\n\
             1. Read issue.md, docs/invariants.md, and logs/incident.log.\n\
             2. Inspect src/normalize.ts, src/project.ts, src/types.ts, and tests/projection.test.ts.\n\
             3. Patch the production implementation so the documented invariants hold as a coherent state machine, not only for the visible examples.\n\
             4. Run bun test after the final edit.\n\
             Finish with the inferred root causes, changed production files, and validation result."
                .to_string()
        }
        ProfileScenarioKind::FeatureRolloutConsistencyBugfix => {
            "Benchmark scenario: feature-rollout-consistency-bugfix.\n\
             Investigate and repair the cross-tenant rollout incident. This is a scoped fixture task; do not inspect unrelated repository files.\n\
             Keep public types, stable hashing, service orchestration, tests, docs, and evidence unchanged.\n\
             Relevant paths are issue.md, docs/invariants.md, logs/incident.log, src/store.ts, src/cache.ts, src/evaluate.ts, src/service.ts, src/types.ts, src/hash.ts, and tests/rollout.test.ts.\n\
             Patch only src/store.ts, src/cache.ts, and src/evaluate.ts so the documented invariants hold coherently.\n\
             Run bun test tests/rollout.test.ts after the final edit.\n\
             Finish with the root causes, changed production files, and validation result."
                .to_string()
        }
        ProfileScenarioKind::FrontierRuleTransfer => {
            "Benchmark scenario: frontier-rule-transfer.\n\
             Infer the latent graph transformation from demonstrations and implement a general solver. This is a scoped fixture task; do not inspect unrelated repository files.\n\
             Read task.md, examples.json, src/types.ts, src/solver.ts, and tests/public.test.ts. Do not inspect or modify tests/.harness.\n\
             Implement only src/solver.ts. The same rule must explain path choice, early termination, selected nodes, and checksum behavior without hardcoding ids.\n\
             Run bun test tests/public.test.ts after the final edit.\n\
             Finish with the inferred rule and validation result."
                .to_string()
        }
        ProfileScenarioKind::TerminalRepair => {
            "Benchmark scenario: terminal-repair.\n\
             The reporting service fails to start. This is a scoped fixture task; do not inspect unrelated repository files unless a concrete blocker requires it.\n\
             Use bun for JavaScript commands and validate through the terminal.\n\
             Do not list the scenario directory; the relevant paths are: package.json, src/index.js, config/settings.json, and data/report.csv.\n\
             Required actions:\n\
             1. Run bun run start from cwd to capture the current failure.\n\
             2. Read the config or code path the error points at.\n\
             3. Repair the configuration with the smallest change so the service starts. Do not modify src/index.js or data/report.csv.\n\
             4. Run bun run start again and confirm it prints REPORT OK with the computed row count and top team.\n\
             Finish with each breakage you found, the fix applied, and the final output."
                .to_string()
        }
        ProfileScenarioKind::MultiHopAnalysis => {
            "Benchmark scenario: multi-hop-analysis.\n\
             Answer the question by joining the policy rules with both data files. This is a scoped fixture task; do not inspect unrelated repository files.\n\
             Required actions:\n\
             1. Read question.md.\n\
             2. Read policy.md.\n\
             3. Read data/orders.csv and data/refunds.csv.\n\
             4. Write answer.json with exactly the keys product, region, and netRevenue.\n\
             5. Write answer.md with a short explanation naming the included order ids.\n\
             Finish with the computed net revenue and the orders included."
                .to_string()
        }
        ProfileScenarioKind::ToolRecovery => {
            "Benchmark scenario: tool-recovery.\n\
             Required actions:\n\
             1. First attempt to read source/missing-note.md. This path is intentionally missing; do not skip this failing probe.\n\
             2. Recover by checking source/note.md.\n\
             3. Verify it contains: Spark recovery path verified.\n\
             Finish with what failed, how you recovered, and whether verification passed."
                .to_string()
        }
        ProfileScenarioKind::AssetRipperExploration
        | ProfileScenarioKind::FiveMExploration
        | ProfileScenarioKind::Cpp2IlExploration
        | ProfileScenarioKind::Il2CppInteropExploration => {
            unreachable!("exploration prompts are returned before the benchmark prompt match")
        }
        other => profile_scenario_prompts(other, 45_000)
            .ok()
            .and_then(|prompts| prompts.into_iter().next())
            .unwrap_or_else(|| format!("Benchmark scenario: {}", other.name())),
    }
}

pub(crate) fn natural_compaction_scenario_prompts(target_tokens: usize) -> Result<Vec<String>> {
    let turn_count = 3usize;
    let max_chars = MAX_SCENARIO_TARGET_TOKENS.saturating_mul(APPROX_CHARS_PER_TOKEN);
    let target_chars = target_tokens
        .saturating_mul(APPROX_CHARS_PER_TOKEN)
        .max(DEFAULT_COMPACT_AFTER_CHARS.saturating_add(4_000))
        .min(max_chars);
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
