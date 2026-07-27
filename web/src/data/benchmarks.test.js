import { describe, expect, it } from "vitest";
import { datasets } from "./benchmarks.js";

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
});
