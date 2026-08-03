# Spark Desktop

Status: released as a focused fork of T3Code in August 2026.

## Product owner

Spark Desktop lives in [ifBars/t3code](https://github.com/ifBars/t3code). The
desktop app is released independently from this Rust harness so each repository
has one clear responsibility:

- `codex-spark-agent` owns the Spark CLI, native tool loop, session protocol,
  profiling, and benchmark evidence;
- `ifBars/t3code` owns the desktop application, renderer, packaging, and
  platform releases.

The first Spark Desktop release is
[v0.8.0](https://github.com/ifBars/t3code/releases/tag/v0.8.0).

## Architecture

The T3Code fork keeps execution server-authoritative. Providers, terminals,
filesystem access, Git operations, checkpoints, and event ordering remain
outside the renderer. Clients render projections and send typed commands.

The fork intentionally retains only the Codex and Spark provider paths. This
repository exposes a versioned streamed desktop protocol for Spark integration;
it does not ship a second desktop shell.

## Product boundaries

- Keep Spark's execution and evidence semantics in the harness.
- Keep desktop layout, application lifecycle, and platform packaging in the
  T3Code fork.
- Add protocol fields rather than coupling the desktop app to Rust internals.
- Preserve explicit unavailable and partial states for usage, pricing, and
  validation evidence.
- Do not recreate a parallel desktop implementation in this repository.

## Validation

Harness changes use the Rust validation suite. Desktop changes use the fork's
`bun fmt`, `bun lint`, `bun typecheck`, and `bun run test` checks plus its
platform release workflow.
