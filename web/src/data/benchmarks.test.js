import { describe, expect, it } from "vitest";
import benchmarkEvidenceData from "./benchmark-evidence.json";
import { coverageLabel, datasets } from "./benchmarks.js";

describe("expanded reasoning dataset", () => {
  const expanded = datasets[0];

  it("is the default dataset with all runner and reasoning combinations", () => {
    expect(expanded.id).toBe("expanded-reasoning-suite");
    expect(expanded.views.map((view) => view.id)).toEqual([
      "overall",
      "coding",
      "math-data",
      "analysis-research",
      "terminal-operations",
      "writing-configuration",
    ]);
    expect(expanded.rows).toHaveLength(6);
    expect(expanded.rows.every((row) => row.runs === 27)).toBe(true);
    expect(new Set(expanded.rows.map((row) => row.runner))).toEqual(new Set(["spark", "codex"]));
    expect(new Set(expanded.rows.map((row) => row.reasoning))).toEqual(
      new Set(["low", "medium", "high"]),
    );
  });

  it("keeps every category view complete and traceable to measured scenarios", () => {
    for (const view of expanded.views) {
      expect(view.scenarioCount).toBe(view.scenarios.length);
      expect(view.rows).toHaveLength(6);
      expect(view.rows.every((row) => row.runs === view.scenarioCount * 3)).toBe(true);
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
      expect(scenario.runCount).toBe(3);
      expect(scenario.rows).toHaveLength(6);
      expect(scenario.rows.every((row) => row.runs === 3)).toBe(true);
      expect(scenario.rows.every((row) => row.qualityMin === undefined)).toBe(true);
      expect(
        scenario.rows.every(
          (row) =>
            row.successRate
            === Number(((row.successfulRuns / row.runs) * 100).toFixed(2)),
        ),
      ).toBe(true);
      expect(new Set(scenario.rows.map((row) => row.runner))).toEqual(
        new Set(["spark", "codex"]),
      );
      expect(new Set(scenario.rows.map((row) => row.reasoning))).toEqual(
        new Set(["low", "medium", "high"]),
      );
    }
  });

  it("keeps every confidence interval around its displayed mean", () => {
    for (const view of expanded.views) {
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

  it("publishes an explicit evidence contract for every dataset", () => {
    for (const dataset of datasets) {
      expect(dataset.evidence.scenarioCount).toBeGreaterThan(0);
      expect(dataset.evidence.taskRuns).toBeGreaterThan(0);
      expect(dataset.evidence.taskRuns).toBe(
        dataset.rows.reduce((total, row) => total + row.runs, 0),
      );
      expect(typeof dataset.evidence.taskFailuresRetained).toBe("boolean");
      expect(dataset.evidence.note.length).toBeGreaterThan(40);
      expect(
        dataset.evidence.providerExclusions === null
          || dataset.evidence.providerExclusions >= 0,
      ).toBe(true);
      expect(dataset.views[0].scenarioCount).toBe(dataset.evidence.scenarioCount);
      expect(dataset.views[0].runCount).toBe(dataset.rows[0].runs);
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
});
