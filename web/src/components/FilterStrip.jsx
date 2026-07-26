import { Check, SlidersHorizontal } from "@phosphor-icons/react";
import {
  datasets,
  reasoningOptions,
  runnerOptions,
  xMetrics,
  yMetrics,
} from "../data/benchmarks.js";
import { sentenceCase } from "../lib/format.js";

function Toggle({ active, color, children, onClick }) {
  return (
    <button
      type="button"
      className="toggle"
      data-active={active}
      onClick={onClick}
      aria-pressed={active}
      style={active && color ? { "--toggle-accent": color } : undefined}
    >
      {active && <Check aria-hidden="true" weight="bold" />}
      <span>{children}</span>
    </button>
  );
}

export function FilterStrip({
  datasetId,
  onDatasetChange,
  xMetric,
  onXMetricChange,
  yMetric,
  onYMetricChange,
  enabledRunners,
  onToggleRunner,
  enabledReasoning,
  onToggleReasoning,
  showRanges,
  onShowRangesChange,
}) {
  return (
    <section className="filter-strip" aria-label="Chart controls">
      <div className="filter-strip__heading">
        <SlidersHorizontal aria-hidden="true" />
        <span>View controls</span>
      </div>

      <label className="field-control">
        <span>Dataset</span>
        <select value={datasetId} onChange={(event) => onDatasetChange(event.target.value)}>
          {datasets.map((dataset) => (
            <option value={dataset.id} key={dataset.id}>
              {dataset.label}
            </option>
          ))}
        </select>
      </label>

      <label className="field-control">
        <span>X axis</span>
        <select value={xMetric} onChange={(event) => onXMetricChange(event.target.value)}>
          {Object.entries(xMetrics).map(([id, metric]) => (
            <option value={id} key={id}>
              {metric.label}
            </option>
          ))}
        </select>
      </label>

      <label className="field-control">
        <span>Y axis</span>
        <select value={yMetric} onChange={(event) => onYMetricChange(event.target.value)}>
          {Object.entries(yMetrics).map(([id, metric]) => (
            <option value={id} key={id}>
              {metric.label}
            </option>
          ))}
        </select>
      </label>

      <fieldset className="toggle-field">
        <legend>Runners</legend>
        <div className="toggle-row">
          {runnerOptions.map((runner) => (
            <Toggle
              active={enabledRunners.has(runner.id)}
              color={runner.color}
              key={runner.id}
              onClick={() => onToggleRunner(runner.id)}
            >
              {runner.shortName}
            </Toggle>
          ))}
        </div>
      </fieldset>

      <fieldset className="toggle-field">
        <legend>Reasoning</legend>
        <div className="toggle-row">
          {reasoningOptions.map((reasoning) => (
            <Toggle
              active={enabledReasoning.has(reasoning)}
              key={reasoning}
              onClick={() => onToggleReasoning(reasoning)}
            >
              {sentenceCase(reasoning)}
            </Toggle>
          ))}
        </div>
      </fieldset>

      <label className="range-control">
        <input
          type="checkbox"
          checked={showRanges}
          onChange={(event) => onShowRangesChange(event.target.checked)}
        />
        <span>Show ranges</span>
      </label>
    </section>
  );
}
