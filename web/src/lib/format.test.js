import { describe, expect, it } from "vitest";
import { chartDomain, formatMetric } from "./format.js";

describe("formatMetric", () => {
  it("formats tokens, duration, and percentages", () => {
    expect(formatMetric("tokens", 105836)).toBe("105.8k");
    expect(formatMetric("duration", 15.92)).toBe("15.9s");
    expect(formatMetric("successRate", 66.67)).toBe("66.7%");
  });
});

describe("chartDomain", () => {
  it("includes reported ranges", () => {
    const rows = [{ tokens: 100, tokensMin: 75, tokensMax: 140 }];
    const definitions = {
      tokens: { key: "tokens", minKey: "tokensMin", maxKey: "tokensMax" },
    };
    const [min, max] = chartDomain(rows, "tokens", definitions);
    expect(min).toBeLessThanOrEqual(75);
    expect(max).toBeGreaterThanOrEqual(140);
  });
});
