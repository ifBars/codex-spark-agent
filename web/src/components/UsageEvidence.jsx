import { ArrowRight, Database, ShieldCheck } from "@phosphor-icons/react";
import { usageHistoryHasActivity } from "../data/usage-history.js";

const metricOrder = [
  ["inputTokens", "Input"],
  ["cachedInputTokens", "Cached input"],
  ["cacheWriteInputTokens", "Cache writes"],
  ["uncachedInputTokens", "Derived uncached input"],
  ["outputTokens", "Output"],
  ["reasoningOutputTokens", "Reasoning subset"],
  ["totalTokens", "Total tokens"],
];

function metricTotal(metric) {
  return Number.isFinite(metric?.total) ? metric.total : null;
}

function formatTokens(value) {
  if (!Number.isFinite(value)) return "Not reported";
  return new Intl.NumberFormat("en-US", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function observationCount(aggregate) {
  return aggregate?.observations ?? aggregate?.totalTokens?.observations ?? null;
}

function metricStatus(metric) {
  if (!metric) return "Not reported";
  if (metric.complete === true) return "Complete";
  if (metric.status) return metric.status;
  return "Partial";
}

export function UsageEvidence({ history }) {
  const hasActivity = usageHistoryHasActivity(history);
  const aggregate = history?.aggregate;
  const totalTokens = metricTotal(aggregate?.totalTokens);
  const totalMetric = aggregate?.totalTokens;
  const modelRows = (history?.byModel ?? []).filter((row) => metricTotal(row.totalTokens) !== null);
  const dayRows = (history?.byDay ?? []).filter((row) => row.day);

  return (
    <section className="usage-evidence" id="usage-evidence" aria-labelledby="usage-evidence-title">
      <header className="usage-evidence__heading">
        <div>
          <p>Local aggregate only</p>
          <h2 id="usage-evidence-title">Usage evidence</h2>
          <span>
            Token activity is source-reported. Pricing availability and account quota stay separate so an unknown Spark rate never becomes a $0 claim.
          </span>
        </div>
        <div className="usage-evidence__status" data-available={hasActivity}>
          <Database aria-hidden="true" />
          <span>{hasActivity ? "Aggregate imported" : "No public aggregate"}</span>
        </div>
      </header>

      {!hasActivity ? (
        <div className="usage-evidence__empty">
          <ShieldCheck aria-hidden="true" />
          <div>
            <strong>Usage history stays local until you choose to publish an aggregate.</strong>
            <p>{history?.reason ?? "No usage-history artifact is available for this build."}</p>
          </div>
          <code>spark usage --history --json --output usage-history.json</code>
        </div>
      ) : (
        <>
          <dl className="usage-evidence__summary">
            <div>
              <dt>Authoritative activity</dt>
              <dd>{formatTokens(totalTokens)}</dd>
              <small>{metricStatus(totalMetric)} total-token coverage</small>
            </div>
            <div>
              <dt>Observed responses</dt>
              <dd>{Number.isFinite(observationCount(aggregate)) ? observationCount(aggregate) : "Not reported"}</dd>
              <small>Deduplicated local usage observations</small>
            </div>
            <div>
              <dt>Pricing availability</dt>
              <dd>{history.pricing?.status === "available" ? "Estimated" : "Unavailable"}</dd>
              <small>{history.pricing?.reason ?? "No pricing record imported"}</small>
            </div>
            <div>
              <dt>Account quota</dt>
              <dd>{history.quota?.status === "available" ? "Imported" : "Not published"}</dd>
              <small>{history.quota?.reason ?? "Quota is a separate account signal"}</small>
            </div>
          </dl>

          <div className="usage-evidence__metrics" aria-label="Token metric breakdown">
            {metricOrder.map(([key, label]) => {
              const metric = aggregate?.[key];
              return (
                <div key={key}>
                  <span>{label}</span>
                  <strong>{formatTokens(metricTotal(metric))}</strong>
                  <small>{metricStatus(metric)}</small>
                </div>
              );
            })}
          </div>

          <div className="usage-evidence__detail">
            <div>
              <span>Coverage</span>
              <p>{dayRows.length} day rows / {modelRows.length} model rows / generated {history.generatedAt ?? "not reported"}</p>
            </div>
            {modelRows.length > 0 ? (
              <div className="usage-evidence__models">
                <span>By model</span>
                <ul>
                  {modelRows.slice(0, 4).map((row) => (
                    <li key={row.model}>
                      <span>{row.model}</span>
                      <strong>{formatTokens(metricTotal(row.totalTokens))}</strong>
                    </li>
                  ))}
                </ul>
              </div>
            ) : null}
            <a href="#methodology">
              Evidence rules
              <ArrowRight aria-hidden="true" />
            </a>
          </div>
          <p className="usage-evidence__note">Reasoning tokens are a subset of output tokens. They are displayed for analysis and are never added to output or total again.</p>
        </>
      )}
    </section>
  );
}
