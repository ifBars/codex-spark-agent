import { mkdir, readFile, writeFile } from "node:fs/promises";
import { dirname, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const METRIC_KEYS = [
  "input_tokens",
  "cached_input_tokens",
  "cache_write_input_tokens",
  "uncached_input_tokens",
  "output_tokens",
  "reasoning_output_tokens",
  "total_tokens",
];

const SENSITIVE_KEYS = new Set([
  "session_id",
  "sessionid",
  "prompt",
  "prompts",
  "message",
  "messages",
  "cwd",
  "path",
  "paths",
  "tool_output",
  "tooloutput",
  "auth",
  "authorization",
  "access_token",
  "refresh_token",
  "raw",
  "payload",
  "trace",
  "transcript",
  "content",
  "command",
  "arguments",
]);

function isObject(value) {
  return value !== null && typeof value === "object" && !Array.isArray(value);
}

function asNonNegativeInteger(value, label, { nullable = false } = {}) {
  if (nullable && value === null) return null;
  if (!Number.isSafeInteger(value) || value < 0) {
    throw new Error(`${label} must be a non-negative integer${nullable ? " or null" : ""}`);
  }
  return value;
}

function assertNoSensitiveKeys(value, path = "$") {
  if (Array.isArray(value)) {
    value.forEach((entry, index) => assertNoSensitiveKeys(entry, `${path}[${index}]`));
    return;
  }
  if (!isObject(value)) return;
  for (const [key, child] of Object.entries(value)) {
    if (SENSITIVE_KEYS.has(key.toLowerCase())) {
      throw new Error(`Refusing privacy-sensitive key at ${path}.${key}`);
    }
    assertNoSensitiveKeys(child, `${path}.${key}`);
  }
}

function safeString(value, label, { nullable = true } = {}) {
  if (nullable && (value === null || value === undefined)) return null;
  if (typeof value !== "string" || value.length === 0 || value.length > 160 || /[\\/]/.test(value)) {
    throw new Error(`${label} must be a short path-free string`);
  }
  return value;
}

function generatedAtForWeb(source) {
  if (source.generated_at !== undefined) return safeString(source.generated_at, "generated_at");
  if (source.generated_at_unix_seconds === undefined) return null;
  const seconds = asNonNegativeInteger(source.generated_at_unix_seconds, "generated_at_unix_seconds");
  return new Date(seconds * 1000).toISOString();
}

function metricForWeb(metric, label) {
  if (!isObject(metric)) return { total: null, reportedObservations: 0, observations: 0, complete: false, status: "unavailable" };
  const total = asNonNegativeInteger(metric.total, `${label}.total`, { nullable: true });
  const reportedObservations = asNonNegativeInteger(
    metric.reported_observations ?? metric.reported_responses ?? 0,
    `${label}.reported_observations`,
  );
  const observations = asNonNegativeInteger(metric.observations ?? 0, `${label}.observations`);
  const complete = metric.complete === true;
  const status = metric.availability ?? metric.status ?? (complete ? "reported" : total === null ? "unavailable" : "partial");
  if (!["reported", "partial", "unavailable"].includes(status)) {
    throw new Error(`${label}.availability must be reported, partial, or unavailable`);
  }
  return { total, reportedObservations, observations, complete, status };
}

function metricsForWeb(metrics, label) {
  if (!isObject(metrics)) throw new Error(`${label}.metrics must be an object`);
  return Object.fromEntries(METRIC_KEYS.map((key) => [
    toCamel(key),
    metricForWeb(metrics[key], `${label}.metrics.${key}`),
  ]));
}

function toCamel(value) {
  return value.replace(/_([a-z])/g, (_, character) => character.toUpperCase());
}

function aggregateForWeb(aggregate, label) {
  if (!isObject(aggregate)) throw new Error(`${label} must be an object`);
  return {
    observations: asNonNegativeInteger(aggregate.observations ?? 0, `${label}.observations`),
    reportingCoverage: {
      observationsWithAnyUsage: asNonNegativeInteger(
        aggregate.reporting_coverage?.observations_with_any_usage ?? 0,
        `${label}.reporting_coverage.observations_with_any_usage`,
      ),
      complete: aggregate.reporting_coverage?.complete === true,
    },
    ...metricsForWeb(aggregate.metrics, label),
  };
}

function rowsForWeb(rows, key, label) {
  if (!Array.isArray(rows)) throw new Error(`${label} must be an array`);
  return rows.map((row, index) => {
    if (!isObject(row)) throw new Error(`${label}[${index}] must be an object`);
    const identity = safeString(row[key], `${label}[${index}].${key}`, { nullable: false });
    return {
      [key === "day" ? "day" : "model"]: identity,
      observations: asNonNegativeInteger(row.observations ?? 0, `${label}[${index}].observations`),
      ...metricsForWeb(row.metrics, `${label}[${index}]`),
    };
  });
}

export function sanitizeUsageHistory(source) {
  if (!isObject(source)) throw new Error("Usage history must be a JSON object");
  assertNoSensitiveKeys(source);
  if (source.schema_version !== "spark.usage_history.v1") {
    throw new Error("Expected schema_version spark.usage_history.v1");
  }
  if (source.kind !== "local_codex_session_history") {
    throw new Error("Expected kind local_codex_session_history");
  }

  const aggregate = aggregateForWeb(source.aggregate, "aggregate");
  const totalTokens = aggregate.totalTokens.total;
  const availability = Number.isFinite(totalTokens)
    ? "available"
    : aggregate.observations > 0 ? "partial" : "unavailable";
  const pricingAvailability = source.pricing?.availability;
  if (pricingAvailability && !["available", "unavailable", "estimated"].includes(pricingAvailability)) {
    throw new Error("pricing.availability must be available, estimated, or unavailable");
  }

  return {
    schemaVersion: 1,
    kind: "spark_usage_history",
    availability,
    reason: availability === "available"
      ? "Imported from a local aggregate generated by spark usage --history."
      : "Imported aggregate does not contain a complete total-token value.",
    generatedAt: generatedAtForWeb(source),
    aggregate,
    byDay: rowsForWeb(source.by_day ?? [], "day", "by_day"),
    byModel: rowsForWeb(source.by_model ?? [], "model", "by_model"),
    pricing: {
      status: pricingAvailability === "available" ? "estimated" : "unavailable",
      reason: safeString(source.pricing?.reason, "pricing.reason")
        ?? "No public API token price is available for gpt-5.3-codex-spark.",
    },
    quota: {
      status: "unavailable",
      reason: "Account quota is intentionally excluded from local history imports.",
    },
  };
}

export async function importUsageHistory(inputPath, outputPath) {
  const source = JSON.parse(await readFile(inputPath, "utf8"));
  const sanitized = sanitizeUsageHistory(source);
  await mkdir(dirname(outputPath), { recursive: true });
  await writeFile(outputPath, `${JSON.stringify(sanitized, null, 2)}\n`);
  return sanitized;
}

function parseArgs(args) {
  const values = new Map();
  for (let index = 0; index < args.length; index += 2) {
    const key = args[index];
    const value = args[index + 1];
    if (!["--input", "--output"].includes(key) || !value) {
      throw new Error("Usage: bun scripts/import_usage_history.mjs --input <history.json> --output <web-data.json>");
    }
    values.set(key, value);
  }
  if (!values.has("--input") || !values.has("--output")) {
    throw new Error("Usage: bun scripts/import_usage_history.mjs --input <history.json> --output <web-data.json>");
  }
  return values;
}

const isMainModule = process.argv[1] && resolve(process.argv[1]) === fileURLToPath(import.meta.url);
if (isMainModule) {
  try {
    const args = parseArgs(process.argv.slice(2));
    const output = await importUsageHistory(resolve(args.get("--input")), resolve(args.get("--output")));
    console.log(`usage_history=${output.availability}`);
    console.log(`output=${resolve(args.get("--output"))}`);
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
