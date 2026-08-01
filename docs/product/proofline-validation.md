# Proofline prototype validation plan

Status: planned validation; no participant sessions, interviews, or acceptance
results have been recorded as of 2026-08-01.

## Purpose and decision

Proofline is the selected desktop direction: a local-first, evidence-review
surface for the Spark runtime, not a replacement IDE. This plan tests whether
the selected hierarchy makes a fast worker *easier to verify* than the CLI and
whether it preserves the safety, provenance, and honest-usage boundaries in
the product strategy.

This plan is a prototype usability and instrumentation gate. It does **not**
replace the customer-discovery protocol, the five-task matched baseline, or
the beta score in [customer-discovery.md](customer-discovery.md). In
particular, a prototype passing these gates is not evidence of VWPM improvement
or market validation. Every subsequent pilot attempt belongs in
[beta-ledger.csv](beta-ledger.csv) with its actual evidence class.

The prototype decision is:

> Advance from a read-only Proofline shell to a limited pilot only if users can
> inspect outcome, provenance, changes, failed validation, approvals, and
> partial usage without being confused by the interface or its authority
> boundaries.

## Target participant screen

Recruit participants who are active software engineers, tech leads, developer
productivity engineers, or AI-platform engineers. They must have used at least
two coding-agent products in the previous month and have reviewed a change,
test failure, or agent output in the previous two weeks. At least half of a
wave should have authority to approve or reject agent work; at least one must
be a developer-platform or engineering-enablement lead.

Exclude a participant from this test when they only use autocomplete, cannot
describe an objective engineering gate, require autonomous writes to evaluate
the product, or cannot use a local, non-production repository fixture. Record
role, organization-size band, weekly agent-task band, prior agent products,
and data restrictions using a pseudonymous participant ID. Do not put contact
details, repository names, prompts, source code, or screen recordings in this
repository.

## Test environment and fixtures

- Use a local, disposable fixture repository with deterministic command output,
  simulated tool and approval events, one successful validation, and one
  failed validation. Do not use a participant's production repository in
  waves 1 or 2.
- Run the prototype offline except for the model connection that the participant
  explicitly sees. Do not issue provider benchmark calls for this plan.
- Test cold and warm launches on the same recorded hardware; report OS,
  CPU/RAM band, renderer version, fixture revision, and whether model output
  was live or replayed.
- Start every session with a fresh local profile. Retain raw event logs only on
  the test machine for 30 days unless the participant asks for earlier purge.
  The repository receives only aggregated, redacted findings.

## Five realistic prototype tasks

Each task has a concrete answer or artifact so completion is observable. The
facilitator must not teach the control location before the participant attempts
it. Rotate task order after the first two tasks to reduce learning bias.

| Task | Scenario | Participant outcome | Evidence gate |
| --- | --- | --- | --- |
| 1. Read a Repo Brief | Ask why a focused module is the likely ownership boundary for a known issue. | State the answer, one cited file/line, one uncertainty, and the next check. | Cited source exists in the fixture and the participant opens it from the transcript. |
| 2. Inspect a completed change | A run reports a completed two-file change with a checkpoint and passing validation. | Identify what changed, open the diff, and say whether the result is safe to review now. | Correct files and validation state are identified without opening a terminal. |
| 3. Recover from failed validation | A run changes a fixture then a deterministic test fails. | Find the failed command/output, distinguish failure from model completion, and choose retry, inspect diff, or restore checkpoint. | The participant takes an informed recovery action and can explain its effect. |
| 4. Resolve an approval | A pending shell or file-change approval appears during an otherwise streaming run. | Locate the approval, name the requested authority, and approve or deny it according to a printed policy card. | The choice matches the policy card; no participant believes the run is merely stalled. |
| 5. Interpret usage and hand off | A completed run has source-reported token counts, partial history coverage, and unavailable pricing. | Explain what is known, what is unavailable, and decide whether to share the evidence record with a reviewer. | The participant does not infer a dollar cost or full quota from unavailable/partial data. |

## 20-minute moderated usability script

The moderator records timestamps, observable actions, quotes, completion,
misconceptions, and needed hints. They do not describe Proofline as faster,
cheaper, safer, or more accurate.

| Minute | Moderator action | Capture |
| --- | --- | --- |
| 0-2 | Confirm consent, local-data boundary, role screen, and the participant's last agent-review workflow. Explain that the software—not the participant—is being tested. | Screen outcome, prior workflow, fixture mode. |
| 2-3 | Ask the participant to launch Proofline and narrate what they believe the initial screen contains. | Cold-start and first-visible-activity timestamps; first impression of authority and status ribbon. |
| 3-6 | Give Task 1. Ask: “What would you trust here, and what would you check next?” | Time to cited evidence; transcript reading problems. |
| 6-9 | Give Task 2. Do not name the changed-files, validation, or checkpoint sections. | Time to diff; whether completion is confused with validation. |
| 9-12 | Give Task 3. Ask the participant to think aloud before choosing a recovery action. | Failed-validation location, action selected, checkpoint understanding. |
| 12-15 | Give Task 4 with the policy card. Do not point to the approval control. | Approval discovery, decision correctness, any perceived stall. |
| 15-18 | Give Task 5. Ask: “Can you tell what this cost?” then “What could you honestly report to a manager?” | Token, quota, partialness, and pricing comprehension. |
| 18-20 | Ask for the highest-friction moment, one missing evidence item, and whether this would change the next action on a recent real task. Close without soliciting purchase intent. | Qualitative disproofs, candidate workflow, follow-up permission. |

Use a single neutral hint after 90 seconds of no progress. Record it as a
task failure for the unassisted metric even if the participant completes the
task after the hint. Stop the session if an unsafe action, unexpected external
access, or unrecoverable prototype crash occurs.

## Measurable gates and prototype thresholds

Instrument timings automatically and reconcile them with the moderator sheet.
Report each denominator, assisted/unassisted completion, and all failures. Do
not drop incomplete sessions or successful-only attempts.

| Gate | Operational definition | Pass threshold for a wave |
| --- | --- | --- |
| Startup | `app_ready` occurs after process start and the project/thread rail can accept input. Measure 10 cold and 10 warm launches per test build. | Cold median <= 3 s and p95 <= 5 s; warm median <= 1.5 s. Any startup crash is a blocker. |
| First visible activity | On a replayed run, the first ordered activity event is visibly rendered after the participant submits. | Median <= 500 ms and p95 <= 1 s from event receipt; no blank/indeterminate state longer than 2 s. |
| Approval discoverability | Participant finds and correctly resolves the pending approval in Task 4 without a hint. | >= 80% unassisted; median <= 30 s; zero participants describe it as an unexplained stall. |
| Transcript readability | Participant completes Task 1 and identifies the outcome, cited evidence, and uncertainty. | >= 80% unassisted; median <= 75 s; no more than one critical hierarchy/readability defect per five sessions. |
| Changed-file and diff inspection | Participant completes Task 2 and identifies the changed files and validation outcome. | >= 80% unassisted; median <= 60 s; no participant confuses checkpoint state with Git history. |
| Failed-validation recovery | Participant completes Task 3 by finding the failure and selecting an appropriate next action. | >= 80% unassisted; median <= 75 s; zero incorrect claims that a failed validation is a successful completion. |
| Usage/pricing comprehension | Participant completes Task 5 and correctly distinguishes source-reported tokens, partial history, quota, and unavailable pricing. | >= 90% correct; zero claims that unavailable price means free or that partial usage is complete. |
| Safety and privacy | Renderer action and logged payload remain within the local fixture, declared permissions, and schema below. | Zero critical data, write, or policy incidents. Any incident blocks the next wave. |

For a five-person wave, report counts next to percentages (for example, 4/5),
not a claim of statistical generalization. A gate that misses its threshold,
or two participants encountering the same severe confusion, creates a design
fix and a repeat test before progression. Performance thresholds are product
targets, not a substitute for the beta's VWPM, deterministic-gate,
acceptance-without-rework, repeat-use, and safety score.

## Event and telemetry contract

The Rust runtime is authoritative. The renderer may emit interaction events
but cannot synthesize lifecycle, approval, validation, checkpoint, usage, or
terminal state. All durable events use an increasing per-thread `sequence` and
an `event_id` suitable for deduplication.

```json
{
  "schema": "spark.proofline.validation.v1",
  "event_id": "uuid",
  "occurred_at": "RFC-3339 UTC",
  "session_id": "random local ID",
  "participant_id": "pseudonymous ID or null",
  "thread_id": "random local ID",
  "sequence": 42,
  "event_type": "app_ready|run_submitted|activity_rendered|approval_presented|approval_decided|transcript_evidence_opened|diff_opened|validation_reported|checkpoint_actioned|usage_viewed|task_outcome|privacy_boundary_checked",
  "run_state": "starting|running|awaiting_approval|completed|failed|cancelled",
  "surface": "rail|transcript|evidence_drawer|composer|status_ribbon",
  "latency_ms": 241,
  "result": "success|failure|abandoned|hinted",
  "metadata": {
    "task_id": "proofline-1..5",
    "approval_kind": "command|file_change|none",
    "validation_state": "passed|failed|not_run|unknown",
    "usage_state": "authoritative|partial|unavailable",
    "pricing_state": "available|unavailable"
  }
}
```

Allowed telemetry is limited to timestamps, pseudonymous IDs, event types,
state labels, durations, control/surface identifiers, task outcomes, and
coarse environment metadata (OS family, renderer version, hardware band).
Event payloads must not contain prompt text, assistant text, source paths,
diff content, repository names, shell commands, environment variables, model
credentials, tool arguments or outputs, raw token values, real user IDs, IP
addresses, recordings, or clipboard data. Keep any linkage from a pseudonym to
a real person outside Spark artifacts.

Store raw local events encrypted at rest where the OS supports it; retain them
for at most 30 days and expose a visible purge action. Export only aggregate
wave summaries and redacted defect labels. Mark token/pricing fields
`partial` or `unavailable` rather than inferring values. Never send telemetry
to a remote service by default.

## Three-wave sequence

### Wave 1 — internal dogfood and instrumentation shakeout

Run five internal participants against the disposable replay fixture. The aim
is to validate event ordering, privacy filtering, task scripts, and obvious
hierarchy defects—not to claim user demand or VWPM. Advance only when all
critical safety/privacy checks pass and at least six of the seven UX gates
pass. Open a concrete fix for every failure; rerun the affected task after a
fix. Add the attempt to the ledger as `dogfood` and `measured` only when its
timings and outcomes are actually captured.

### Wave 2 — instrumented external design-partner usability

Recruit five to eight screened engineers from accessible instrumentation
partners first (OpenHands, Continue, or Aider are hypotheses in the pipeline,
not contacts or commitments). Use the same fixture and script, then one
partner-supplied *sanitized*, read-only example if separately approved. Check
whether the evidence model is comprehensible across unfamiliar workflows.
Advance only if no critical safety incident occurs, all safety and
usage/pricing gates pass, and at least 80% unassisted completion holds for
approval, transcript, diff, and failed-validation tasks. Record actual
outreach/interview evidence before changing any prospect's pipeline stage.

### Wave 3 — two-week measured design-partner pilot

With three to five screened organizations, apply the existing beta protocol:
five matched baseline tasks before judging pilot progress, at least 20
measured attempts across at least two workflows, and at least one Repo Brief.
Use the existing 75/100 score, its 1.25x VWPM, 70% all-attempt gate pass, 60%
acceptance-without-substantive-rework, 25% time-to-verified-artifact, week-two
repeat-use, and zero-critical-incident requirements unchanged. The broader
30-day wedge remains unvalidated until three organizations complete the 90
recent-comparable-task study and at least two improve VWPM by 25% or more
while meeting every existing gate.

## Reporting and next decisions

After each wave, publish a short internal evidence note with build/fixture
versions, participant-screen counts, task denominators, gate results, timing
distribution, assisted completions, safety/privacy audit, unresolved defects,
and the decision to fix, repeat, advance, or stop. A result may say
"inconclusive"; it may not claim validation from a mockup, a prospect list,
or an internal replay.

The first build priority is therefore a read-only Proofline shell with a
replayable ordered-event fixture, an explicit approval state, a changed-file
and validation evidence path, and honest usage/pricing labels. Mutation,
multi-provider orchestration, and cloud telemetry remain out of scope until
this plan produces passing evidence.
