# Usage and cost accounting contract

Status: product contract, checked 2026-08-01

Spark exposes three related measurements, but they must never be presented as
the same thing:

1. **Account quota** is the provider-reported percentage consumed in a rolling
   window, plus reset time, credits, plan, and spend-control state when present.
2. **Run usage** is the token accounting returned by completed model responses.
3. **Estimated cost** is a derived API-equivalent value that exists only when an
   exact public model rate and a complete token breakdown are available.

Missing data is unavailable, not zero. Partial data stays partial. A quota
percentage is not a token count, and neither is a dollar charge.

## Source hierarchy

### Account quota

The preferred local source is the authenticated Codex usage response used by
Codex itself. Current Codex source reads either `/api/codex/usage` or
`/wham/usage`, then normalizes primary and secondary windows, credits, plan,
individual spend control, and named additional limits. The public app-server
surface exposes the same class of data through `account/rateLimits/read`.

Spark may show:

- `plan_type`;
- primary, secondary, and named additional limit windows;
- `used_percent`, window duration, and reset timestamp;
- credit availability, unlimited state, and backend-reported balance;
- individual limit, used value, remaining percentage, and reset time; and
- the source, fetch time, and confidence state.

Spark must not infer message counts, tokens, or dollars from these percentages.
The Codex backend is an observed authenticated product interface, not a stable
public billing API, so fetch and decode failures must be visible and actionable.

Evidence:

- [Codex rate-limit client](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/backend-client/src/client/rate_limit_resets.rs)
- [Codex account protocol](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/app-server-protocol/src/protocol/v2/account.rs)
- [CodexBar OAuth decoder](https://github.com/steipete/CodexBar/blob/299385a7a8f13c2ab3d34c5a24094935fe1c4511/Sources/CodexBarCore/Providers/Codex/CodexOAuth/CodexOAuthUsageFetcher.swift)

### Run usage

Completed Responses payloads are authoritative for the request they describe.
Spark records each field independently and reports how many completed responses
provided it:

- `input_tokens`;
- cached input tokens from `input_tokens_details.cached_tokens`;
- cache-write input tokens from `input_tokens_details.cache_write_tokens`;
- `output_tokens`;
- reasoning output tokens from `output_tokens_details.reasoning_tokens`; and
- `total_tokens`.

Cached reads and cache writes are subsets of input tokens. Reasoning tokens are
a subset of output tokens. Derived uncached input therefore uses clamped
subtraction, while reasoning tokens are never added to output a second time.

For one completed response:

```text
cached = clamp(cached_input_tokens, 0, input_tokens)
cache_write = clamp(cache_write_input_tokens, 0, input_tokens - cached)
uncached_input = input_tokens - cached - cache_write
```

Session-log scanners require stronger accounting. Codex `token_count` events can
contain per-event `last_token_usage` and cumulative `total_token_usage` snapshots,
and forked or interleaved workers can repeat or lower those counters. `spark usage
--history` is a local-only, aggregate-only `spark.usage_history.v1` ingestion path:
it prefers `last_token_usage`, falls back to non-negative cumulative deltas, reports
counter resets, selects one duplicate live/archived session copy, and removes fork
replay only when lineage plus cumulative-counter evidence matches. It never sums
every JSONL snapshot.

Its output has aggregate, exact-model, and day breakdowns. Every metric includes a
reported total, observation coverage, completeness, and availability. The scanner
does not emit prompts, messages, raw tool output, credentials, session paths, or
working directories. Its pricing field is always unavailable: local token counters
are not a billing record.

Evidence:

- [Responses API usage schema](https://developers.openai.com/api/reference/resources/responses/methods/create)
- [Codex token protocol](https://github.com/openai/codex/blob/ee0247f95a6fe2b094ba2253d82cae2a2b4c2dff/codex-rs/protocol/src/protocol.rs#L2064-L2088)
- [CodexBar local scanner](https://github.com/steipete/CodexBar/blob/299385a7a8f13c2ab3d34c5a24094935fe1c4511/Sources/CodexBarCore/Vendored/CostUsage/CostUsageScanner.swift)

### Estimated cost

Cost is optional derived metadata, not provider billing truth. It requires:

- an exact, normalized model identity;
- a dated and versioned rate-card source;
- a declared processing tier and region assumption;
- complete token fields required by that rate card; and
- a formula version retained with the result.

When those conditions hold, the base token formula is:

```text
estimated_cost =
  uncached_input * input_rate
  + cached_input * cached_input_rate
  + cache_write_input * cache_write_rate
  + output_tokens * output_rate
```

Unknown models remain unpriced while their tokens remain visible. Pricing
fallback may normalize harmless aliases or dated suffixes only when the billing
identity is still exact; similarity is not proof of price.

The current official API rate card lists prices for `gpt-5.3-codex` and several
GPT-5.6 models, but does not list `gpt-5.3-codex-spark`. OpenAI's Codex pricing
documentation says Spark is a ChatGPT Pro research preview, is not available in
the API at launch, and has a separate usage limit that may change with demand.
Therefore Spark's API-token price is **unavailable**. It must not inherit the
`gpt-5.3-codex` rate and must not be displayed as free or `$0`.

Evidence:

- [OpenAI API pricing](https://platform.openai.com/docs/pricing)
- [Codex plan usage and Spark availability](https://learn.chatgpt.com/docs/pricing#what-are-the-usage-limits-for-my-plan)
- [CodexBar cost formula](https://github.com/steipete/CodexBar/blob/299385a7a8f13c2ab3d34c5a24094935fe1c4511/Sources/CodexBarCore/Vendored/CostUsage/CostUsagePricing.swift#L598-L633)

## Product display rules

Human and machine-readable surfaces use the same states:

| State | Meaning | Display rule |
| --- | --- | --- |
| `exact` | Provider returned the value and all required fields decoded. | Show the value and source. |
| `partial` | Some completed responses or optional fields did not report it. | Show known coverage; do not aggregate as complete. |
| `estimated` | Derived from complete usage and an explicit rate-card assumption. | Prefix with `Estimated`; include formula, rate date, tier, and source. |
| `stale` | Cached value is older than its declared freshness window. | Preserve its original fetch time and mark stale. |
| `unavailable` | The provider or public rate card does not expose the value. | Show `Unavailable` and a short reason; never coerce to zero. |

The first CLI surface should prioritize account quota and current-run token
usage. Local historical cost scanning is a later feature because it needs
fork-aware cumulative accounting, bounded disk scans, cache invalidation, and
privacy controls. Browser-cookie dashboard scraping is out of scope for the
worker plane.

## Third-party lessons

The useful product patterns are broader than a single implementation:

- **CodexBar** combines provider quota with a local, fork-aware session scan and
  keeps source, freshness, and confidence visible. Its hard-coded zero price for
  `gpt-5.3-codex-spark` is not supported by OpenAI's public pricing and must not
  be copied.
- **CodeBurn** shows the value of local-first, multi-agent rollups, bounded
  incremental parsing, pricing snapshots, and user overrides. Its Codex parser
  also demonstrates why Spark needs conformance tests: at the reviewed commit,
  it passes `output_tokens + reasoning_output_tokens` into its output-cost
  calculation, while Codex defines reasoning as part of output usage.
- **Codex Usage Desktop** demonstrates demand for a small live-limits plus local
  history surface, but its documented unknown-model-to-zero fallback is unsafe
  for product claims.

Evidence:

- [CodeBurn Codex parser](https://github.com/getagentseal/codeburn/blob/e7576e7fa50bd0f36986bdfb11d58678d4ddb163/src/providers/codex.ts)
- [CodeBurn pricing loader](https://github.com/getagentseal/codeburn/blob/e7576e7fa50bd0f36986bdfb11d58678d4ddb163/src/models.ts)
- [Codex Usage Desktop](https://github.com/itvincent-git/codex-usage-desktop)

## Product and benchmark implications

- Spark Bench may compare authoritative tokens even when dollar cost is
  unavailable.
- Public cost claims require a balanced paired dataset, exact model identity,
  every-attempt denominators, and versioned pricing assumptions.
- Subscription quota percentages and reset credits should be operational
  context, not a benchmark axis.
- A future team dashboard should separate quota, token activity, estimated API
  equivalent, and actual invoiced spend into different cards and schemas.
- Pricing refreshes must never rewrite historical estimates without retaining
  the rate-card and formula versions that produced them.
