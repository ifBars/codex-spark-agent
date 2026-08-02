# Proofline lifecycle smoke — 2026-08-01

## Decision

Do not run the 10 cold + 10 warm launch calibration yet. The bounded one-cold + one-warm smoke found two eligibility blockers that would censor every observed launch. Keep the desktop candidate non-countable, retain `Network gate pending` in the product UI, and do not recruit the first design partner on this build.

This is a calibration-system smoke test, not launch-performance evidence.

## Candidate identity

- Proofline executable SHA-256: `1c1f3162bb90c5c75bcb3a77914588bd1e2bb30135a31c7d58ad67158766b530`
- Fixture SHA-256: `29415e0ce8f7659093e01032ce52365197c59d0010bad7aa4361048fdb86abe5`
- Attempts: 1 cold, 1 warm
- Countable attempts: 0 of 2
- Startup crashes: 0
- Profile isolation: verified for both attempts

## Observations

| Mode | Host UI ready | External TCP | Stable visual evidence | Outcome |
| --- | ---: | --- | --- | --- |
| Cold | 427 ms | Observed | Not established | Censored |
| Warm | 426 ms | Observed | Not established | Censored |

The host-owned launch lifecycle receipt path completed in both attempts. This launch-only smoke did not submit a run, so `run_to_first_visible_ms` was not requested and remained null by design. Separately, the external launch-readiness observer reported `visible_frames_disagreed`. Owned WebView2 processes also established non-loopback remote connections in both attempts. Endpoint, process, command-line, profile-path, and screenshot details were deliberately excluded from this note.

## Gate status

- Exact-build identity: observed by the harness, but insufficient for eligibility while other gates fail.
- Isolated WebView2 profile: passed.
- Sampled network observation: external activity observed; an enforceable no-network boundary is not yet implemented.
- External visual reconciliation: failed.
- Required 10 cold + 10 warm protocol: not run.
- Release eligibility: failed closed.

## Follow-up

1. Establish an enforceable runtime network boundary for Proofline without relying on unsupported browser flags or broad machine-level rules. Polling alone cannot prove the absence of short-lived connections.
2. Make the external visual observer recognize a stable Proofline-specific launch anchor before reconciling it with the host monotonic timeline.
3. Repeat a one-cold + one-warm smoke.
4. Run the 10 + 10 protocol only after both smoke attempts clear every preliminary eligibility gate.

The external raw evidence directory was permanently deleted after this redacted result was recorded.
