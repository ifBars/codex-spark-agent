const runners = [
  { id: "spark", label: "Spark", color: "#1769d2" },
  { id: "codex", label: "Codex CLI", color: "#e34a18" },
];
const reasoningLevels = ["low", "medium", "high"];

export function FrontierPendingChart({
  view,
  enabledRunners,
  enabledReasoning,
}) {
  const configurations = runners.flatMap((runner) =>
    reasoningLevels
      .filter((reasoning) =>
        enabledRunners.has(runner.id) && enabledReasoning.has(reasoning))
      .map((reasoning) => ({ ...runner, reasoning })));
  const plot = { left: 82, top: 24, right: 956, bottom: 270 };
  const y = (score) =>
    plot.bottom - (score / 100) * (plot.bottom - plot.top);
  const configurationLeft = plot.left + 32;
  const configurationRight = plot.right - 32;
  const x = (index) =>
    configurations.length === 1
      ? (configurationLeft + configurationRight) / 2
      : configurationLeft
        + (index / Math.max(1, configurations.length - 1))
          * (configurationRight - configurationLeft);

  return (
    <div className="chart-region frontier-pending">
      <div className="chart-region__heading">
        <div>
          <h2>{view.label}</h2>
          <p>{view.description}</p>
          <small>
            {view.sample} · failed and provider-limited runs are not plotted
          </small>
        </div>
        <span className="frontier-pending__status">Calibration pending</span>
      </div>

      <div className="frontier-pending__canvas">
        <svg
          viewBox="0 0 1040 350"
          role="img"
          aria-labelledby="frontier-chart-title frontier-chart-description"
        >
          <title id="frontier-chart-title">Frontier suite calibration chart</title>
          <desc id="frontier-chart-description">
            {configurations.length} visible runner and reasoning configurations
            are awaiting valid benchmark results. The intended difficulty band
            ends at 65 percent. No scores are plotted.
          </desc>

          <rect
            className="frontier-pending__band"
            x={plot.left}
            y={y(view.targetCeiling)}
            width={plot.right - plot.left}
            height={plot.bottom - y(view.targetCeiling)}
          />

          {[0, 25, 50, 65, 75, 100].map((tick) => (
            <g className="frontier-pending__grid" key={tick}>
              <line
                x1={plot.left}
                x2={plot.right}
                y1={y(tick)}
                y2={y(tick)}
                data-ceiling={tick === view.targetCeiling}
              />
              <text x={plot.left - 16} y={y(tick) + 4} textAnchor="end">
                {tick}%
              </text>
            </g>
          ))}

          <text
            className="frontier-pending__band-label"
            x={plot.left + 12}
            y={y(view.targetCeiling) + 20}
          >
            Intended difficulty band · ≤ {view.targetCeiling}%
          </text>

          {configurations.map((configuration, index) => (
            <g
              className="frontier-pending__configuration"
              key={`${configuration.id}-${configuration.reasoning}`}
            >
              <line
                x1={x(index)}
                x2={x(index)}
                y1={plot.top}
                y2={plot.bottom}
                style={{ "--runner-color": configuration.color }}
              />
              <rect
                x={x(index) - 22}
                y={plot.bottom - 6}
                width="44"
                height="12"
                style={{ "--runner-color": configuration.color }}
              />
              <text x={x(index)} y={plot.bottom + 31} textAnchor="middle">
                {configuration.label}
              </text>
              <text
                className="frontier-pending__reasoning"
                x={x(index)}
                y={plot.bottom + 49}
                textAnchor="middle"
              >
                {configuration.reasoning}
              </text>
            </g>
          ))}

          <g className="frontier-pending__empty-state">
            <rect x="350" y="93" width="340" height="84" />
            <text x="520" y="124" textAnchor="middle">No valid score yet</text>
            <text x="520" y="148" textAnchor="middle">
              Quota failures are excluded from this chart
            </text>
          </g>
        </svg>
      </div>

      <div className="frontier-pending__tasks">
        <span>Validated task fixtures</span>
        {view.scenarioDetails.map((scenario) => (
          <a key={scenario.id} href={scenario.url} target="_blank" rel="noreferrer">
            <strong>{scenario.label}</strong>
            <small>{scenario.validationSignals} scoring signals</small>
          </a>
        ))}
      </div>
    </div>
  );
}
