# Luna delegation observation: Proofline Wave 1

Date: 2026-08-01

Status: three measured local research tasks. This is a routing observation, not a
paired model benchmark or a dollar-cost claim.

The first two tasks ran through the v0.6.0 Spark harness. The third ran through
the then-installed PATH binary, which was subsequently verified as stale
v0.4.1 and upgraded to the released v0.6.0 build. All three used
`gpt-5.6-luna`, medium reasoning, Ask mode, a fresh named session, and
trace/profile. The third task used a 70,000-token input guard and an explicit
eight-read ceiling.

| Task | Elapsed | Requests | Read-only tool calls | Provider input | Cached input | Output | Total |
| --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Audit whether Proofline Wave 1 is runnable | 88.636 s | 6 | 23 | 140,321 | 98,560 | 4,424 | 144,745 |
| Define the five-session falsifiable PM gate | 74.295 s | 3 | 6 | 39,788 | 25,728 | 3,769 | 43,557 |
| Design the participant-countable measurement host | 61.703 s | 3 | 8 | 55,082 | 30,848 | 3,157 | 58,239 |
| Combined | 224.634 s | 12 | 37 | 235,191 | 155,136 | 11,350 | 246,541 |

Pricing remains unavailable. Provider token activity is not a bill, and these
rows do not show that Luna is cheaper than Spark, Terra, or another model.

## Decision value

The reports converged on two product-critical corrections:

- the selected Proofline concept is launchable as an interface but cannot run
  the five-person protocol yet;
- approval, failed-validation recovery, cited evidence, partial usage, ordered
  replay, and privacy-safe measurement must exist as deterministic fixture
  states before recruitment; and
- Wave 1 is a falsification and instrumentation gate. It cannot establish
  market demand or VWPM improvement without matched human baseline data; and
- a browser-only recorder cannot own authoritative process-start timing,
  encrypted durable capture, or immutable fixture verification. A narrow
  native host must own those claims while browser/Sites remains rehearsal-only.

The parent PM verified these claims against
[`proofline-validation.md`](proofline-validation.md), the renderer state, the
current tests, [`strategy.md`](strategy.md), and the existing Repo Brief
dogfood record before accepting them as implementation direction.

## Harness observations

- All three tasks completed without compaction, tool failures, or repeated tool
  calls.
- The broader readiness audit used 23 file tools and 144,745 tokens. Its five
  consecutive tool-only turns show that a bounded research lane can still
  over-collect when the scope includes both product docs and renderer code.
- The narrower PM audit used six reads and 43,557 tokens. A precise file list
  and decision contract materially reduced exploration.
- The measurement-host lane obeyed its eight-read ceiling, used 58,239 tokens,
  and produced the native-host trust boundary later accepted into issue #9.
- The longest response request in the measurement-host lane was 49.895 s.
  Together with the earlier multi-minute task totals, Luna remains suitable for
  background review here, not the interactive default.
- The third task exposed release-installation drift: a published tag did not
  mean the PATH binary had been upgraded. Release verification now includes
  `spark --version` plus a feature smoke test.

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
