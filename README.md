# Codex Spark Agent

An experimental Rust harness for running `gpt-5.3-codex-spark` as a local coding agent.

I built this because Spark is fast, but speed alone does not make a useful coding agent. The harness gives it a focused tool loop, normal chat sessions, repo skills, compaction, traces, and benchmarks that show where a run actually went wrong.

This is research software, not an official OpenAI or Codex project. It uses the ChatGPT/Codex backend shape observed in Codex-like clients, which can change.

## Quick start

Download the [latest release](https://github.com/ifBars/codex-spark-agent/releases/latest) for your platform:

| Platform | Archive |
| --- | --- |
| Windows x64 | `spark-<version>-windows-x64.zip` |
| Linux x64 | `spark-<version>-linux-x64.zip` |
| macOS Apple Silicon | `spark-<version>-macos-arm64.zip` |

Extract the archive, put `spark` on your `PATH`, then run:

```text
spark --version
spark setup
spark chat
```

Confirm that `spark --version` matches the downloaded release before relying on
new commands. If it reports an older version, another `spark` binary appears
earlier on `PATH`; replace or reorder that installation first.

You need a ChatGPT account that can use Codex OAuth. Windows may ask you to unblock the downloaded archive before extraction.

Or run one task directly:

```text
spark chat "Inspect this repo and explain how commands are dispatched."
```

Prefer a desktop workspace? [Spark Desktop v0.8.0](https://github.com/ifBars/t3code/releases/tag/v0.8.0)
is the supported GUI for Codex and this harness. It is maintained as a focused
T3Code fork and replaces the retired desktop prototypes from this repository.

## What you get

- Interactive and one-shot coding sessions
- Native file, search, edit, command, and browser tools
- Repo-local skills and reusable prompt commands
- Stdio and HTTP MCP tool discovery
- SQLite-backed session history
- Remote-first compaction for long conversations
- Raw traces, diagnostics, and repeatable benchmark scenarios
- A read-only Spark explorer for native Codex

Spark is intentionally smaller than the official Codex CLI. I am not trying to replace Codex. This is a place to inspect and improve the parts that usually get hand-waved away: tool arguments, patch quality, command output, context pressure, recovery, and measurable task quality.

## Reasoning cost-quality benchmark

[**Explore the interactive benchmark charts →**](https://ifbars.github.io/codex-spark-agent/)

Spark Bench presents one consolidated evidence catalog. Switch among compatible
evidence cohorts and the overall, coding, math/data, analysis,
terminal/operations, writing/configuration, and frontier charts; then toggle
runners and reasoning levels, change cost axes, inspect uncertainty ranges, and
drill into measured tasks through the scenario lens. Cohorts remain separate
inside the catalog so unlike runs are never averaged into a misleading score.
The evidence strip exposes measured and pending scenario coverage, task-run
counts, task/provider exclusions, and scoring behavior. Provider-limited
attempts are excluded, so the experimental Frontier chart stays explicitly
pending instead of publishing a fabricated zero.

The current measured dataset covers nine difficult scenarios with three attempts at every runner/reasoning combination. Charts plot 129 successful task runs; 33 failed task attempts are excluded before aggregation, and no provider/API failures occurred in that snapshot. Error bars are 95% intervals across available successful-only scenario means. `inventory-rebalance-plan`, `experiment-rollout-audit`, `feature-rollout-consistency-bugfix`, and `frontier-rule-transfer` are newer validated fixtures but remain explicitly pending until balanced successful reruns are available; they are not silently mixed into measured curves. The measured ordering is not forced to be monotonic, and category labels distinguish broad, developing, early, and pilot coverage.

The [category aggregates](docs/benchmarks/reasoning-cost-quality-categories-2026-07-26.csv) are reproducibly derived from the [scenario-level rows](docs/benchmarks/reasoning-cost-quality-expanded-scenarios-2026-07-26.csv) using the published [view specification](docs/benchmarks/reasoning-benchmark-views-2026-07-26.json). A checked-in [evidence manifest](docs/benchmarks/reasoning-benchmark-evidence-2026-07-26.json) derives the explorer's run counts, exclusions, pending fixtures, and artifact links from those sources. The [overall aggregate](docs/benchmarks/reasoning-cost-quality-expanded-2026-07-26.csv) and [methodology](docs/benchmarks/reasoning-cost-quality-expanded-2026-07-26.md) remain available, as do the original pilot and 144-run success-only chart.

List the quick comparison slice before running it:

```powershell
.\scripts\quick_comparison_benchmark.ps1 -ListScenarios
.\scripts\quick_harness_benchmark.ps1 -ListScenarios
```

Comparison reports include a **Report Inputs** section so results remain auditable. It records `benchmark_suite`, `benchmark_model`, `reasoning_effort`, `repeat`, `timeout_seconds`, `scenario_count`, `codex_bin`, `codex_command_path`, `codex_command_version`, `command_path`, `command_version`, and the complete `inputs` manifest. It also records comparison controls including `codex_preflight_timeout_seconds`, `ignore_user_config`, `isolated_codex_home`, `allow_harness_request_failure_comparison`, `allow_codex_request_failure_comparison`, `skip_codex_preflight`, `preflight_only`, and `fail_on_directional_comparison`. Reports emit an **input freshness warning** when their source rows no longer match those inputs. Use `--fail-on-directional-comparison` when a directional result should fail the command.

The PowerShell wrapper exposes the equivalent `-FailOnDirectionalComparison` switch. Machine-readable output also carries `scenarios`, `rerun_command`, `resume_command`, `retry_after_seconds`, `retry_at_local`, and `retry_at_utc`. A preflight-only run prints these stable fields for automation:

```text
codex_preflight_status=...
codex_preflight_codex_path=...
codex_preflight_codex_version=...
codex_preflight_rerun_command=...
codex_preflight_resume_command=...
```

## Use Spark

Start an interactive session:

```powershell
spark chat
```

Run against another workspace:

```powershell
spark chat --cwd C:\path\to\repo "Find the config loader and trace its callers."
```

Inspect without allowing edits or command execution:

```powershell
spark chat --mode ask "Review the public API surface."
```

Create a standalone repository brief with only local, read-only filesystem tools:

```powershell
spark brief "How are commands dispatched?" --cwd . --path src --format text
spark brief "Trace configuration loading." --path src/config.rs --format json --trace
```

`spark brief` defaults to text output, medium reasoning, a 120-second deadline, and no trace.
It has a deterministic budget of 16 local filesystem tool invocations: after that, it advertises no
tools and must synthesize from its gathered evidence. It never advertises web search, MCP, subagents,
shell, browser, or write tools. Text writes only
the Markdown brief to stdout; JSON writes one versioned report envelope. A missing brief contract
returns a nonzero exit code while still emitting that report.

Save a trace and print a profile after the run:

```powershell
spark chat --trace --profile "Fix the failing test and explain the patch."
```

Inspect the source-reported ChatGPT Codex quota, credits, and any rate-limit windows:

```powershell
spark usage
spark usage --json
```

This command reads your saved Spark OAuth login and calls the Codex account-usage service. It reports provider quota/credit metadata exactly as available, but it does not infer token counts, messages, dollars, or a price for `gpt-5.3-codex-spark`. The JSON includes a source-labeled pricing-availability record so automation can distinguish unavailable pricing from a zero price.

Aggregate local Codex CLI token history without making an account or provider request:

```powershell
spark usage --history
spark usage --history --json --since-days 30
spark usage --history --codex-home D:\\CodexHome --max-files 500 --output .spark-profile\\usage-history.json
```

The history scanner reads only `sessions/**/*.jsonl` and `archived_sessions/*.jsonl` beneath `CODEX_HOME` (or `~/.codex`), and emits an aggregate-only `spark.usage_history.v1` report suitable for Spark Bench. It never includes prompts, messages, raw tool output, credentials, session paths, or working directories. Totals are coverage-labeled: input contains cached and cache-write subsets; reasoning output remains a diagnostic subset of output. A `files_truncated` or partial-coverage signal means the report is not complete. It does not estimate dollars or subscription spend.

<details>
<summary><strong>More chat options</strong></summary>

Choose the reasoning effort:

```powershell
spark chat --reasoning-effort low "Answer briefly."
```

Append harness-specific system instructions:

```powershell
spark chat --system-prompt "Keep the review focused on runtime behavior." "Review this repo."
```

Resume a named session:

```powershell
spark chat --session refactor-tools
```

Run `spark chat --help` for the complete option list.

</details>

### Interactive chat

The commands I use most are:

- `/help` for the full command list
- `/status` for session and context pressure
- `/profile` for live profiler data
- `/compact` to compact the conversation
- `/session ...` to manage saved sessions
- `/skills` and `/skill load <name>` to manage skills
- `/commands` to list reusable prompts
- `/subagent ...` to run one scoped worker and save its compact brief
- `/agents` to inspect or cancel managed workers
- `/new`, `/save`, `/clear`, and `/exit` for session control

Up and Down move through submitted-command history while preserving the draft you were typing. PageUp, PageDown, and the mouse wheel scroll the transcript. Hover a message to copy it. Completed responses show elapsed time, token usage, average output speed, and time to first token when the API reports them.

### Multi-agent work

Spark keeps its interactive model as the default. For focused helper work, `explore` inherits that Spark model, while `research`, `review`, and `plan` default to `gpt-5.6-luna`. Set `SPARK_ADVANCED_SUBAGENT_MODEL` or pass `--model`/`model` to override the advanced choice.

Use `/subagent review src/agent` for one helper. During an agent run, the model can use `subagent.spawn` for independent helpers, `subagent.wait` to merge a compact brief, `subagent.followup` after a completed brief, `subagent.steer` to replace a running worker or continue a completed one, `subagent.cancel`, and `subagent.list`. The default concurrency limit is three; set `SPARK_SUBAGENT_MAX_CONCURRENCY` to a value from 1 through 8 when a workspace or entitlement warrants a different bound.

Workers are read-only by default. A delegated patch must explicitly request `mode=work` and list non-overlapping relative `ownership` paths; it also requires the parent chat to be in work mode. These workers are restricted to native filesystem writes inside those paths, with shell, browser, MCP, and nested-worker execution disabled. This is a harness guard, not a replacement for OS sandboxing. Worker reports are capped to a compact brief and parent traces record spawn, wait, follow-up, steering, cancellation, and report events with the worker profile.

## Use Spark as a Codex explorer

Spark can provide the read-only explorer behind native Codex while the parent task keeps its normal memory, plugins, skills, and workflow:

```text
spark setup --non-interactive --skip-login --skip-skill-migration --codex
```

Setup registers the persistent `spark_harness` MCP server and installs a native Codex explorer definition. New Codex tasks can then delegate bounded repository investigation to Spark and receive a compact evidence brief instead of a second full transcript.

If `spark_harness` or the explorer definition already exists, rerun setup with `--force-codex`. Existing files are backed up before replacement.

The MCP server can also be started directly:

```powershell
spark mcp-server
```

## Skills and prompt commands

Repo-local skills live at:

```text
.agents/skills/<name>/SKILL.md
```

Load one explicitly:

```powershell
spark chat --skill rust-patterns "Review src/tools.rs."
```

Spark also detects skill mentions such as `@rust-patterns` in the prompt. Compiled skill summaries are cached under `.spark/skills/` and rebuilt when the source changes.

Reusable prompt commands can live in:

```text
.agents/commands/<name>.md
.spark/commands/<name>.md
.claude/commands/<name>.md
```

List commands or preview an expansion:

```powershell
spark commands
spark commands review src/main.rs
```

Nested files become namespaced commands, so `.claude/commands/db/migrate.md` becomes `/db:migrate`. If the same command exists in more than one folder, `.agents` wins, then `.spark`, then `.claude`.

## MCP servers

Spark can use MCP servers from your global Codex config or the current repo's `.mcp.json` and `.spark/mcp.json`. Inside chat, use `/mcp` to list servers, change workspace overrides, or refresh tool discovery.

## Traces

Use traces when you need to know why a run felt clean or messy:

```powershell
spark chat --trace --profile "Fix the failing test."
spark traces --summary --limit 10
spark analyze-trace --timeline
```

The profiler tracks request size, token pressure, tool failures, repeated calls, command duration, compaction, and cache hits.

Trace files are local and can contain prompts, source, tool output, command output, and model responses. Spark never removes them automatically. Inspect the default 30-day policy, then review a dry run before explicitly confirming a purge:

```powershell
spark trace-retention --cwd .
spark trace-retention --cwd . --older-than-days 14 --purge
spark trace-retention --cwd . --older-than-days 14 --purge --confirm
```

Plain inspection prints only policy and counts. The dry run and final command list the exact resolved `.spark-runs/run-<timestamp>` directories beneath the selected workspace; only the final command removes them. Keep traces private and choose a retention period appropriate for the workspace.

Benchmark workflows and profiler internals are documented in [Development and internals](docs/development.md).

## Setup and authentication

`spark setup` creates local app and session storage, runs device-code login, and can migrate existing skills into the current repo.

<details>
<summary><strong>Setup and login commands</strong></summary>

Run setup without prompts:

```powershell
spark setup --non-interactive
```

Useful flags:

- `--skip-login` creates local storage without signing in.
- `--skip-skill-migration` leaves `.agents/skills` untouched.
- `--skill-source C:\path\to\skills` selects a migration source.
- `--cwd C:\path\to\repo` selects the repo that receives skills.
- `--codex` installs the native Codex explorer integration.
- `--force-codex` replaces an existing integration after backing it up.

You can also run authentication separately:

```powershell
spark login
spark login --device
spark auth-status
```

Existing installs continue to use `~/.spark-codex/auth.json`. New installs use the platform app-data directory selected by the OS. Never commit auth files.

</details>

## Security

This project is not a sandbox.

- `cmd.exec` runs commands on your machine.
- Work mode can edit files inside the selected workspace.
- MCP servers may add tools with their own security boundaries.
- Traces can contain prompts, source, tool output, command output, and model responses.
- Spark does not automatically delete traces; `spark trace-retention --purge --confirm` only removes eligible local run directories under the selected workspace's `.spark-runs/` root.

Only run Spark in workspaces you trust. Keep `.spark-runs/`, `.spark-profile/`, `.spark/`, `.spark-codex/`, and `~/.spark-codex/` private.

## Development

Building from source, validation, transport details, profiling, and benchmark workflows are covered in [Development and internals](docs/development.md).

## License

[MIT](LICENSE)
