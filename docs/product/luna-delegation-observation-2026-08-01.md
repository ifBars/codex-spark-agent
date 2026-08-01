# Luna delegation observation: Proofline Wave 1

Date: 2026-08-01

Status: two measured local research tasks. This is a routing observation, not a
paired model benchmark or a dollar-cost claim.

Both tasks ran through the v0.6.0 Spark harness with `gpt-5.6-luna`, medium
reasoning, Ask mode, a fresh named session, trace/profile enabled, and an
80,000-token input guard. The tasks were independent and ran concurrently.

| Task | Elapsed | Requests | Read-only tool calls | Provider input | Cached input | Output | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Audit whether Proofline Wave 1 is runnable | 88.636 s | 6 | 23 | 140,321 | 98,560 | 4,424 | 144,745 |
| Define the five-session falsifiable PM gate | 74.295 s | 3 | 6 | 39,788 | 25,728 | 3,769 | 43,557 |
| Combined | 162.931 s | 9 | 29 | 180,109 | 124,288 | 8,193 | 188,302 |

Pricing remains unavailable. Provider token activity is not a bill, and these
rows do not show that Luna is cheaper than Spark, Terra, or another model.

## Decision value

The two reports independently converged on a product-critical correction:

- the selected Proofline concept is launchable as an interface but cannot run
  the five-person protocol yet;
- approval, failed-validation recovery, cited evidence, partial usage, ordered
  replay, and privacy-safe measurement must exist as deterministic fixture
  states before recruitment; and
- Wave 1 is a falsification and instrumentation gate. It cannot establish
  market demand or VWPM improvement without matched human baseline data.

The parent PM verified these claims against
[`proofline-validation.md`](proofline-validation.md), the renderer state, the
current tests, [`strategy.md`](strategy.md), and the existing Repo Brief
dogfood record before accepting them as implementation direction.

## Harness observations

- Both tasks completed without compaction, tool failures, or repeated tool
  calls.
- The broader readiness audit used 23 file tools and 144,745 tokens. Its five
  consecutive tool-only turns show that a bounded research lane can still
  over-collect when the scope includes both product docs and renderer code.
- The narrower PM audit used six reads and 43,557 tokens. A precise file list
  and decision contract materially reduced exploration.
- The longest response request in each task was 58.817 s and 63.418 s. Luna is
  therefore suitable for background review here, not the interactive default.

## Routing decision

Keep Spark as the interactive worker and Terra as the default implementation
delegate. Use Luna for independent research, review, and planning when:

1. the question is falsifiable and bounded;
2. the starting files and output contract are explicit;
3. multi-minute latency is acceptable; and
4. the parent verifies the result before changing product direction.

For future Luna research, start with the smallest evidence set and a target of
no more than eight read-only tool calls. Expand only when a named uncertainty
requires it. Do not claim cost savings until a versioned price source and a
comparison-valid task matrix exist.
