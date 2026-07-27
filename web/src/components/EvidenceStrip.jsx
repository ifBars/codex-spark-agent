function exclusionText(evidence) {
  const parts = [];
  if (evidence.taskFailureExclusions !== null) {
    parts.push(`${evidence.taskFailureExclusions} task`);
  }
  if (evidence.providerExclusions !== null) {
    parts.push(`${evidence.providerExclusions} provider/API`);
  }
  return parts.length > 0 ? parts.join(" · ") : "Not classified";
}

export function EvidenceStrip({ evidence }) {
  const coverage = evidence.pendingScenarioCount
    ? `${evidence.scenarioCount} measured · ${evidence.pendingScenarioCount} pending`
    : `${evidence.scenarioCount} measured scenarios`;
  const provenanceClass = evidence.pendingScenarios.length === 0
    ? "evidence-strip__provenance evidence-strip__provenance--sources-only"
    : "evidence-strip__provenance";

  return (
    <section className="evidence-strip" aria-labelledby="evidence-status-title">
      <div className="evidence-strip__lead">
        <p id="evidence-status-title">Evidence status</p>
        <strong>{evidence.status}</strong>
      </div>

      <dl>
        <div>
          <dt>Coverage</dt>
          <dd>{coverage}</dd>
        </div>
        <div>
          <dt>Matrix</dt>
          <dd>{evidence.taskRuns} task runs</dd>
        </div>
        <div>
          <dt>Exclusions</dt>
          <dd>{exclusionText(evidence)}</dd>
        </div>
        <div>
          <dt>Scoring</dt>
          <dd>{evidence.taskFailuresRetained ? "Task failures retained" : "Successful rows only"}</dd>
        </div>
      </dl>

      <p className="evidence-strip__note">{evidence.note}</p>

      <div className={provenanceClass}>
        {evidence.pendingScenarios.length > 0 && (
          <div>
            <span>Pending validation</span>
            <p>
              {evidence.pendingScenarios.map((scenario) => (
                <a key={scenario.id} href={scenario.url} target="_blank" rel="noreferrer">
                  {scenario.label} · {scenario.validationSignals} signals
                </a>
              ))}
            </p>
          </div>
        )}
        <div>
          <span>Artifacts</span>
          <p>
            {evidence.sources.map((source) => (
              <a key={source.path} href={source.url} target="_blank" rel="noreferrer">
                {source.label}
              </a>
            ))}
          </p>
        </div>
      </div>
    </section>
  );
}
