---
name: code-review
description: Review code changes systematically for introduced correctness, security, reliability, concurrency, and lifecycle regressions while avoiding style noise and false positives.
---

# Code Review

Use an evidence ledger so broad diffs do not turn into a memory test.

## Evidence pass

1. Read the request or contract, the complete diff, and every changed source and test file. When the task supplies local review artifacts and names the relevant files, do not spend time surveying unrelated repository state.
2. Create a private ledger with one row per changed production file: changed behavior, relevant invariant, boundary or failure cases, existing test coverage, and verdict (`safe`, `finding`, or `needs evidence`). Do not finalize until every row has a verdict.
3. Treat tests as evidence of what is covered, not proof that untested behavior is correct.

## Semantic sweeps

Apply the sweeps that fit each changed behavior:

- Authorization and isolation: exact privilege checks, tenant/account scope, public-id collisions, cache and lookup keys, secret exposure.
- Boundaries and identity: zero/one/maximum values, inclusive versus exclusive comparisons, falsy values versus absent values, stable ordering, tied cursors, deterministic retry identities.
- Async and lifecycle: awaited collection work, the same callback identity for registration and cleanup, failure before success is recorded, retry behavior after a rejected operation, atomic check-and-act under concurrency.
- State and side effects: partial failure, duplicate delivery, ordering, stale state, cleanup, rollback, and whether success can be reported before durable work completes.

## Finding gate

Report a finding only when all of these are concrete:

1. The changed line introduces or exposes the behavior.
2. A specific input, ordering, boundary, or failure triggers it.
3. The user or system impact follows from that trigger.
4. A minimal safe fix and focused regression test can be named.

Do not report style preferences, speculative architecture concerns, or correct defensive code. Rank findings by impact, not file order. For structured output, follow the requested schema exactly and keep each finding self-contained so its evidence, impact, fix, and test can be evaluated independently.
