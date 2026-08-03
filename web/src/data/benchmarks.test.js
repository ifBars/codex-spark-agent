import { describe, expect, it } from "vitest";
import benchmarkEvidenceData from "./benchmark-evidence.json";
import { benchmarkCohorts, coverageLabel, datasets } from "./benchmarks.js";

describe("expanded reasoning dataset", () => {
  const expanded = benchmarkCohorts[0];

  it("publishes one consolidated Spark Bench dataset", () => {
    expect(datasets).toHaveLength(1);
    expect(datasets[0].id).toBe("spark-bench");
    expect(datasets[0].cohorts).toBe(benchmarkCohorts);
    expect(benchmarkCohorts.map((cohort) => cohort.id)).toEqual([
      "expanded-reasoning-suite",
      "success-baseline",
      "real-world-quick-slice",
      "real-world-spark-extension",
    ]);
  });

  it("is the default dataset with all runner and reasoning combinations", () => {
    expect(expanded.id).toBe("expanded-reasoning-suite");
    expect(expanded.views.map((view) => view.id)).toEqual([
      "overall",
      "coding",
      "math-data",
      "analysis-research",
      "terminal-operations",
      "writing-configuration",
      "frontier",
    ]);
    expect(expanded.rows).toHaveLength(6);
    expect(expanded.rows.every((row) => row.runs > 0 && row.runs <= 27)).toBe(true);
    expect(expanded.rows.every((row) => row.successfulRuns === row.runs)).toBe(true);
    expect(expanded.rows.reduce((sum, row) => sum + row.runs, 0)).toBe(129);
    expect(new Set(expanded.rows.map((row) => row.runner))).toEqual(new Set(["spark", "codex"]));
    expect(new Set(expanded.rows.map((row) => row.reasoning))).toEqual(
      new Set(["low", "medium", "high"]),
    );
  });

  it("keeps every measured category successful-only and traceable", () => {
    for (const view of expanded.views.filter((candidate) => candidate.status !== "pending")) {
      expect(view.scenarioCount).toBe(view.scenarios.length);
      expect(view.rows).toHaveLength(6);
      expect(view.rows.every((row) => row.runs > 0)).toBe(true);
      expect(view.rows.every((row) => row.successfulRuns === row.runs)).toBe(true);
      expect(view.rows.every((row) => row.excludedRuns >= 0)).toBe(true);
      expect(new Set(view.rows.map((row) => row.runner))).toEqual(new Set(["spark", "codex"]));
      expect(new Set(view.rows.map((row) => row.reasoning))).toEqual(
        new Set(["low", "medium", "high"]),
      );
    }
  });

  it("publishes all measured scenarios as six-point drill-down views", () => {
    expect(expanded.scenarioViews).toHaveLength(9);
    expect(new Set(expanded.scenarioViews.map((view) => view.id))).toEqual(
      new Set(expanded.views[0].scenarios),
    );
    for (const scenario of expanded.scenarioViews) {
      expect(scenario.description.length).toBeGreaterThan(50);
      expect(scenario.runCount).toBeNull();
      expect(scenario.rows.length).toBeGreaterThan(0);
      expect(scenario.rows.length).toBeLessThanOrEqual(6);
      expect(scenario.rows.every((row) => row.runs > 0 && row.runs <= 3)).toBe(true);
      expect(scenario.rows.every((row) => row.successfulRuns === row.runs)).toBe(true);
      expect(scenario.rows.every((row) => row.qualityMin === undefined)).toBe(true);
      expect(
        scenario.rows.every(
          (row) =>
            row.successRate
            === Number(((row.successfulRuns / row.runs) * 100).toFixed(2)),
        ),
      ).toBe(true);
      expect(new Set(scenario.rows.map((row) => `${row.runner}/${row.reasoning}`)).size)
        .toBe(scenario.rows.length);
    }
  });

  it("keeps every confidence interval around its displayed mean", () => {
    for (const view of expanded.views) {
      for (const row of view.rows ?? []) {
        expect(row.qualityMin).toBeLessThanOrEqual(row.quality);
        expect(row.qualityMax).toBeGreaterThanOrEqual(row.quality);
        expect(row.tokensMin).toBeLessThanOrEqual(row.tokens);
        expect(row.tokensMax).toBeGreaterThanOrEqual(row.tokens);
        expect(row.durationMin).toBeLessThanOrEqual(row.duration);
        expect(row.durationMax).toBeGreaterThanOrEqual(row.duration);
      }
    }
  });

  it("publishes an explicit evidence contract for every dataset", () => {
    for (const dataset of benchmarkCohorts) {
      expect(dataset.evidence.scenarioCount).toBeGreaterThan(0);
      expect(dataset.evidence.taskRuns).toBeGreaterThan(0);
      expect(dataset.evidence.taskRuns).toBe(
        dataset.rows.reduce((total, row) => total + row.runs, 0),
      );
      expect(typeof dataset.evidence.taskFailuresRetained).toBe("boolean");
      expect(dataset.evidence.taskFailuresRetained).toBe(false);
      expect(dataset.evidence.taskFailureExclusions).toBeGreaterThanOrEqual(0);
      expect(dataset.evidence.note.length).toBeGreaterThan(40);
      expect(
        dataset.evidence.providerExclusions === null
          || dataset.evidence.providerExclusions >= 0,
      ).toBe(true);
      expect(dataset.views[0].scenarioCount).toBe(dataset.evidence.scenarioCount);
      expect(dataset.evidence.pendingScenarioCount).toBe(
        dataset.evidence.pendingScenarios.length,
      );
      expect(dataset.evidence.sources.length).toBeGreaterThan(0);
      expect(
        dataset.evidence.sources.every((source) =>
          source.url.startsWith("https://github.com/ifBars/codex-spark-agent/blob/main/")),
      ).toBe(true);
    }
  });

  it("publishes the frontier suite as an honest pending chart", () => {
    const frontier = expanded.views.find((view) => view.id === "frontier");
    expect(frontier.status).toBe("pending");
    expect(frontier.targetCeiling).toBe(65);
    expect(frontier.scenarioCount).toBe(2);
    expect(frontier.rows).toEqual([]);
    expect(frontier.scenarioDetails.map((scenario) => scenario.id)).toEqual([
      "feature-rollout-consistency-bugfix",
      "frontier-rule-transfer",
    ]);
    expect(frontier.scenarioDetails.every((scenario) => scenario.validationSignals === 6))
      .toBe(true);
  });

  it("publishes validated harder fixtures separately from measured points", () => {
    expect(benchmarkEvidenceData.schemaVersion).toBe(1);
    expect(benchmarkEvidenceData.generatedFrom).toBe(
      "docs/benchmarks/reasoning-benchmark-evidence-2026-07-26.json",
    );
    expect(expanded.evidence.pendingScenarios.map((scenario) => scenario.id)).toEqual([
      "inventory-rebalance-plan",
      "experiment-rollout-audit",
      "feature-rollout-consistency-bugfix",
      "frontier-rule-transfer",
    ]);
    expect(
      expanded.evidence.pendingScenarios.every(
        (scenario) => scenario.validationSignals === 6,
      ),
    ).toBe(true);
  });

  it("labels narrow category coverage without implying mature rankings", () => {
    expect(coverageLabel(1)).toBe("Pilot coverage");
    expect(coverageLabel(2)).toBe("Early coverage");
    expect(coverageLabel(4)).toBe("Developing coverage");
    expect(coverageLabel(9)).toBe("Broad coverage");
    expect(coverageLabel(null)).toBe("Historical");
  });

  it("publishes recent development slices without inventing intervals", () => {
    const paired = benchmarkCohorts.find((dataset) => dataset.id === "real-world-quick-slice");
    const sparkOnly = benchmarkCohorts.find(
      (dataset) => dataset.id === "real-world-spark-extension",
    );

    expect(paired.hasIntervals).toBe(false);
    expect(paired.evidence.taskRuns).toBe(8);
    expect(paired.rows).toHaveLength(2);
    expect(new Set(paired.rows.map((row) => row.runner))).toEqual(new Set(["spark", "codex"]));
    expect(paired.rows.find((row) => row.runner === "codex")?.runnerName)
      .toBe("Codex CLI 0.146.0");
    expect(paired.rows.every((row) => row.reasoning === "medium" && row.successfulRuns === row.runs))
      .toBe(true);
    expect(paired.scenarioViews).toHaveLength(4);
    expect(paired.scenarioViews.every((view) => view.rows.length === 2)).toBe(true);

    expect(sparkOnly.hasIntervals).toBe(false);
    expect(sparkOnly.evidence.taskRuns).toBe(8);
    expect(sparkOnly.rows).toHaveLength(1);
    expect(sparkOnly.rows[0].runner).toBe("spark");
    expect(sparkOnly.scenarioViews).toHaveLength(8);
    expect(sparkOnly.scenarioViews.every((view) => view.rows.length === 1)).toBe(true);
  });
});
