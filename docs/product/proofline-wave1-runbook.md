# Proofline Wave 1 facilitator runbook

Status: ready to facilitate once the **build and replay fixture prerequisites**
below are met. This is a five-person internal usability and instrumentation
wave, not product validation, a VWPM study, or permission to approach external
design partners. No participant session has been recorded by this runbook.

This is the operational companion to
[proofline-validation.md](proofline-validation.md). It turns its protocol and
[issue #6](https://github.com/ifBars/codex-spark-agent/issues/6) into a
repeatable session kit. If this runbook conflicts with the validation plan,
the validation plan wins and the discrepancy must be recorded in the evidence
note before a decision is made.

## What this wave can and cannot prove

Wave 1 is meant to catch ordering, hierarchy, authority-label, privacy-filter,
and measurement defects in a disposable local replay. It can support only a
decision to fix, repeat, advance to the *external usability* wave, or stop.

It cannot establish demand, willingness to pay, real-work productivity, safety
of autonomous writes, complete usage coverage, price, quota, or VWPM. A green
build, a passing test suite, an internal code review, and an agent simulation
are **not human evidence** and do not complete issue #6.

### Baseline and VWPM capture boundary

Wave 1 has no matched real-work baseline. For every Wave 1 evidence note,
record `VWPM = not measured`, `baseline = not collected`, and `reason =
disposable replay usability wave`. Do not infer a baseline from task-card
timings or count a successful fixture interaction as useful work. The first
valid VWPM comparison belongs to the later design-partner protocol in
[customer-discovery.md](customer-discovery.md): five recent comparable tasks
per condition, actual engineer minutes, deterministic gate outcome, and the
same verified useful-unit definition. Keep this wave's timing data strictly as
usability instrumentation and hand it forward only as a hypothesis/input to
that study.

## Roles and materials

| Role | Responsibility | May not do |
| --- | --- | --- |
| Facilitator | Runs the script verbatim, starts timers, records only allowed observations, and calls stops. | Teach the UI, solve tasks, or characterize Proofline as faster, cheaper, safer, or more accurate. |
| Note taker (optional) | Records the redacted worksheet while the facilitator moderates. | Record names, prompts, code, commands, paths, screens, or raw event payloads. |
| Build owner | Pins the build, replay bundle, environment, and telemetry allowlist before the first session. | Substitute a new build or fixture mid-wave. |
| Privacy owner | Verifies the local-only boundary and completes the purge checklist. | Export raw logs or the identity-to-pseudonym mapping to this repository. |
| Participant | Thinks aloud and performs the tasks using the printed policy card. | Use a production repository or provide credentials, source code, customer data, or a personal prompt. |

Prepare one facilitator packet per session:

- this runbook and the five task cards below;
- a one-page printed approval policy card;
- a fresh, disposable local profile and a fixed replay fixture;
- an offline timer with second resolution;
- one local raw-event location, encrypted at rest where the OS supports it;
- the redacted worksheet at [proofline-wave1-template.csv](proofline-wave1-template.csv);
- the empty aggregate report at
  [proofline-wave1-report-template.md](proofline-wave1-report-template.md).

Do not prepare screen recording, clipboard capture, remote telemetry, a shared
production folder, or a participant's real repository.

## Participant screen and recruitment draft

Fill five slots only after the candidate passes this screen:

- active software engineer, technical lead, developer-productivity engineer,
  or AI-platform engineer;
- has used two or more coding-agent products in the past month;
- reviewed a change, test failure, or agent output in the last two weeks;
- can evaluate a local, non-production fixture without autonomous writes;
- across the wave, at least three participants can approve/reject agent work
  and at least one is a developer-platform or enablement lead.

Exclude candidates who only use autocomplete, cannot name an objective
engineering gate, need autonomous writes to evaluate the surface, cannot use a
local fixture, or will need to expose confidential material. Assign a random
pseudonymous ID such as `P01`; keep the identity mapping and contact details
outside Spark artifacts.

Use this as **draft copy only**. It authorizes no contact and asserts no
product claim:

> We are running a 20-minute internal usability session on a local, disposable
> Proofline fixture. We are testing the interface, not you. Please do not use
> production code, credentials, customer data, or your own prompts. The
> session uses a pseudonymous ID and we retain only redacted aggregate notes.
> Are you open to participating if the local-data boundary works for you?

## Consent and data boundary (read verbatim)

> Today we are testing a prototype interface, not your performance. Please
> think aloud, but do not read, paste, or open production code, credentials,
> customer data, personal prompts, repository names, shell history, or any
> sensitive material. The fixture is disposable and local. We will record only
> timestamps, pseudonymous ID, task outcomes, allowed interaction categories,
> coarse environment information, and redacted defect labels. We will not put
> recordings, raw prompts, source paths, commands, token values, raw logs, or
> your identity in the repository. You may stop, skip a question, or request
> early deletion of local raw logs at any time. Do you consent to continue?

Record only `consent=yes/no`, not a quote or signature. If no, do not launch
the app; delete the empty local session folder and record a non-session screen
count only.

## Preflight: pin the build and replay fixture

The current CLI snapshot command is intentionally privacy-minimized and marks
conversation, changed files, validations, checkpoints, approvals, usage, and
fork lineage unavailable. The React prototype uses simulated fixture data.
Neither alone supplies the full ordered replay required by Tasks 1-5. The
build owner must therefore provide and validate a named, fixed replay bundle
before Wave 1 starts. Do not treat a hand-authored screen state as an adequate
substitute.

For the whole wave, pin the following in the local facilitator record and the
aggregate report:

| Item | Required value | Verification |
| --- | --- | --- |
| Source build | Full Git commit SHA and clean/dirty status. | `git rev-parse HEAD`; `git status --porcelain` must be empty except for an explicitly recorded evidence export outside source. |
| Frontend dependency state | `desktop/proofline/bun.lock` revision and installed dependency check. | `bun --cwd desktop/proofline install --frozen-lockfile`; then `bun --cwd desktop/proofline run build`. |
| Native measurement host | Windows Tauri build from the same clean commit. | `bun --cwd desktop/proofline tauri build --no-bundle`; launch the resulting binary and require host preflight `countable=true`. Browser/Sites mode is never valid evidence. |
| Replay fixture | Immutable fixture ID, revision, SHA-256, and fixture manifest. | Hash the bundle; verify manifest has all five scenarios and their expected outcomes before every session. |
| Runtime mode | `replayed` or `live-model`. | Wave 1 defaults to replayed. Any live-model use needs a separately approved run, visible provider boundary, and must not replace a replayed task result. |
| Environment | OS family/version, CPU/RAM band, display scale, renderer/runtime version, and network state. | Record a coarse environment ID, not hostname, user name, IP, or serial number. |
| Privacy allowlist | Event schema version and exported fields. | Compare the export against the allowlist in `proofline-validation.md`; reject any forbidden field. |

### Required replay manifest

The replay bundle must declare, and the facilitator must validate before the
first participant, all of the following:

1. ordered event sequence with monotonic per-thread IDs;
2. an outcome plus a cited fixture file/line and an uncertainty for Task 1;
3. a two-file change, passing validation, and checkpoint for Task 2;
4. a failed deterministic validation, command/result evidence, and a safe
   recovery choice for Task 3;
5. a pending approval with a requested authority and a policy-card answer for
   Task 4;
6. source-reported token observations with explicitly partial history and
   unavailable pricing for Task 5;
7. no prompts, source code, repository paths, shell commands, credentials,
   tool arguments/results, raw token values, or external links in exported
   telemetry.

If any element is absent, fail preflight, document the missing element as a
build/fixture blocker, and do not count a participant session.

## Launch timing and reset procedure

### Startup samples (before participant sessions)

Use the same pinned machine and build for 10 cold and 10 warm samples:

- **Cold:** fully exit the renderer; clear only the disposable test profile;
  wait until no app process remains; start a local timer at process launch;
  stop at `app_ready` when the rail accepts input.
- **Warm:** leave the renderer installed and the fixture cached; fully exit the
  renderer but do not clear its cache; repeat the same timing rule.
- Record all 20 values locally. Export only count, median, p95, build/fixture,
  and redacted failure category. Any startup crash blocks the wave.

For Task 1's submitted replay, start `first_visible_activity` timing at the
fixture's `run_submitted` event receipt and stop when the first ordered
activity is visibly rendered. Record the event sequence, not message text.
Use the runtime instrumented timestamp as the primary value and the timer as a
cross-check; reconcile discrepancies above 100 ms in the notes.

### Between-session reset

1. Close the renderer and confirm no active run or pending approval remains.
2. Delete the disposable profile and restore the fixture from its pinned
   read-only archive; do not alter the source checkout.
3. Verify build SHA, fixture SHA-256, manifest revision, and replay mode.
4. Create a new random local session ID and pseudonymous participant ID.
5. Run the fixture smoke path: initial view, Task 2 passed validation, Task 3
   failed validation, Task 4 pending approval, Task 5 partial usage/unavailable
   price. Do not record it as a participant result.
6. Confirm the local event folder is empty for the new session and that the
   export path points outside the repository.

If the fixture, build, mode, or environment materially changes, stop the wave;
do not pool results across the change. Begin a new wave with a new evidence
note.

## Moderator timing and neutral script

Start a 20-minute session timer after consent. Read task cards exactly; do not
mention the intended UI section, label, or correct answer. Ask only neutral
prompts such as “What are you looking for?” and “What makes you say that?”

| Minute | Moderator action | Required capture |
| --- | --- | --- |
| 0-2 | Read consent/data boundary; complete screen; ask about last agent-review workflow in broad terms. | Consent, screen outcome, role/agent-task bands, fixture mode. |
| 2-3 | Ask participant to launch Proofline and narrate the initial screen. | `app_ready`, first impression of authority/status; no quote containing sensitive data. |
| 3-6 | Give Task 1. | First visible activity, time to cited evidence, unassisted outcome. |
| 6-9 | Give Task 2. | Time to diff/evidence, validation interpretation, checkpoint distinction. |
| 9-12 | Give Task 3. | Failure location, recovery action, whether completion was confused with success. |
| 12-15 | Give Task 4 and policy card. | Approval discovery time, authority explanation, policy-card match, perceived stall. |
| 15-18 | Give Task 5. | Usage/pricing interpretation, handoff choice, invented-dollar or completeness claim. |
| 18-20 | Ask highest-friction moment, one missing evidence item, and whether it changes the next action on a recent task. | Redacted defect labels only; no purchase-intent prompting. |

### Hint, failure, and abandonment rules

- Give **one neutral hint** only after 90 seconds of no progress or when the
  participant explicitly asks for help. Say: “Please continue looking at the
  information available in the prototype.” Do not name a control or location.
- Mark the task `hint_used=true`, `completed_unassisted=false`, and preserve
  the original elapsed time even if it completes afterward.
- If the participant abandons, runs out of allocated time, or cannot answer
  the task card, record `outcome=abandoned` or `failure`; do not drop it.
- A moderator navigation error or fixture failure is a test-invalidating
  incident, not a participant failure. Stop, preserve only allowed diagnostics,
  reset, and schedule a new full session after correction.
- Do not offer hints on policy-card content. A wrong approval decision is an
  outcome, not a teaching moment during the session.

## Task cards

Cut or display one card at a time. The text is deliberately neutral.

### Task 1 - Read a Repo Brief

> You are joining a focused investigation. What do you believe is the likely
> ownership boundary? Show one cited piece of evidence, name one uncertainty,
> and tell me what you would check next.

Pass only if the cited file/line exists in the fixed fixture and was opened
from the transcript/evidence view. Do not count a terminal lookup as success.

### Task 2 - Inspect a completed change

> This run says it completed a two-file change. What changed, what is the
> validation state, and would you review this now? Please show me what you
> used to decide.

Pass only if the participant identifies the fixed two-file change and its
passing validation without opening a terminal. Record confusion between a
checkpoint and Git history separately.

### Task 3 - Recover from failed validation

> This run made a change, but something did not finish as expected. Find the
> relevant evidence. What would you do next, and why?

Pass only if the participant finds the deterministic failure, distinguishes it
from model completion, and chooses retry, inspect diff, or restore checkpoint
with an accurate explanation of its effect.

### Task 4 - Resolve an approval

> A run is in progress. Use this policy card to decide whether to approve or
> deny the request. Tell me what authority is being requested.

Pass only if the participant finds the pending approval without a hint, names
the requested authority, and matches the printed policy card. Do not let a
participant approve a real command or write.

### Task 5 - Interpret usage and hand off

> You are preparing a handoff. What can you honestly say about this run's
> usage or price? What is unavailable? Would you share this evidence record
> with a reviewer, and why?

Pass only if the participant distinguishes source-reported tokens from quota,
identifies partial history and unavailable pricing, and does not invent a
dollar value, declare it free, or claim complete coverage.

## Local capture and redacted export

The local raw event ledger may contain only the telemetry allowlist from the
validation plan: timestamps, random IDs, event types/states, durations,
surface/control IDs, task outcomes, and coarse environment metadata. Encrypt
it at rest where supported. Keep the identity mapping entirely outside Spark
artifacts.

After every session:

1. Check the raw ledger against the forbidden-field list in
   `proofline-validation.md`.
2. Replace event/session IDs with wave-local pseudonyms before any worksheet
   entry.
3. Enter only one redacted row per task in
   `proofline-wave1-template.csv` with `record_type=task`. Record each launch
   sample with `record_type=startup`, `startup_sample_kind=cold|warm`, and a
   one-based `startup_sample_index`; leave task-only columns empty.
4. Convert free-text notes into a short defect label such as
   `approval-not-found` or `price-interpreted-as-free`. Do not include a quote,
   path, command, prompt, code, or name.
5. Store completed redacted worksheets and the aggregate evidence note in the
   approved internal evidence location. Do not commit individual rows to this
   repository unless a privacy owner confirms their redaction.
6. In the repository, link only a redacted aggregate report and tracked fixes.

## Stop, fix, repeat, advance rubric

| Condition | Immediate action | Decision |
| --- | --- | --- |
| Any critical data, write, or policy incident; unexpected external access; or startup crash | Stop the wave, disable the affected build/fixture, preserve only allowed incident metadata, and perform early purge if requested. | **Stop.** A new build and full rerun are required. |
| Fixture integrity, telemetry allowlist, event order, or moderator protocol fails | Mark affected session invalid; do not count it as participant failure; correct and preflight again. | **Fix and repeat** the full affected session. |
| Two participants encounter the same severe confusion, or a task gate misses its threshold | Open a concrete design fix, identify owner, and retest the affected task after the fix. | **Fix and repeat.** Do not pool pre-fix and post-fix task outcomes. |
| Zero critical incidents; all safety/privacy checks pass; at least six of seven UX gates pass; every failure and assisted attempt is reported | Publish the aggregate report, link each defect/fix, and hold a decision review. | **Eligible to advance** to Wave 2, not automatically advanced. |
| Incomplete cohort, invalid data, mixed fixtures/builds, or unresolved discrepancies | Publish an inconclusive note with denominators and blockers. | **Repeat** or **stop**; do not claim a pass. |

The seven UX gates are startup, first visible activity, approval discovery,
transcript readability, changed-file inspection, failed-validation recovery,
and usage/pricing comprehension. The safety/privacy gate is mandatory and is
not one of the six-of-seven allowance.

Issue #6 lists every gate as a checklist item. The governing validation plan
uses the “six of seven UX gates plus zero critical safety/privacy incidents”
advance rule. Record the individual results for all seven and treat the issue
checklist as evidence completeness, not an all-gates-pass rewrite.

## Evidence handoff and purge checklist

### Evidence handoff

Before the decision review, the facilitator and privacy owner must jointly
confirm:

- build SHA, fixture revision/SHA-256, runtime mode, and environment ID are
  present for every valid session;
- five completed sessions use one pinned replay fixture and 20-minute script;
- all startup samples and all task attempts, including incomplete, hinted, and
  failed attempts, appear in the facilitator worksheet denominators; the host
  aggregate does not establish this complete protocol denominator;
- the evidence note explicitly says `VWPM = not measured` and `baseline = not
  collected`; no fixture timing is presented as a productivity result;
- the aggregate evidence note includes counts, medians/p95s, gate decisions,
  incident review, defects, fix owners, and a single decision: fix/repeat,
  advance, or stop;
- only redacted aggregate findings are linked from issue #6; the report says
  that the wave is bounded usability evidence, not VWPM or market validation;
- tracked fixes/retests are linked before an advance recommendation.

### Purge/reset

At the earlier of participant request or 30 days after the session:

1. delete the participant's local raw event ledger, disposable profile, and
   fixture copy using the organization-approved secure deletion procedure;
2. revoke any temporary local account/permission and remove fixture artifacts;
3. retain only the approved redacted aggregate evidence and tracked defects;
4. record `purge_completed_at` and `purge_operator` in the private facilitator
   log, not in the repository;
5. verify the repository contains no raw logs, recordings, identity mapping,
   prompts, source code, paths, commands, tokens, or credentials.

## Known implementation dependency

The repository now contains a Windows-first Tauri measurement host around the
React Proofline surface. It byte-attests the canonical five-scenario renderer
and referenced evidence, requires a full clean embedded Git identity, keeps
host-generated identity/order/time inside an AES-GCM ledger with a DPAPI-
protected key, accepts only constrained categorical participant events, and
produces an aggregate-only local export. It supports explicit early purge and,
on the next preflight after the 30-day deadline, lazily crypto-erases expired
artifacts before rotating the namespace. It has no timer or background purge,
so expiry cleanup is not guaranteed without a later preflight.

That foundation does **not** complete Wave 1. Production native preflight
deliberately reports `countable=false`. A monotonic process-entry boundary,
main-webview page-load diagnostic, and challenge-bound readiness/visible-state
receipts now exist, but they require Proofline-specific external visual calibration,
exact-build attestation, an enforceable runtime network boundary, and official cold/warm sampling.
Sampled socket polling cannot prove that no connection occurred.
None is described as actual first paint. Crash/restart ledger recovery,
timer/background expiry purge, proof
of cleanup when no later preflight occurs, a non-Windows key protector,
privacy-owner sign-off, complete worksheet denominators, and five real
participant sessions remain outstanding. Keep issue #6 open and do not count a
session. Do not simulate participants or backfill the worksheet from agent
output.
