import { describe, expect, it } from "vitest";
import reasoningSweepData from "./reasoning-sweep.json";
import {
  benchmarkCohorts,
  coverageLabel,
  datasets,
  frontierComparison,
} from "./benchmarks.js";

describe("current reasoning sweep", () => {
  const cohort = benchmarkCohorts[0];
  const source = reasoningSweepData.dataset;

  it("publishes one corrected benchmark cohort", () => {
    expect(datasets).toHaveLength(1);
    expect(datasets[0].id).toBe("spark-bench");
    expect(datasets[0].cohorts).toBe(benchmarkCohorts);
    expect(benchmarkCohorts).toHaveLength(1);
    expect(cohort.id).toBe("current-reasoning-sweep");
    expect(frontierComparison).toBeNull();
  });

  it("keeps all runner and reasoning combinations", () => {
    expect(cohort.rows).toHaveLength(6);
    expect(new Set(cohort.rows.map((row) => row.runner))).toEqual(new Set(["spark", "codex"]));
    expect(new Set(cohort.rows.map((row) => row.reasoning))).toEqual(
      new Set(["low", "medium", "high"]),
    );
    expect(cohort.rows.reduce((sum, row) => sum + row.runs, 0))
      .toBe(source.validatedAttempts);
    expect(cohort.rows.reduce((sum, row) => sum + row.excludedRuns, 0))
      .toBe(source.failedAttempts);
  });

  it("publishes six measured task types and a separate Frontier view", () => {
    expect(cohort.views.map((view) => view.id)).toEqual([
      "overall",
      "code-changes",
      "quantitative-analysis",
      "investigation",
      "terminal-operations",
      "writing-configuration",
      "stateful-policy",
      "frontier",
    ]);
    const taskTypes = cohort.views.slice(1).filter((view) => view.id !== "frontier");
    expect(taskTypes).toHaveLength(6);
    expect(cohort.views.at(-1).scenarios).toEqual([
      "feature-rollout-consistency-bugfix",
      "frontier-rule-transfer",
    ]);
  });

  it("keeps every view complete and traceable", () => {
    for (const view of cohort.views) {
      expect(view.scenarioCount).toBe(view.scenarios.length);
      expect(view.rows).toHaveLength(6);
      expect(view.rows.every((row) => row.runs >= 0)).toBe(true);
      expect(view.rows.every((row) => row.runs + row.excludedRuns
        === view.scenarioCount * source.expectedRepeats)).toBe(true);
      expect(new Set(view.rows.map((row) => `${row.runner}/${row.reasoning}`)).size).toBe(6);
    }
  });

  it("keeps every interval around its displayed mean", () => {
    for (const view of cohort.views) {
      for (const row of view.rows) {
        expect(row.qualityMin).toBeLessThanOrEqual(row.quality);
        expect(row.qualityMax).toBeGreaterThanOrEqual(row.quality);
        expect(row.tokensMin).toBeLessThanOrEqual(row.tokens);
        expect(row.tokensMax).toBeGreaterThanOrEqual(row.tokens);
        expect(row.durationMin).toBeLessThanOrEqual(row.duration);
        expect(row.durationMax).toBeGreaterThanOrEqual(row.duration);
      }
    }
  });

  it("retains failures in reliability totals without converting them to quality zeros", () => {
    expect(cohort.evidence.taskFailuresRetained).toBe(true);
    expect(cohort.evidence.attemptedTaskRuns).toBe(source.totalAttempts);
    expect(cohort.evidence.taskRuns).toBe(source.validatedAttempts);
    expect(cohort.evidence.taskFailureExclusions).toBe(source.failedAttempts);
    expect(cohort.evidence.providerExclusions).toBe(0);
    expect(source.attempts).toHaveLength(source.scenarioCount * 6);
    expect(source.attempts.filter((row) => row.failed > 0)
      .reduce((sum, row) => sum + row.failed, 0)).toBe(source.failedAttempts);
  });

  it("uses weighted validator quality and records a common-task control", () => {
    expect(cohort.evidence.note).toContain("weighted validator scores");
    expect(source.commonScenarioIds.length).toBeGreaterThan(0);
    expect(source.rows.every((row) => Number.isFinite(row.quality))).toBe(true);
    expect(source.rows.every((row) => Number.isFinite(row.commonScenarioQuality))).toBe(true);
  });

  it("links to the corrected source artifact", () => {
    expect(cohort.source).toBe(
      `https://github.com/ifBars/codex-spark-agent/blob/main/${source.sourcePath}`,
    );
  });

  it("labels narrow coverage without implying broad evidence", () => {
    expect(coverageLabel(1)).toBe("Pilot coverage");
    expect(coverageLabel(2)).toBe("Early coverage");
    expect(coverageLabel(4)).toBe("Developing coverage");
    expect(coverageLabel(12)).toBe("Broad coverage");
    expect(coverageLabel(null)).toBe("Historical");
  });
});
