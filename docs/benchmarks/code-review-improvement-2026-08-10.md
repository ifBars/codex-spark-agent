# Code-review improvement benchmark — 2026-08-10

## Metric

The primary metric is the three-repeat mean task-quality score for the existing `pull-request-review` scenario. The refactored scenario contains eleven independently weighted regressions and two correct changes used as false-positive controls. Each finding is evaluated from a structured `review.json` object with its own source, evidence, impact, fix, and regression-test fields. The task prompt names only the review procedure and output schema; it does not identify expected findings.

The comparison is successful when the primary metric improves by at least 10% relative while `dependency-upgrade-triage` remains at 100 task quality and review completion, tool reliability, and scope discipline do not materially regress.

## Paired result

| Measure | Baseline | Focused code-review skill | Change |
| --- | ---: | ---: | ---: |
| Pull-request-review task quality | 77.0 | 87.0 | +13.0% relative |
| Pull-request-review process score | 67.9 | 81.5 | +13.6 points |
| Pull-request-review tool/item calls | 43 | 39 | -4 |
| Dependency-upgrade-triage task quality | 100.0 | 100.0 | unchanged |
| Dependency-upgrade-triage process score | 94.5 | 92.8 | -1.7 points |

The required review-quality threshold was 84.7 (`77.0 × 1.10`). The observed 87.0 clears it by 2.3 points. Both paired variants completed all six runs. Each had two tool failures across the combined slice, and every failure recovered. The focused variant also improved review expected-call coverage and reduced total tool calls, so the small control-process movement is not a material reliability or scope regression.

## Evidence

- Baseline manifest: `.spark-profile/benchmarks/analysis-run-1786424071527.json`
- Focused-skill manifest: `.spark-profile/benchmarks/analysis-run-1786424391156.json`
- Paired comparison: `.spark-profile/benchmarks/analysis-comparison-1786424399090.json`

These generated artifacts are local benchmark evidence and are not source-controlled.
