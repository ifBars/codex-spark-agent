# Codex Spark Agent

`codex-spark-agent` is an experimental Rust CLI for running `gpt-5.3-codex-spark` as a local coding agent.

I built it because Spark is fast and interesting, but it needs a tighter loop to be useful on real repos. This harness gives it a small native tool set, streamed Codex backend calls, session history, repo-local skill loading, trace capture, and enough profiling to see when it starts drifting.

This is not an official OpenAI or Codex project. It talks to the ChatGPT/Codex backend shape observed by Codex-like clients, and that surface can change.

## What it does

Spark runs in a local workspace and can inspect files, edit files, search with ripgrep-style queries, run shell commands, load repo skills, compact long conversations, and write trace files for later inspection.

The project is intentionally smaller than the official Codex CLI. The goal is not to replace Codex. The goal is to make Spark easier to experiment with when you care about the boring but important parts: tool arguments, patch quality, command output, context growth, and recovery after a bad observation.

Current shape:

- one `spark` binary,
- ChatGPT/Codex OAuth login,
- interactive chat and one-shot prompts,
- native file and command tools,
- SQLite-backed named sessions,
- repo-local skill loading from `.agents/skills`,
- reusable prompt commands from `.agents/commands`, `.spark/commands`, and `.claude/commands`,
- trace and profile output for local runs,
- benchmark scenarios for comparing harness behavior.

## Status

This is research software. It is useful for local experiments and controlled coding tasks, but it is not a sandboxed agent runtime.

Only run it in workspaces you trust. `cmd.exec` runs commands on your machine, and trace files can contain prompt text, file snippets, command output, and other local data.

## Install

Requirements:

- Rust stable with edition 2024 support
- A ChatGPT account that can authenticate through the Codex OAuth flow

Build from source:

```powershell
git clone https://github.com/ifBars/codex-spark-agent.git
cd codex-spark-agent
cargo build
```

Run through Cargo:

```powershell
cargo run --bin spark -- --help
```

## First-run setup

The setup command walks through the first local configuration pass:

```powershell
cargo run --bin spark -- setup
```

It creates Spark's app/session storage, signs in with device-code auth, can migrate existing skills into the current repo's `.agents/skills`, and can refresh the local skill cache.

For unattended setup, use defaults and skip prompts:

```powershell
cargo run --bin spark -- setup --non-interactive
```

Useful setup flags:

- `--skip-login` creates local storage without authenticating.
- `--skip-skill-migration` leaves `.agents/skills` untouched.
- `--skill-source C:\path\to\skills` migrates skills from a specific directory.
- `--cwd C:\path\to\repo` chooses the repo that receives migrated skills.

## Login

Browser login:

```powershell
cargo run --bin spark -- login
```

Device-code login:

```powershell
cargo run --bin spark -- login --device
```

Auth tokens are stored under Spark's local app data directory. Existing installs continue to use `~/.spark-codex/auth.json`; new installs use the platform data directory selected by the OS. Do not commit auth files.

## Basic use

Start an interactive chat:

```powershell
cargo run --bin spark -- chat
```

Run one prompt:

```powershell
cargo run --bin spark -- chat "Inspect this repo and summarize the package layout."
```

Run against another workspace:

```powershell
cargo run --bin spark -- chat --cwd C:\path\to\repo "Find the config loader."
```

Choose reasoning effort for a one-shot or interactive chat:

```powershell
cargo run --bin spark -- chat --reasoning-effort low "Answer briefly."
```

Append harness-specific system instructions:

```powershell
cargo run --bin spark -- chat --system-prompt "You are Relay in Discord." "Who are you?"
```

Use read-only mode when you want inspection without edits or commands that mutate files:

```powershell
cargo run --bin spark -- chat --mode ask "Review the public API surface."
```

Save trace and profile output for a run:

```powershell
cargo run --bin spark -- chat --trace --profile "Fix the failing test and explain the patch."
```

Analyze the latest trace:

```powershell
cargo run --bin spark -- analyze-trace --timeline
```

## Interactive chat

Inside `spark chat`, these are the commands I use most:

- `/help` shows available commands.
- `/status` shows session and context pressure.
- `/profile` prints the live profiler JSON.
- `/compact` compacts the active conversation.
- `/session ...` manages saved sessions.
- `/skills`, `/skill load <name>`, and `/skill refresh` manage repo skills.
- `/commands` lists reusable prompt commands.
- `/<command> [args]` expands a Markdown prompt command and runs it.
- `/new`, `/save`, `/clear`, and `/exit` do what they say.

Interactive chat starts a fresh timestamped session unless you pass `--session <name>`. Named sessions live in Spark's local app data directory.

## Skills

Repo-local skills live here:

```text
.agents/skills/<name>/SKILL.md
```

You can load them explicitly:

```powershell
cargo run --bin spark -- chat --skill rust-patterns "Review src/tools.rs."
```

Or mention them in the prompt:

```powershell
cargo run --bin spark -- chat "Use @rust-patterns and review src/tools.rs."
```

Spark compiles a compact skill summary on first use and caches it under `.spark/skills/`. If the source `SKILL.md` changes, the cache is rebuilt on the next load.

## Prompt commands

Reusable prompt commands live in any of these folders:

```text
.agents/commands/<name>.md
.spark/commands/<name>.md
.claude/commands/<name>.md
```

Use `.agents/commands` when you are bringing existing agent-harness workflows into Spark. Use `.spark/commands` for Spark-local prompts. Spark also imports legacy Claude Code project commands from `.claude/commands`. If multiple folders define the same command name, `.agents` wins, then `.spark`, then `.claude`.

Nested command files use Claude-style namespacing. For example, `.claude/commands/db/migrate.md` becomes `/db:migrate`.

List commands:

```powershell
cargo run --bin spark -- commands
```

Preview an expanded command without running the agent:

```powershell
cargo run --bin spark -- commands review src/main.rs
```

Run one in chat:

```powershell
cargo run --bin spark -- chat
# then type: /review src/main.rs
```

Command files are Markdown. Optional frontmatter can provide a description, and `{{args}}` or Claude's `$ARGUMENTS` placeholder marks where slash-command arguments should be inserted:

```markdown
---
description: Review a focused change
---

Review this change:

{{args}}
```

If a command has no argument placeholder, Spark appends the arguments under an `Arguments:` heading.

## Built-in tools

The harness exposes a small tool set:

- `fs.read`
- `fs.list`
- `fs.stat`
- `fs.search`
- `fs.write`
- `fs.replace`
- `fs.edit`
- `fs.rename`
- `cmd.exec`
- `browser.run`
- configured MCP tools

The file tools stay under the selected `--cwd`. Recursive listing and search skip generated folders like `target/`, `.git/`, `node_modules/`, `.spark/`, `.spark-runs/`, and `.spark-profile/` unless you point at them directly.

`cmd.exec` runs through PowerShell on Windows. Output is bounded before it is sent back to Spark, so noisy commands are easier to recover from.

`browser.run` runs a stateless Playwright Chromium pass for browser-backed inspection and local UI smoke checks. It uses Bun to install Playwright under `.spark/browser-tools`, can perform simple CSS-selector actions, and can write screenshots under the workspace when requested.

MCP servers are discovered from global Codex config plus repo-local `.mcp.json` and `.spark/mcp.json` files. The harness supports stdio servers with `command`/`args` and HTTP MCP endpoints with `url`/`http_headers`; disabled servers are skipped, and unavailable servers are reported as warnings without blocking the run. Discovered tools are exposed in work mode as function names like `mcp__context7__resolve-library-id`. Set `SPARK_DISABLE_MCP=1` to skip MCP discovery for benchmark or offline runs.

## Traces and profiling

Use `--trace` when you want raw evidence for a run. Trace files are written under `.spark-runs/`.

Use `--profile` when you want a compact summary after a prompt. The profiler tracks request size, approximate token pressure, tool calls, failures, repeated calls, command duration, compactions, cache hits, and other signals that explain why a run felt clean or messy.

Useful commands:

```powershell
cargo run --bin spark -- traces --summary --limit 10
cargo run --bin spark -- traces --summary --diagnostic tool_failure_recovered --aggregate
cargo run --bin spark -- analyze-trace .spark-runs\run-1780481349210
cargo run --bin spark -- analyze-trace --timeline
```

## Benchmarks

The benchmark commands run repeatable local scenarios through Spark and, optionally, through other agent CLIs for comparison.

```powershell
cargo run --bin spark -- profile-scenario repo-survey
cargo run --bin spark -- profile-scenario rust-notes-tui-scaffold
cargo run --bin spark -- profile-benchmark scaffolding --repeat 3
cargo run --bin spark -- profile-benchmark-report --suite scaffolding
```

Available benchmark suites include:

- `core`
- `survey`
- `scaffolding`
- `editing`
- `real-world`

There are also comparison runners for Codex CLI and OpenCode:

```powershell
cargo run --bin spark -- codex-cli-benchmark real-world --timeout-seconds 360
cargo run --bin spark -- opencode-benchmark real-world --timeout-seconds 360 --pure
cargo run --bin spark -- benchmark-compare --suite real-world --codex-cli-report .spark-profile/codex-cli/report.json
```

For a bounded Spark-vs-Codex CLI pass over real-world development tasks, use the paired quick comparison script:

```powershell
.\scripts\quick_comparison_benchmark.ps1
```

It runs the selected Spark harness scenarios, reports from that exact run manifest, runs the same scenarios through the logged-in `codex` binary, and writes a comparison report under `.spark-profile/benchmarks/`. The quick scripts print machine-readable run metadata such as `benchmark_suite`, `benchmark_model`, `reasoning_effort`, `repeat`, `max_turns`, `scenario_count`, and `scenarios`; the paired comparison script also prints `timeout_seconds` and `codex_bin`. The default quick slice includes real development tasks such as bugfixes, merge conflict resolution, config migration, issue triage, CI failure triage, pull request review, dependency upgrade triage, ops reporting, sourced writing, and small Rust scaffolding. Pass `-Scenario rust-failing-test-bugfix` or another scenario list when you want a smaller smoke run before a broader comparison.

To inspect the selected quick slice without running Spark or native Codex, use:

```powershell
.\scripts\quick_comparison_benchmark.ps1 -ListScenarios
.\scripts\quick_harness_benchmark.ps1 -ListScenarios
```

Before spending a Spark run, the quick comparison script preflights the logged-in `codex` binary with the selected model and reasoning effort. It writes a timestamped `*-codex-preflight-*.json` status artifact under `.spark-profile/benchmarks/` and prints `codex_preflight_status=...`, `codex_preflight_codex_path=...`, `codex_preflight_codex_version=...`, `codex_preflight_rerun_command=...`, and `codex_preflight_resume_command=...` so blocked native comparisons are auditable and easy to resume. The artifact includes `scenario_count`, `scenarios`, `repeat`, `max_turns`, `timeout_seconds`, `codex_preflight_timeout_seconds`, `codex_command_path`, `codex_command_version`, `rerun_command`, and `resume_command` for the quick slice that was about to run. The rerun command preserves the exact invocation, while the resume command omits only `-PreflightOnly` and pins `-CodexBin` to the resolved native binary path when available, so a successful retry can proceed into the full Spark-vs-native comparison against the same executable. Native Codex benchmark reports also include `codex_bin`, `codex_command_path`, and `codex_command_version`, and each native row carries `command_path` and `command_version`; final comparison JSON, CSV, and HTML rows preserve those row-level fields for provenance. Final comparison JSON also includes an `inputs` object, and the HTML includes a `Report Inputs` table plus an input freshness warning, so mixed fresh/stale benchmark reports show their source paths, modified times, row counts, scenario coverage, and modified-time span. It also records structured comparison validity under `aggregate.diagnostics.comparison_validity`; pass `--fail-on-directional-comparison` to `benchmark-compare`, or `-FailOnDirectionalComparison` to the quick comparison script, when CI should write artifacts but exit nonzero if stale inputs or skipped provider/API rows make the headline directional. It also records switch state fields: `ignore_user_config`, `isolated_codex_home`, `allow_harness_request_failure_comparison`, `allow_codex_request_failure_comparison`, `skip_codex_preflight`, `preflight_only`, and `fail_on_directional_comparison`. When the provider returns retry guidance, the artifact includes the original `retry_hint`, `retry_after_seconds`, and machine-readable `retry_at_local` / `retry_at_utc` timestamps; the same retry fields are printed to stdout when available. If native Codex is rate-limited or otherwise unavailable, it prints `codex_preflight=failed` and exits before running Spark. Pass `-SkipCodexPreflight` to defer that check to the full `codex-cli-benchmark` step. Provider/API outages are reported as non-comparable rows with scenario summaries such as `request_failure_scenarios=scenario:count`, so usage limits, quota failures, and rate limits are not scored as task performance. If at least one valid Spark or native Codex row remains, the quick comparison continues and prints terminal summaries such as `harness_provider_api_failure_scenarios=config-migration:1` or `codex_provider_api_failure_scenarios=config-migration:1`; the final comparison excludes only the provider/API rows. Ordinary Spark task failures, including local max-turns stops, remain comparable and are scored normally.

To check native Codex availability without running either benchmark, use:

```powershell
.\scripts\quick_comparison_benchmark.ps1 -PreflightOnly
```

When `-IsolatedCodexHome` is used, the quick script skips this upfront native preflight because the isolated `CODEX_HOME` is prepared inside the Rust benchmark runner.

These commands write reports under `.spark-profile/`. Keep that folder private unless you have reviewed the contents.

## Compaction

Spark has a 128k context window. This harness compacts older context before the request gets too large, and it can also compact after long tool-only streaks.

Common knobs:

```powershell
cargo run --bin spark -- chat --compact-after-tokens 40000 --max-input-tokens 125000
cargo run --bin spark -- chat --compact-after-tool-only-turns 0 "Run without tool-streak compaction."
```

The default tool-only compaction threshold is `12`. Setting it to `0` disables that guardrail for comparison runs.

## Security notes

This project is not a sandbox.

- `cmd.exec` runs commands on your machine.
- File tools can read and write under the selected workspace.
- Traces may include local source, prompts, tool output, command output, and model responses.
- Keep `.spark-runs/`, `.spark-profile/`, `.spark/`, `.spark-codex/`, and `~/.spark-codex/` private.

## Development

Run checks:

```powershell
cargo fmt
cargo check
cargo test
```

List tool schemas:

```powershell
cargo run --bin spark -- tools
```

List skill cache status:

```powershell
cargo run --bin spark -- skills
```

List reusable prompt commands:

```powershell
cargo run --bin spark -- commands
```

## License

MIT
