# Agent Instructions

This is a Rust CLI harness for GPT-5.3-Codex-Spark. Keep changes modular, traceable, and easy to profile.

## Tooling

- Use `cargo fmt`, `cargo check`, and `cargo test` for Rust validation.
- Never set or rely on `CARGO_TARGET_DIR`. Use the repository default `target/` directory and resolve any conflicts directly.
- If JavaScript tooling is added or used, use `bun` instead of npm, pnpm, or yarn.
- Do not use `codegraph status`; if codegraph is needed, use direct search/context/query commands.

## Quick Harness Benchmark

- For changes that affect the agent loop, native tools, command execution, compaction, tracing, profiling, or benchmark scoring, run the quick harness benchmark before and after the change when practical:

  ```powershell
  .\scripts\quick_harness_benchmark.ps1
  ```

- The quick benchmark runs a bounded, intentionally nontrivial `real-world` suite slice with one repeat: `precise-patch`, `multi-file-patch`, `github-issue-bugfix`, `rust-failing-test-bugfix`, `typescript-reducer-bugfix`, `merge-conflict-resolution`, `github-issue-triage`, `ci-failure-triage`, `pull-request-review`, `dependency-upgrade-triage`, `technical-essay`, `config-migration`, `experiment-rollout-audit`, `multi-module-bugfix`, `terminal-repair`, `multi-hop-analysis`, `policy-support-agent`, `rust-log-analyzer-scaffold`, and `rust-notes-tui-scaffold`. This is meant to track real harness performance across precise edits, coordinated multi-file work, issue-style repair, test-driven Rust/TypeScript fixes, merge repair, grounded triage, review, dependency migration triage, sourced writing, migrations, dirty-data experiment analysis, cross-module SWE-style repair, terminal-first environment repair, multi-hop grounded analysis, multi-turn policy compliance, and small-to-moderate Rust scaffolding without paying for the full suite.
- To inspect the live quick slice instead of trusting this prose, run:

  ```powershell
  .\scripts\quick_harness_benchmark.ps1 -ListScenarios
  ```
- The default slice should not be so easy that completion and quality scores are always perfect. If it starts saturating, add or rotate in a validated longer scenario rather than relying only on process-score movement.
- When evaluating a harness behavior change, run this command once before the change and once after the change, then compare the generated report rows under `.spark-profile/benchmarks/`.
- Use the before/after report rows to compare completion, quality, process score, tool failures, repeated tool calls, tool-only streaks, request duration, and max input tokens. Do not treat small timing deltas as regressions unless they repeat or come with worse quality/process signals.
- Skip this benchmark for docs-only edits, cosmetic TUI-only changes, and isolated test-only refactors unless the change touches benchmark-visible behavior.

## Structure

- Organize Rust code by domain, not by dumping new logic into `main.rs`.
- Prefer small module folders with `mod.rs` plus focused files when a domain grows.
- Keep public APIs narrow. Use `pub(crate)` or private items unless another module genuinely needs the symbol.
- Avoid adding new Cargo crates until there is a real ownership boundary, reusable library surface, or independent test/build concern. Start with modules first.

## File Size

- Target Rust source files at or below 150 KB.
- Do not make an already oversized file larger unless the same change also reduces or splits nearby code.
- When touching an oversized file, first look for a natural extraction boundary such as formatting, trace analysis, command parsing, tool implementations, auth, or session handling.
- New files should stay under 150 KB unless there is a clear short-term migration reason. If a migration temporarily creates a larger file, document the next split and keep moving toward the 150 KB target.

## Current Refactor Debt

- `src/main.rs` should be split into command modules and profile-scenario modules.
- `src/agent.rs` should be split into loop/session/compaction/trace writer modules.
- `src/tools.rs` should be split by native tool family.
- `src/profiler/` is being split into profiler core, trace analysis, formatting, and focused tests; continue that direction instead of recreating a monolithic profiler file.

## Spark Harness Direction

- Build around observed Spark behavior from traces. Do not try to prompt away quirks when the harness can profile, route, compact, or expose them more reliably.
- Do not add a synthetic completion tool unless profiling proves it is necessary. Prefer response completion behavior and trace diagnostics first.
- Keep compaction Codex-aligned: remote `/responses/compact` first, local pressure fallback only when needed, and trace every compaction boundary.
- Preserve natural chat/session behavior. Interactive `spark chat` should remain a normal conversation agent, not only a benchmark runner.

## Benchmark Sandbox Model

The benchmark sandbox protects scenario integrity through layered confinement:

### Filesystem (`fs.*`) Isolation

- **Workspace confinement**: All `fs.*` tools resolve paths against the benchmark workspace (`.spark-profile/benchmark-workspaces/<suite>-<stamp>/<scenario>-<repeat>/`). Paths that try to `..` escape the workspace are rejected at `src/tools/paths.rs:19-35` (`resolve_under`).
- **Read roots**: The `read_roots` mechanism (`src/tools/paths.rs:55-88`) controls which additional directories `fs.read`, `fs.list`, and `fs.search` can access beyond the workspace. For fixture scenarios (bugfixes, scaffolds, triage, writing, etc.), read roots are **empty** — the `fs.*` tools cannot resolve any path outside the workspace. For repo-survey scenarios (`RepoSurvey`, `SteamNetworkLibSurvey`, `S1ApiSurvey`, `RepoArchitectureSurvey`, `BenchmarkDesignSurvey`), the source repo root is added as a read root so the agent can analyze the codebase. Controlled by `benchmark_read_roots` at `src/benchmark/workspace.rs:43-53`.
- **Deny-list**: Even when read roots are granted, `resolve_read_path` at `src/tools/paths.rs:40-53` rejects resolution of paths under `.spark-runs`, `.spark-profile`, `.spark-scenarios`, or `.git` within any read root. This prevents survey scenarios from reading trace dirs, benchmark reports, or leftover fixture solution files.

### Command Execution (`cmd.exec`) — **Unsandboxed**

- `cmd.exec` runs in the benchmark workspace as its working directory but has **no shell or path restriction**. The agent can execute arbitrary commands, navigate the filesystem, and read/write anywhere the OS user can.
- For strict isolation, set `SPARK_CMD_EXEC_DOCKER_CONTAINER` to run `cmd.exec` inside a Docker container with a confined workdir.
- Profiler diagnostics scan `cmd.exec` traces for out-of-scope path probes (references to `.spark-runs`, `.spark-profile`, `.spark-scenarios` in command strings or workdir arguments).

### Trace Mirroring

- After each run, the trace directory is mirrored from the benchmark workspace back to `.spark-runs/` in the source repo via `mirror_trace_to_source` (`src/benchmark/workspace.rs:55-70`). The copy skips `.git`, `.codegraph`, `.spark-profile`, `.spark-runs`, `.spark-scenarios`, `target`, `node_modules`, `.vite`, and `dist` directories.
- Mirroring reads traces from the workspace copy, so the agent never sees prior traces from the source repo.
