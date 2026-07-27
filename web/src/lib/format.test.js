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
  const definitions = {
    tokens: { key: "tokens", minKey: "tokensMin", maxKey: "tokensMax" },
    quality: { key: "quality", minKey: "qualityMin", maxKey: "qualityMax" },
  };

  it("focuses the default domain on point estimates", () => {
    const rows = [
      { tokens: 100, tokensMin: 10, tokensMax: 500 },
      { tokens: 200, tokensMin: 80, tokensMax: 450 },
    ];
    const [min, max] = chartDomain(rows, "tokens", definitions);
    expect(min).toBeLessThan(100);
    expect(max).toBeGreaterThan(200);
    expect(max).toBeLessThan(500);
  });

  it("can include reported ranges when a full-interval domain is needed", () => {
    const rows = [{ tokens: 100, tokensMin: 75, tokensMax: 140 }];
    const [min, max] = chartDomain(rows, "tokens", definitions, {
      includeRanges: true,
    });
    expect(min).toBeLessThanOrEqual(75);
    expect(max).toBeGreaterThanOrEqual(140);
  });

  it("zooms percentage metrics while respecting valid bounds", () => {
    const rows = [
      { quality: 88, qualityMin: 50, qualityMax: 100 },
      { quality: 97, qualityMin: 80, qualityMax: 100 },
    ];
    const [min, max] = chartDomain(rows, "quality", definitions, {
      bounds: [0, 100],
    });
    expect(min).toBeGreaterThan(0);
    expect(min).toBeLessThan(88);
    expect(max).toBeGreaterThan(97);
    expect(max).toBeLessThanOrEqual(100);
  });
});
