import { useId } from "react";
import { xMetrics, yMetrics } from "../data/benchmarks.js";
import { chartDomain, formatMetric, metricRange, sentenceCase } from "../lib/format.js";

const WIDTH = 1000;
const HEIGHT = 520;
const FRAME = { left: 78, right: 34, top: 34, bottom: 70 };
const reasoningOrder = { low: 0, medium: 1, high: 2 };
const plotWidth = WIDTH - FRAME.left - FRAME.right;
const plotHeight = HEIGHT - FRAME.top - FRAME.bottom;

function ticks(min, max, count = 5) {
  return Array.from({ length: count + 1 }, (_, index) => min + ((max - min) * index) / count);
}

function scale(value, domain, range) {
  const [d0, d1] = domain;
  const [r0, r1] = range;
  return r0 + ((value - d0) / (d1 - d0 || 1)) * (r1 - r0);
}

export function CostQualityChart({
  rows,
  xMetric,
  yMetric,
  showRanges,
  selectedPoint,
  onSelectPoint,
  rangeKind,
}) {
  const titleId = useId();
  const xDomain = chartDomain(rows, xMetric, xMetrics);
  const yDomain = chartDomain(rows, yMetric, yMetrics, { fixedPercent: true });
  const x = (value) => scale(value, xDomain, [FRAME.left, WIDTH - FRAME.right]);
  const y = (value) => scale(value, yDomain, [HEIGHT - FRAME.bottom, FRAME.top]);
  const xTicks = ticks(...xDomain);
  const yTicks = ticks(...yDomain);

  const grouped = ["spark", "codex"]
    .map((runner) => rows.filter((row) => row.runner === runner).sort((a, b) => reasoningOrder[a.reasoning] - reasoningOrder[b.reasoning]))
    .filter((group) => group.length > 0);

  return (
    <div className="chart-region">
      <div className="chart-region__heading">
        <div>
          <h2>{yMetrics[yMetric].label} vs. {xMetrics[xMetric].label}</h2>
          <p>Lower is better horizontally; higher is better vertically. The upper-left is the ideal zone.</p>
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
          viewBox={`0 0 ${WIDTH} ${HEIGHT}`}
          role="img"
          aria-labelledby={titleId}
        >
          <title id={titleId}>
            {yMetrics[yMetric].label} versus {xMetrics[xMetric].label} for visible benchmark points. The upper-left quadrant marks the ideal zone.
          </title>

          <g className="ideal-zone" aria-hidden="true">
            <rect
              x={FRAME.left}
              y={FRAME.top}
              width={plotWidth / 2}
              height={plotHeight / 2}
            />
            <text x={FRAME.left + 14} y={FRAME.top + 22}>
              Ideal zone
            </text>
          </g>

          <g className="chart-grid">
            {yTicks.map((tick) => (
              <line key={`y-${tick}`} x1={FRAME.left} x2={WIDTH - FRAME.right} y1={y(tick)} y2={y(tick)} />
            ))}
            {xTicks.map((tick) => (
              <line key={`x-${tick}`} x1={x(tick)} x2={x(tick)} y1={FRAME.top} y2={HEIGHT - FRAME.bottom} />
            ))}
          </g>

          <g className="chart-axes">
            <line x1={FRAME.left} x2={FRAME.left} y1={FRAME.top} y2={HEIGHT - FRAME.bottom} />
            <line x1={FRAME.left} x2={WIDTH - FRAME.right} y1={HEIGHT - FRAME.bottom} y2={HEIGHT - FRAME.bottom} />
          </g>

          <g className="chart-labels">
            {yTicks.map((tick) => (
              <text key={`yl-${tick}`} x={FRAME.left - 16} y={y(tick) + 4} textAnchor="end">
                {formatMetric(yMetric, tick, true)}
              </text>
            ))}
            {xTicks.map((tick) => (
              <text key={`xl-${tick}`} x={x(tick)} y={HEIGHT - FRAME.bottom + 28} textAnchor="middle">
                {formatMetric(xMetric, tick, true)}
              </text>
            ))}
            <text className="axis-title" x={(FRAME.left + WIDTH - FRAME.right) / 2} y={HEIGHT - 16} textAnchor="middle">
              {xMetrics[xMetric].label}
            </text>
            <text
              className="axis-title"
              x={18}
              y={(FRAME.top + HEIGHT - FRAME.bottom) / 2}
              textAnchor="middle"
              transform={`rotate(-90 18 ${(FRAME.top + HEIGHT - FRAME.bottom) / 2})`}
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

            return (
              <g className="chart-point" key={`${row.runner}-${row.reasoning}`}>
                {showRanges && xRange && (
                  <g className="range-mark" style={{ stroke: row.color }}>
                    <line x1={x(xRange[0])} x2={x(xRange[1])} y1={cy} y2={cy} />
                    <line x1={x(xRange[0])} x2={x(xRange[0])} y1={cy - 6} y2={cy + 6} />
                    <line x1={x(xRange[1])} x2={x(xRange[1])} y1={cy - 6} y2={cy + 6} />
                  </g>
                )}
                {showRanges && yRange && (
                  <g className="range-mark" style={{ stroke: row.color }}>
                    <line x1={cx} x2={cx} y1={y(yRange[0])} y2={y(yRange[1])} />
                    <line x1={cx - 6} x2={cx + 6} y1={y(yRange[0])} y2={y(yRange[0])} />
                    <line x1={cx - 6} x2={cx + 6} y1={y(yRange[1])} y2={y(yRange[1])} />
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
                  x={labelToLeft ? cx - 14 : cx + 14}
                  y={cy - 13}
                  textAnchor={labelToLeft ? "end" : "start"}
                >
                  {sentenceCase(row.reasoning)}
                </text>
              </g>
            );
          })}
        </svg>

        <aside className="point-inspector" aria-live="polite">
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
        </aside>
      </div>
    </div>
  );
}
