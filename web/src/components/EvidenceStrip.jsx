function exclusionText(value) {
  if (value === null) return "Not classified";
  return `${value} provider/API`;
}

export function EvidenceStrip({ evidence }) {
  const coverage = evidence.pendingScenarioCount
    ? `${evidence.scenarioCount} measured · ${evidence.pendingScenarioCount} pending`
    : `${evidence.scenarioCount} measured scenarios`;

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
          <dd>{exclusionText(evidence.providerExclusions)}</dd>
        </div>
        <div>
          <dt>Scoring</dt>
          <dd>{evidence.taskFailuresRetained ? "Task failures retained" : "Successful rows only"}</dd>
        </div>
      </dl>

      <p className="evidence-strip__note">{evidence.note}</p>
    </section>
  );
}
