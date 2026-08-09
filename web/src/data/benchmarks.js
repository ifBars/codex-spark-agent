import reasoningSweepData from "./reasoning-sweep.json";

const repositoryBlobRoot = "https://github.com/ifBars/codex-spark-agent/blob/main";
const dataset = reasoningSweepData.dataset;

const runnerMeta = {
  spark: { id: "spark", name: "Spark harness", shortName: "Spark", color: "#1769d2" },
  codex: { id: "codex", name: "Codex CLI", shortName: "Codex CLI", color: "#e34a18" },
};

const taskTypes = [
  {
    id: "code-changes",
    label: "Code changes",
    description: "Implementation and repair tasks with behavioral validation.",
    scenarios: [
      "config-migration",
      "multi-module-bugfix",
      "stateful-reconciliation-bugfix",
      "feature-rollout-consistency-bugfix",
    ],
  },
  {
    id: "quantitative-analysis",
    label: "Quantitative analysis",
    description: "Tasks that require exact calculations from several inputs.",
    scenarios: [
      "ops-report",
      "multi-hop-analysis",
      "inventory-rebalance-plan",
      "experiment-rollout-audit",
    ],
  },
  {
    id: "investigation",
    label: "Investigation",
    description: "Evidence gathering, cross-file reasoning, and grounded conclusions.",
    scenarios: [
      "technical-essay",
      "multi-hop-analysis",
      "policy-support-agent",
      "experiment-rollout-audit",
    ],
  },
  {
    id: "terminal-operations",
    label: "Terminal and operations",
    description: "Repair and operational reporting through a working project environment.",
    scenarios: ["terminal-repair", "ops-report", "inventory-rebalance-plan"],
  },
  {
    id: "writing-configuration",
    label: "Writing and configuration",
    description: "Structured writing and coordinated configuration changes.",
    scenarios: ["technical-essay", "config-migration", "policy-support-agent"],
  },
  {
    id: "stateful-policy",
    label: "Stateful systems and policy",
    description: "Tasks where ordering, isolation, or policy constraints determine correctness.",
    scenarios: [
      "stateful-reconciliation-bugfix",
      "feature-rollout-consistency-bugfix",
      "frontier-rule-transfer",
      "policy-support-agent",
    ],
  },
];

const frontier = {
  id: "frontier",
  label: "Frontier",
  description: "Harder transfer and consistency tasks, shown separately from task-type averages.",
  scenarios: ["feature-rollout-consistency-bugfix", "frontier-rule-transfer"],
  wide: true,
};

function artifactUrl(path) {
  return `${repositoryBlobRoot}/${path}`;
}

function mean(values) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function round(value, digits = 2) {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

function interval(values, bounds = null) {
  if (values.length === 0) return [null, null];
  const average = mean(values);
  if (values.length === 1) return [average, average];
  const variance = values.reduce((sum, value) => sum + ((value - average) ** 2), 0)
    / (values.length - 1);
  const margin = 1.96 * Math.sqrt(variance / values.length);
  const minimum = bounds ? Math.max(bounds[0], average - margin) : average - margin;
  const maximum = bounds ? Math.min(bounds[1], average + margin) : average + margin;
  return [round(minimum), round(maximum)];
}

function aggregateRows(scenarios) {
  return ["spark", "codex"].flatMap((runner) =>
    ["low", "medium", "high"].map((reasoning) => {
      const attempts = dataset.attempts.filter(
        (row) => row.runner === runner
          && row.reasoning === reasoning
          && scenarios.includes(row.scenario),
      );
      if (attempts.length !== scenarios.length) {
        throw new Error(`Incomplete ${runner}/${reasoning} data for ${scenarios.join(", ")}`);
      }
      const scored = attempts.filter((row) => Number.isFinite(row.quality));
      if (scored.length === 0) {
        throw new Error(`No validated outcomes for ${runner}/${reasoning}`);
      }
      const passed = attempts.reduce((sum, row) => sum + row.passed, 0);
      const totalAttempts = attempts.reduce((sum, row) => sum + row.attempts, 0);
      const quality = mean(scored.map((row) => row.quality));
      const tokens = mean(scored.map((row) => row.tokens));
      const duration = mean(scored.map((row) => row.duration));
      const [qualityMin, qualityMax] = interval(scored.map((row) => row.quality), [0, 100]);
      const [tokensMin, tokensMax] = interval(scored.map((row) => row.tokens), [0, Infinity]);
      const [durationMin, durationMax] = interval(scored.map((row) => row.duration), [0, Infinity]);
      return {
        runner,
        runnerName: dataset.runnerNames[runner] ?? runnerMeta[runner].name,
        runnerShortName: runnerMeta[runner].shortName,
        color: runnerMeta[runner].color,
        reasoning,
        runs: passed,
        successfulRuns: passed,
        excludedRuns: totalAttempts - passed,
        quality: round(quality),
        qualityMin,
        qualityMax,
        commonScenarioQuality: dataset.rows.find(
          (row) => row.runner === runner && row.reasoning === reasoning,
        )?.commonScenarioQuality ?? null,
        completion: round(mean(scored.map((row) => row.completion))),
        process: round(mean(scored.map((row) => row.process))),
        tokens: Math.round(tokens),
        tokensMin: Math.round(tokensMin),
        tokensMax: Math.round(tokensMax),
        duration: round(duration),
        durationMin,
        durationMax,
        successRate: round((passed / totalAttempts) * 100),
        attemptPassRate: round((passed / totalAttempts) * 100),
        passRate: round((passed / totalAttempts) * 100),
      };
    }));
}

function makeView(definition) {
  return {
    ...definition,
    scenarioCount: definition.scenarios.length,
    hasIntervals: true,
    rows: aggregateRows(definition.scenarios),
  };
}

const overall = makeView({
  id: "overall",
  label: "Overall",
  description: "Scenario-balanced results across the complete corrected sweep.",
  scenarios: dataset.attempts
    .map((row) => row.scenario)
    .filter((scenario, index, scenarios) => scenarios.indexOf(scenario) === index),
});

const views = [overall, ...taskTypes.map(makeView), makeView(frontier)];
const source = artifactUrl(dataset.sourcePath);
const evidence = {
  status: "Paired reasoning sweep",
  date: dataset.date,
  scenarioCount: dataset.scenarioCount,
  taskRuns: dataset.validatedAttempts,
  attemptedTaskRuns: dataset.totalAttempts,
  taskFailureExclusions: dataset.failedAttempts,
  taskFailuresRetained: true,
  providerExclusions: dataset.providerExclusions,
  pendingScenarioCount: 0,
  pendingScenarios: [],
  note: "Outcome quality includes weighted validator scores from every non-infrastructure attempt. Tool-path compliance remains a separate process score.",
  sources: [{ label: "Attempt outcomes", path: dataset.sourcePath, url: source }],
};

export const benchmarkCohorts = [{
  id: dataset.id,
  label: "Current reasoning sweep",
  shortLabel: "Current",
  date: dataset.date,
  sample: `${dataset.totalAttempts} paired attempts`,
  rangeKind: "95% interval across scenario means",
  source,
  description: `${dataset.scenarioCount} tasks at low, medium, and high reasoning, repeated ${dataset.expectedRepeats} times for each runner.`,
  evidence,
  views,
  scenarioViews: [],
  rows: overall.rows,
  attempts: dataset.attempts,
  commonScenarioIds: dataset.commonScenarioIds,
  expectedRepeats: dataset.expectedRepeats,
}];

export const frontierComparison = null;

export const datasets = [{
  id: "spark-bench",
  label: "Spark Bench",
  description: "Measured outcome quality, reliability, cost, and process behavior.",
  cohorts: benchmarkCohorts,
}];

export const runnerOptions = Object.values(runnerMeta);
export const reasoningOptions = ["low", "medium", "high"];

export function coverageLabel(scenarioCount) {
  if (scenarioCount === null) return "Historical";
  if (scenarioCount <= 1) return "Pilot coverage";
  if (scenarioCount <= 2) return "Early coverage";
  if (scenarioCount <= 4) return "Developing coverage";
  return "Broad coverage";
}

export const xMetrics = {
  tokens: {
    key: "tokens", minKey: "tokensMin", maxKey: "tokensMax",
    label: "Average total API tokens", shortLabel: "Tokens",
  },
  duration: {
    key: "duration", minKey: "durationMin", maxKey: "durationMax",
    label: "Average task duration", shortLabel: "Time",
  },
};

export const yMetrics = {
  quality: {
    key: "quality", minKey: "qualityMin", maxKey: "qualityMax",
    label: "Validated outcome quality", shortLabel: "Quality",
  },
  successRate: {
    key: "successRate", minKey: null, maxKey: null,
    label: "Task pass rate", shortLabel: "Pass rate",
  },
  attemptPassRate: {
    key: "attemptPassRate", minKey: null, maxKey: null,
    label: "Task pass rate", shortLabel: "Pass rate",
  },
};
