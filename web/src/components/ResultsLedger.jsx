import { ArrowSquareOut } from "@phosphor-icons/react";
import { xMetrics, yMetrics } from "../data/benchmarks.js";
import { formatMetric, sentenceCase } from "../lib/format.js";

export function ResultsLedger({ rows, xMetric, yMetric, source, rangeKind }) {
  return (
    <section className="ledger" aria-labelledby="ledger-title">
      <div className="ledger__heading">
        <div>
          <h2 id="ledger-title">Visible points</h2>
          <p>{rows.length} aggregate points · {rangeKind}</p>
        </div>
        <a href={source} target="_blank" rel="noreferrer">
          Source CSV <ArrowSquareOut aria-hidden="true" />
        </a>
      </div>

      <div className="ledger__scroll">
        <table>
          <thead>
            <tr>
              <th>Runner</th>
              <th>Reasoning</th>
              <th>{yMetrics[yMetric].shortLabel}</th>
              <th>{xMetrics[xMetric].shortLabel}</th>
              <th>Quality range</th>
              <th>Token range</th>
              <th>Runs</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((row) => (
              <tr key={`${row.runner}-${row.reasoning}`}>
                <td data-label="Runner">
                  <span className="runner-cell">
                    <i style={{ background: row.color }} />
                    {row.runnerShortName}
                  </span>
                </td>
                <td data-label="Reasoning">{sentenceCase(row.reasoning)}</td>
                <td data-label={yMetrics[yMetric].shortLabel} className="numeric">
                  {formatMetric(yMetric, row[yMetrics[yMetric].key])}
                </td>
                <td data-label={xMetrics[xMetric].shortLabel} className="numeric">
                  {formatMetric(xMetric, row[xMetrics[xMetric].key])}
                </td>
                <td data-label="Quality range" className="numeric">
                  {formatMetric("quality", row.qualityMin)}–{formatMetric("quality", row.qualityMax)}
                </td>
                <td data-label="Token range" className="numeric">
                  {formatMetric("tokens", row.tokensMin)}–{formatMetric("tokens", row.tokensMax)}
                </td>
                <td data-label="Runs" className="numeric">{row.runs}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
