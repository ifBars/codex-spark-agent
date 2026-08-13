# Current reasoning sweep

Published August 12, 2026. This is a paired 12-scenario sweep of Spark harness and native Codex CLI using `gpt-5.3-codex-spark` at low, medium, and high reasoning.

- 216 total attempts; 165 passed and 51 failed.
- Quality is the scenario-balanced mean of weighted validator scores across every non-infrastructure attempt, including partial scores from failed tasks. Pass rate reports full task success.
- 5 scenarios fully passed for every runner and reasoning level; the common-scenario column provides a same-task control.
- 3 attempts per runner/reasoning/scenario cell provide a repeat check; conclusions remain bounded to these fixtures.

| Runner | Reasoning | Outcome quality | Full-pass common-task quality | Process | Pass rate | Tokens | Duration | Near ceiling |
| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |
| Spark harness | Low | 88.0 | 100.0 | 84.7 | 77.8% | 88,526 | 31.6s | 8/12 |
| Spark harness | Medium | 92.0 | 100.0 | 89.2 | 86.1% | 63,317 | 41.4s | 10/12 |
| Spark harness | High | 89.6 | 100.0 | 82.6 | 83.3% | 96,182 | 40.3s | 10/12 |
| Codex CLI 0.147.0 | Low | 79.6 | 100.0 | 97.2 | 66.7% | 201,380 | 50.2s | 5/12 |
| Codex CLI 0.147.0 | Medium | 81.2 | 100.0 | 96.6 | 75.0% | 254,541 | 52.7s | 8/12 |
| Codex CLI 0.147.0 | High | 81.4 | 100.0 | 96.8 | 69.4% | 214,474 | 54.6s | 5/12 |

Source comparison: `real-world-comparison-1786590614322.json` (local profiling artifact; absolute trace paths are not published).
