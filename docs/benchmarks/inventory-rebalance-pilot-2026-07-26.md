# Inventory rebalance planning pilot

This is development evidence for `inventory-rebalance-plan`, not a replacement for
the published Capability Atlas dataset.

The task joins transfer options, warehouse constraints, and SKU penalties; rejects
late transfers; and asks for exact optimal plans under base and contingency budgets.
The validator independently checks schema, both selections, both metric groups, and
the decision memo. A fixture test enumerates all 16,384 possible subsets to derive
the two optima from the checked-in inputs.

## Valid task runs

| Runner | Effort | Valid n | Mean quality | Mean completion | Mean validation | Mean process |
| --- | --- | ---: | ---: | ---: | ---: | ---: |
| Spark harness | Low | 4 | 79.5 | 74.5 | 82.5 | 80.0 |
| Spark harness | Medium | 3 | 94.7 | 86.0 | 100.0 | 76.0 |
| Spark harness | High | 4 | 95.0 | 87.0 | 100.0 | 85.3 |
| Codex CLI | Low | 2 | 100.0 | 100.0 | 100.0 | — |
| Codex CLI | Medium | 1 | 100.0 | 100.0 | 100.0 | — |
| Codex CLI | High | 2 | 100.0 | 100.0 | 100.0 | — |

These means combine the corrected one-repeat pilot and the later three-repeat
consistency pass. They are scenario evidence, not broad runner rankings.

## Exclusions and limitations

- The first low-effort pair is excluded because an ambiguous output path in the
  benchmark prompt caused the harness to write correct artifacts to the wrong
  directory. This is recorded as a benchmark-design failure, not a model score.
- Eight requested consistency runs are excluded as infrastructure failures: one
  Spark request failure and seven Codex CLI provider usage-limit failures. Their
  task scores are blank in the raw ledger rather than recorded as zero.
- Codex CLI therefore has only one or two valid observations per effort. That is
  insufficient for a balanced curve or confidence interval.
- Most corrected runs reached 100 validation, while completion and process scores
  still varied. The isolated low harness score of 30 demonstrates some headroom,
  but this single scenario still saturates too often to support a category curve
  by itself.

The main atlas remains unchanged until a second independently validated difficult
quantitative scenario and a balanced rerun are available.

Raw run ledger:
[`inventory-rebalance-pilot-runs-2026-07-26.csv`](inventory-rebalance-pilot-runs-2026-07-26.csv).
