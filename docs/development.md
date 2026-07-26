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

Available suites include `core`, `survey`, `scaffolding`, `editing`, `reasoning`, and `real-world`. Reasoning scenarios can report weighted validation checks while the normal exit code still records full completion.

The quick comparison script preflights native Codex before spending a Spark run and writes resumable status artifacts when the provider is unavailable. Comparison reports keep provider failures separate from task failures and record runner versions, scenario coverage, and input freshness.

Reports are written under `.spark-profile/`. Keep reports and traces private unless you have reviewed their contents.

Run the quick harness benchmark before and after changes to the agent loop, native tools, command execution, compaction, tracing, profiling, or benchmark scoring when practical. Docs-only changes do not need it.
