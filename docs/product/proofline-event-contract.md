# Proofline renderer event contract

Status: architecture contract for an additive desktop proof-of-concept. It is
not an implemented Tauri API. Source audit completed 2026-08-01 against
`agent/proofline-mvp`.

## Decision

Proofline should be a local renderer over a small, versioned **snapshot plus
ordered delta** boundary. Reuse the harness's in-process display events for
live activity and its existing session/usage readers for read-only summaries.
Do not make the renderer parse traces, raw Responses payloads, or Codex JSONL
files directly.

The first desktop slice is read-only: open a local Spark session, render its
current conversation state and a completed run's evidence, then stream a new
run. Mutating tools, terminal access, browser access, and any future approval
remain explicit backend actions with a visible authority boundary.

## Audited seams and reuse limits

| Surface | Source evidence | Reuse in Proofline | Limit the GUI must not hide |
| --- | --- | --- | --- |
| Live agent activity | `src/agent/mod.rs`: `AgentDisplayEvent`, `AgentDisplay::Shared`, `AgentRunner::use_shared_display`, `take_shared_display_events` | Map existing display variants to renderer deltas. `src/chat/tui.rs`: `Tui::apply_agent_events` already consumes them. | Events are crate-private, not `Serialize`, lack event ID/timestamp/run ID, and the shared vector is drained rather than subscribed to. The bridge must add a public envelope. |
| Ordered loop | `src/agent/run_loop.rs`: `AgentRunner::run_until_idle` | Preserve source order: request start; streamed reasoning/text/search; response completion; tool batch/calls/results; then another request or terminal profile. | One provider request can retry or switch between WebSocket and HTTP/SSE. Never label retry/fallback as completion. |
| Response measures | `AgentDisplayEvent::ResponseComplete`; populated in `run_until_idle` | Render duration, provider-reported output tokens when present, time to first token, and calculated output-token rate. | These are response diagnostics, not a bill, account quota, input-token total, or complete run ledger. |
| Tool evidence | `src/agent/run_loop.rs`: `ToolBatchStart`, `ToolCall`, `ToolResult`; durable `tool-result` trace write | Render name, serialized args, success/failure, duration, output size, and error. | No event has changed-files, diff, checkpoint, validation classification, or approval decision. Do not infer them from arbitrary shell output. |
| Compaction/errors/profile | `src/agent/run_loop.rs`: `emit_compaction_*`, `record_terminal_error`, `emit_profile_summary` | Render compaction as a state transition and warnings/profile as diagnostics. | A warning does not prove every affected tool result is in the view; snapshot recovery remains required. |
| Session persistence | `src/agent/mod.rs`: `AgentSnapshot`, `snapshot`, `restore_snapshot`, `load_session_named`, `save_session_named`; `src/session/store.rs`: `SessionStore::{list,load,save,rename,delete}` | List session names/timestamps and load/save through deliberate backend commands. | Snapshot holds input, request sequence, profiler, skills, mode, reasoning effort, goal, and memory flag—not a transcript, title, fork relation, review decision, or evidence index. `SessionStore::list` is alphabetical, not recent-first. |
| Durable traces | `src/agent/trace.rs`: `TraceWriter`; `src/agent/run_loop.rs` writes request input, raw response, tool results, errors, profile | Offer only an explicit local “open evidence” diagnostic path. | Traces can contain prompts, tool arguments/results, paths, raw provider payloads, and cwd. They are neither safe default renderer input nor a stable public schema. |
| Local history usage | `src/usage_history.rs`: `scan_history`, `UsageHistoryReport`, `parse_session`, `lineage_roots` | Reuse serialized `spark.usage_history.v1` for aggregate, Spark Bench-compatible usage. Its source says `network: false`. | It scans Codex JSONL, not Spark SQLite sessions; returns aggregates only; price is deliberately unavailable. |
| Fork accounting | `src/usage_history.rs`: `session_meta.forked_from_id`, `select_unique_sessions`, `lineage_roots`, replay suppression | Show aggregate fork diagnostics and coverage. | Spark's session store has no fork field or Codex-session mapping. A per-thread tree is a new feature, not recoverable fact. |
| Permissions/modes | `src/tools/policy.rs`: `AgentMode::{Ask,Work}`; `src/tools/mod.rs`: `invoke_with_read_roots`; `src/agent/subagent.rs`: ownership enforcement | Use Ask/Work as the capability summary; preserve explicit delegated-write ownership. | Work enables all native tools; `cmd.exec` remains an OS-user shell; no current core approval event or per-command prompt exists. The mock’s full-access control cannot claim an OS sandbox. |
| MCP server | `src/mcp_server/protocol.rs` | Reuse cancellation-token pattern for desktop cancellation. | It exposes `explore_repo` over JSON-RPC stdio, not a GUI stream, and its concurrent `JoinSet` processing has no UI ordering contract. |

## Transport

Use a Tauri command to start or attach to a run and pass a Tauri
`ipc::Channel<ProoflineDelta>` for the live stream. Current Tauri v2
documentation recommends Channels for ordered, high-throughput delivery where
rapid asynchronous events could otherwise be processed out of order. Use
ordinary commands for request/response actions: snapshot, session list, local
usage scan, prompt submission, and an explicit evidence-open request.

Do not use a sequence of global Tauri events for the primary run stream. If
events later carry app-wide notifications, include run ID and sequence but
treat them as advisory, not transcript truth.

The bridge owns one bounded relay task per run:

1. Allocate a stable `run_id`, start the `AgentRunner`, and call
   `use_shared_display()` before the run begins.
2. Send the complete snapshot at `sequence: 0`.
3. Drain display events in source order, map each exactly once, and increment
   sequence monotonically for that run.
4. On cancellation, relay the core warning/terminal state and close only after
   a terminal delta. A reconnecting frontend fetches a fresh snapshot; it does
   not guess missing deltas.

Limit Tauri capabilities/permissions to named Proofline commands for the main
window. Do not grant broad filesystem, shell, process, or plugin access merely
because the Rust harness can use them. Each backend command validates session
name, workspace scope, requested mode, and future approval ID independently of
the renderer.

## Proposed versioned wire schema

This is a proposed bridge schema, not a claim about an existing public Rust
API. Null or unavailable data stays explicit so the UI can say “unavailable”
instead of showing zero.

```json
{
  "schema_version": "spark.proofline.v1",
  "kind": "snapshot",
  "run_id": "local UUID",
  "sequence": 0,
  "captured_at_unix_ms": 0,
  "data": {
    "active_session": {
      "id": "local session name",
      "updated_at_unix_seconds": 0,
      "mode": "ask",
      "reasoning_effort": "medium",
      "memory_enabled": false,
      "request_sequence": 0
    },
    "threads": [{"id": "...", "label": "...", "updated_at_unix_seconds": 0}],
    "conversation": [],
    "run": {"state": "idle", "model": "gpt-5.3-codex-spark"},
    "usage": {
      "source": "none | spark.usage_history.v1",
      "coverage": "unavailable | partial | reported",
      "pricing": {"availability": "unavailable", "reason": "..."}
    },
    "capabilities": {"mode": "ask", "shell": false, "browser": false}
  }
}
```

```json
{
  "schema_version": "spark.proofline.v1",
  "kind": "delta",
  "run_id": "same local UUID",
  "sequence": 1,
  "captured_at_unix_ms": 0,
  "event": "assistant.delta",
  "data": {"text": "..."}
}
```

Required v1 delta mappings are lossless maps of `AgentDisplayEvent`:

| Bridge event | Source variant | Required data |
| --- | --- | --- |
| `run.request_started` | `RequestStart` | `turn`, `input_chars` |
| `assistant.message` / `assistant.delta` | `Assistant` / `AssistantDelta` | `text` |
| `run.response_completed` | `ResponseComplete` | duration, nullable output tokens, nullable TTFT, nullable output-token rate |
| `reasoning.started`, `reasoning.summary`, `reasoning.finished` | reasoning variants | summary text only where present |
| `compaction.started`, `compaction.finished` | compaction variants | trigger/input chars or notice |
| `tools.batch_started`, `tool.called`, `tool.completed` | tool variants | count; name/args; name/ok/duration/output chars/nullable error |
| `connection.retry`, `connection.recovered`, `transport.fallback` | same-named variants | all source fields |
| `run.notice`, `run.warning`, `run.profile` | `System`, `Warning`, `Profile` | `text` |

Add `change_set.recorded`, `validation.recorded`, `checkpoint.recorded`,
`approval.requested`, `approval.resolved`, and `usage.refreshed` only after
the backend earns them. Every such record needs a stable ID, provenance,
timestamp, and source command/tool; generic message text is not review evidence.

## Usage, price, and fork rules

- Present local history as **source-reported token observations**, including
  report availability and scan diagnostics next to the figure.
- Price remains `Unavailable` until a separately specified, versioned price
  source and cost methodology exist. Do not multiply token fields by an assumed
  rate or imply a subscription has a per-run charge.
- Preserve the existing scanner’s replay de-duplication across known
  `forked_from_id` lineage and its fork diagnostics in Spark Bench and the
  aggregate Proofline usage panel.
- Do not render a per-thread fork tree in v1. First add durable Spark-side
  `thread_id`, optional `parent_thread_id`, and an auditable mapping policy to
  imported Codex session identities.

## Implementation sequence

1. Create a desktop-facing Rust module with serializable `ProoflineSnapshot`
   and `ProoflineDelta`; retain internal `AgentDisplayEvent` and add exhaustive
   mapping tests.
2. Add read-only `list_sessions`, `load_snapshot`, and `scan_usage_history`
   commands. Use `SessionStore::list` metadata; do not expose SQLite or raw
   history paths to the frontend.
3. Add `start_run` with Channel relay, IDs/sequences, bounded buffering,
   cancellation, terminal states, and snapshot recovery.
4. Render the selected Proofline hierarchy. Mark changed files, validations,
   checkpoints, approvals, and pricing absent until typed backend records exist.
5. Add a narrow mode-policy command. Ask is read-only; Work needs a deliberate
   local-authority explanation. Do not equate either with an OS sandbox. Add
   approve/deny only with a tool-dispatch interception point and durable record.
6. Add opt-in trace opening/redaction and per-thread lineage only after schema
   and retention review, then run the planned Proofline validation protocol.

## Explicit unknowns and non-goals

- No Tauri crate, desktop host, frontend-to-Rust command API, or wire schema
  exists in this repository yet.
- No source-backed changed-file/diff/checkpoint/validation/approval model
  exists. The mock is direction, not evidence that those controls work.
- `AgentSnapshot.input` is provider conversation state, not a privacy-minimized
  GUI transcript; an adapter needs redaction and retention policy.
- Local usage is aggregate-only and may be partial due to file bounds, malformed
  input, duplicates, missing metadata, counter resets, or unknown fork replay
  evidence. It is deliberately not billing.
- `cmd.exec` has no OS-level confinement without the separately configured
  Docker execution mode. The GUI alone cannot promise “private” or “full
  access” as security properties.
- The first release does not replace CLI/TUI; it is a proof/review surface that
  shares the existing runner and preserves `spark chat` as a natural agent.

## Sources

- Local: `src/agent/mod.rs`, `src/agent/run_loop.rs`, `src/agent/trace.rs`,
  `src/chat/tui.rs`, `src/session/store.rs`, `src/usage_history.rs`,
  `src/tools/policy.rs`, `src/tools/mod.rs`, `src/agent/subagent.rs`, and
  `src/mcp_server/protocol.rs`.
- Tauri v2 documentation consulted 2026-08-01 via Context7: Channels and the
  async-listener ordering note; managed Rust state; permissions/capabilities.
