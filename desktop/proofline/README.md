# Proofline for Spark

Proofline is an interactive product prototype for a local evidence-and-review surface over the Spark harness. It preserves the selected outcome-first concept: recent tasks in the rail, a completed outcome, changed-file and validation evidence, a collapsible work trace, an instruction composer, and visible usage and authority boundaries.

This directory is a renderer prototype with simulated fixture data. It is not connected to the Rust agent loop, session store, filesystem, tools, pricing, or provider account in a browser build. The Wave 1 renderer contains a narrow Tauri adapter that can call `wave1_preflight`, `wave1_start_session`, `wave1_append_event`, `wave1_preview_aggregate`, and `wave1_purge_session` when the host exposes them. The proposed integration boundary is specified in [proofline-event-contract.md](../../docs/product/proofline-event-contract.md), and the user-validation gate is specified in [proofline-validation.md](../../docs/product/proofline-validation.md).

## Local preview

```powershell
bun install
bun run dev
```

Use `bun run build` and `bun run test:sites` for the verified production-shaped build and worker checks.

## Prototype truth boundaries

- The rich fork-history screen is explicitly labeled simulated fixture data. Its file paths, validation durations, checkpoint, and token values are illustrative—not local repository or provider facts.
- Other task-rail entries deliberately render no files, validation, work trace, checkpoint, or token values until fixture or runtime records exist; they cannot inherit the fork fixture.
- Any displayed token values are fixture observations, not billing or account quota.
- Pricing stays unavailable until Spark has a versioned price source and methodology.
- `Ask (read-only tools)` and `Work (OS-user access)` describe capability modes. Neither is an OS-sandbox or privacy guarantee; Work is explicitly not sandboxed or privately confined.
- The file inspector is a simulated review drawer, not a local file or diff viewer.
- Composer submission stages an instruction only inside the prototype; it does not send a Spark request.
- The browser/Sites adapter is deliberately rehearsal-only and non-countable. It never falls back to synthetic measurements or network telemetry when a Tauri host is unavailable.
- A countable Wave 1 session requires host-verified fixture/build metadata, a host-supplied random session namespace, a participant ID in the `P01`–`P99` range, and host acknowledgements for app-ready and first visible activity timing.
- The renderer sends and previews only fixed categorical outcomes (success, failure, hinted, abandoned). Aggregate previews retain hint and abandonment denominators and reject prompts, paths, commands, diffs, raw token values, identities, timestamps, and free-text telemetry.
- Purge is host-owned. The renderer clears its UI state only after a purge acknowledgement; browser reload retains no measurement ledger.
