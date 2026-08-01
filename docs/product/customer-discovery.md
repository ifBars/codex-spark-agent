# Spark customer discovery and beta rubric

This packet is for interviews with developer-productivity, engineering-
enablement, and AI-platform leads at 50–500 engineer organizations already
using multiple coding agents.

Use the public-source [design-partner pipeline](design-partner-pipeline.md) to
prioritize conversations; inclusion in that list is a research hypothesis, not
customer validation.

## Screening

Proceed when the contact:

- owns developer productivity, engineering enablement, AI platform, or internal
  developer tools;
- has at least two coding agents in regular engineering use;
- can describe a recent task that required verification, rework, or
  investigation;
- has repositories, CI, tests, issue trackers, or review artifacts available
  for a bounded pilot;
- can nominate at least three engineers for a two-week beta; and
- agrees to measure engineer minutes, accepted outputs, and gate results.

Prefer teams with at least 20 agent tasks per week and existing deterministic
automation.

Downgrade or stop when the team uses only autocomplete, cannot identify a
recent task or failure, has no objective artifact or check, requires autonomous
writes before a read-only pilot, or will provide only qualitative feedback.

## Outreach message

Subject: How your team verifies coding-agent work

> Hi [Name] — I am speaking with engineering-platform leaders about how teams
> run and verify multiple coding agents. I am not researching another editor. I
> want to understand what happens after an agent says "done": how much human
> time is spent checking the result, what evidence is trusted, and where
> parallel agents create coordination problems. Would you be open to a
> 30-minute conversation about one recent real example? We can keep it
> vendor-neutral and skip a product demo unless it becomes useful.

## 30-minute interview

### 0–3 minutes: context

- What does your team own?
- How many engineers and repositories are in scope?
- Which coding agents are actually used, and by whom?

### 3–9 minutes: recent workflow

- Walk through the last agent-assisted task from request to merge or rejection.
- What did the engineer inspect manually?
- Which tools, tests, or reviews were involved?
- Where did the process pause, repeat, or change direction?

Keep asking "What happened next?" until reaching a concrete artifact and
decision.

### 9–16 minutes: failure and verification

- Describe the last wrong, incomplete, or difficult-to-trust agent result.
- How was it discovered?
- How many engineer minutes were spent?
- What evidence would have shortened the investigation?
- Which checks are deterministic, and which require human judgment?

### 16–21 minutes: parallel work

- When do engineers use more than one agent on the same task or repository?
- What coordination problem appears first?
- Which work is safe to delegate to a fast, read-only worker?
- What would make the team refuse to run that worker?

Do not introduce Repo Brief before these questions.

### 21–26 minutes: workflow-card ranking

Show each workflow card below. For each, ask:

- Have you done this in the last month?
- How often?
- What does the current process cost in engineer time?
- What evidence makes the result acceptable?
- What makes it unusable?

Rank cards by current pain, not hypothetical interest.

### 26–30 minutes: beta and close

- Which repository and workflow are safest to pilot?
- What data may be logged, and what must remain local?
- Who approves the beta?
- What measured result justifies continued use after two weeks?
- Who else experiences this problem directly?
- Is there a recent anonymized artifact that can define the first gate?

## Workflow cards

### 1. Read-only Repo Brief

- Input: question, repository, and optional starting paths.
- Output: answer, file-and-line evidence, risks, and next inspection.
- Gate: cited paths and lines exist; required sections are present; no files
  change; no prohibited tools run.
- Metric: verified briefs per engineer minute.

### 2. CI or test-failure triage

- Input: failing log, repository, and recent change.
- Output: likely cause, supporting evidence, and next check.
- Gate: cited signature matches the log; the next check is executable; no
  unsupported root-cause claim is presented as confirmed.
- Metric: time from log receipt to an accepted next action.

### 3. Evidence-gated pull-request review

- Input: diff, affected files, tests, and repository instructions.
- Output: severity-ranked findings and missing-test risks.
- Gate: findings reference changed lines or relevant tests; no path is
  fabricated; a reviewer labels each finding actionable or non-actionable.
- Metric: accepted findings per review minute and escaped-finding rate.

### 4. Dependency or API upgrade reconnaissance

- Input: manifest, usage search, migration notes, and tests.
- Output: affected call sites, compatibility risks, and migration checklist.
- Gate: cited usages exist and the checklist covers known call sites.
- Metric: engineer minutes saved before implementation.

### 5. Parallel specialist verification

- Input: independent architecture, test, security, documentation, or API checks.
- Output: merged evidence packet with worker provenance and disagreements.
- Gate: claims have evidence; conflicts and duplicates are visible; worker
  failures remain in the packet.
- Metric: verified evidence units per engineer minute versus a single-worker
  baseline.

## Evidence ledger

Record interviews, baselines, and beta attempts in
[`beta-ledger.csv`](beta-ledger.csv). Keep participant and organization fields
pseudonymous in the repository; store contact details outside this artifact.

For every interview and beta attempt, capture:

- organization size, repositories, and data constraints;
- agents, active users, tasks per week, and task types;
- the prompt, artifact, duration, human steps, and final outcome;
- tests, lint, build, review, security, policy, and manual checks;
- engineer minutes, rework, delay, or escaped defect;
- concurrent-worker count, isolation method, and conflict rate;
- read-only, local-execution, and trace-retention boundaries;
- matched baseline and beta outcome; and
- champion, approver, follow-up date, and buying owner.

Label each entry `observed`, `measured`, or `reported`. "This would save time"
is not beta evidence.

## Beta score

Use five recent comparable tasks as a matched baseline. A verified useful unit
must pass its deterministic gate, be accepted by the designated engineer, and
lead to a concrete action or decision.

| Dimension | Weight | Passing signal |
| --- | ---: | --- |
| Verified useful work per engineer minute | 35 | At least 1.25× baseline |
| Deterministic gate pass rate | 20 | At least 70% of all attempts |
| Accepted without substantive rework | 15 | At least 60% |
| Time to verified artifact | 15 | At least 25% faster than baseline |
| Repeat usage | 10 | At least three engineers use it in week two |
| Safety and containment | 5 | No critical data, write, or policy incident |

The beta passes at 75/100 only when it also includes:

- at least 20 measured attempts;
- at least two workflow types;
- at least one Repo Brief workflow; and
- no critical safety incident.

Twenty attempts is the evidence floor for evaluating one beta cohort, not the
market-validation target. The 30-day product test should span three
organizations and 90 recent comparable tasks: 45 measured with the team's
current workflow and 45 with Spark assistance, randomized where practical.
The wedge is validated only if at least two organizations improve VWPM by 25%
or more while meeting the gate, acceptance, repeat-use, and safety thresholds
above.

A failed beta should still identify the failed hypothesis: wrong workflow,
insufficient evidence, poor latency, unsafe integration boundary, or no
measurable time advantage.
