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
spark brief "Explain the command dispatch path." --path src
```

## Native tools

The built-in tool set covers:

```text
fs.read       fs.write       fs.rename
fs.list       fs.replace     cmd.exec
fs.stat       fs.edit        browser.run
fs.search     tool.search
```

File tools stay under the selected workspace and skip common generated folders unless asked to inspect one directly. `cmd.exec` uses PowerShell on Windows and bounds noisy output before returning it to the model.

Core workspace file and command tools are advertised on every local work turn. `tool.search` activates deferred GitHub, web, browser, subagent, and MCP capabilities only when the task needs them, keeping unrelated schemas out of Spark's request context.

`browser.run` performs a stateless Playwright Chromium pass for browser inspection and local UI smoke tests. It uses Bun for its local Playwright setup and can save screenshots inside the workspace.

Spark lazily discovers MCP servers from global Codex config, `.mcp.json`, and `.spark/mcp.json` when `tool.search` requests a specialist capability. It supports stdio servers and HTTP endpoints. Set `SPARK_DISABLE_MCP=1` to disable discovery for offline or controlled benchmark runs.

## Transport and compaction

Responses are WebSocket-first. Spark keeps the connection alive across tool turns and chains `previous_response_id`, so continuation requests send only new input. It falls back to HTTP/SSE if the socket fails before streaming begins. Spark responses also have a 120-second total deadline: a pre-output WebSocket stall retries once over HTTP with full history. Set `SPARK_RESPONSE_DEADLINE_SECONDS` to `10`–`900`, or `0` to disable this Spark-only guard.

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

Traces are retained locally until the user removes them; Spark has no automatic deletion job. Use `spark trace-retention --cwd .` to inspect the default 30-day policy. `--purge` is a dry run, and deletion requires the separate `--confirm` flag:

```powershell
spark trace-retention --cwd . --older-than-days 14 --purge
spark trace-retention --cwd . --older-than-days 14 --purge --confirm
```

Plain inspection prints only the policy and counts; the dry run and confirmed purge list exact candidates. The purge only accepts resolved, direct `.spark-runs/run-<timestamp>` children of the selected workspace. Treat all trace contents as private before retaining, sharing, or deleting them.

## Repository briefs

`spark brief` is a standalone, local-filesystem-only read pass for a concrete repository question.
It accepts repeated `--path` starting points, rejects absolute or parent-traversing paths before
authentication/provider work, and uses the default Spark model with `low`, `medium`, `high`, or
`xhigh` reasoning. It has a 120-second standalone deadline by default; this is a request deadline,
not an agent-loop turn cap.

```powershell
spark brief "Where does authentication enter the client?" --cwd . --path src --format text
spark brief "Trace auth handling." --path src/auth.rs --format json --trace
```

The model can use only `fs.read`, `fs.list`, `fs.stat`, and `fs.search`; the harness removes hosted
web search, MCP, subagents, shell, browser, and writes from the advertised schema. Repo Brief has a
deterministic budget of 16 local filesystem invocations. The first 16 dispatch normally; subsequent
calls receive a compact `tool_budget_reached` result and the next model request has an empty tool
schema, requiring natural synthesis from gathered evidence. Its compaction threshold is 320,000 JSON
characters (remote-first compaction remains available at that threshold), avoiding premature evidence
loss during a compact read pass. Text stdout is
only the Markdown brief. JSON stdout is one schema-versioned `repo_brief` object with profile and
authoritative provider response-usage data when the provider supplied it. The deterministic contract
diagnostic requires Answer, Evidence, Risks/unknowns, and Next inspection headings plus one or more
repository-relative `file:line` citations. Incomplete contracts and deadline/error reports have a
stable nonzero exit code but still emit their JSON envelope.

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

Compare two Spark harness configurations directly by labeling their saved reports:

```powershell
spark benchmark-compare --suite real-world `
  --harness-variant baseline=.spark-profile/benchmarks/real-world-before.json `
  --harness-variant progressive=.spark-profile/benchmarks/real-world-after.json `
  --baseline-runner baseline
```

Harness-variant reports normalize the selected baseline to a 100-point average
Resource Efficiency index. The paired index combines duration (70%), total input
tokens (20%), and tool calls (10%), with damping for extreme ratios. Benchmark
Index multiplies that resource result by a completion/quality/execution-hygiene
gate. Execution hygiene is the former process score; it remains separate so
token savings cannot hide failures, retries, repeated calls, or tool-loop churn.
Uncached input is reported alongside total input but is not silently substituted
into the index because cache-read footprint and novel input answer different
cost and context-pressure questions.

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

Publish the reviewed reasoning sweep with Bun:

```powershell
bun scripts/publish_reasoning_sweep.mjs `
  --input .spark-profile/benchmarks/real-world-comparison-<stamp>.json `
  --output-json web/src/data/reasoning-sweep.json `
  --output-csv docs/benchmarks/reasoning-sweep-current-2026-08-12.csv `
  --output-summary docs/benchmarks/reasoning-sweep-current-2026-08-12.md `
  --expected-repeats 3 `
  --expected-scenarios 12 `
  --date "August 12, 2026"
```

Generate the input with `benchmark-compare --group-by-reasoning` and do not pass
`--successful-only`. Outcome quality averages every weighted validator score,
including partial scores from failed tasks; pass rate reports full task success.
Provider/API failures are filtered as infrastructure and block publication.
The generated CSV, summary, and web JSON are checked in so the displayed data
can be reviewed without rerunning provider benchmarks. Validate the data adapter
with:

```powershell
cd web
bun run test:data
```

The quick comparison script preflights native Codex before spending a Spark run and writes resumable status artifacts when the provider is unavailable. Comparison reports keep provider failures separate from task failures and record runner versions, scenario coverage, and input freshness.

The quick harness script also fails after preserving its report when every row is
a request failure, preventing an infrastructure-only batch from appearing to be
task-performance evidence. Use `-AllowRequestFailureReport` only when intentionally
capturing provider diagnostics.

Reports are written under `.spark-profile/`. Keep reports and traces private unless you have reviewed their contents.

Run the quick harness benchmark before and after changes to the agent loop, native tools, command execution, compaction, tracing, profiling, or benchmark scoring when practical. Docs-only changes do not need it.
