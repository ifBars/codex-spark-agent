import { useMemo, useState } from "react";
import { CostQualityChart } from "./CostQualityChart.jsx";
import { ResultsLedger } from "./ResultsLedger.jsx";

export function ScenarioLens({
  scenarioViews,
  enabledRunners,
  enabledReasoning,
  xMetric,
  yMetric,
  source,
}) {
  const [scenarioId, setScenarioId] = useState(scenarioViews[0]?.id ?? "");
  const scenario = scenarioViews.find((candidate) => candidate.id === scenarioId)
    ?? scenarioViews[0];
  const rows = useMemo(
    () =>
      scenario.rows.filter(
        (row) => enabledRunners.has(row.runner) && enabledReasoning.has(row.reasoning),
      ),
    [enabledReasoning, enabledRunners, scenario],
  );

  return (
    <section
      className="scenario-lens"
      id="benchmark-scenarios"
      aria-labelledby="scenario-lens-title"
    >
      <header className="scenario-lens__heading">
        <div>
          <p>Measured task detail</p>
          <h2 id="scenario-lens-title">Scenario lens</h2>
          <span>
            Inspect the six runner/reasoning means behind the category curves.
            Per-run dispersion is unavailable in the published aggregate, so this
            view deliberately omits error bars.
          </span>
        </div>

        <label className="field-control scenario-lens__selector">
          <span>Scenario</span>
          <select
            value={scenario.id}
            onChange={(event) => {
              setScenarioId(event.target.value);
            }}
          >
            {scenarioViews.map((candidate) => (
              <option value={candidate.id} key={candidate.id}>
                {candidate.label}
              </option>
            ))}
          </select>
        </label>
      </header>

      <div className="scenario-lens__chart">
        <CostQualityChart
          rows={rows}
          xMetric={xMetric}
          yMetric={yMetric}
          showRanges={false}
          rangeKind={scenario.rangeKind}
          contextLabel={scenario.label}
          description={scenario.description}
          meta={`1 measured task · ${scenario.runCount} runs per level`}
          wide
        />
      </div>

      <ResultsLedger
        rows={rows}
        xMetric={xMetric}
        yMetric={yMetric}
        source={source}
        rangeKind={scenario.rangeKind}
        title="Task configurations"
        pointLabel="runner/reasoning points"
        showRangeColumns={false}
      />
    </section>
  );
}
