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

function formatOptionalInt(value) {
  return Number.isFinite(value) ? new Intl.NumberFormat("en-US").format(value) : "Not reported";
}

function usagePricingStatus(pricing = {}) {
  if (pricing.status === "estimated" || pricing.status === "available") return "Estimated";
  if (pricing.status === "partial") return "Partial";
  return "Unavailable";
}

export function UsageEvidence({ history }) {
  const hasActivity = usageHistoryHasActivity(history);
  const aggregate = history?.aggregate;
  const totalTokens = metricTotal(aggregate?.totalTokens);
  const totalMetric = aggregate?.totalTokens;
  const modelRows = (history?.byModel ?? []).filter((row) => metricTotal(row.totalTokens) !== null);
  const dayRows = (history?.byDay ?? []).filter((row) => row.day);

  const source = history?.source ?? {};
  const scope = history?.scope ?? {};
  const scan = history?.scan ?? {};
  const forkReplaySkips = Number.isFinite(scan.forkReplayedObservationsSkipped)
    ? scan.forkReplayedObservationsSkipped
    : 0;
  const forkWithoutEvidence = Number.isFinite(scan.forkObservationsWithoutCumulativeEvidence)
    ? scan.forkObservationsWithoutCumulativeEvidence
    : 0;
  const filesDiscovered = Number.isFinite(scan.filesDiscovered) ? scan.filesDiscovered : 0;
  const filesScanned = Number.isFinite(scan.filesScanned) ? scan.filesScanned : 0;
  const filesUnreadable = Number.isFinite(scan.filesUnreadable) ? scan.filesUnreadable : 0;
  const malformedLines = Number.isFinite(scan.malformedLines) ? scan.malformedLines : 0;
  const partialObservations = Number.isFinite(scan.partialObservations) ? scan.partialObservations : 0;
  const cumulativeFallbackObservations = Number.isFinite(scan.cumulativeFallbackObservations)
    ? scan.cumulativeFallbackObservations
    : 0;
  const filesTruncated = scan.filesTruncated === true;

  const scopeText = [
    scope.sinceDays === null || scope.sinceDays === undefined ? null : `${scope.sinceDays}d`,
    scope.maxFiles === null || scope.maxFiles === undefined ? null : `max ${scope.maxFiles} files`,
  ].filter(Boolean).join(` ${"\u2022"} `);

  const scanProfile = scopeText.length > 0 ? scopeText : "full local scan";
  const pricingStatus = usagePricingStatus(history?.pricing);
  const usageConfidence = hasActivity
    ? forkReplaySkips === 0 && forkWithoutEvidence === 0 && filesUnreadable === 0
      ? "high"
      : "medium"
    : "low";

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

      <div className="usage-evidence__command" aria-label="Usage history capture command">
        <code>
          <span className="usage-evidence__prompt">spark usage --history --json --output usage-history.json</span>
        </code>
        <span className="usage-evidence__command-meta">
          scoped by {scanProfile}
        </span>
      </div>

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
              <dt>Fork replay filtered</dt>
              <dd>{formatOptionalInt(forkReplaySkips)}</dd>
              <small>Duplicate forked observations skipped</small>
            </div>
            <div>
              <dt>Fork source signal</dt>
              <dd>{formatOptionalInt(forkWithoutEvidence)}</dd>
              <small>Forked observations without cumulative deltas</small>
            </div>
            <div>
              <dt>Pricing signal</dt>
              <dd>{pricingStatus}</dd>
              <small>{history.pricing?.reason ?? "No pricing record imported"}</small>
            </div>
            <div>
              <dt>Account quota</dt>
              <dd>{history.quota?.status === "available" ? "Imported" : "Not published"}</dd>
              <small>{history.quota?.reason ?? "Quota is separate from benchmark signals"}</small>
            </div>
            <div>
              <dt>History source</dt>
              <dd>{source.kind ?? "local"} ({source.network ? "networked" : "offline"})</dd>
              <small>{source.codexHomeSource ? `source=${source.codexHomeSource}` : "source unknown"}</small>
            </div>
            <div>
              <dt>Scan profile</dt>
              <dd>{scopeText || "unbounded scan"}</dd>
              <small>Files: {filesScanned}/{filesDiscovered}, truncated={filesTruncated ? "yes" : "no"}</small>
            </div>
            <div>
              <dt>Evidence confidence</dt>
              <dd>{usageConfidence.toUpperCase()}</dd>
              <small>fork + completeness integrity</small>
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
              <p>
                Scan: {filesScanned}/{filesDiscovered} files {`\u2022`} partial updates {partialObservations} {`\u2022`} fallback deltas {cumulativeFallbackObservations}
              </p>
              <p>
                Readability: {filesUnreadable} unreadable files {`\u2022`} malformed lines {malformedLines}
              </p>
              {filesTruncated ? <p>Scan was truncated by max-files bound.</p> : null}
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
