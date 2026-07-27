# Development and internals

This document covers source builds, validation, profiling, benchmark workflows, and implementation details. For normal installation and use, start with the [README](../README.md).

## Build from source

Source builds require Rust stable with edition 2024 support:

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

## Native tools

The built-in tool set covers:

```text
fs.read       fs.write       fs.rename
fs.list       fs.replace     cmd.exec
fs.stat       fs.edit        browser.run
fs.search
```

File tools stay under the selected workspace and skip common generated folders unless asked to inspect one directly. `cmd.exec` uses PowerShell on Windows and bounds noisy output before returning it to the model.

`browser.run` performs a stateless Playwright Chromium pass for browser inspection and local UI smoke tests. It uses Bun for its local Playwright setup and can save screenshots inside the workspace.

Spark discovers MCP servers from global Codex config, `.mcp.json`, and `.spark/mcp.json`. It supports stdio servers and HTTP endpoints. Set `SPARK_DISABLE_MCP=1` to disable discovery for offline or controlled benchmark runs.

## Transport and compaction

Responses are WebSocket-first. Spark keeps the connection alive across tool turns and chains `previous_response_id`, so continuation requests send only new input. It falls back to HTTP/SSE if the socket fails before streaming begins.

Long sessions use remote `/responses/compact` first, with a local pressure fallback when needed. Compaction boundaries are included in traces. Spark runs until it completes, is cancelled, or reaches a context/input safety guard.

Compaction controls:

```powershell
spark chat --compact-after-tokens 40000 --max-input-tokens 125000
spark chat --compact-after-tool-only-turns 0 "Run without tool-streak compaction."
```

The default tool-only threshold is `12`. Setting it to `0` disables that guardrail for comparison work.

## Tracing and profiling

```powershell
spark chat --trace --profile "Fix the failing test."
spark traces --summary --limit 10
spark analyze-trace --timeline
```

The profiler tracks request size, token pressure, tool failures, repeated calls, command duration, compaction, and cache hits. Traces are written under `.spark-runs/`.

## Benchmarks

For a bounded regression pass across real development tasks:

```powershell
.\scripts\quick_harness_benchmark.ps1
```

Compare the same slice against the logged-in native Codex CLI:

```powershell
.\scripts\quick_comparison_benchmark.ps1
```

List the selected scenarios without running them:

```powershell
.\scripts\quick_harness_benchmark.ps1 -ListScenarios
.\scripts\quick_comparison_benchmark.ps1 -ListScenarios
```

Lower-level commands:

```powershell
spark profile-scenario repo-survey
spark profile-benchmark real-world --repeat 3
spark profile-benchmark-report --suite real-world
spark codex-cli-benchmark real-world --timeout-seconds 360
spark opencode-benchmark real-world --timeout-seconds 360 --pure
spark benchmark-compare --suite real-world --codex-cli-report .spark-profile/codex-cli/report.json
```

Available suites include `core`, `survey`, `scaffolding`, `editing`, `reasoning`, `coding`, `quantitative`, `analysis`, `operations`, `writing`, and `real-world`. The category suites intentionally overlap when a task exercises more than one real-world skill. Reasoning scenarios can report weighted validation checks while the normal exit code still records full completion.

`inventory-rebalance-plan` is the current difficult quantitative planning fixture.
It requires exhaustive constrained selection under two budgets and produces six
independently weighted validation signals. Its initial cross-runner development
evidence, including excluded prompt and provider failures, is documented in
[`benchmarks/inventory-rebalance-pilot-2026-07-26.md`](benchmarks/inventory-rebalance-pilot-2026-07-26.md).

`experiment-rollout-audit` adds a separate data-quality surface: assignment
conflicts and exclusions, event and order deduplication, half-open attribution
windows, refund joins, uplift calculations, and a multi-gate rollout decision.
Its expected metrics are recomputed from the published fixtures in tests rather
than trusted as unexplained constants. It is the current quick-slice quantitative
task because the inventory pilot saturated too often; the inventory scenario
remains available in the `quantitative`, `operations`, `reasoning`, and
`real-world` suites for broader runs.

Run a focused category exactly like any other suite:

```powershell
spark profile-benchmark coding --reasoning-effort medium --repeat 3
spark codex-cli-benchmark quantitative --reasoning-effort medium --repeat 3
```

The published web views are rebuilt from reviewed scenario rows with Bun:

```powershell
bun scripts/build_benchmark_views.mjs
```

The view specification, evidence manifest, generated category CSV, and web JSON are checked in so the aggregation and displayed provenance are reviewable without rerunning provider benchmarks. Verify that all generated artifacts are current with:

```powershell
bun scripts/build_benchmark_views.mjs --check
```

The quick comparison script preflights native Codex before spending a Spark run and writes resumable status artifacts when the provider is unavailable. Comparison reports keep provider failures separate from task failures and record runner versions, scenario coverage, and input freshness.

The quick harness script also fails after preserving its report when every row is
a request failure, preventing an infrastructure-only batch from appearing to be
task-performance evidence. Use `-AllowRequestFailureReport` only when intentionally
capturing provider diagnostics.

Reports are written under `.spark-profile/`. Keep reports and traces private unless you have reviewed their contents.

Run the quick harness benchmark before and after changes to the agent loop, native tools, command execution, compaction, tracing, profiling, or benchmark scoring when practical. Docs-only changes do not need it.
