import { readFile, writeFile } from "node:fs/promises";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const scriptDir = dirname(fileURLToPath(import.meta.url));
const repoRoot = resolve(scriptDir, "..");
const benchmarkDir = join(repoRoot, "docs", "benchmarks");
const webDataDir = join(repoRoot, "web", "src", "data");
const viewSpecPath = join(benchmarkDir, "reasoning-benchmark-views-2026-07-26.json");
const outputCsvPath = join(benchmarkDir, "reasoning-cost-quality-categories-2026-07-26.csv");
const outputJsonPath = join(webDataDir, "expanded-reasoning-views.json");

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

await writeFile(outputCsvPath, `${csv}\n`);
await writeFile(
  outputJsonPath,
  `${JSON.stringify(
    {
      datasetId: spec.datasetId,
      method: spec.method,
      generatedFrom: spec.source,
      views,
    },
    null,
    2,
  )}\n`,
);

console.log(`benchmark_views=${views.length}`);
console.log(`category_rows=${csvRows.length}`);
console.log(`csv=${outputCsvPath}`);
console.log(`web_json=${outputJsonPath}`);
