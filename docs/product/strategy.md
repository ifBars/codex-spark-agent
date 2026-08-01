# Spark product strategy

Status: working product direction, August 2026

## Product thesis

Spark is the fast worker plane for software agents: a local, observable runtime
that uses ultra-fast models for repository reconnaissance and tight feedback
loops, then hands compact evidence to an engineer or stronger parent agent.

Spark should complement Codex, Claude, and other coding agents rather than
compete as another editor or general-purpose autonomous developer.

The product layers have distinct jobs:

- **Spark CLI** is the local reference experience.
- **Spark Agent** is the worker runtime and model-routing surface.
- **Spark Harness** supplies traces, deterministic validation, replay, budgets,
  and regression evidence.
- **Spark Bench** publishes the evidence; it is not the core product.

## Initial customer

The first design partner is a developer-productivity, engineering-enablement,
or AI-platform lead at a 50-500 engineer organization that:

- regularly uses at least two coding agents;
- can identify recent verification or rework costs;
- has repositories with CI, tests, review artifacts, or other objective gates;
- can nominate at least three engineers for a two-week pilot; and
- will measure engineer time and accepted artifacts instead of relying only on
  satisfaction surveys.

This is not initially for teams using only autocomplete or seeking a replacement
IDE.

## Entry wedge

The first product workflow is **Repo Brief**: bounded, read-only repository
reconnaissance that returns:

1. a direct answer;
2. repository-relative file and line evidence;
3. risks or unknowns; and
4. the next highest-value inspection or validation step.

Repo Brief is useful on its own and as a compact handoff to a parent coding
agent. It is the lowest-risk way to test whether fast intelligence produces
verified useful work rather than merely more output.

The next workflow candidates are:

- CI and test-failure triage;
- evidence-gated pull-request review;
- dependency or API upgrade reconnaissance; and
- parallel specialist verification with visible disagreements.

## Differentiation

Speed and parallel execution are necessary but not durable advantages. The
defensible product must combine them with:

- provider-neutral worker and handoff protocols;
- deterministic, coding-specific artifact validation;
- complete attempt, failure, latency, and usage accounting;
- replayable traces that turn real failures into regression cases;
- explicit workspace, tool, privacy, and budget boundaries; and
- routing policies measured against real workflow outcomes.

## Market map

The funded market is already crowded at both ends:

- OpenAI Codex and Cursor own broad developer-facing surfaces across CLI, IDE,
  cloud, and enterprise administration.
- Cognition and Factory sell managed, parallel autonomous software workers.
- LangSmith and Arize Phoenix provide broad, model-neutral tracing and
  evaluation infrastructure.

Company-reported traction and funding make a frontal attack on those categories
an unattractive starting point: [OpenAI reports more than four million weekly
Codex users](https://openai.com/index/scaling-codex-to-enterprises-worldwide/),
[Cognition reports more than $1 billion raised](https://cognition.com/blog/series-d),
[Factory announced a $150 million Series C](https://factory.ai/news/series-c),
[LangChain announced a $125 million round](https://www.langchain.com/blog/series-b),
and [Arize announced a $70 million Series C](https://arize.com/blog/arize-ai-raises-70m-series-c-to-build-the-gold-standard-for-ai-evaluation-observability/).
These are vendor claims, not independently audited market measurements.

Spark's opening is between those layers: a fast verification worker that plugs
into the agents teams already use, answers bounded repository questions, and
returns inspectable evidence with deterministic product gates. The product
promise is cheaper supervision and faster verification, not another primary
coding surface.

### Worker-routing observation

Luna is useful as a background research lane, not an interactive default. In
four observed delegated tasks, a focused competitive scan took 69 seconds, a
12-candidate prospect pipeline took 252 seconds, a source-level CodexBar usage
audit took 239 seconds, and a broad repository/product audit took 461 seconds.
The task surface exposed no token or dollar-cost data,
so a cheaper-Luna claim remains unverified. Keep Spark interactive; route
bounded research, review, and planning to Luna when multi-minute latency is
acceptable, and retain Terra for implementation until measured evidence
supports a different policy.

## North-star metric

The north-star metric is **verified useful work per engineer minute** (VWPM):

```text
VWPM = verified useful units / active engineer minutes
```

A verified useful unit must:

1. pass its deterministic workflow gate;
2. be accepted by the designated engineer; and
3. lead to a concrete next action, merged change, closed investigation, or
   documented decision.

Supporting metrics are:

- time to verified artifact;
- all-attempt deterministic gate pass rate;
- acceptance without substantive rework;
- parent-agent context or token savings;
- human intervention and retry rate;
- worker safety or policy incidents; and
- priced cost per verified unit once a versioned rate card exists.

Raw completions, successful-only averages, tokens, and benchmark scores are not
useful work by themselves.

## Evidence and claim gates

Current local benchmarks support only scoped, dated observations. Public claims
must retain the full attempt denominator and distinguish task failures from
provider failures. Token counts must not be described as dollar cost without a
versioned pricing source and formula.

The [usage and cost accounting contract](usage-accounting.md) separates account
quota, authoritative response tokens, and API-equivalent estimates. Spark has
no public API rate as of the contract date, so its dollar cost remains
unavailable rather than zero or inherited from another Codex model.

The first authenticated [Repo Brief dogfood observation](repo-brief-dogfood-2026-08-01.md)
passed its structural and safety contracts but failed human acceptance while
using 20 model requests and 37 tool calls. Product gates must therefore keep
contract validity, claim correctness, and acceptance without rework as separate
measurements.

Before making a broad speed, quality, or cost claim:

- run a balanced, version-pinned paired matrix;
- retain every attempt and exclusion;
- report paired duration, usage, quality, and confidence intervals;
- validate on unseen holdout tasks; and
- blind-review a sample of passing artifacts for validator disagreement.

If measurement is partial, the product must show unavailable or partial rather
than fabricate zero.

## 30/60/90-day sequence

### Days 0-30: safe local worker beta

- Ship Repo Brief as a standalone CLI and Codex MCP workflow.
- Enforce read-only, local-filesystem-only tools in the harness.
- Publish stable text and JSON output contracts with latency and usage data.
- Add trace retention, redaction, and purge controls.
- Ship account quota and run-usage visibility with explicit unavailable,
  partial, and estimated states.
- Complete five customer interviews and recruit a measured beta cohort.
- Repair benchmark provenance and authoritative response-usage accounting.

### Days 31-60: controlled worker plane

- Persist worker lifecycle, child traces, cancellation, and retry state.
- Add explicit Spark, Luna, and frontier-worker routing policies.
- Isolate concurrent workspaces and surface ownership or conflict state.
- Standardize compact handoffs and deterministic workflow validators.
- Run a two-week beta with three to five design partners.

### Days 61-90: evidence platform

- Ingest traces from multiple coding-agent providers.
- Turn failed production tasks into replayable regression suites.
- Add team policy, retention, budget, and audit controls.
- Extend Spark Bench into a private team evidence view.
- Test paid hosted governance and analytics around the open local runner.

## Explicit non-goals

- Another general-purpose IDE or chat interface.
- A claim that Spark replaces Codex, Claude, or a frontier model.
- Autonomous multi-agent writes before the read-only worker plane is measured.
- Generic LLM observability without coding-specific validators.
- A cloud control plane before local privacy, provenance, and safety contracts
  are credible.
- Optimizing benchmark scores without corresponding user-workflow evidence.

## Decision rule

Continue investing in a workflow only when it increases measured VWPM while
preserving its deterministic gate and safety boundary. A faster unverified
answer is not product progress.
