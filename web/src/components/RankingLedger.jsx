import { formatMetric, metricRange, sentenceCase } from "../lib/format.js";
import { xMetrics, yMetrics } from "../data/benchmarks.js";

export function RankingLedger({ rows, xMetric, yMetric, hasIntervals = true }) {
  const ranked = [...rows].sort(
    (left, right) => right[yMetrics[yMetric].key] - left[yMetrics[yMetric].key],
  );

  return (
    <aside className="ranking-ledger" aria-labelledby="ranking-title">
      <header>
        <div>
          <h2 id="ranking-title">Overall ranking</h2>
          <p>Sorted by {yMetrics[yMetric].label.toLowerCase()}.</p>
        </div>
      </header>

      <div className="ranking-ledger__columns" aria-hidden="true">
        <span>Rank / runner</span>
        <span>{yMetrics[yMetric].shortLabel}</span>
        <span>{xMetrics[xMetric].shortLabel}</span>
      </div>

      <ol>
        {ranked.map((row, index) => {
          const range = metricRange(row, yMetric, yMetrics);
          const uncertainty =
            row[`${yMetric}Ci`] ?? (range ? (range[1] - range[0]) / 2 : null);
          return (
            <li key={`${row.runner}-${row.reasoning}`}>
              <span className="ranking-ledger__rank">{index + 1}</span>
              <span className="ranking-ledger__name">
                <i style={{ background: row.color }} />
                {row.runnerShortName} — {sentenceCase(row.reasoning)}
              </span>
              <span>
                {formatMetric(yMetric, row[yMetrics[yMetric].key])}
                {uncertainty !== null && (
                  <small>
                    ±{formatMetric(yMetric, uncertainty)}
                  </small>
                )}
              </span>
              <span>{formatMetric(xMetric, row[xMetrics[xMetric].key])}</span>
            </li>
          );
        })}
      </ol>

      <div className="ranking-ledger__method">
        <strong>Methodology status</strong>
        <span>Measured task runs</span>
        <span>Scenario-balanced means</span>
        <span>Failed attempts excluded</span>
        <span>{hasIntervals ? "95% confidence ranges" : "No interval published"}</span>
      </div>
    </aside>
  );
}
