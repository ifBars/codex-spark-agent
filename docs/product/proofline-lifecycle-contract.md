# Proofline native lifecycle contract

Status: implementation contract for issue #11. This document defines what Proofline may measure and claim; it does not itself make a launch countable.

## Decision

Proofline uses one native monotonic clock domain and host-owned identifiers for lifecycle evidence. Renderer messages are timestamp-free categorical receipts bound to opaque host challenges. Browser/Sites mode remains rehearsal-only.

Tauri page-load completion is observable and useful as a diagnostic boundary, but it is not a first-paint guarantee. Proofline records it as `page_load_finished`, never as `first_paint` or `first_visible`.

Production countability remains disabled until all of the following are true for the exact reviewed build:

1. The native host records process start before constructing the Tauri application.
2. Tauri reports page-load completion for the main Proofline webview.
3. The renderer returns exactly one timestamp-free `ui_ready` receipt for the active launch challenge after the task rail and composer can accept input.
4. A separate external Windows observer calibrates that host-stamped receipt against stable visible Proofline chrome.
5. Ten cold and ten warm launch attempts pass the issue #11 thresholds with failures preserved in the denominator.
6. An enforceable runtime network boundary and privacy review pass. Sampled socket polling is diagnostic evidence, not proof that no connection occurred.

## Clock and authority

- `Instant` captured at Rust `run()` entry is the sole duration origin.
- The host stamps every boundary on receipt using elapsed monotonic time.
- Wall-clock time is optional audit context and never computes a duration.
- The renderer cannot supply timestamps, durations, event IDs, sequences, launch classification, or lifecycle phase names beyond the exact receipt requested by the host.
- Host-issued `launch_id`, challenge, and sequence are opaque to the renderer.
- The optional `SPARK_PROOFLINE_LIFECYCLE_REPORT_PATH` calibration sink atomically exposes only `spark.proofline.lifecycle.status.v1`. It is absent during normal use and never contains challenges, identifiers, wall-clock timestamps, process data, paths, prompts, or raw events.

## Launch state machine

```text
initialized
  -> process_started
  -> page_load_finished
  -> ui_ready_received
  -> calibrated
  -> countable

Any missing, duplicate-contradictory, stale, timed-out, unsupported, or out-of-order boundary
  -> ineligible
```

An identical duplicate receipt for the active challenge is idempotent. A stale launch ID or challenge is rejected without changing state. A contradictory duplicate or invalid transition makes the sample ineligible. Page-load completion after `ui_ready_received` is invalid ordering.

## Run visibility state machine

Launch readiness and task-run visibility are separate protocols.

```text
idle
  -> run_submitted (host stamps and issues run challenge)
  -> first_visible_received (renderer returns timestamp-free receipt)
  -> run_terminal
```

The renderer may acknowledge `first_visible` only after the submitted run has produced a visible state change and that state has survived two animation frames. The host stamps receipt. This is a calibrated renderer receipt, not a compositor first-paint claim.

Existing `activity_rendered` remains an untimed, non-authoritative Wave 1 category and cannot satisfy either lifecycle protocol.

## Public duration semantics

| Field | Proves | Does not prove |
| --- | --- | --- |
| `process_to_page_load_ms` | Host-monotonic elapsed time to Tauri main-webview page-load completion | First paint, interactivity, or human perception |
| `process_to_ui_ready_ms` | Host-monotonic elapsed time to receipt of a challenge-bound renderer readiness acknowledgement | Compositor first paint or completion of background work |
| `page_load_to_ui_ready_ms` | Host-monotonic gap between those two boundaries | JavaScript execution time in isolation |
| `run_to_first_visible_ms` | Host-monotonic elapsed time from accepted run submission to a challenge-bound visible-state receipt | Model latency in isolation or compositor first paint |

The UI and aggregate export must use these names. It must not shorten them to `first_paint_ms`.

## Countability and calibration

A lifecycle sample can become countable only when fixture, build, protected storage, lifecycle ordering, Proofline-specific external visual calibration, and an enforceable runtime network boundary all pass. Until then, preflight returns `countable=false` with machine-readable reasons. A clean polling interval cannot set `no_network_verified=true` or make a candidate eligible.

Cold launch means a new process after all owned Proofline/WebView descendants have exited, with the disposable Proofline profile reset and the fixture restored. Warm launch also starts a new process, but retains the disposable profile/cache and unchanged fixture. Neither mode clears operating-system caches or unrelated user data.

All 20 attempts remain in the denominator. Crashes, timeouts, visual disagreement, network activity, invalid preflight, shutdown failure, or environment drift are censored failures rather than omitted samples. Observed durations alone produce nearest-rank median and p95, while every censored reason remains visible. Any startup crash blocks the gate.

## Privacy

Committed evidence contains only aggregate durations, denominators, censored-reason counts, threshold outcomes, exact build/fixture attestations, and boolean observer results. Raw screenshots, process diagnostics, connection endpoints, prompts, paths, commands, participant identifiers, launch IDs, challenges, and event payloads stay outside the repository and are disposable after review.

## Release gate

Do not set production `countable=true`, recruit participant P01, publish a bundled desktop release, or close issue #11 until deterministic native and renderer tests, the 20-launch Windows calibration, Proofline-specific external visual reconciliation, an enforceable no-network boundary, and independent review all pass for one exact clean build.
