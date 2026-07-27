import { useId } from "react";
import { xMetrics, yMetrics } from "../data/benchmarks.js";
import { chartDomain, formatMetric, metricRange, sentenceCase } from "../lib/format.js";

const WIDTH = 1000;
const HEIGHT = 520;
const FRAME = { left: 78, right: 34, top: 34, bottom: 70 };
const reasoningOrder = { low: 0, medium: 1, high: 2 };
const labelOffsets = {
  spark: {
    low: { dx: -12, dy: -10, anchor: "end" },
    medium: { dx: 12, dy: -28, anchor: "start" },
    high: { dx: 12, dy: -10, anchor: "start" },
  },
  codex: {
    low: { dx: 12, dy: -18, anchor: "start" },
    medium: { dx: -12, dy: 18, anchor: "end" },
    high: { dx: 12, dy: -6, anchor: "start" },
  },
};

function ticks(min, max, count = 5) {
  return Array.from({ length: count + 1 }, (_, index) => min + ((max - min) * index) / count);
}

function scale(value, domain, range) {
  const [d0, d1] = domain;
  const [r0, r1] = range;
  return r0 + ((value - d0) / (d1 - d0 || 1)) * (r1 - r0);
}

function clamp(value, [min, max]) {
  return Math.min(max, Math.max(min, value));
}

export function CostQualityChart({
  rows,
  xMetric,
  yMetric,
  showRanges,
  selectedPoint,
  onSelectPoint,
  rangeKind,
  contextLabel,
  description,
  meta,
  compact = false,
  wide = false,
  showInspector = true,
}) {
  const titleId = useId();
  const condensed = compact || wide;
  const height = condensed ? 360 : HEIGHT;
  const frame = condensed
    ? { left: 70, right: 28, top: 28, bottom: 62 }
    : FRAME;
  const plotWidth = WIDTH - frame.left - frame.right;
  const plotHeight = height - frame.top - frame.bottom;
  const xDomain = chartDomain(rows, xMetric, xMetrics);
  const yDomain = chartDomain(rows, yMetric, yMetrics, { bounds: [0, 100] });
  const x = (value) => scale(value, xDomain, [frame.left, WIDTH - frame.right]);
  const y = (value) => scale(value, yDomain, [height - frame.bottom, frame.top]);
  const rangeX = (value) => x(clamp(value, xDomain));
  const rangeY = (value) => y(clamp(value, yDomain));
  const xTicks = ticks(...xDomain);
  const yTicks = ticks(...yDomain);

  const grouped = ["spark", "codex"]
    .map((runner) => rows.filter((row) => row.runner === runner).sort((a, b) => reasoningOrder[a.reasoning] - reasoningOrder[b.reasoning]))
    .filter((group) => group.length > 0);

  return (
    <div className={`chart-region${compact ? " chart-region--compact" : ""}`}>
      <div className="chart-region__heading">
        <div>
          <h2>{contextLabel} — {yMetrics[yMetric].label} vs. {xMetrics[xMetric].label}</h2>
          <p>
            {description ?? "Lower is better horizontally; higher is better vertically."}
          </p>
          {meta && <small>{meta} · {rangeKind} · Axes fit estimates; long ranges clip at bounds</small>}
        </div>
        <div className="chart-legend" aria-label="Runner legend">
          {grouped.map((group) => (
            <span key={group[0].runner}>
              <i style={{ background: group[0].color }} />
              {group[0].runnerShortName}
            </span>
          ))}
          <span>
            <i className="ideal-zone-key" />
            Ideal zone
          </span>
        </div>
      </div>

      <div className="chart-canvas">
        <svg
          className="benchmark-chart"
          viewBox={`0 0 ${WIDTH} ${height}`}
          role="img"
          aria-labelledby={titleId}
        >
          <title id={titleId}>
            {contextLabel}: {yMetrics[yMetric].label} versus {xMetrics[xMetric].label} for visible benchmark points. The upper-left quadrant marks the ideal zone.
          </title>

          <g className="ideal-zone" aria-hidden="true">
            <rect
              x={frame.left}
              y={frame.top}
              width={plotWidth / 2}
              height={plotHeight / 2}
            />
            <text x={frame.left + 14} y={frame.top + 22}>
              Ideal zone
            </text>
          </g>

          <g className="chart-grid">
            {yTicks.map((tick) => (
              <line key={`y-${tick}`} x1={frame.left} x2={WIDTH - frame.right} y1={y(tick)} y2={y(tick)} />
            ))}
            {xTicks.map((tick) => (
              <line key={`x-${tick}`} x1={x(tick)} x2={x(tick)} y1={frame.top} y2={height - frame.bottom} />
            ))}
          </g>

          <g className="chart-axes">
            <line x1={frame.left} x2={frame.left} y1={frame.top} y2={height - frame.bottom} />
            <line x1={frame.left} x2={WIDTH - frame.right} y1={height - frame.bottom} y2={height - frame.bottom} />
          </g>

          <g className="chart-labels">
            {yTicks.map((tick) => (
              <text key={`yl-${tick}`} x={frame.left - 16} y={y(tick) + 4} textAnchor="end">
                {formatMetric(yMetric, tick, true)}
              </text>
            ))}
            {xTicks.map((tick) => (
              <text key={`xl-${tick}`} x={x(tick)} y={height - frame.bottom + 28} textAnchor="middle">
                {formatMetric(xMetric, tick, true)}
              </text>
            ))}
            <text className="axis-title" x={(frame.left + WIDTH - frame.right) / 2} y={height - 16} textAnchor="middle">
              {xMetrics[xMetric].label}
            </text>
            <text
              className="axis-title"
              x={18}
              y={(frame.top + height - frame.bottom) / 2}
              textAnchor="middle"
              transform={`rotate(-90 18 ${(frame.top + height - frame.bottom) / 2})`}
            >
              {yMetrics[yMetric].label}
            </text>
          </g>

          {grouped.map((group) => (
            <polyline
              className="runner-line"
              key={`line-${group[0].runner}`}
              points={group.map((row) => `${x(row[xMetrics[xMetric].key])},${y(row[yMetrics[yMetric].key])}`).join(" ")}
              style={{ stroke: group[0].color }}
            />
          ))}

          {rows.map((row) => {
            const xRange = metricRange(row, xMetric, xMetrics);
            const yRange = metricRange(row, yMetric, yMetrics);
            const active =
              selectedPoint?.runner === row.runner && selectedPoint?.reasoning === row.reasoning;
            const cx = x(row[xMetrics[xMetric].key]);
            const cy = y(row[yMetrics[yMetric].key]);
            const labelToLeft = cx > WIDTH - 190;
            const preferredLabel = labelOffsets[row.runner][row.reasoning];
            const labelX = labelToLeft ? cx - 14 : cx + preferredLabel.dx;
            const labelAnchor = labelToLeft ? "end" : preferredLabel.anchor;

            return (
              <g className="chart-point" key={`${row.runner}-${row.reasoning}`}>
                {showRanges && xRange && (
                  <g className="range-mark" style={{ stroke: row.color }}>
                    <line x1={rangeX(xRange[0])} x2={rangeX(xRange[1])} y1={cy} y2={cy} />
                    <line x1={rangeX(xRange[0])} x2={rangeX(xRange[0])} y1={cy - 6} y2={cy + 6} />
                    <line x1={rangeX(xRange[1])} x2={rangeX(xRange[1])} y1={cy - 6} y2={cy + 6} />
                  </g>
                )}
                {showRanges && yRange && (
                  <g className="range-mark" style={{ stroke: row.color }}>
                    <line x1={cx} x2={cx} y1={rangeY(yRange[0])} y2={rangeY(yRange[1])} />
                    <line x1={cx - 6} x2={cx + 6} y1={rangeY(yRange[0])} y2={rangeY(yRange[0])} />
                    <line x1={cx - 6} x2={cx + 6} y1={rangeY(yRange[1])} y2={rangeY(yRange[1])} />
                  </g>
                )}
                <circle
                  className="point-target"
                  cx={cx}
                  cy={cy}
                  r={active ? 10 : 8}
                  fill={row.color}
                  stroke={active ? "#171715" : "#fffaf2"}
                  strokeWidth={active ? 4 : 3}
                  tabIndex="0"
                  role="button"
                  aria-label={`${row.runnerName}, ${row.reasoning} reasoning: ${formatMetric(yMetric, row[yMetrics[yMetric].key])} ${yMetrics[yMetric].label}, ${formatMetric(xMetric, row[xMetrics[xMetric].key])} ${xMetrics[xMetric].label}`}
                  onClick={() => onSelectPoint(row)}
                  onFocus={() => onSelectPoint(row)}
                  onMouseEnter={() => onSelectPoint(row)}
                />
                <text
                  className="point-label"
                  x={labelX}
                  y={cy + preferredLabel.dy}
                  textAnchor={labelAnchor}
                >
                  {sentenceCase(row.reasoning)}
                </text>
              </g>
            );
          })}
        </svg>

        {showInspector && <aside className="point-inspector" aria-live="polite">
          {selectedPoint ? (
            <>
              <div className="point-inspector__runner">
                <i style={{ background: selectedPoint.color }} />
                {selectedPoint.runnerShortName} — {sentenceCase(selectedPoint.reasoning)}
              </div>
              <dl>
                <div>
                  <dt>{yMetrics[yMetric].shortLabel}</dt>
                  <dd>{formatMetric(yMetric, selectedPoint[yMetrics[yMetric].key])}</dd>
                </div>
                <div>
                  <dt>{xMetrics[xMetric].shortLabel}</dt>
                  <dd>{formatMetric(xMetric, selectedPoint[xMetrics[xMetric].key])}</dd>
                </div>
                <div>
                  <dt>Runs</dt>
                  <dd>{selectedPoint.runs}</dd>
                </div>
              </dl>
              {showRanges && (
                <p>{rangeKind} is shown directly on the chart.</p>
              )}
            </>
          ) : (
            <p>Hover or focus a point to inspect it.</p>
          )}
        </aside>}
      </div>
    </div>
  );
}
