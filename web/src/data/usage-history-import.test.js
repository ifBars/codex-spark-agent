import { describe, expect, it } from "vitest";
import { sanitizeUsageHistory } from "../../../scripts/import_usage_history.mjs";

function fixture() {
  const metric = (total) => ({
    total,
    reported_observations: 2,
    observations: 2,
    complete: true,
    availability: "reported",
  });
  const metrics = Object.fromEntries([
    "input_tokens",
    "cached_input_tokens",
    "cache_write_input_tokens",
    "uncached_input_tokens",
    "output_tokens",
    "reasoning_output_tokens",
    "total_tokens",
  ].map((key, index) => [key, metric((index + 1) * 100)]));
  return {
    schema_version: "spark.usage_history.v1",
    kind: "local_codex_session_history",
    generated_at_unix_seconds: 1_754_077_800,
    aggregate: {
      observations: 2,
      reporting_coverage: { observations_with_any_usage: 2, complete: true },
      metrics,
    },
    by_day: [{ day: "2026-08-01", observations: 2, metrics }],
    by_model: [{ model: "gpt-5.3-codex-spark", observations: 2, metrics }],
    pricing: { availability: "unavailable", reason: "no_public_api_price" },
    source: { kind: "codex_local_session_history", network: false },
  };
}

describe("usage history importer", () => {
  it("keeps only public aggregate evidence and does not price Spark", () => {
    const imported = sanitizeUsageHistory(fixture());
    expect(imported.availability).toBe("available");
    expect(imported.aggregate.totalTokens.total).toBe(700);
    expect(imported.aggregate.reasoningOutputTokens.total).toBe(600);
    expect(imported.generatedAt).toBe("2025-08-01T19:50:00.000Z");
    expect(imported.byDay).toHaveLength(1);
    expect(imported.byModel[0].model).toBe("gpt-5.3-codex-spark");
    expect(imported.pricing).toEqual({ status: "unavailable", reason: "no_public_api_price" });
    expect(JSON.stringify(imported)).not.toContain("source");
  });

  it("rejects raw session identifiers, prompts, paths, and auth data", () => {
    for (const key of ["session_id", "prompt", "cwd", "tool_output", "access_token", "raw"]) {
      const source = fixture();
      source[key] = "private";
      expect(() => sanitizeUsageHistory(source)).toThrow(/privacy-sensitive/);
    }
  });

  it("strips additive fields instead of serializing arbitrary payloads", () => {
    const source = fixture();
    source.extra = { harmless: "discarded" };
    source.aggregate.extra_metric = 99;
    const imported = sanitizeUsageHistory(source);
    expect(imported.extra).toBeUndefined();
    expect(imported.aggregate.extraMetric).toBeUndefined();
  });
});
