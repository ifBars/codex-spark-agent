# Spark desktop direction

Status: Proofline selected for implementation, August 2026

## Product decision

Spark Desktop should be a calm, local-first workspace for the Spark harness, not
an IDE replacement. The default surface is a narrow project and thread rail, one
readable transcript, a bottom composer, compact live activity, and an optional
evidence drawer. The Rust harness remains authoritative for execution, state,
permissions, checkpoints, usage, and event ordering.

The desktop product should make Spark's actual advantage visible: fast useful
work with inspectable evidence. It should not add enough editor chrome or
provider orchestration to hide that advantage.

## Patterns to borrow

- [T3 Code](https://t3.codes/) treats the UI as a control plane over agent
  runtimes. Its [architecture](https://github.com/pingdotgg/t3code/blob/main/docs/internals/overview.md)
  keeps sessions, workspaces, terminals, version control, and checkpoints in an
  authoritative server and streams typed events to clients.
- [Cursor Agent](https://docs.cursor.com/agent) makes operating mode explicit;
  its [checkpoints](https://docs.cursor.com/en/agent/chat/checkpoints) and usage
  surfaces give recovery and resource use visible product states.
- [Codex app-server](https://github.com/openai/codex/blob/main/codex-rs/app-server/README.md)
  models approvals, file changes, tools, usage, and thread lifecycle as protocol
  events rather than generic chat text.
- [CodexBar](https://github.com/steipete/CodexBar) demonstrates the value of a
  compact, source-labeled usage surface. Spark must retain stricter unavailable
  and partial states where authoritative price or quota data is missing.

Spark should reject full IDE chrome in its primary view, ambiguous full-access
modes, raw reasoning transcripts, generic spinners, cloud execution by default,
and cost comparisons that treat unlike token sources as equivalent.

## Architecture

Use a Tauri 2 shell with a web renderer over a Rust-owned application boundary.
Tauri's [IPC model](https://v2.tauri.app/concept/inter-process-communication/)
and [capability permissions](https://v2.tauri.app/security/capabilities/) fit a
local harness whose renderer must not own shell, filesystem, credential, or
permission authority.

The first extraction should create a reusable Rust core boundary for:

- run and thread lifecycle;
- ordered assistant, tool, approval, diff, checkpoint, and usage events;
- process supervision and cancellation;
- filesystem, Git, and permission policy;
- local persistence and artifact references; and
- benchmark and validation evidence.

The renderer owns presentation, filtering, interaction, and optimistic UI only.
Events need monotonic sequence identifiers and durable terminal states so the
transcript cannot disagree with the actual workspace. Thread metadata and event
indexes can live in SQLite; large logs, diffs, and benchmark evidence remain
local artifacts.

An authenticated web or mobile client can be added later against the same local
runtime. It is not part of the first release.

## MVP

The list below is the destination for the first usable desktop release, not one
ungated implementation batch. The execution sequence is:

1. **Prototype and contract:** preserve the selected hierarchy, remove
   misleading mock evidence, and expose a privacy-minimized read-only snapshot
   from the Rust harness.
2. **Internal Wave 1:** run five real participants through evidence inspection,
   failed validation, approval, and partial-usage tasks. Fix and repeat when a
   gate misses.
3. **Read-only Tauri shell:** only after Wave 1 passes, stream ordered Repo Brief
   and validation evidence through a Rust-owned Channel with a fresh snapshot
   as the recovery boundary.
4. **Controlled actions:** add approvals, checkpoints, and mutation only after
   typed backend records, explicit authority, cancellation, and restore paths
   exist and pass the next validation wave.

This sequence prevents a polished chat shell from outrunning the worker-plane
evidence that differentiates Spark.

The first usable desktop slice includes:

- local project selection and project-scoped thread history;
- new, resume, rename, archive, and search thread actions;
- a streaming transcript with collapsed, expandable tool activity;
- visible model, reasoning, and permission controls in the composer;
- inline command and file-change approvals;
- stop, cancel, retry, and continue controls;
- turn-level changes and local checkpoint restore;
- per-thread latency, source-reported usage, and validation outcome; and
- explicit local, model-network, and remote-environment status.

Defer multi-provider orchestration, remote access, worktree and pull-request
automation, a plugin marketplace, full benchmark dashboards, and complex
multi-agent boards. Multiple active runs should still be represented in the
core event model so a compact queue can evolve without a protocol rewrite.

## Selected visual direction: Proofline

Proofline is the implementation target. It uses a warm paper-white surface,
quiet orange accent, narrow chronological thread rail, and document-like work
review. A completed turn leads with its outcome, changed files, validation, and
checkpoint evidence; the underlying model steps and tool calls remain collapsed
under **How Spark worked** until requested. The bottom composer keeps model,
reasoning, and workspace authority visible, while a persistent status ribbon
shows branch, checkpoint, elapsed time, source-reported tokens, pricing
availability, and local/privacy state.

This direction gives Spark a recognizable evidence-first identity without
turning the primary experience into an IDE or monitoring console. The first
prototype should match the selected mockup's density and hierarchy before adding
secondary surfaces.

## Explored visual concepts

1. **Velocity Desk**: a light daily-driver workspace closest to the supplied
   compact reference. It prioritizes the project/thread rail, transcript, and
   composer.
2. **Pulse Stack**: a dark operations workspace with compact active-worker
   status and a persistent evidence drawer. It makes parallel work clearest.
3. **Proofline (selected)**: a warm, document-like review workspace where
   completed work, validation, and checkpoints read as a calm evidence record.

The implementation should be tested against startup time, time to first visible token,
approval discoverability, transcript readability, and the cost of inspecting a
diff or failed validation.

## Product risks

- The GUI becomes slower or noisier than the CLI it wraps.
- Pending approvals look like a stalled model rather than an actionable state.
- Checkpoints are mistaken for Git history.
- partial usage or missing price data is presented as complete or free.
- benchmark views encourage cherry-picked success-only claims.
- platform webview differences degrade the compact layout.

The desktop north star remains the same as the harness: verified useful work per
engineer minute, with local ownership and honest evidence.
