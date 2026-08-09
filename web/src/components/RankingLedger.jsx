import { useId } from "react";
import { formatMetric, metricRange, sentenceCase } from "../lib/format.js";
import { xMetrics, yMetrics } from "../data/benchmarks.js";

export function RankingLedger({ rows, xMetric, yMetric }) {
  const titleId = useId();
  const ranked = [...rows].sort(
    (left, right) => right[yMetrics[yMetric].key] - left[yMetrics[yMetric].key],
  );

  return (
    <aside className="ranking-ledger" aria-labelledby={titleId}>
      <header>
        <div>
          <h2 id={titleId}>Overall ranking</h2>
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
    </aside>
  );
}
