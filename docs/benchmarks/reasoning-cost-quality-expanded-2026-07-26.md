# Expanded reasoning cost-quality benchmark

This dataset compares `gpt-5.3-codex-spark` at low, medium, and high reasoning through the Spark harness and native Codex CLI 0.145.0.

## Sample

- Nine difficulty-focused scenarios
- Three repeats per scenario and reasoning level
- Up to twenty-seven successful task-runs per chart point
- 129 successful task-runs plotted
- 33 failed task attempts excluded
- Zero provider/API failures

The scenarios are `technical-essay`, `config-migration`, `ops-report`, `multi-module-bugfix`, `terminal-repair`, `multi-hop-analysis`, `policy-support-agent`, `rust-notes-tui-scaffold`, and `stateful-reconciliation-bugfix`.

The three reasoning levels ran concurrently within each runner family. Spark and native Codex ran in separate phases. Chart inputs are generated with `benchmark-compare --successful-only`: failed task attempts, validation failures, and timed-out attempts are excluded before scenario means are computed. Explicit provider/API failures are excluded separately; none occurred in this snapshot.

## Aggregation

Each scenario with at least one successful attempt contributes equally to a runner/reasoning headline mean. The error bars are 95% Student's t intervals across the available successful-only scenario means, rather than treating repeated runs as fully independent observations. This keeps repeated runs from creating false precision while preventing a failed attempt from becoming an artificial zero.

Weighted quality uses the benchmark's scenario-specific behavioral validation. Success rate remains a separate binary measure. Total tokens are input plus output tokens summed per task-run; Spark totals come from reported API usage for every request, while native Codex totals come from its JSON event report. Duration is wall-clock task time.

The measured ordering is not forced to be monotonic. Confidence intervals overlap, and low reasoning outperformed medium or high on some fixtures in this sample.

## Category views

The interactive app also presents five overlapping real-world task families derived from the same 162 task-runs:

- **Coding:** four implementation, migration, scaffolding, and cross-module repair scenarios.
- **Math & data:** two scenarios requiring exact calculations or joined quantitative evidence.
- **Analysis:** three grounded synthesis and policy-reasoning scenarios.
- **Terminal & ops:** two terminal-first repair and operational-reporting scenarios.
- **Writing & config:** three grounded prose, migration, and policy-output scenarios.

These are different views of one measured matrix, not additional benchmark runs. A scenario may appear in more than one family because real tasks cross skill boundaries. Each view again weights its included scenario means equally and computes its own 95% Student's t interval. Narrow two-scenario views produce very wide intervals and should be treated as early category coverage, not stable rankings.

## Scenario lens

The explorer also exposes each of the nine measured scenarios as a runner/reasoning chart. A configuration is absent when none of its three attempts passed validation; otherwise its point is the mean of one to three successful attempts. The scenario-level source artifact does not retain per-run dispersion, so the task drill-down deliberately omits error bars rather than inferring or fabricating intervals. Overall and category charts continue to show their scenario-level 95% intervals.

Rebuild the category aggregates and web data with:

```powershell
bun scripts/build_benchmark_views.mjs
```

To regenerate the successful-only scenario source from a reviewed comparison report:

```powershell
bun scripts/export_successful_comparison_rows.mjs --input <comparison.json> --output docs/benchmarks/reasoning-cost-quality-expanded-scenarios-2026-07-26.csv --expected-repeats 3 --expected-groups 54
```

## Artifacts

- [`reasoning-cost-quality-expanded-2026-07-26.csv`](reasoning-cost-quality-expanded-2026-07-26.csv) contains the six chart points.
- [`reasoning-cost-quality-expanded-scenarios-2026-07-26.csv`](reasoning-cost-quality-expanded-scenarios-2026-07-26.csv) contains all 54 runner/reasoning/scenario aggregates.
- [`reasoning-benchmark-views-2026-07-26.json`](reasoning-benchmark-views-2026-07-26.json) defines the published category membership and measured scenario catalog.
- [`reasoning-benchmark-evidence-2026-07-26.json`](reasoning-benchmark-evidence-2026-07-26.json) defines the reviewable evidence contract, pending validated fixtures, and linked source artifacts.
- [`reasoning-cost-quality-categories-2026-07-26.csv`](reasoning-cost-quality-categories-2026-07-26.csv) contains all 36 overall and category chart points.
- The ignored local comparison report was generated from three exact Spark manifests and three isolated native Codex reports with `benchmark-compare --group-by-reasoning`.
