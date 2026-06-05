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

The file tools stay under the selected `--cwd`. Recursive listing and search skip generated folders like `target/`, `.git/`, `node_modules/`, `.spark/`, `.spark-runs/`, and `.spark-profile/` unless you point at them directly.

`cmd.exec` runs through PowerShell on Windows. Output is bounded before it is sent back to Spark, so noisy commands are easier to recover from.

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

## License

MIT
