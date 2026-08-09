# Spark Bench web data

Usage metrics belong in task-comparable benchmark charts only when they were captured for the same model, runner, and scenario. Account-history aggregates are not part of the public site.

## Publishing measured chart data

The public site shows one corrected reasoning matrix, six task-type views, a separate Frontier view, reasoning-level curves, and observed failures. Small standalone slices are not published as separate sections.

Generate chart data only from a fresh `benchmark-compare --group-by-reasoning` report with no mixed-input, directional, or provider/API warnings. Do not pass `--successful-only`; weighted validator scores from failed tasks are part of outcome quality.

```powershell
bun ..\scripts\publish_reasoning_sweep.mjs `
  --input ..\.spark-profile\benchmarks\real-world-comparison-<stamp>.json `
  --output-json src\data\reasoning-sweep.json `
  --output-csv ..\docs\benchmarks\reasoning-sweep-current-2026-08-09.csv `
  --output-summary ..\docs\benchmarks\reasoning-sweep-current-2026-08-09.md `
  --expected-repeats 2 `
  --expected-scenarios 12 `
  --date "August 9, 2026"
```

The publisher rejects invalid comparison inputs, derives Spark total-token counts from provider response usage in the referenced traces, keeps native Codex command-version provenance, and writes reviewable aggregate artifacts. Failed tasks retain their measured partial validator score and remain in the pass-rate denominator. Provider/API failures are excluded from measurement and prevent publication. Absolute trace paths are not published.
