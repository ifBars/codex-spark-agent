# Expanded reasoning cost-quality benchmark

This dataset compares `gpt-5.3-codex-spark` at low, medium, and high reasoning through the Spark harness and native Codex CLI 0.145.0.

## Sample

- Nine difficulty-focused scenarios
- Three repeats per scenario and reasoning level
- Twenty-seven task-runs per chart point
- 162 task-runs total
- Zero provider/API failures

The scenarios are `technical-essay`, `config-migration`, `ops-report`, `multi-module-bugfix`, `terminal-repair`, `multi-hop-analysis`, `policy-support-agent`, `rust-notes-tui-scaffold`, and `stateful-reconciliation-bugfix`.

The three reasoning levels ran concurrently within each runner family. Spark and native Codex ran in separate phases. Ordinary task failures, validation failures, tool failures, and timeouts remain in the score. Only explicit provider/API failures would be excluded; none occurred.

## Aggregation

Each scenario contributes equally to the headline mean. The error bars are 95% Student's t intervals across the nine scenario means (`df = 8`), rather than treating the three repeats as fully independent observations. This keeps repeated runs from creating false precision.

Weighted quality uses the benchmark's scenario-specific behavioral validation. Success rate remains a separate binary measure. Total tokens are input plus output tokens summed per task-run; Spark totals come from reported API usage for every request, while native Codex totals come from its JSON event report. Duration is wall-clock task time.

The measured ordering is not forced to be monotonic. Confidence intervals overlap, and low reasoning outperformed medium or high on some fixtures in this sample.

## Artifacts

- [`reasoning-cost-quality-expanded-2026-07-26.csv`](reasoning-cost-quality-expanded-2026-07-26.csv) contains the six chart points.
- [`reasoning-cost-quality-expanded-scenarios-2026-07-26.csv`](reasoning-cost-quality-expanded-scenarios-2026-07-26.csv) contains all 54 runner/reasoning/scenario aggregates.
- The ignored local comparison report was generated from three exact Spark manifests and three isolated native Codex reports with `benchmark-compare --group-by-reasoning`.
