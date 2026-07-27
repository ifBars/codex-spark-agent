import benchmarkEvidenceData from "./benchmark-evidence.json";
import expandedReasoningData from "./expanded-reasoning-views.json";

const repositoryBlobRoot = "https://github.com/ifBars/codex-spark-agent/blob/main";

const runnerMeta = {
  spark: { id: "spark", name: "Spark harness", shortName: "Spark", color: "#1769d2" },
  codex: { id: "codex", name: "Codex CLI 0.145.0", shortName: "Codex CLI", color: "#e34a18" },
};

function artifactUrl(path) {
  return `${repositoryBlobRoot}/${path}`;
}

function evidenceFor(datasetId) {
  const evidence = benchmarkEvidenceData.datasets.find((dataset) => dataset.id === datasetId);
  if (!evidence) throw new Error(`Missing benchmark evidence for ${datasetId}`);
  return {
    ...evidence,
    pendingScenarios: evidence.pendingScenarios.map((scenario) => ({
      ...scenario,
      url: artifactUrl(scenario.evidencePath),
    })),
    sources: evidence.sources.map((source) => ({
      ...source,
      url: artifactUrl(source.path),
    })),
  };
}

function point(runner, reasoning, values) {
  return {
    runner,
    runnerName: runnerMeta[runner].name,
    runnerShortName: runnerMeta[runner].shortName,
    color: runnerMeta[runner].color,
    reasoning,
    ...values,
  };
}

const measuredExpandedViews = expandedReasoningData.views.map((view) => ({
  ...view,
  rows: view.rows.map(({ runner, reasoning, ...values }) =>
    point(runner, reasoning, values),
  ),
}));
const expandedScenarioViews = expandedReasoningData.scenarioViews.map((view) => ({
  ...view,
  rows: view.rows.map(({ runner, reasoning, ...values }) =>
    point(runner, reasoning, values),
  ),
}));

const expandedEvidence = evidenceFor("expanded-reasoning-suite");
const baselineEvidence = evidenceFor("success-baseline");
const frontierScenarios = expandedEvidence.pendingScenarios.filter((scenario) =>
  scenario.categories.includes("frontier"));
const frontierView = {
  id: "frontier",
  label: "Frontier",
  status: "pending",
  wide: true,
  description:
    "Two deliberately out-of-reach transfer and consistency tasks are ready, but quota-limited attempts are excluded until a balanced successful matrix can be measured.",
  scenarioCount: frontierScenarios.length,
  scenarios: frontierScenarios.map((scenario) => scenario.id),
  scenarioDetails: frontierScenarios,
  targetCeiling: 65,
  runCount: null,
  sample: "2 frontier tasks · calibration pending",
  rows: [],
};
const expandedViews = [...measuredExpandedViews, frontierView];

const datasetDefinitions = [
  {
    id: "expanded-reasoning-suite",
    label: "Expanded reasoning suite",
    shortLabel: "Expanded",
    date: expandedEvidence.date,
    sample: `${expandedEvidence.taskRuns} successful task runs`,
    rangeKind: "95% CI across scenario means",
    source: expandedEvidence.sources[0].url,
    description:
      `A ${expandedEvidence.taskRuns}-run successful-only matrix with scenario-balanced averages; ${expandedEvidence.taskFailureExclusions} failed task attempts and all provider/API failures are excluded.`,
    evidence: expandedEvidence,
    views: expandedViews,
    scenarioViews: expandedScenarioViews,
    rows: expandedViews[0].rows,
  },
  {
    id: "success-baseline",
    label: "Eight-task baseline",
    shortLabel: "Baseline",
    date: baselineEvidence.date,
    sample: `${baselineEvidence.taskRuns} successful task runs`,
    rangeKind: "95% confidence interval",
    source: baselineEvidence.sources[0].url,
    description:
      "The earlier paired matrix reports successful-row means across eight real-world tasks. It is useful for comparison, but its success-only quality scores are visibly saturated.",
    evidence: baselineEvidence,
    rows: [
      point("spark", "low", {
        quality: 99.04, qualityMin: 97.67, qualityMax: 100,
        tokens: 80538, tokensMin: 71365, tokensMax: 89711,
        duration: 11, durationMin: 9.3, durationMax: 12.7,
        successRate: 100, runs: 23, excludedRuns: 1,
      }),
      point("spark", "medium", {
        quality: 99.08, qualityMin: 97.77, qualityMax: 100,
        tokens: 85762, tokensMin: 73374, tokensMax: 98150,
        duration: 12.2, durationMin: 9.7, durationMax: 14.7,
        successRate: 100, runs: 24, excludedRuns: 0,
      }),
      point("spark", "high", {
        quality: 99.08, qualityMin: 97.77, qualityMax: 100,
        tokens: 86065, tokensMin: 74150, tokensMax: 97980,
        duration: 12.5, durationMin: 10.5, durationMax: 14.5,
        successRate: 100, runs: 24, excludedRuns: 0,
      }),
      point("codex", "low", {
        quality: 94.17, qualityMin: 91.31, qualityMax: 97.03,
        tokens: 146390, tokensMin: 127094, tokensMax: 165686,
        duration: 25.3, durationMin: 20.3, durationMax: 30.3,
        successRate: 100, runs: 23, excludedRuns: 1,
      }),
      point("codex", "medium", {
        quality: 97.22, qualityMin: 95.03, qualityMax: 99.41,
        tokens: 149439, tokensMin: 109441, tokensMax: 189437,
        duration: 27.3, durationMin: 21.7, durationMax: 32.9,
        successRate: 100, runs: 23, excludedRuns: 1,
      }),
      point("codex", "high", {
        quality: 97.83, qualityMin: 95.72, qualityMax: 99.94,
        tokens: 135090, tokensMin: 119621, tokensMax: 150559,
        duration: 21.3, durationMin: 19.1, durationMax: 23.5,
        successRate: 100, runs: 23, excludedRuns: 1,
      }),
    ],
  },
];

export const datasets = datasetDefinitions.map((dataset) => ({
  ...dataset,
  views:
    dataset.views ??
    [{
      id: "overall",
      label: "Overall",
      description: dataset.description,
      sample: dataset.sample,
      scenarioCount: dataset.evidence.scenarioCount,
      runCount: dataset.rows[0]?.runs ?? null,
      rows: dataset.rows,
    }],
}));

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
    label: "Total API tokens", shortLabel: "Tokens",
  },
  duration: {
    key: "duration", minKey: "durationMin", maxKey: "durationMax",
    label: "Duration", shortLabel: "Time",
  },
};

export const yMetrics = {
  quality: {
    key: "quality", minKey: "qualityMin", maxKey: "qualityMax",
    label: "Weighted quality", shortLabel: "Quality",
  },
  successRate: {
    key: "successRate", minKey: null, maxKey: null,
    label: "Full-pass rate", shortLabel: "Pass rate",
  },
};
