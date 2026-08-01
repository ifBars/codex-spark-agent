# Spark design-partner pipeline

Research cut: 2026-08-01. This is a public-source prospecting artifact, not
evidence that any candidate wants the product. Vendor case-study metrics are
company-reported and not independently audited. No outreach has been sent.

Track stage changes in the machine-readable
[`design-partner-pipeline.csv`](design-partner-pipeline.csv); do not mark a
candidate contacted, interviewed, or validated without the corresponding
external evidence.

Scores are PM inference on a 0-10 scale: pain evidence / measurability / public
access likelihood / data-boundary fit.

## Candidate pipeline

| Candidate | Public fit evidence | First workflow | Scores | Main disqualifier |
| --- | --- | --- | --- | --- |
| PlanetScale | [Cursor reports](https://cursor.com/blog/planetscale) 2,000+ monthly PR reviews and quantified review-load savings | Repo Brief plus PR/CI evidence digest | 9 / 9 / 7 / 8 | Existing review automation may already be sufficient |
| Amplitude | [Cursor reports](https://cursor.com/blog/amplitude) 1,000+ autonomous runs per week with risk classification and merge metrics | Agent-run evidence ledger from issue through CI and merge | 9 / 9 / 6 / 7 | Strong internal platform may build this itself |
| Money Forward | [Cursor reports](https://cursor.com/blog/money-forward) multi-tool evaluation, asynchronous agents, and agent-generated tests | Affected-module brief plus test-gate evidence | 8 / 9 / 6 / 8 | Enterprise procurement and internal evaluation capability |
| Notion | [Cursor reports](https://cursor.com/blog/notion) a provider-neutral harness for task delegation, repo exploration, and bug triage | Issue-context Repo Brief followed by CI triage | 8 / 8 / 6 / 7 | Existing integration may make Spark redundant |
| Coinbase | [Cursor reports](https://cursor.com/blog/coinbase) concurrent agents, agent-authored PRs, and internal evaluation infrastructure | Evidence audit for agent-authored PRs | 9 / 9 / 4 / 6 | Regulated boundary and weak public engineering route |
| Virgin Atlantic | [OpenAI reports](https://openai.com/index/virgin-atlantic/) Codex use for testing, refactoring, and internal applications | Read-only CI triage and test-evidence brief | 6 / 8 / 4 / 8 | Only one coding-agent family is publicly confirmed |
| Dropbox | [Cursor reports](https://cursor.com/blog/dropbox) broad engineering adoption across review, testing, docs, and migrations | Monorepo ownership and affected-surface brief | 8 / 8 / 5 / 6 | Much larger than the initial customer band |
| OpenHands | Maintains a public [agent SDK](https://github.com/OpenHands/software-agent-sdk/) and [benchmark suite](https://github.com/OpenHands/benchmarks) | Reproducible evidence packet for benchmark runs | 9 / 10 / 9 / 9 | Strong workflow validator, weak willingness-to-pay validator |
| Aider | Maintains a multi-model coding agent and [test-backed benchmark](https://github.com/Aider-AI/aider/tree/main/benchmark) | Patch, test, latency, usage, and failure evidence | 8 / 10 / 9 / 9 | Small team with an existing benchmark |
| Continue | Maintains an [open coding-agent platform](https://github.com/continuedev/continue) and source-controlled AI checks | Read-only explanation of failed agentic CI checks | 9 / 9 / 9 / 9 | Directly adjacent product surface |
| Cline | Maintains an [agent SDK, IDE extension, and CLI](https://github.com/cline/cline) | Cross-run evidence summary for background agents | 8 / 8 / 9 / 8 | Owns its agent loop and may not want an external worker |
| SWE-bench | Provides [reproducible issue and patch evaluation](https://github.com/SWE-bench/SWE-bench) | Benchmark provenance and failure-classification brief | 9 / 10 / 9 / 10 | Research infrastructure, not a conventional buyer |

## Sequencing

Use OpenHands, Continue, and Aider as accessible instrumentation partners. They
can invalidate the evidence schema quickly, but they do not prove a commercial
buyer. Pursue PlanetScale, Amplitude, and Money Forward in parallel as the first
economic-buyer interviews because each has public evidence of agent volume and
objective engineering gates.

Public inbound routes exist for
[PlanetScale](https://planetscale.com/contact),
[Amplitude](https://www.amplitude.com/contact), and
[Money Forward](https://corp.moneyforward.com/en/contact/). These routes are
recorded for planning only; using them requires separate outreach authorization.

## First three measured experiments

### 1. Agent-run evidence packet

- Partner profile: OpenHands or Aider.
- Sample: 30 completed benchmark tasks, stratified by pass/fail.
- Baseline: existing run artifact and evaluator output.
- Spark condition: Repo Brief plus normalized trace, usage, failure class, and
  deterministic gate evidence.
- Pass: at least 70% accepted as correct, at least 60% accepted without
  substantive rework, and at least 25% higher VWPM.
- Disproof: maintainers cannot identify a decision made faster or more reliably
  from the packet.

### 2. Agentic CI-failure triage

- Partner profile: Continue or an internal developer-platform team.
- Sample: 30 recent failed agentic checks or CI runs.
- Baseline: current engineer investigation.
- Spark condition: read-only root-cause candidates, exact log/repo evidence,
  and one executable next check.
- Pass: at least 70% evidence-gate pass, at least 60% accepted without
  substantive rework, and median time to accepted next action at least 25%
  faster.
- Disproof: cited evidence does not change the engineer's next action.

### 3. High-volume PR verification

- Partner profile: PlanetScale or Amplitude.
- Sample: 30 low- or medium-risk agent-authored PRs.
- Baseline: existing review and CI workflow.
- Spark condition: affected-surface Repo Brief, test/gate status, explicit
  unknowns, and reviewer decision.
- Pass: at least 25% higher VWPM, at least 60% acceptance without substantive
  rework, repeat use by three engineers in week two, and zero critical safety
  incidents.
- Disproof: Spark duplicates existing bots without reducing review time or
  increasing trusted evidence.

Together these experiments form the 90-task market-validation target. Results
must be reported per partner as well as in aggregate; one strong cohort cannot
hide two failed ones.

## Outreach hooks

- OpenHands: Spark can turn each benchmark run into a compact reproducible
  evidence packet without replacing the agent or evaluator.
- Continue: Spark can explain failed agentic checks with exact repository and
  test evidence while remaining read-only.
- Aider: Spark can add normalized latency, usage, trace, and test-gate evidence
  around the existing multi-model benchmark.
- PlanetScale: Spark can test whether a compact evidence brief reduces the
  remaining human review load around semantic-risk PRs.
- Amplitude: Spark can provide a measurable evidence ledger from autonomous
  task intake through risk classification, CI, review, and merge.
