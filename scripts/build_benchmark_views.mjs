import { readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const benchmarkDir = join(repoRoot, "docs", "benchmarks");
const webDataDir = join(repoRoot, "web", "src", "data");
const viewSpecPath = join(benchmarkDir, "reasoning-benchmark-views-2026-07-26.json");
const evidenceSpecPath = join(
  benchmarkDir,
  "reasoning-benchmark-evidence-2026-07-26.json",
);
const outputCsvPath = join(benchmarkDir, "reasoning-cost-quality-categories-2026-07-26.csv");
const outputJsonPath = join(webDataDir, "expanded-reasoning-views.json");
const outputEvidenceJsonPath = join(webDataDir, "benchmark-evidence.json");
const checkMode = process.argv.includes("--check");

function parseCsv(text) {
  const rows = [];
  let row = [];
  let field = "";
  let quoted = false;

  for (let index = 0; index < text.length; index += 1) {
    const character = text[index];
    if (character === '"') {
      if (quoted && text[index + 1] === '"') {
        field += '"';
        index += 1;
      } else {
        quoted = !quoted;
      }
    } else if (character === "," && !quoted) {
      row.push(field);
      field = "";
    } else if ((character === "\n" || character === "\r") && !quoted) {
      if (character === "\r" && text[index + 1] === "\n") index += 1;
      row.push(field);
      if (row.some((value) => value.length > 0)) rows.push(row);
      row = [];
      field = "";
    } else {
      field += character;
    }
  }

  if (field.length > 0 || row.length > 0) {
    row.push(field);
    rows.push(row);
  }

  const [headers, ...values] = rows;
  return values.map((cells) =>
    Object.fromEntries(headers.map((header, index) => [header, cells[index] ?? ""])),
  );
}

function csvCell(value) {
  const text = String(value);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function mean(values) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

const tCritical95 = new Map([
  [1, 12.706],
  [2, 4.303],
  [3, 3.182],
  [4, 2.776],
  [5, 2.571],
  [6, 2.447],
  [7, 2.365],
  [8, 2.306],
  [9, 2.262],
  [10, 2.228],
  [11, 2.201],
  [12, 2.179],
  [13, 2.16],
  [14, 2.145],
  [15, 2.131],
  [16, 2.12],
  [17, 2.11],
  [18, 2.101],
  [19, 2.093],
  [20, 2.086],
  [21, 2.08],
  [22, 2.074],
  [23, 2.069],
  [24, 2.064],
  [25, 2.06],
  [26, 2.056],
  [27, 2.052],
  [28, 2.048],
  [29, 2.045],
  [30, 2.042],
]);

function confidence95(values) {
  if (values.length < 2) return 0;
  const average = mean(values);
  const variance =
    values.reduce((sum, value) => sum + (value - average) ** 2, 0) /
    (values.length - 1);
  const critical = tCritical95.get(values.length - 1) ?? 1.96;
  return critical * Math.sqrt(variance) / Math.sqrt(values.length);
}

function round(value, digits = 2) {
  return Number(value.toFixed(digits));
}

function runnerId(name) {
  if (name === "Spark harness") return "spark";
  if (name.startsWith("Codex CLI")) return "codex";
  throw new Error(`Unknown runner: ${name}`);
}

const spec = JSON.parse(await readFile(viewSpecPath, "utf8"));
const evidenceSpec = JSON.parse(await readFile(evidenceSpecPath, "utf8"));
if (evidenceSpec.schemaVersion !== 1 || !Array.isArray(evidenceSpec.datasets)) {
  throw new Error("Benchmark evidence manifest must use schemaVersion 1 with a datasets array");
}

const sourceRows = parseCsv(
  await readFile(join(benchmarkDir, spec.source), "utf8"),
).map((row) => ({
  runner: row.runner,
  reasoning: row.reasoning_effort,
  scenario: row.scenario,
  runs: Number(row.runs),
  successfulRuns: Number(row.successful_runs),
  quality: Number(row.average_quality),
  tokens: Number(row.average_total_tokens),
  duration: Number(row.average_duration_seconds),
}));

const availableScenarios = new Set(sourceRows.map((row) => row.scenario));
if (!Array.isArray(spec.scenarioCatalog) || spec.scenarioCatalog.length === 0) {
  throw new Error("Benchmark view specification must define a scenarioCatalog");
}
const catalogIds = spec.scenarioCatalog.map((scenario) => scenario.id);
if (new Set(catalogIds).size !== catalogIds.length) {
  throw new Error("Benchmark scenario catalog contains duplicate ids");
}
const missingCatalogEntries = [...availableScenarios].filter(
  (scenario) => !catalogIds.includes(scenario),
);
const unmeasuredCatalogEntries = catalogIds.filter(
  (scenario) => !availableScenarios.has(scenario),
);
if (missingCatalogEntries.length > 0 || unmeasuredCatalogEntries.length > 0) {
  throw new Error(
    `Scenario catalog mismatch; missing=${missingCatalogEntries.join("|") || "none"}; unmeasured=${unmeasuredCatalogEntries.join("|") || "none"}`,
  );
}
const datasetIds = new Set();
const evidenceDatasets = [];
for (const definition of evidenceSpec.datasets) {
  if (datasetIds.has(definition.id)) {
    throw new Error(`Duplicate evidence dataset id: ${definition.id}`);
  }
  datasetIds.add(definition.id);

  const runRows = parseCsv(
    await readFile(join(benchmarkDir, definition.runSource), "utf8"),
  );
  const runValues = runRows.map((row) => Number(row[definition.runField]));
  if (runValues.length === 0 || runValues.some((value) => !Number.isFinite(value) || value <= 0)) {
    throw new Error(`${definition.id} has invalid ${definition.runField} values`);
  }
  const taskRuns = runValues.reduce((sum, value) => sum + value, 0);
  const scenarioCount = definition.scenarioField
    ? new Set(runRows.map((row) => row[definition.scenarioField])).size
    : definition.expectedScenarioCount;
  if (scenarioCount !== definition.expectedScenarioCount) {
    throw new Error(
      `${definition.id} has ${scenarioCount} scenarios; expected ${definition.expectedScenarioCount}`,
    );
  }

  let providerExclusions = null;
  if (definition.providerExclusions) {
    const exclusionRows = parseCsv(
      await readFile(join(benchmarkDir, definition.providerExclusions.source), "utf8"),
    );
    const exclusionValues = exclusionRows.map((row) =>
      Number(row[definition.providerExclusions.field]));
    if (exclusionValues.some((value) => !Number.isFinite(value) || value < 0)) {
      throw new Error(`${definition.id} has invalid provider exclusion values`);
    }
    providerExclusions = exclusionValues.reduce((sum, value) => sum + value, 0);
  }

  for (const source of definition.sources) {
    await readFile(join(repoRoot, source.path), "utf8");
  }
  for (const pending of definition.pendingScenarios) {
    await readFile(join(repoRoot, pending.evidencePath), "utf8");
  }

  evidenceDatasets.push({
    id: definition.id,
    status: definition.status,
    date: definition.date,
    scenarioCount,
    pendingScenarioCount: definition.pendingScenarios.length,
    taskRuns,
    providerExclusions,
    taskFailuresRetained: definition.taskFailuresRetained,
    pendingScenarios: definition.pendingScenarios,
    sources: definition.sources,
    note: definition.note,
  });
}

const expandedEvidence = evidenceSpec.datasets.find(
  (dataset) => dataset.id === spec.datasetId,
);
if (!expandedEvidence || expandedEvidence.runSource !== spec.source) {
  throw new Error("Expanded evidence source must match the benchmark view source");
}
for (const pending of expandedEvidence.pendingScenarios) {
  if (availableScenarios.has(pending.id)) {
    throw new Error(`${pending.id} is both pending and present in measured scenario rows`);
  }
}

const views = spec.views.map((view) => {
  const missing = view.scenarios.filter((scenario) => !availableScenarios.has(scenario));
  if (missing.length > 0) {
    throw new Error(`${view.id} references missing scenarios: ${missing.join(", ")}`);
  }

  const rows = [];
  for (const runner of ["Spark harness", "Codex CLI 0.145.0"]) {
    for (const reasoning of ["low", "medium", "high"]) {
      const group = sourceRows.filter(
        (row) =>
          row.runner === runner &&
          row.reasoning === reasoning &&
          view.scenarios.includes(row.scenario),
      );
      if (group.length !== view.scenarios.length) {
        throw new Error(
          `${view.id}/${runner}/${reasoning} has ${group.length} of ${view.scenarios.length} scenarios`,
        );
      }

      const qualityValues = group.map((row) => row.quality);
      const tokenValues = group.map((row) => row.tokens);
      const durationValues = group.map((row) => row.duration);
      const quality = mean(qualityValues);
      const tokens = mean(tokenValues);
      const duration = mean(durationValues);
      const qualityCi = confidence95(qualityValues);
      const tokensCi = confidence95(tokenValues);
      const durationCi = confidence95(durationValues);
      const runs = group.reduce((sum, row) => sum + row.runs, 0);
      const successfulRuns = group.reduce((sum, row) => sum + row.successfulRuns, 0);

      rows.push({
        runner: runnerId(runner),
        runnerLabel: runner,
        reasoning,
        runs,
        scenarios: group.length,
        successfulRuns,
        successRate: round((successfulRuns / runs) * 100),
        quality: round(quality),
        qualityMin: round(Math.max(0, quality - qualityCi)),
        qualityMax: round(Math.min(100, quality + qualityCi)),
        qualityCi: round(qualityCi),
        tokens: Math.round(tokens),
        tokensMin: Math.max(0, Math.round(tokens - tokensCi)),
        tokensMax: Math.round(tokens + tokensCi),
        tokensCi: Math.round(tokensCi),
        duration: round(duration),
        durationMin: round(Math.max(0, duration - durationCi)),
        durationMax: round(duration + durationCi),
        durationCi: round(durationCi),
      });
    }
  }

  return {
    ...view,
    scenarioCount: view.scenarios.length,
    runCount: rows[0].runs,
    sample: `${view.scenarios.length} ${view.scenarios.length === 1 ? "task" : "tasks"} × 3 runs per level`,
    rows,
  };
});

const scenarioViews = spec.scenarioCatalog.map((scenario) => {
  const rows = sourceRows
    .filter((row) => row.scenario === scenario.id)
    .map((row) => ({
      runner: runnerId(row.runner),
      runnerLabel: row.runner,
      reasoning: row.reasoning,
      runs: row.runs,
      successfulRuns: row.successfulRuns,
      successRate: round((row.successfulRuns / row.runs) * 100),
      quality: row.quality,
      tokens: row.tokens,
      duration: row.duration,
    }));
  if (rows.length !== 6) {
    throw new Error(`${scenario.id} has ${rows.length} of 6 runner/reasoning rows`);
  }
  const combinations = new Set(rows.map((row) => `${row.runner}/${row.reasoning}`));
  if (combinations.size !== 6) {
    throw new Error(`${scenario.id} has duplicate runner/reasoning rows`);
  }

  return {
    ...scenario,
    scenarioCount: 1,
    runCount: rows[0].runs,
    sample: `${rows[0].runs} measured runs per runner/reasoning point`,
    rangeKind: "Three-run mean; no per-run interval published",
    rows,
  };
});

const csvHeaders = [
  "view",
  "view_label",
  "runner",
  "reasoning_effort",
  "runs",
  "scenarios",
  "successful_runs",
  "success_rate",
  "average_quality",
  "quality_ci95",
  "average_total_tokens",
  "tokens_ci95",
  "average_duration_seconds",
  "duration_ci95",
];
const csvRows = views.flatMap((view) =>
  view.rows.map((row) => [
    view.id,
    view.label,
    row.runnerLabel,
    row.reasoning,
    row.runs,
    row.scenarios,
    row.successfulRuns,
    row.successRate.toFixed(2),
    row.quality.toFixed(2),
    row.qualityCi.toFixed(2),
    row.tokens,
    row.tokensCi,
    row.duration.toFixed(2),
    row.durationCi.toFixed(2),
  ]),
);
const csv = [
  csvHeaders.map(csvCell).join(","),
  ...csvRows.map((row) => row.map(csvCell).join(",")),
].join("\n");

async function emitGenerated(path, content) {
  if (checkMode) {
    const current = await readFile(path, "utf8");
    if (current !== content) {
      throw new Error(`Generated artifact is out of date: ${path}`);
    }
    return;
  }
  await writeFile(path, content);
}

await emitGenerated(outputCsvPath, `${csv}\n`);
await emitGenerated(
  outputJsonPath,
  `${JSON.stringify(
    {
      datasetId: spec.datasetId,
      method: spec.method,
      generatedFrom: spec.source,
      views,
      scenarioViews,
    },
    null,
    2,
  )}\n`,
);
await emitGenerated(
  outputEvidenceJsonPath,
  `${JSON.stringify(
    {
      schemaVersion: evidenceSpec.schemaVersion,
      generatedFrom: "docs/benchmarks/reasoning-benchmark-evidence-2026-07-26.json",
      datasets: evidenceDatasets,
    },
    null,
    2,
  )}\n`,
);

console.log(`mode=${checkMode ? "check" : "write"}`);
console.log(`benchmark_views=${views.length}`);
console.log(`scenario_views=${scenarioViews.length}`);
console.log(`category_rows=${csvRows.length}`);
console.log(`csv=${outputCsvPath}`);
console.log(`web_json=${outputJsonPath}`);
console.log(`evidence_json=${outputEvidenceJsonPath}`);
