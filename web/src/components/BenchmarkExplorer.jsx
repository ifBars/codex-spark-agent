import { useMemo, useState } from "react";
import { ArrowRight, Flask, Info } from "@phosphor-icons/react";
import usageHistory from "../data/usage-history.json";
import {
  coverageLabel,
  datasets,
  reasoningOptions,
  runnerOptions,
} from "../data/benchmarks.js";
import { BenchmarkAtlasNav } from "./BenchmarkAtlasNav.jsx";
import { CostQualityChart } from "./CostQualityChart.jsx";
import { EvidenceStrip } from "./EvidenceStrip.jsx";
import { FilterStrip } from "./FilterStrip.jsx";
import { FrontierPendingChart } from "./FrontierPendingChart.jsx";
import { RankingLedger } from "./RankingLedger.jsx";
import { ResultsLedger } from "./ResultsLedger.jsx";
import { ScenarioLens } from "./ScenarioLens.jsx";
import { UsageEvidence } from "./UsageEvidence.jsx";

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

  const dataset = datasets.find((candidate) => candidate.id === datasetId) ?? datasets[0];
  const visibleViews = useMemo(
    () =>
      dataset.views.map((view) => ({
        ...view,
        rows: (view.rows ?? []).filter(
          (row) => enabledRunners.has(row.runner) && enabledReasoning.has(row.reasoning),
        ),
      })),
    [dataset, enabledReasoning, enabledRunners],
  );
  const overallView = visibleViews[0];
  const rows = overallView.rows;

  return (
    <>
      <div className="page-frame atlas-shell">
        <BenchmarkAtlasNav
          views={dataset.views}
          scenarioCount={dataset.scenarioViews?.length ?? 0}
        />

        <main className="atlas-main">
          <header className="page-intro page-intro--atlas">
            <div>
              <p className="page-intro__context">GPT-5.3 Codex Spark</p>
              <h1>Capability Atlas</h1>
              <p className="page-intro__summary">
                Compare reasoning cost, quality, and completion across real-world benchmark families for the Spark harness and native Codex CLI.
              </p>
            </div>
            <div className="dataset-note">
              <span>{dataset.date}</span>
              <strong>{overallView.sample ?? dataset.sample}</strong>
              <p>{overallView.description}</p>
            </div>
          </header>

          <EvidenceStrip evidence={dataset.evidence} />

          <UsageEvidence history={usageHistory} />

          <FilterStrip
            datasetId={datasetId}
            onDatasetChange={(value) => {
              setDatasetId(value);
            }}
            xMetric={xMetric}
            onXMetricChange={setXMetric}
            yMetric={yMetric}
            onYMetricChange={setYMetric}
            enabledRunners={enabledRunners}
            onToggleRunner={(runner) => {
              setEnabledRunners((current) => toggleSet(current, runner));
            }}
            enabledReasoning={enabledReasoning}
            onToggleReasoning={(reasoning) => {
              setEnabledReasoning((current) => toggleSet(current, reasoning));
            }}
            showRanges={showRanges}
            onShowRangesChange={setShowRanges}
          />

          <section
            className="atlas-overview"
            id={`benchmark-${overallView.id}`}
            aria-label={`${overallView.label} benchmark`}
          >
            <CostQualityChart
              rows={rows}
              xMetric={xMetric}
              yMetric={yMetric}
              showRanges={showRanges}
              rangeKind={dataset.rangeKind}
              contextLabel={overallView.label}
              description={overallView.description}
              meta={`${overallView.scenarioCount ?? "Historical"} tasks · ${dataset.evidence.taskRuns} successful runs · ${coverageLabel(overallView.scenarioCount)}`}
              wide
              showTooltip={false}
            />
            <RankingLedger
              rows={rows}
              xMetric={xMetric}
              yMetric={yMetric}
              hasIntervals={dataset.hasIntervals ?? true}
            />
          </section>

          {visibleViews.length > 1 && (
            <div className="atlas-category-grid" aria-label="Category benchmarks">
              {visibleViews.slice(1).map((view) => (
                <section
                  className={`atlas-category${view.wide ? " atlas-category--wide" : ""}`}
                  id={`benchmark-${view.id}`}
                  key={view.id}
                  aria-label={`${view.label} benchmark`}
                >
                  {view.status === "pending" ? (
                    <FrontierPendingChart
                      view={view}
                      enabledRunners={enabledRunners}
                      enabledReasoning={enabledReasoning}
                    />
                  ) : (
                    <CostQualityChart
                      rows={view.rows}
                      xMetric={xMetric}
                      yMetric={yMetric}
                      showRanges={showRanges}
                      rangeKind={dataset.rangeKind}
                      contextLabel={view.label}
                      description={view.description}
                      meta={`${view.scenarioCount} tasks · ${view.rows.reduce((sum, row) => sum + row.runs, 0)} successful runs · ${coverageLabel(view.scenarioCount)}`}
                      compact
                    />
                  )}
                </section>
              ))}
            </div>
          )}

          {dataset.scenarioViews?.length > 0 ? (
            <ScenarioLens
              scenarioViews={dataset.scenarioViews}
              enabledRunners={enabledRunners}
              enabledReasoning={enabledReasoning}
              xMetric={xMetric}
              yMetric={yMetric}
              source={dataset.source}
            />
          ) : null}

          <ResultsLedger
            rows={rows}
            xMetric={xMetric}
            yMetric={yMetric}
            source={dataset.source}
            rangeKind={dataset.rangeKind}
            showRangeColumns={dataset.hasIntervals ?? true}
          />
        </main>
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
                Each fixture uses scenario-specific behavioral checks. Successful attempts retain granular quality differences instead of collapsing every pass to a perfect score.
              </p>
            </article>
            <article>
              <h3>Failure handling</h3>
              <p>
                Failed task attempts and provider/API failures are excluded before chart aggregation. Their counts remain visible in the evidence strip so a missing run cannot silently become a zero score.
              </p>
            </article>
            <article>
              <h3>Scope</h3>
              <p>
                Category charts reuse the same measured runs and weight each included scenario equally. Narrow views have wider uncertainty and are not standalone model-quality claims.
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
