# Codex Spark Agent

`codex-spark-agent` is an experimental Rust CLI harness for testing `gpt-5.3-codex-spark` as a small coding agent.

It is intentionally simpler than the official Codex CLI: one binary, ChatGPT/Codex OAuth, streaming Responses calls, a small native tool set, session files, skill loading, trace capture, and profiling signals for model behavior.

> This project is unofficial and experimental. It talks to the ChatGPT Codex backend shape observed by Codex-like clients. That surface can change.

## Why

`gpt-5.3-codex-spark` behaves best when the harness works with its native Responses/function-call behavior instead of forcing a heavy synthetic action protocol.

This repo is built to profile and iterate on that harness shape:

- direct streamed calls to the Codex Responses backend,
- native function tools for file and command actions,
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

Print the latest trace as a compact timeline:

```powershell
cargo run --bin spark -- analyze-trace --timeline
```

List recent traces:

```powershell
cargo run --bin spark -- traces
```

Compare recent traces:

```powershell
cargo run --bin spark -- traces --summary --limit 10
cargo run --bin spark -- traces --summary --scenario compaction-pressure --aggregate --limit 20
cargo run --bin spark -- traces --summary --diagnostic tool_failure_recovered --aggregate
cargo run --bin spark -- traces --json --scenario tool-recovery --aggregate
cargo run --bin spark -- traces --jsonl --diagnostic request_failure
```

Run a repeatable profiling scenario through the real agent loop:

```powershell
cargo run --bin spark -- profile-scenario repo-survey
cargo run --bin spark -- profile-scenario natural-compaction --target-tokens 45000
cargo run --bin spark -- profile-scenario compaction-pressure --target-tokens 45000
cargo run --bin spark -- profile-scenario file-edit
cargo run --bin spark -- profile-scenario file-ops
cargo run --bin spark -- profile-scenario tool-recovery
cargo run --bin spark -- profile-scenario skill-use
cargo run --bin spark -- profile-scenario tool-recovery --repeat 5
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
- `/session rename <new>` renames the active session.
- `/session rename <old> <new>` renames a saved session.
- `/session delete <name>` deletes a saved inactive session.
- `/new` starts a new timestamped session.
- `/new <name>` starts a new named session.
- `/save` saves the current session.
- `/clear` clears the active conversation.
- `/skills` lists repo-local skills and cache status.
- `/skill load <name>` loads a skill into the conversation.
- `/skill refresh` force-rebuilds compiled skill caches.
- `/exit` quits.

Interactive chat defaults to `~/.spark-codex/sessions/default.json`. One-shot prompts only persist when `--session <name>` is supplied. Interactive sessions autosave after completed prompts and after agent-loop errors, so failed long-context runs can still be inspected, compacted, or resumed. Session files include a lightweight `schema_version`; older versionless session files still load.

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
- `fs.stat`
- `fs.search`
- `fs.write`
- `fs.replace`
- `fs.edit`
- `fs.rename`
- `cmd.exec`

Tool schemas are intentionally small. The harness accepts JSON-string or object arguments, reports bad arguments as tool observations, and preserves function-call outputs in the next request.

`fs.list` and `fs.search` skip generated/runtime directories such as `target/`, `.git/`, `node_modules/`, `.spark/`, `.spark-runs/`, and `.spark-profile/` during recursive discovery. Direct paths remain readable, so profiling artifacts can still be inspected explicitly. `fs.stat` returns compact path metadata, including `exists:false` for missing workspace paths, without reading file contents. `fs.read` returns line-window metadata (`returned_lines`, `total_lines`, `has_more`, and `next_offset`) so Spark can choose the next chunk without guessing.

`fs.write` creates parent directories when needed and reports whether the file was newly created, plus previous and new byte counts. `fs.rename` moves one file or directory inside the workspace, creates destination parents, and refuses to overwrite an existing destination. Both tools report `created_parent_dirs`, which helps Spark notice when a mutation created an unexpected path segment. That makes common file mutations visible in traces and profile timelines without falling back to shell commands.

Repeated read-only observations from `fs.read`, `fs.list`, `fs.stat`, and `fs.search` are served from a per-run cache, including failed observations such as missing files. File mutation tools and `cmd.exec` clear that cache so Spark can retry after the workspace may have changed.

Native tool invocation errors return structured observations with `error_kind`, `message`, `tool`, `args_shape`, and a retry `hint`. This gives Spark a compact recovery path when it sends incomplete arguments, picks a stale path, or calls an unavailable tool.

`cmd.exec` bounds stdout and stderr before returning them to Spark. Long streams keep their head and tail plus `stdout_chars`/`stderr_chars` and truncation flags, which keeps command-heavy runs from poisoning the next request with accidental massive output. Timed-out commands are returned as failed tool observations with `timed_out` and `timeout_ms` fields. Non-zero exits keep stdout/stderr and add an explicit error such as `command exited with code 7` so Spark can recover or choose a narrower command.

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

Automatic compaction prints a concise event line such as `compaction: responses_compact 220000->80000 chars in 1234ms`. Full compaction reports still go into traces and profile summaries.

In interactive chat, `/compact` runs the same remote-first compaction path immediately and saves the active session afterward. Use it before a large follow-up when a natural conversation has accumulated noisy tool output.

`/status` and `/profile` include live context pressure for the active session, so long natural conversations can be checked before the next message triggers auto compaction.

If remote compaction still leaves history above the threshold, the harness applies a local pressure pass before sending the next Spark request. Traces mark this as `local_pressure` under the remote compaction report so those runs can be separated from clean remote compactions during profiling. Local pressure and fallback reports split `compacted_tool_outputs` from `compacted_messages` so long-context failures can be correlated with noisy tool output versus broad conversation history.

If remote compaction fails, local preview compaction trims older tool outputs and older messages. Recent messages are preserved when possible, but an oversized recent prompt can still be compacted if it alone keeps the retained transcript above the threshold. Locally compacted messages carry a `[spark local message compaction]` marker with original size, preview size, retained shape, deterministic retained-intent lines, extracted required native file-tool actions, and an explicit exact-content note. Treat the local fallback as a debugging path, not the preferred steady state.

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
- derived diagnostics for request failures, repeated tool loops, mutation-created parent dirs, weak or expanding compaction, and near-limit context pressure.

Use `--trace` to save run metadata, raw request, response, tool-result, compaction, and profile JSON files under `.spark-runs/`. Use `spark traces` to list recent trace directories, or `spark traces --summary` to compare recent runs by request size, latency, tools, compactions, and diagnostics. `spark traces --json` emits matching analyzed traces as one JSON object, and `spark traces --jsonl` emits one record per matching trace for scripts. `--aggregate` adds an aggregate object in JSON mode or an aggregate record in JSONL mode. `spark traces --diagnostic <kind>` filters analyzed traces by diagnostic kind; repeat the flag to require multiple diagnostics. Use `spark analyze-trace` without a path to summarize the latest trace.

Each trace includes `000-trace-metadata.json` with the model, workspace, turn cap, profile flag, interactive/session mode, compaction threshold, and input guard used for the run. `analyze-trace` includes that metadata in its summary so profiling results can be compared across harness settings.

`spark profile-scenario <name>` runs canned prompts through the same `AgentRunner` used by `spark chat`, with tracing and profile output enabled by default. `repo-survey` exercises normal read/list/search behavior. `file-edit` creates an ignored `.spark-scenarios/file-edit/` scratch fixture and exercises native read/edit/write verification. `file-ops` creates an ignored `.spark-scenarios/file-ops/` scratch fixture and exercises native write/rename/stat/read/search verification. `tool-recovery` creates an ignored `.spark-scenarios/tool-recovery/` fixture, intentionally asks Spark to read a missing path, then checks whether it recovers with the corrected path. `skill-use` loads the tracked `@rust-patterns` skill through the automatic mention path, then exercises native read/search behavior while traces verify that the compiled skill context was actually present. `natural-compaction` sends three normal conversation turns through one runner, defaulting to about 45k total estimated prompt tokens, so retained chat history crosses the default compaction threshold below Spark's 128k context window. `compaction-pressure` generates one synthetic long-context prompt with the same default size. Scenario runs store their name, prompt sizing, expected native tool groups/calls, and expected loaded skills in trace metadata, and they print a trace summary row even when Spark fails. Use `--repeat <n>` to run a scenario multiple times with a fresh runner and fixture each time; when tracing is enabled, the command prints an aggregate row for those repeated runs. Use `spark traces --summary --scenario <name> --aggregate` to compare recent matching runs by success/failure count, recovery rate, scenario tool/call/skill completion, max context pressure, latency, tools, compactions, and diagnostics.

Token counts are approximate and use a simple 4 chars/token estimate. They are profiling signals for comparing harness behavior, not authoritative tokenizer output.

Profile summaries keep compaction reports compact by replacing raw compaction payloads with `raw_summary` metadata. The full raw compaction response remains in the separate `*-compaction.json` trace file.

Multiple tool results from the same Spark response are kept as separate trace files instead of overwriting each other. `analyze-trace` reconstructs both successful and failed tool observations from those files.

If a run fails after starting, the harness saves a `*-error.json` trace entry and a profile summary where possible. `analyze-trace` reports those errors alongside the exact request-size sequence, which is useful when profiling long-context and max-turn failure points.

`analyze-trace` recomputes its summary from raw trace files, so older embedded `*-profile-summary.json` files do not mask newer diagnostics.

`analyze-trace` also emits a compact `timeline` array with per-turn request size, approximate token pressure, response latency, response text size, tool-call signatures, tool-result status, timeout/truncation/error-kind markers, compactions, and terminal errors. Use `spark analyze-trace --timeline` for a human-readable version that correlates Spark failures with context growth, slow requests, repeated tools, and compaction boundaries before opening the raw JSON files.

When a native tool observation fails, `analyze-trace` reports whether Spark later recovered with a successful observation from the same tool. Summary rows include `recoveries=<recovered>/<failed>`, and timeline output prints a `tool-recovery` header. This helps distinguish clean runs, recovered path drift, and unrecovered tool failures.

When local compaction retained required native file-tool actions, `analyze-trace` compares those actions with observed tool calls and reports executed, missing, and delayed required actions. `spark analyze-trace --timeline` also prints a `required-actions` header so post-compaction detours are visible without opening raw JSON.

Profiling scenarios also store expected native tool groups in trace metadata. `analyze-trace` reports `scenario_tools=<satisfied>/<total>` in summary rows and prints a `scenario-tools` timeline header, so a run that finishes without hard errors can still be checked against the intended harness behavior.

Scenarios can also store exact expected native calls with path arguments. `analyze-trace` reports `scenario_calls=<satisfied>/<total>` and prints a `scenario-calls` timeline header, which separates clean path-following runs from runs where Spark used the right tool type but drifted to a wrong path and later recovered.

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
