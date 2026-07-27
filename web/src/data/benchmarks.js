import expandedReasoningData from "./expanded-reasoning-views.json";

const runnerMeta = {
  spark: { id: "spark", name: "Spark harness", shortName: "Spark", color: "#1769d2" },
  codex: { id: "codex", name: "Codex CLI 0.145.0", shortName: "Codex CLI", color: "#e34a18" },
};

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

const expandedViews = expandedReasoningData.views.map((view) => ({
  ...view,
  rows: view.rows.map(({ runner, reasoning, ...values }) =>
    point(runner, reasoning, values),
  ),
}));

const datasetDefinitions = [
  {
    id: "expanded-reasoning-suite",
    label: "Expanded reasoning suite",
    shortLabel: "Expanded",
    date: "July 26, 2026",
    sample: "9 difficult tasks × 3 runs per level",
    rangeKind: "95% CI across scenario means",
    source:
      "https://github.com/ifBars/codex-spark-agent/blob/main/docs/benchmarks/reasoning-cost-quality-categories-2026-07-26.csv",
    description:
      "A 162-run matrix with scenario-balanced averages, behavioral quality scoring, and zero provider/API failures.",
    views: expandedViews,
    rows: expandedViews[0].rows,
  },
  {
    id: "granular-pilot",
    label: "Granular pilot",
    shortLabel: "Pilot",
    date: "July 26, 2026",
    sample: "1 stateful task × 3 runs per level",
    rangeKind: "Observed min–max",
    source:
      "https://github.com/ifBars/codex-spark-agent/blob/main/docs/benchmarks/reasoning-cost-quality-pilot-2026-07-26.csv",
    description:
      "Weighted behavioral validation keeps incomplete task work visible instead of collapsing every failure to a blanket score.",
    rows: [
      point("spark", "low", {
        quality: 30, qualityMin: 0, qualityMax: 65,
        tokens: 92256, tokensMin: 72239, tokensMax: 110410,
        duration: 13.03, durationMin: 8.197, durationMax: 15.475,
        successRate: 0, runs: 3,
      }),
      point("spark", "medium", {
        quality: 65, qualityMin: 45, qualityMax: 85,
        tokens: 105836, tokensMin: 95940, tokensMax: 122852,
        duration: 15.92, durationMin: 13.348, durationMax: 19.282,
        successRate: 0, runs: 3,
      }),
      point("spark", "high", {
        quality: 83.33, qualityMin: 65, qualityMax: 100,
        tokens: 155585, tokensMin: 114098, tokensMax: 190849,
        duration: 21.15, durationMin: 16.324, durationMax: 27.87,
        successRate: 33.33, runs: 3,
      }),
      point("codex", "low", {
        quality: 63.33, qualityMin: 45, qualityMax: 80,
        tokens: 150039, tokensMin: 119485, tokensMax: 184990,
        duration: 24.11, durationMin: 18.589, durationMax: 27.74,
        successRate: 0, runs: 3,
      }),
      point("codex", "medium", {
        quality: 68.33, qualityMin: 45, qualityMax: 100,
        tokens: 150080, tokensMin: 118989, tokensMax: 205713,
        duration: 26.45, durationMin: 18.565, durationMax: 40.226,
        successRate: 33.33, runs: 3,
      }),
      point("codex", "high", {
        quality: 93.33, qualityMin: 80, qualityMax: 100,
        tokens: 200418, tokensMin: 124929, tokensMax: 282246,
        duration: 24.96, durationMin: 19.673, durationMax: 30.632,
        successRate: 66.67, runs: 3,
      }),
    ],
  },
  {
    id: "success-baseline",
    label: "Eight-task baseline",
    shortLabel: "Baseline",
    date: "July 26, 2026",
    sample: "8 tasks × 3 runs per level",
    rangeKind: "95% confidence interval",
    source:
      "https://github.com/ifBars/codex-spark-agent/blob/main/docs/benchmarks/reasoning-cost-quality-2026-07-26.csv",
    description:
      "The earlier paired matrix reports successful-row means across eight real-world tasks. It is useful for comparison, but its success-only quality scores are visibly saturated.",
    rows: [
      point("spark", "low", {
        quality: 99.04, qualityMin: 97.67, qualityMax: 100,
        tokens: 80538, tokensMin: 71365, tokensMax: 89711,
        duration: 11, durationMin: 9.3, durationMax: 12.7,
        successRate: 95.83, runs: 24,
      }),
      point("spark", "medium", {
        quality: 99.08, qualityMin: 97.77, qualityMax: 100,
        tokens: 85762, tokensMin: 73374, tokensMax: 98150,
        duration: 12.2, durationMin: 9.7, durationMax: 14.7,
        successRate: 100, runs: 24,
      }),
      point("spark", "high", {
        quality: 99.08, qualityMin: 97.77, qualityMax: 100,
        tokens: 86065, tokensMin: 74150, tokensMax: 97980,
        duration: 12.5, durationMin: 10.5, durationMax: 14.5,
        successRate: 100, runs: 24,
      }),
      point("codex", "low", {
        quality: 94.17, qualityMin: 91.31, qualityMax: 97.03,
        tokens: 146390, tokensMin: 127094, tokensMax: 165686,
        duration: 25.3, durationMin: 20.3, durationMax: 30.3,
        successRate: 95.83, runs: 24,
      }),
      point("codex", "medium", {
        quality: 97.22, qualityMin: 95.03, qualityMax: 99.41,
        tokens: 149439, tokensMin: 109441, tokensMax: 189437,
        duration: 27.3, durationMin: 21.7, durationMax: 32.9,
        successRate: 95.83, runs: 24,
      }),
      point("codex", "high", {
        quality: 97.83, qualityMin: 95.72, qualityMax: 99.94,
        tokens: 135090, tokensMin: 119621, tokensMax: 150559,
        duration: 21.3, durationMin: 19.1, durationMax: 23.5,
        successRate: 95.83, runs: 24,
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
      scenarioCount: null,
      rows: dataset.rows,
    }],
}));

export const runnerOptions = Object.values(runnerMeta);
export const reasoningOptions = ["low", "medium", "high"];

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
