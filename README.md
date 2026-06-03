# Codex Spark Agent

`codex-spark-agent` is an experimental Rust CLI harness for testing `gpt-5.3-codex-spark` as a small coding agent.

It is intentionally simpler than the official Codex CLI: one binary, ChatGPT/Codex OAuth, streaming Responses calls, a small native tool set, session files, skill loading, trace capture, and profiling signals for model behavior.

> This project is unofficial and experimental. It talks to the ChatGPT Codex backend shape observed by Codex-like clients. That surface can change.

## Why

`gpt-5.3-codex-spark` behaves best when the harness works with its native Responses/function-call behavior instead of forcing a heavy synthetic action protocol.

This repo is built to profile and iterate on that harness shape:

- direct streamed calls to the Codex Responses backend,
- native function tools for file, command, and completion actions,
- Codex-like remote compaction support,
- trace files for debugging model/tool loops,
- small profiling summaries for repeated calls, compaction, and cache hits,
- repo-local skills that compile into compact Spark-facing context automatically.

## Status

This is early research software. It is useful for local profiling and controlled coding tasks, but it is not a hardened sandboxed agent runtime.

Current defaults assume you are running it on your own machine and in a repository you trust.

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

Run through Cargo during development:

```powershell
cargo run --bin spark -- --help
```

## Login

Browser login:

```powershell
cargo run --bin spark -- login
```

Device-code login:

```powershell
cargo run --bin spark -- login --device
```

Auth tokens are stored under `~/.spark-codex/auth.json`. Do not commit that file. The repo `.gitignore` also excludes a repo-local `.spark-codex/` directory in case you override paths while testing.

## Basic Usage

Interactive chat:

```powershell
cargo run --bin spark -- chat
```

One-shot prompt:

```powershell
cargo run --bin spark -- chat "Inspect this repo and summarize its package layout."
```

Run against another workspace:

```powershell
cargo run --bin spark -- chat --cwd C:\path\to\repo "Find the config loader."
```

Trace and profile a run:

```powershell
cargo run --bin spark -- chat --profile --trace "Profile this coding task."
```

Analyze a saved trace:

```powershell
cargo run --bin spark -- analyze-trace .spark-runs\run-1780481349210
```

Analyze the latest local trace:

```powershell
cargo run --bin spark -- analyze-trace
```

List recent traces:

```powershell
cargo run --bin spark -- traces
```

By default each user message loops until Spark finishes. `--max-turns <n>` is available as an optional safety cap.

## Interactive Commands

Inside `spark chat`:

- `/help` shows commands.
- `/status` prints conversation/profile status, including live context pressure before the next Spark request.
- `/profile` prints the full live profiler JSON.
- `/compact` manually runs Codex-like compaction on the active conversation.
- `/session` shows the active session.
- `/session list` lists saved sessions.
- `/session open <name>` opens an existing session.
- `/session use <name>` opens the named session or creates it if missing.
- `/session new <name>` starts a new named session.
- `/new` starts a new timestamped session.
- `/new <name>` starts a new named session.
- `/save` saves the current session.
- `/clear` clears the active conversation.
- `/skills` lists repo-local skills and cache status.
- `/skill load <name>` loads a skill into the conversation.
- `/skill refresh` force-rebuilds compiled skill caches.
- `/exit` quits.

Interactive chat defaults to `~/.spark-codex/sessions/default.json`. One-shot prompts only persist when `--session <name>` is supplied.

## Skills

Source skills live under:

```text
.agents/skills/<name>/SKILL.md
```

Compiled Spark-facing skill summaries are cached under:

```text
.spark/skills/<name>.json
```

The cache is keyed by the SHA-256 hash of the source `SKILL.md`.

That means normal use is automatic:

```powershell
cargo run --bin spark -- chat "Use @rust-patterns and review src/tools.rs."
cargo run --bin spark -- chat --skill rust-patterns "Review src/tools.rs."
```

If the source skill changes, Spark recompiles the compact skill cache on first load. `spark skills` and `/skills` only list status; they do not compile. `spark skills --refresh` and `/skill refresh` are optional prebuild commands.

## Built-In Tools

- `fs.read`
- `fs.list`
- `fs.search`
- `fs.write`
- `fs.replace`
- `fs.edit`
- `cmd.exec`

Tool schemas are intentionally small. The harness accepts JSON-string or object arguments, reports bad arguments as tool observations, and preserves function-call outputs in the next request.

`fs.list` and `fs.search` skip generated/runtime directories such as `target/`, `.git/`, `node_modules/`, `.spark/`, `.spark-runs/`, and `.spark-profile/` during recursive discovery. Direct paths remain readable, so profiling artifacts can still be inspected explicitly. `fs.read` returns line-window metadata (`returned_lines`, `total_lines`, `has_more`, and `next_offset`) so Spark can choose the next chunk without guessing.

Repeated read-only observations from `fs.read`, `fs.list`, and `fs.search` are served from a per-run cache, including failed observations such as missing files. File mutation tools and `cmd.exec` clear that cache so Spark can retry after the workspace may have changed.

`cmd.exec` bounds stdout and stderr before returning them to Spark. Long streams keep their head and tail plus `stdout_chars`/`stderr_chars` and truncation flags, which keeps command-heavy runs from poisoning the next request with accidental massive output.

## Compaction

Spark has a 128k context window. This harness has two guardrails:

- `--compact-after-chars` defaults to `160000` request JSON characters.
- `--max-input-chars` defaults to `500000` request JSON characters.

You can tune those in approximate model-token terms instead:

```powershell
cargo run --bin spark -- chat --compact-after-tokens 40000 --max-input-tokens 125000
```

Token thresholds use the same simple 4 chars/token estimate as the profiler. Pass either the `*-chars` or `*-tokens` form for a threshold, not both.

When history grows past the compaction threshold, the harness first tries Codex-like remote compaction through `/responses/compact`, normalizes returned `compaction_summary` items to Codex-style `compaction` items, and replaces live history with the compacted transcript.

In interactive chat, `/compact` runs the same remote-first compaction path immediately and saves the active session afterward. Use it before a large follow-up when a natural conversation has accumulated noisy tool output.

`/status` and `/profile` include live context pressure for the active session, so long natural conversations can be checked before the next message triggers auto compaction.

If remote compaction still leaves history above the threshold, the harness applies a local pressure pass before sending the next Spark request. Traces mark this as `local_pressure` under the remote compaction report so those runs can be separated from clean remote compactions during profiling.

If remote compaction fails, local preview compaction trims older tool outputs and older messages. Treat the local fallback as a debugging path, not the preferred steady state.

## Profiling

Use `--profile` for a compact summary after a prompt:

- request count,
- max/average input size,
- exact request-size sequence,
- Spark request and compaction duration,
- approximate token sequence and percent of Spark's 128k context window,
- response text size,
- tool count by tool,
- tool result and failure counts by tool,
- tool duration totals and max duration by tool,
- tool output truncation counts,
- repeated tool call signatures,
- consecutive duplicate calls,
- read-only cache hits,
- remote/local compaction counts,
- derived diagnostics for request failures, repeated tool loops, weak or expanding compaction, and near-limit context pressure.

Use `--trace` to save run metadata, raw request, response, tool-result, compaction, and profile JSON files under `.spark-runs/`. Use `spark traces` to list recent trace directories. Use `spark analyze-trace` without a path to summarize the latest trace.

Each trace includes `000-trace-metadata.json` with the model, workspace, turn cap, compaction threshold, and input guard used for the run. `analyze-trace` includes that metadata in its summary so profiling results can be compared across harness settings.

Token counts are approximate and use a simple 4 chars/token estimate. They are profiling signals for comparing harness behavior, not authoritative tokenizer output.

Profile summaries keep compaction reports compact by replacing raw compaction payloads with `raw_summary` metadata. The full raw compaction response remains in the separate `*-compaction.json` trace file.

Multiple tool results from the same Spark response are kept as separate trace files instead of overwriting each other. `analyze-trace` reconstructs both successful and failed tool observations from those files.

If a run fails after starting, the harness saves a `*-error.json` trace entry and a profile summary where possible. `analyze-trace` reports those errors alongside the exact request-size sequence, which is useful when profiling long-context and max-turn failure points.

`analyze-trace` recomputes its summary from raw trace files, so older embedded `*-profile-summary.json` files do not mask newer diagnostics.

`.spark-runs/`, `.spark-profile/`, `.spark/`, `target/`, and local auth/session state are ignored by git.

## Observed Spark Notes

These are observations from local profiling and may change:

- The Codex backend requires `stream: true`; non-streaming requests returned `400`.
- `response.completed.response.output` can be empty even when stream events include completed output items, so the client reconstructs output from `response.output_item.done`.
- With `store: false`, prior reasoning/message IDs should not be replayed. The harness carries forward minimal assistant messages, function calls, and matching function outputs.
- Tool names are normalized for the backend, so local names like `fs.read` become wire names like `fs_read`.
- Turns finish when Spark returns a normal assistant message without further function calls; there is no synthetic completion tool.
- Targeted tools such as `fs.search`, `fs.replace`, and `fs.edit` reduce repeated broad listing/reading behavior compared with only exposing generic read/write/shell tools.

## Security Notes

This is not a sandbox.

- `cmd.exec` runs commands on your machine.
- File tools can read and write under the selected `--cwd`.
- Traces may contain prompt text, file snippets, tool outputs, or other sensitive local data.
- Keep `.spark-runs/`, `.spark-profile/`, `.spark/`, and `~/.spark-codex/` private.

## Development

Run checks:

```powershell
cargo fmt
cargo test
cargo check
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
