function failureText(evidence) {
  const parts = [];
  if (evidence.taskFailureExclusions !== null) {
    const count = evidence.taskFailureExclusions;
    parts.push(`${count} task${count === 1 ? "" : "s"}`);
  }
  if (evidence.providerExclusions !== null) {
    const count = evidence.providerExclusions;
    parts.push(`${count} provider/API failure${count === 1 ? "" : "s"}`);
  }
  return parts.length > 0 ? parts.join(" · ") : "Not classified";
}

export function EvidenceStrip({ evidence, idPrefix = "evidence" }) {
  const coverage = evidence.pendingScenarioCount
    ? `${evidence.scenarioCount} measured · ${evidence.pendingScenarioCount} pending`
    : `${evidence.scenarioCount} measured scenarios`;

  return (
    <section className="evidence-strip" aria-labelledby={`${idPrefix}-evidence-status-title`}>
      <div className="evidence-strip__lead">
        <p id={`${idPrefix}-evidence-status-title`}>Run details</p>
        <strong>{evidence.status}</strong>
      </div>

      <dl>
        <div>
          <dt>Tasks</dt>
          <dd>{coverage}</dd>
        </div>
        <div>
          <dt>Attempts</dt>
          <dd>
            {evidence.attemptedTaskRuns
              ? `${evidence.attemptedTaskRuns} attempts · ${evidence.taskRuns} passed`
              : `${evidence.taskRuns} passing runs`}
          </dd>
        </div>
        <div>
          <dt>Failed attempts</dt>
          <dd>{failureText(evidence)}</dd>
        </div>
        <div>
          <dt>Quality</dt>
          <dd>Weighted task checks</dd>
        </div>
      </dl>
    </section>
  );
}
