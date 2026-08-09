import { useMemo, useState } from "react";
import { ArrowRight } from "@phosphor-icons/react";
import { benchmarkCohorts } from "../data/benchmarks.js";
import { CostQualityChart } from "./CostQualityChart.jsx";
import { EvidenceStrip } from "./EvidenceStrip.jsx";
import { FrontierPendingChart } from "./FrontierPendingChart.jsx";
import { RankingLedger } from "./RankingLedger.jsx";
import { ReasoningLevelChart } from "./ReasoningLevelChart.jsx";
import { ResultsLedger } from "./ResultsLedger.jsx";

const metricViews = [
  {
    id: "efficiency",
    label: "Quality / tokens",
    shortLabel: "Efficiency",
    xMetric: "tokens",
    yMetric: "quality",
  },
  {
    id: "speed",
    label: "Quality / time",
    shortLabel: "Speed",
    xMetric: "duration",
    yMetric: "quality",
  },
  {
    id: "reliability",
    label: "Validation / tokens",
    shortLabel: "Reliability",
    xMetric: "tokens",
    yMetric: "attemptPassRate",
  },
];

const allRunners = new Set(["spark", "codex"]);
const allReasoning = new Set(["low", "medium", "high"]);

function relativeLabel(value, baseline, noun) {
  const difference = Math.round(((value / baseline) - 1) * 100);
  if (noun === "time") {
    return `${Math.abs(difference)}% ${difference <= 0 ? "less" : "more"} time`;
  }
  return `${Math.abs(difference)}% ${difference <= 0 ? "fewer" : "more"} ${noun}`;
}

function signedPercent(value, baseline) {
  const difference = Math.round(((value / baseline) - 1) * 100);
  return `${difference >= 0 ? "+" : ""}${difference}%`;
}

function signedDelta(value, baseline) {
  const delta = value - baseline;
  return `${delta >= 0 ? "+" : ""}${delta.toFixed(1)}`;
}

function titleCase(value) {
  return value
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function CohortSection({ dataset, index, xMetric, yMetric }) {
  const overallView = dataset.views[0];
  const categories = dataset.views.slice(1).filter((view) => view.id !== "frontier");
  const frontierView = dataset.views.find((view) => view.id === "frontier");
  const rows = overallView.rows;
  const sectionNumber = String(index + 1).padStart(2, "0");

  return (
    <section
      className="cohort-section"
      id={`cohort-${dataset.id}`}
      aria-labelledby={`cohort-${dataset.id}-title`}
    >
      <header className="cohort-section__header">
        <div className="cohort-section__number" aria-hidden="true">{sectionNumber}</div>
        <div className="cohort-section__title">
          <p>{dataset.date}</p>
          <h2 id={`cohort-${dataset.id}-title`}>{dataset.label}</h2>
        </div>
        <p className="cohort-section__summary">{dataset.description}</p>
      </header>

      <EvidenceStrip evidence={dataset.evidence} idPrefix={dataset.id} />

      <div className="cohort-section__visual">
        <CostQualityChart
          rows={rows}
          xMetric={xMetric}
          yMetric={yMetric}
          showRanges={dataset.hasIntervals ?? true}
          contextLabel={overallView.label}
          meta={`${overallView.scenarioCount} tasks · ${(dataset.evidence.attemptedTaskRuns ?? dataset.evidence.taskRuns + (dataset.evidence.taskFailureExclusions ?? 0))} attempts`}
          wide
        />
        <RankingLedger
          rows={rows}
          xMetric={xMetric}
          yMetric={yMetric}
        />
      </div>

      <ResultsLedger
        rows={rows}
        xMetric={xMetric}
        yMetric={yMetric}
        source={dataset.source}
        rangeKind={dataset.rangeKind}
        title={`${dataset.label} results`}
        showRangeColumns={dataset.hasIntervals ?? true}
      />

      {categories.length > 0 ? (
        <div className="cohort-section__categories">
          <header className="category-heading">
            <div>
              <p>Task types</p>
              <h3>Results by task type</h3>
            </div>
            <span>
              These charts regroup the same runs. Small task groups are useful for debugging,
              not broad rankings.
            </span>
          </header>
          <div className="atlas-category-grid" aria-label="Task type benchmarks">
            {categories.map((view) => (
              <section
                className={`atlas-category${view.wide ? " atlas-category--wide" : ""}`}
                id={`benchmark-${view.id}`}
                key={view.id}
                aria-label={`${view.label} benchmark`}
              >
                {view.status === "pending" ? (
                  <FrontierPendingChart
                    view={view}
                    enabledRunners={allRunners}
                    enabledReasoning={allReasoning}
                  />
                ) : (
                  <CostQualityChart
                    rows={view.rows}
                    xMetric={xMetric}
                    yMetric={yMetric}
                    showRanges={view.hasIntervals ?? true}
                    contextLabel={view.label}
                    description={view.description}
                    meta={`${view.scenarioCount} tasks · ${view.rows.reduce(
                      (sum, row) => sum + row.runs + (row.excludedRuns ?? 0),
                      0,
                    )} attempts`}
                    compact
                  />
                )}
              </section>
            ))}
          </div>
        </div>
      ) : null}

      {frontierView ? (
        <div className="cohort-section__categories cohort-section__frontier">
          <header className="category-heading">
            <div>
              <p>Harder tasks</p>
              <h3>Frontier</h3>
            </div>
            <span>Transfer and consistency tasks kept separate from the task-type averages.</span>
          </header>
          <div className="atlas-category-grid" aria-label="Frontier benchmark">
            <section
              className="atlas-category atlas-category--wide"
              id={`benchmark-${frontierView.id}`}
              aria-label={`${frontierView.label} benchmark`}
            >
              {frontierView.status === "pending" ? (
                <FrontierPendingChart
                  view={frontierView}
                  enabledRunners={allRunners}
                  enabledReasoning={allReasoning}
                />
              ) : (
                <CostQualityChart
                  rows={frontierView.rows}
                  xMetric={xMetric}
                  yMetric={yMetric}
                  showRanges={frontierView.hasIntervals ?? true}
                  contextLabel={frontierView.label}
                  description={frontierView.description}
                  meta={`${frontierView.scenarioCount} tasks · ${frontierView.rows.reduce(
                    (sum, row) => sum + row.runs + (row.excludedRuns ?? 0),
                    0,
                  )} attempts`}
                  compact
                />
              )}
            </section>
          </div>
        </div>
      ) : null}
    </section>
  );
}

function ReliabilitySection({ dataset }) {
  const runnerRows = ["spark", "codex"].map((runner) => {
    const rows = dataset.rows.filter((row) => row.runner === runner);
    const passed = rows.reduce((sum, row) => sum + row.runs, 0);
    const failed = rows.reduce((sum, row) => sum + (row.excludedRuns ?? 0), 0);
    return {
      runner,
      runnerName: rows[0].runnerShortName,
      color: rows[0].color,
      runs: passed,
      failed,
      attemptPassRate: (passed / (passed + failed)) * 100,
    };
  });
  const failedAttempts = dataset.attempts.filter((attempt) => attempt.failed > 0);
  const totalFailures = runnerRows.reduce((sum, row) => sum + row.failed, 0);

  return (
    <section className="reliability-overview" id="reliability" aria-labelledby="reliability-title">
      <header className="reliability-overview__heading">
        <div>
          <p>Task reliability</p>
          <h2 id="reliability-title">How often each runner succeeds or fails</h2>
        </div>
        <span>
          Every attempt stays in this denominator. Partial validator scores remain in quality;
          only full task passes count as success.
        </span>
      </header>

      <div className="reliability-overview__bars">
        {runnerRows.map((row) => {
          const failed = row.failed;
          const attempts = row.runs + failed;
          return (
            <article key={row.runner}>
              <div className="reliability-overview__runner">
                <span><i style={{ background: row.color }} />{row.runnerName}</span>
                <strong>{row.attemptPassRate.toFixed(1)}%</strong>
              </div>
              <div
                className="reliability-bar"
                role="img"
                aria-label={`${row.runnerName}: ${row.runs} passed and ${failed} failed out of ${attempts} attempts`}
              >
                <i
                  className="reliability-bar__passed"
                  style={{ width: `${row.attemptPassRate}%`, background: row.color }}
                />
                <i className="reliability-bar__failed" />
              </div>
              <dl>
                <div><dt>Passed</dt><dd>{row.runs}</dd></div>
                <div><dt>Failed</dt><dd>{failed}</dd></div>
                <div><dt>Attempts</dt><dd>{attempts}</dd></div>
              </dl>
            </article>
          );
        })}
      </div>

      <div className="failure-ledger">
        <div>
          <p>Observed failures</p>
          <strong>{totalFailures} failed attempts</strong>
        </div>
        <div className="failure-ledger__detail">
          {failedAttempts.length > 0 ? (
            <ul>
              {failedAttempts.map((attempt) => (
                <li key={`${attempt.runner}-${attempt.reasoning}-${attempt.scenario}`}>
                  <span>{attempt.runnerName}</span>
                  <strong>{titleCase(attempt.scenario)}</strong>
                  <small>{titleCase(attempt.reasoning)} · {attempt.failed}/{attempt.attempts} failed</small>
                </li>
              ))}
            </ul>
          ) : null}
        </div>
      </div>
    </section>
  );
}

function ReasoningSection({ dataset }) {
  return (
    <section className="reasoning-overview" id="reasoning" aria-labelledby="reasoning-title">
      <header className="reasoning-overview__heading">
        <div>
          <p>Reasoning levels</p>
          <h2 id="reasoning-title">What changes from low to high</h2>
        </div>
        <span>
          Same {dataset.evidence.scenarioCount} tasks and {dataset.evidence.attemptedTaskRuns / 6} attempts per runner/reasoning level.
        </span>
      </header>
      <div className="reasoning-overview__charts">
        <ReasoningLevelChart
          rows={dataset.rows}
          metric="quality"
          title="Outcome quality"
          subtitle="Weighted task validators only"
        />
        <ReasoningLevelChart
          rows={dataset.rows}
          metric="passRate"
          title="Task pass rate"
          subtitle="Every attempt remains in the denominator"
        />
        <ReasoningLevelChart
          rows={dataset.rows}
          metric="tokens"
          title="Average token use"
          subtitle="Total API tokens per measured task attempt"
        />
        <ReasoningLevelChart
          rows={dataset.rows}
          metric="process"
          title="Process score"
          subtitle="Tool use, recovery, and completion behavior"
        />
      </div>
      <p className="reasoning-overview__note">
        Spark improved at each reasoning level in this run. Codex improved from low to medium,
        then changed little at high. Spark high was also the slowest setting.
      </p>
    </section>
  );
}

export function BenchmarkExplorer() {
  const [metricViewId, setMetricViewId] = useState(metricViews[0].id);
  const metricView = metricViews.find((view) => view.id === metricViewId) ?? metricViews[0];

  const summary = useMemo(() => {
    const successfulRuns = benchmarkCohorts.reduce(
      (total, cohort) => total + cohort.evidence.taskRuns + (cohort.extraTaskRuns ?? 0),
      0,
    );
    const reportedTaskFailures = benchmarkCohorts.reduce(
      (total, cohort) => total
        + (cohort.evidence.taskFailureExclusions ?? 0)
        + (cohort.extraTaskFailures ?? 0),
      0,
    );
    const dates = benchmarkCohorts.map((cohort) => cohort.date);
    return {
      publishedAttempts: successfulRuns + reportedTaskFailures,
      successfulRuns,
      reportedTaskFailures,
      latestDate: dates.reduce((latest, date) =>
        Date.parse(date) > Date.parse(latest) ? date : latest,
      dates[0]),
    };
  }, []);

  const insights = useMemo(() => {
    const primary = benchmarkCohorts[0];
    const sparkOverall = primary.rows.find(
      (row) => row.runner === "spark" && row.reasoning === "medium",
    );
    const codexOverall = primary.rows.find(
      (row) => row.runner === "codex" && row.reasoning === "medium",
    );
    const frontierRows = primary.views.find((view) => view.id === "frontier").rows;
    const sparkFrontier = frontierRows.find(
      (row) => row.runner === "spark" && row.reasoning === "medium",
    );
    const codexFrontier = frontierRows.find(
      (row) => row.runner === "codex" && row.reasoning === "medium",
    );
    const pairedTasks = primary.attempts
      .filter((row) => row.reasoning === "medium" && Number.isFinite(row.quality))
      .reduce((pairs, row) => {
        if (row.runner !== "spark") return pairs;
        const codex = primary.attempts.find(
          (candidate) => candidate.runner === "codex"
            && candidate.reasoning === "medium"
            && candidate.scenario === row.scenario
            && Number.isFinite(candidate.quality),
        );
        if (codex) pairs.push({ spark: row, codex });
        return pairs;
      }, []);
    return {
      cohortDate: primary.date,
      overallQualityDelta: signedDelta(sparkOverall.quality, codexOverall.quality),
      overallTokenComparison: relativeLabel(sparkOverall.tokens, codexOverall.tokens, "tokens"),
      overallTimeComparison: relativeLabel(sparkOverall.duration, codexOverall.duration, "time"),
      sparkAttemptPassRate: sparkOverall.attemptPassRate,
      codexAttemptPassRate: codexOverall.attemptPassRate,
      sparkQualityLeads: pairedTasks.filter(({ spark, codex }) => spark.quality > codex.quality).length,
      codexQualityLeads: pairedTasks.filter(({ spark, codex }) => codex.quality > spark.quality).length,
      qualityTies: pairedTasks.filter(({ spark, codex }) => codex.quality === spark.quality).length,
      frontierQualityDelta: signedDelta(sparkFrontier.quality, codexFrontier.quality),
      frontierProcessDelta: signedDelta(sparkFrontier.process, codexFrontier.process),
      frontierTokenDelta: signedPercent(sparkFrontier.tokens, codexFrontier.tokens),
      frontierTimeDelta: signedPercent(sparkFrontier.duration, codexFrontier.duration),
      sparkFrontierPassRate: sparkFrontier.attemptPassRate,
      codexFrontierPassRate: codexFrontier.attemptPassRate,
    };
  }, []);

  return (
    <>
      <div className="page-frame evidence-site">
        <header className="evidence-hero" id="evidence">
          <div className="evidence-hero__copy">
            <p className="page-intro__context">Spark harness vs. Codex CLI</p>
            <h1>Spark Bench</h1>
            <p>
              One paired sweep across the same 12 tasks at low, medium, and high reasoning.
              Compare outcome quality, pass rate, speed, token use, and process behavior.
            </p>
          </div>

        </header>

        <dl className="evidence-summary" aria-label="Combined evidence summary">
          <div>
            <dt>Published attempts</dt>
            <dd>{summary.publishedAttempts}</dd>
            <small>Across the complete paired matrix</small>
          </div>
          <div>
            <dt>Task passes</dt>
            <dd>{summary.successfulRuns}</dd>
            <small>Full validator pass</small>
          </div>
          <div>
            <dt>Failed task attempts</dt>
            <dd>{summary.reportedTaskFailures}</dd>
            <small>Included in success rates</small>
          </div>
          <div>
            <dt>Latest run</dt>
            <dd>{summary.latestDate?.replace(", 2026", "")}</dd>
            <small>{benchmarkCohorts[0].expectedRepeats} repeats per task and setting</small>
          </div>
        </dl>

        <section className="insights" aria-labelledby="insights-title">
          <header className="insights__heading">
            <div>
              <p>Latest run</p>
              <h2 id="insights-title">Current results</h2>
            </div>
            <span>Reasoning matrix · {insights.cohortDate}</span>
          </header>

          <div className="insights__grid">
            <article>
              <i data-color="blue" aria-hidden="true" />
              <p>Medium reasoning quality</p>
              <h3>{insights.overallQualityDelta} quality points</h3>
              <dl>
                <div><dt>Spark leads</dt><dd>{insights.sparkQualityLeads}</dd></div>
                <div><dt>Codex leads</dt><dd>{insights.codexQualityLeads}</dd></div>
                <div><dt>Ties</dt><dd>{insights.qualityTies}</dd></div>
              </dl>
            </article>

            <article>
              <i data-color="orange" aria-hidden="true" />
              <p>Medium reasoning efficiency</p>
              <h3>{insights.overallTokenComparison}</h3>
              <dl>
                <div><dt>Time</dt><dd>{insights.overallTimeComparison}</dd></div>
                <div><dt>Spark pass</dt><dd>{insights.sparkAttemptPassRate.toFixed(1)}%</dd></div>
                <div><dt>Codex pass</dt><dd>{insights.codexAttemptPassRate.toFixed(1)}%</dd></div>
              </dl>
            </article>

            <article>
              <i data-color="ink" aria-hidden="true" />
              <p>Frontier tradeoff</p>
              <h3>{insights.sparkFrontierPassRate.toFixed(0)}% vs. {insights.codexFrontierPassRate.toFixed(0)}% passed</h3>
              <dl>
                <div><dt>Quality</dt><dd>{insights.frontierQualityDelta} pts</dd></div>
                <div><dt>Process</dt><dd>{insights.frontierProcessDelta} pts</dd></div>
                <div><dt>Tokens / time</dt><dd>{insights.frontierTokenDelta} / {insights.frontierTimeDelta}</dd></div>
              </dl>
            </article>
          </div>
        </section>

        <ReliabilitySection dataset={benchmarkCohorts[0]} />
        <ReasoningSection dataset={benchmarkCohorts[0]} />

        <div className="evidence-layout">
          <aside className="evidence-nav" aria-label="Page sections">
            <p>On this page</p>
            <nav>
              {benchmarkCohorts.map((cohort, index) => (
                <a href={`#cohort-${cohort.id}`} key={cohort.id}>
                  <span>{String(index + 1).padStart(2, "0")}</span>
                  {cohort.shortLabel}
                </a>
              ))}
              <a href="#reliability">
                <span>{String(benchmarkCohorts.length + 1).padStart(2, "0")}</span>
                Reliability
              </a>
              <a href="#reasoning">
                <span>{String(benchmarkCohorts.length + 2).padStart(2, "0")}</span>
                Reasoning
              </a>
              <a href="#methodology">
                <span>{String(benchmarkCohorts.length + 3).padStart(2, "0")}</span>
                Notes
              </a>
            </nav>
          </aside>

          <div className="evidence-main">
            <section className="metric-switcher" aria-labelledby="metric-switcher-title">
              <div>
                <p>Chart</p>
                <h2 id="metric-switcher-title">Compare quality, speed, or reliability</h2>
              </div>
              <div className="metric-switcher__tabs" role="tablist" aria-label="Chart metric">
                {metricViews.map((view) => (
                  <button
                    type="button"
                    role="tab"
                    aria-selected={metricView.id === view.id}
                    data-active={metricView.id === view.id}
                    onClick={() => setMetricViewId(view.id)}
                    key={view.id}
                  >
                    <span>{view.shortLabel}</span>
                    <small>{view.label}</small>
                  </button>
                ))}
              </div>
            </section>

            {benchmarkCohorts.map((cohort, index) => (
              <CohortSection
                dataset={cohort}
                index={index}
                xMetric={metricView.xMetric}
                yMetric={metricView.yMetric}
                key={cohort.id}
              />
            ))}

          </div>
        </div>
      </div>

      <section className="methodology-band" id="methodology" aria-labelledby="methodology-title">
        <div className="methodology-band__inner">
          <div className="methodology-title">
            <div>
              <p>Notes</p>
              <h2 id="methodology-title">Reading the results</h2>
            </div>
          </div>

          <div className="methodology-copy">
            <article>
              <h3>Runs stay separate</h3>
              <p>
                Results from different dates, runner versions, and task sets are not averaged together.
              </p>
            </article>
            <article>
              <h3>Failures count</h3>
              <p>
                Failed tasks stay in pass-rate totals. Their weighted validator score also remains in quality.
              </p>
            </article>
            <article>
              <h3>Small samples have limits</h3>
              <p>
                One-run and narrow task groups can show where to investigate. They are not broad rankings.
              </p>
            </article>
          </div>

          <a
            className="methodology-link"
            href={benchmarkCohorts[0].source}
            target="_blank"
            rel="noreferrer"
          >
            View source data
            <ArrowRight aria-hidden="true" />
          </a>
        </div>
      </section>
    </>
  );
}
