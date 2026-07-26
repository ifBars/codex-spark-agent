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
spark setup
spark chat
```

You need a ChatGPT account that can use Codex OAuth. Windows may ask you to unblock the downloaded archive before extraction.

Or run one task directly:

```text
spark chat "Inspect this repo and explain how commands are dispatched."
```

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

Save a trace and print a profile after the run:

```powershell
spark chat --trace --profile "Fix the failing test and explain the patch."
```

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
- `/new`, `/save`, `/clear`, and `/exit` for session control

Up and Down move through submitted-command history while preserving the draft you were typing. PageUp, PageDown, and the mouse wheel scroll the transcript. Hover a message to copy it. Completed responses show elapsed time, token usage, average output speed, and time to first token when the API reports them.

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

## Tools and MCP

The built-in tool set covers:

```text
fs.read       fs.write       fs.rename
fs.list       fs.replace     cmd.exec
fs.stat       fs.edit        browser.run
fs.search
```

File tools stay under the selected workspace and skip common generated folders unless asked to inspect one directly. `cmd.exec` uses PowerShell on Windows and bounds noisy output before returning it to the model.

`browser.run` performs a stateless Playwright Chromium pass for browser inspection and local UI smoke tests. It uses Bun for its local Playwright setup and can save screenshots inside the workspace.

Spark discovers MCP servers from global Codex config, `.mcp.json`, and `.spark/mcp.json`. It supports stdio servers and HTTP endpoints. Use `/mcp` inside chat to inspect or change workspace overrides, or set `SPARK_DISABLE_MCP=1` for offline and benchmark runs.

## Traces and benchmarks

Use traces when you need to know why a run felt clean or messy:

```powershell
spark chat --trace --profile "Fix the failing test."
spark traces --summary --limit 10
spark analyze-trace --timeline
```

The profiler tracks request size, token pressure, tool failures, repeated calls, command duration, compaction, and cache hits.

For a bounded regression pass across real development tasks:

```powershell
.\scripts\quick_harness_benchmark.ps1
```

Compare the same slice against the logged-in native Codex CLI:

```powershell
.\scripts\quick_comparison_benchmark.ps1
```

List the scenarios before spending a run:

```powershell
.\scripts\quick_harness_benchmark.ps1 -ListScenarios
.\scripts\quick_comparison_benchmark.ps1 -ListScenarios
```

### Reasoning cost-quality pilot

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="docs/assets/reasoning-cost-quality-pilot-dark.svg">
  <img src="docs/assets/reasoning-cost-quality-pilot.svg" alt="Weighted task quality plotted against total API tokens for Spark and native Codex at low, medium, and high reasoning." width="960">
</picture>

This pilot ran one difficult stateful bugfix once per runner and reasoning level. Weighted behavioral checks expose partial progress that pass/fail scoring hides. It is useful as a scoring sanity check, not a broad performance claim.

The raw values are in [`docs/benchmarks/reasoning-cost-quality-pilot-2026-07-26.csv`](docs/benchmarks/reasoning-cost-quality-pilot-2026-07-26.csv). The earlier 144-run success-only chart remains in [`docs/assets/reasoning-cost-quality.svg`](docs/assets/reasoning-cost-quality.svg).

<details>
<summary><strong>Benchmark commands and output</strong></summary>

```powershell
spark profile-scenario repo-survey
spark profile-benchmark real-world --repeat 3
spark profile-benchmark-report --suite real-world
spark codex-cli-benchmark real-world --timeout-seconds 360
spark opencode-benchmark real-world --timeout-seconds 360 --pure
spark benchmark-compare --suite real-world --codex-cli-report .spark-profile/codex-cli/report.json
```

Available suites include `core`, `survey`, `scaffolding`, `editing`, `reasoning`, and `real-world`. Reasoning scenarios can report weighted validation checks while the normal exit code still records full completion.

The quick comparison script preflights native Codex before spending a Spark run and writes resumable status artifacts when the provider is unavailable. Comparison reports keep provider failures separate from task failures and record runner versions, scenario coverage, and input freshness.

Reports are written under `.spark-profile/`. Traces are written under `.spark-runs/`. Keep both private unless you have reviewed their contents.

</details>

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

## Transport and compaction

Responses are WebSocket-first. Spark keeps the connection alive across tool turns and chains `previous_response_id`, so continuation requests send only new input. It falls back to HTTP/SSE if the socket fails before streaming begins.

Long sessions use remote `/responses/compact` first, with a local pressure fallback when needed. Compaction boundaries are included in traces. Spark runs until it completes, is cancelled, or reaches a context/input safety guard.

<details>
<summary><strong>Compaction controls</strong></summary>

```powershell
spark chat --compact-after-tokens 40000 --max-input-tokens 125000
spark chat --compact-after-tool-only-turns 0 "Run without tool-streak compaction."
```

The default tool-only threshold is `12`. Setting it to `0` disables that guardrail for comparison work.

</details>

## Security

This project is not a sandbox.

- `cmd.exec` runs commands on your machine.
- Work mode can edit files inside the selected workspace.
- MCP servers may add tools with their own security boundaries.
- Traces can contain prompts, source, tool output, command output, and model responses.

Only run Spark in workspaces you trust. Keep `.spark-runs/`, `.spark-profile/`, `.spark/`, `.spark-codex/`, and `~/.spark-codex/` private.

## Development

Building from source requires Rust stable with edition 2024 support:

```powershell
git clone https://github.com/ifBars/codex-spark-agent.git
cd codex-spark-agent
cargo install --path .
```

Run the normal checks:

```powershell
cargo fmt
cargo check
cargo test
```

Useful inspection commands:

```powershell
spark tools
spark skills
spark commands
```

The quick harness benchmark is expected before and after changes to the agent loop, tools, command execution, compaction, tracing, profiling, or benchmark scoring. Docs-only changes do not need it.

## License

[MIT](LICENSE)
