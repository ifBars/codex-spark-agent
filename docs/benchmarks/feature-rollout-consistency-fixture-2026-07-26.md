# Feature rollout consistency fixture

`feature-rollout-consistency-bugfix` is a validated multi-file coding fixture
awaiting balanced Spark-versus-Codex runs. It has no published model score yet.

The incident combines six production invariants that must hold together:

| Signal | Weight |
| --- | ---: |
| Tenant-isolated config storage | 20 |
| Monotonic config revisions | 15 |
| Decision precedence | 20 |
| Stable bounded rollout | 15 |
| Tenant- and revision-aware cache | 20 |
| End-to-end revision behavior | 10 |

The agent may change only the store, cache, and evaluator. Public types, stable
hashing, service orchestration, incident evidence, and tests stay fixed.
Independent harness checks exercise stale retries, equal revisions, cross-tenant
collisions, allow/deny conflicts, mutable email addresses, percentage bounds,
and cached decisions across revisions.

The checked-in oracle test proves the broken fixture fails and a coherent repair
passes all six weighted dimensions. The fixture will remain pending until each
runner and reasoning level has comparable valid repeats.
