# Current reasoning sweep

Published August 9, 2026. This is a paired 12-scenario sweep of Spark harness and native Codex CLI using `gpt-5.3-codex-spark` at low, medium, and high reasoning.

- 144 total attempts; 107 passed and 37 failed.
- Quality is the scenario-balanced mean of weighted validator scores across every non-infrastructure attempt, including partial scores from failed tasks. Pass rate reports full task success.
- 4 scenarios fully passed for every runner and reasoning level; the common-scenario column provides a same-task control.
- 2 attempts per runner/reasoning/scenario cell provide a repeat check; conclusions remain bounded to these fixtures.

| Runner | Reasoning | Outcome quality | Full-pass common-task quality | Process | Pass rate | Tokens | Duration | Near ceiling |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Spark harness | Low | 84.8 | 100.0 | 82.8 | 70.8% | 182,394 | 37.7s | 7/12 |
| Spark harness | Medium | 89.4 | 100.0 | 84.5 | 79.2% | 146,772 | 26.8s | 9/12 |
| Spark harness | High | 90.8 | 100.0 | 85.4 | 87.5% | 174,073 | 59.8s | 10/12 |
| Codex CLI 0.146.0 | Low | 80.3 | 100.0 | 98.0 | 66.7% | 141,786 | 21.5s | 7/12 |
| Codex CLI 0.146.0 | Medium | 82.1 | 100.0 | 97.1 | 70.8% | 165,398 | 39.9s | 7/12 |
| Codex CLI 0.146.0 | High | 82.3 | 100.0 | 97.3 | 70.8% | 166,823 | 29.4s | 8/12 |

Source comparison: `real-world-comparison-1786268975816.json` (local profiling artifact; absolute trace paths are not published).
