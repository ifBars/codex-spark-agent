# Experiment rollout audit fixture

`experiment-rollout-audit` is a validated benchmark fixture awaiting a balanced
Spark-versus-Codex run. It is not part of the published Capability Atlas curves
and has no reported model score yet.

The task requires an agent to:

- resolve duplicate and conflicting experiment assignments;
- apply explicit user exclusions;
- deduplicate events and repeated order identifiers;
- enforce a half-open 72-hour attribution window;
- join refunds only to attributed orders;
- calculate conversion, revenue, refund, and uplift metrics; and
- evaluate three launch gates before writing a decision memo.

Its weighted validation signals are:

| Signal | Weight |
| --- | ---: |
| Exact output schema | 10 |
| Assignment and event data quality | 20 |
| Control attribution metrics | 15 |
| Treatment attribution metrics | 20 |
| Uplift and guardrail calculations | 20 |
| Decision memo and launch gates | 15 |

The fixture test independently recomputes expected metrics from the checked-in
CSV inputs. A mutation matrix also proves that schema, data-quality, control,
treatment, uplift, decision, and memo defects lose only their intended
partial-credit dimensions.

The fixture will enter published category and overall aggregates only after each
runner and reasoning level has comparable valid repeats. Provider/API failures
will remain exclusions rather than task scores.
