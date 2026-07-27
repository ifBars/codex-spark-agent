import { useMemo, useState } from "react";
import { ArrowRight, Flask, Info } from "@phosphor-icons/react";
import { datasets, reasoningOptions, runnerOptions } from "../data/benchmarks.js";
import { CostQualityChart } from "./CostQualityChart.jsx";
import { FilterStrip } from "./FilterStrip.jsx";
import { ResultsLedger } from "./ResultsLedger.jsx";

function toggleSet(current, value) {
  const next = new Set(current);
  if (next.has(value)) {
    if (next.size > 1) next.delete(value);
  } else {
    next.add(value);
  }
  return next;
}

export function BenchmarkExplorer() {
  const [datasetId, setDatasetId] = useState(datasets[0].id);
  const [xMetric, setXMetric] = useState("tokens");
  const [yMetric, setYMetric] = useState("quality");
  const [enabledRunners, setEnabledRunners] = useState(new Set(runnerOptions.map((runner) => runner.id)));
  const [enabledReasoning, setEnabledReasoning] = useState(new Set(reasoningOptions));
  const [showRanges, setShowRanges] = useState(true);
  const [selectedPoint, setSelectedPoint] = useState(null);

  const dataset = datasets.find((candidate) => candidate.id === datasetId) ?? datasets[0];
  const rows = useMemo(
    () =>
      dataset.rows.filter(
        (row) => enabledRunners.has(row.runner) && enabledReasoning.has(row.reasoning),
      ),
    [dataset, enabledReasoning, enabledRunners],
  );
  const selectedVisible = selectedPoint && rows.includes(selectedPoint) ? selectedPoint : rows[0];

  return (
    <>
      <div className="page-frame">
        <header className="page-intro">
          <div>
            <p className="page-intro__context">GPT-5.3 Codex Spark</p>
            <h1>Benchmark explorer</h1>
            <p className="page-intro__summary">
              Compare reasoning cost, quality, duration, and full-pass rate across the Spark harness and native Codex CLI.
            </p>
          </div>
          <div className="dataset-note">
            <span>{dataset.date}</span>
            <strong>{dataset.sample}</strong>
            <p>{dataset.description}</p>
          </div>
        </header>

        <FilterStrip
          datasetId={datasetId}
          onDatasetChange={(value) => {
            setDatasetId(value);
            setSelectedPoint(null);
          }}
          xMetric={xMetric}
          onXMetricChange={setXMetric}
          yMetric={yMetric}
          onYMetricChange={setYMetric}
          enabledRunners={enabledRunners}
          onToggleRunner={(runner) => {
            setEnabledRunners((current) => toggleSet(current, runner));
            setSelectedPoint(null);
          }}
          enabledReasoning={enabledReasoning}
          onToggleReasoning={(reasoning) => {
            setEnabledReasoning((current) => toggleSet(current, reasoning));
            setSelectedPoint(null);
          }}
          showRanges={showRanges}
          onShowRangesChange={setShowRanges}
        />

        <CostQualityChart
          rows={rows}
          xMetric={xMetric}
          yMetric={yMetric}
          showRanges={showRanges}
          selectedPoint={selectedVisible}
          onSelectPoint={setSelectedPoint}
          rangeKind={dataset.rangeKind}
        />

        <ResultsLedger
          rows={rows}
          xMetric={xMetric}
          yMetric={yMetric}
          source={dataset.source}
          rangeKind={dataset.rangeKind}
        />
      </div>

      <section className="methodology-band" id="methodology" aria-labelledby="methodology-title">
        <div className="methodology-band__inner">
          <div className="methodology-title">
            <Flask aria-hidden="true" />
            <div>
              <p>How to read this</p>
              <h2 id="methodology-title">Methodology and caveats</h2>
            </div>
          </div>

          <div className="methodology-copy">
            <article>
              <h3>Weighted validation</h3>
              <p>
                Each fixture uses scenario-specific behavioral checks. Incomplete task work retains partial quality instead of receiving one blanket score.
              </p>
            </article>
            <article>
              <h3>Failure handling</h3>
              <p>
                Genuine task failures stay in the aggregate. Provider and API failures are excluded because they do not measure task performance.
              </p>
            </article>
            <article>
              <h3>Scope</h3>
              <p>
                This is bounded benchmark evidence, not a general model-quality claim. Switch datasets to compare the expanded suite, pilot, and saturated baseline.
              </p>
            </article>
          </div>

          <a className="methodology-link" href={dataset.source} target="_blank" rel="noreferrer">
            <Info aria-hidden="true" />
            Inspect source data
            <ArrowRight aria-hidden="true" />
          </a>
        </div>
      </section>
    </>
  );
}
