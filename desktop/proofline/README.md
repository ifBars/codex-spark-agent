# Proofline for Spark

Proofline is an interactive product prototype for a local evidence-and-review surface over the Spark harness. It preserves the selected outcome-first concept: recent tasks in the rail, a completed outcome, changed-file and validation evidence, a collapsible work trace, an instruction composer, and visible usage and authority boundaries.

This directory is a renderer prototype with simulated fixture data. It is not connected to the Rust agent loop, session store, filesystem, tools, pricing, or provider account in a browser build. The native host exposes a narrow Wave 1 capture adapter plus a separate lifecycle protocol for host-stamped page-load, interactive-readiness, and visible-state receipts. The lifecycle protocol remains non-countable until its external calibration gates pass. The proposed integration boundary is specified in [proofline-event-contract.md](../../docs/product/proofline-event-contract.md), the lifecycle semantics in [proofline-lifecycle-contract.md](../../docs/product/proofline-lifecycle-contract.md), and the user-validation gate in [proofline-validation.md](../../docs/product/proofline-validation.md).

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
- The native footer says `Network gate pending` until process-level observation proves the exact runtime behavior; it must not use the selected concept's `Private` claim merely because application data is local.
- A countable Wave 1 session requires host-verified fixture/build metadata, a host-supplied random session namespace, a participant ID in the `P01`–`P99` range, calibrated host-stamped lifecycle receipts, a Proofline-specific external visual gate, and an enforceable runtime network boundary. Sampled socket polling is diagnostic only. The current build intentionally does not satisfy those final conditions.
- The renderer sends and previews only fixed categorical outcomes (success, failure, hinted, abandoned). Aggregate previews retain hint and abandonment denominators and reject prompts, paths, commands, diffs, raw token values, identities, timestamps, and free-text telemetry.
- Purge is host-owned. The renderer clears its UI state only after a purge acknowledgement; browser reload retains no measurement ledger.
