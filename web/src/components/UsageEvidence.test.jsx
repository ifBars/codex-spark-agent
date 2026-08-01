import { describe, expect, it } from "vitest";
import { usageHistoryHasActivity } from "../data/usage-history.js";

describe("usageHistoryHasActivity", () => {
  it("does not turn missing history into activity", () => {
    expect(usageHistoryHasActivity({ availability: "unavailable", aggregate: null })).toBe(false);
  });

  it("requires a source-reported total-token aggregate", () => {
    expect(usageHistoryHasActivity({
      availability: "available",
      aggregate: { totalTokens: { total: 1240, complete: true } },
    })).toBe(true);
    expect(usageHistoryHasActivity({
      availability: "available",
      aggregate: { totalTokens: { total: null, complete: false } },
    })).toBe(false);
  });
});
