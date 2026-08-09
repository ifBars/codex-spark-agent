const levels = ["low", "medium", "high"];
const runnerStyles = {
  spark: { color: "#1769d2", dash: undefined, label: "Spark" },
  codex: { color: "#e34a18", dash: "6 5", label: "Codex CLI" },
};

function formatValue(metric, value) {
  if (metric === "passRate") return `${value.toFixed(0)}%`;
  if (metric === "tokens") return `${(value / 1000).toFixed(value >= 100000 ? 0 : 1)}k`;
  return value.toFixed(1);
}

function chartDomain(rows, metric) {
  const values = rows.map((row) => row[metric]);
  const maximum = Math.max(...values);
  const minimum = Math.min(...values);
  if (metric === "passRate") return [0, 100];
  if (metric === "tokens") return [0, Math.ceil((maximum * 1.08) / 10000) * 10000];
  return [Math.max(0, Math.floor(minimum - 2)), Math.min(100, Math.ceil(maximum + 1))];
}

export function ReasoningLevelChart({ rows, metric, title, subtitle }) {
  const width = 420;
  const height = 250;
  const margin = { top: 24, right: 28, bottom: 42, left: 48 };
  const [minimum, maximum] = chartDomain(rows, metric);
  const innerWidth = width - margin.left - margin.right;
  const innerHeight = height - margin.top - margin.bottom;
  const x = (index) => margin.left + (innerWidth * index) / (levels.length - 1);
  const y = (value) => margin.top + ((maximum - value) / (maximum - minimum || 1)) * innerHeight;
  const ticks = Array.from({ length: 5 }, (_, index) =>
    minimum + ((maximum - minimum) * index) / 4);
  const aria = ["spark", "codex"].map((runner) => {
    const values = levels.map((level) => {
      const row = rows.find(
        (candidate) => candidate.runner === runner && candidate.reasoning === level,
      );
      return `${level} ${formatValue(metric, row[metric])}`;
    });
    return `${runnerStyles[runner].label}: ${values.join(", ")}`;
  }).join(". ");

  return (
    <article className="reasoning-level-chart" role="img" aria-label={`${title}. ${aria}`}>
      <header>
        <h4>{title}</h4>
        <p>{subtitle}</p>
      </header>
      <svg viewBox={`0 0 ${width} ${height}`} aria-hidden="true">
        {ticks.map((tick) => (
          <g key={tick}>
            <line
              x1={margin.left}
              x2={width - margin.right}
              y1={y(tick)}
              y2={y(tick)}
              className="reasoning-level-chart__grid"
            />
            <text x={margin.left - 10} y={y(tick) + 4} textAnchor="end">
              {formatValue(metric, tick)}
            </text>
          </g>
        ))}
        {levels.map((level, index) => (
          <text
            className="reasoning-level-chart__level"
            x={x(index)}
            y={height - 12}
            textAnchor="middle"
            key={level}
          >
            {level.charAt(0).toUpperCase() + level.slice(1)}
          </text>
        ))}
        {Object.entries(runnerStyles).map(([runner, style]) => {
          const runnerRows = levels.map((level) =>
            rows.find((row) => row.runner === runner && row.reasoning === level));
          const points = runnerRows
            .map((row, index) => `${x(index)},${y(row[metric])}`)
            .join(" ");
          return (
            <g key={runner}>
              <polyline
                points={points}
                fill="none"
                stroke={style.color}
                strokeDasharray={style.dash}
                className="reasoning-level-chart__line"
              />
              {runnerRows.map((row, index) => (
                <g key={`${runner}-${row.reasoning}`}>
                  <circle
                    cx={x(index)}
                    cy={y(row[metric])}
                    r="5"
                    fill={runner === "spark" ? style.color : "#fffaf2"}
                    stroke={style.color}
                    strokeWidth="2"
                  />
                  <text
                    className="reasoning-level-chart__value"
                    x={x(index)}
                    y={y(row[metric]) + (runner === "spark" ? -11 : 18)}
                    textAnchor="middle"
                    fill={style.color}
                  >
                    {formatValue(metric, row[metric])}
                  </text>
                </g>
              ))}
            </g>
          );
        })}
      </svg>
      {metric === "quality" ? <small>Focused scale: {minimum}–{maximum}</small> : null}
    </article>
  );
}
