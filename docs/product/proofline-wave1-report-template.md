# Proofline Wave 1 evidence note

Status: empty reporting template. Do not treat this file as participant evidence,
proof of VWPM, market validation, or a completed issue #6.

## Build and cohort

- Tested commit:
- Fixture revision:
- Replay bundle SHA-256:
- Renderer/runtime mode:
- OS and hardware band:
- Sessions started / completed:
- Participant-screen counts:
- Raw-event retention/purge owner:

Preflight result: pass / fail / not run

Any mixed build, fixture, runtime mode, or environment invalidates pooled wave
results. Tests, green builds, and agent simulations are not substitutes for the
five real participant sessions required here.

## Baseline and VWPM boundary

- VWPM: not measured
- Matched real-work baseline: not collected
- Reason: disposable replay usability wave; task-card timings are not
  productivity evidence.

Do not replace these values with an estimate. A valid VWPM comparison belongs
to the later matched-task protocol in [customer-discovery.md](customer-discovery.md).

Participant identities, contact details, repository names, prompts, source code,
commands, and recordings remain outside this repository. Attach only the
completed, redacted copy of `proofline-wave1-template.csv`.

## Gate results

| Gate | Denominator | Result | Threshold | Decision |
| --- | ---: | ---: | ---: | --- |
| Cold startup median / p95 |  |  | <= 3 s / <= 5 s |  |
| Warm startup median |  |  | <= 1.5 s |  |
| First visible activity median / p95 |  |  | <= 500 ms / <= 1 s |  |
| Approval discovery, unassisted |  |  | >= 80% |  |
| Transcript evidence, unassisted |  |  | >= 80% |  |
| Changed-file inspection, unassisted |  |  | >= 80% |  |
| Failed-validation recovery, unassisted |  |  | >= 80% |  |
| Usage/pricing comprehension |  |  | >= 90%; zero invented-dollar claims |  |
| Critical safety/privacy incidents |  |  | 0 |  |

## Task denominators and assistance

| Task | Started | Completed unassisted | Completed after hint | Failed/abandoned | Median seconds | Threshold/result |
| --- | ---: | ---: | ---: | ---: | ---: | --- |
| 1. Repo Brief evidence |  |  |  |  |  | >= 80% unassisted; median <= 75 s |
| 2. Completed change inspection |  |  |  |  |  | >= 80% unassisted; median <= 60 s |
| 3. Failed-validation recovery |  |  |  |  |  | >= 80% unassisted; median <= 75 s |
| 4. Approval discovery |  |  |  |  |  | >= 80% unassisted; median <= 30 s |
| 5. Usage/pricing interpretation |  |  |  |  |  | >= 90% correct; zero invented-dollar claims |

Report counts next to percentages, retain incomplete and assisted attempts, and
list every failure. Five participants are a bounded usability wave, not a claim
of statistical generalization.

## Repeated defects and fixes

| Defect | Participants affected | Severity | Fix owner | Retest evidence |
| --- | ---: | --- | --- | --- |
|  |  |  |  |  |

## Decision

Choose exactly one: fix and repeat / eligible to advance to external design
partners / stop. “Eligible” requires a separate decision review; it is not an
automatic advance.

Decision rationale:

## Privacy, retention, and handoff

- Telemetry allowlist audit:
- Forbidden-field check:
- Critical incident review:
- Raw local logs purged or scheduled for purge:
- Redacted aggregate artifact linked to issue #6:
- Fix/retest issues linked:
