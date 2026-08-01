import { readFile, readdir, writeFile } from "node:fs/promises";
import { isAbsolute, join, resolve } from "node:path";

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) {
    throw new Error(`Missing required ${name} argument`);
  }
  return process.argv[index + 1];
}

function csvCell(value) {
  const text = String(value);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

const inputPath = resolve(argument("--input"));
const outputPath = resolve(argument("--output"));
const expectedRepeats = Number(argument("--expected-repeats"));
const expectedGroups = Number(argument("--expected-groups"));
const report = JSON.parse(await readFile(inputPath, "utf8"));

if (report.inputs?.successful_only !== true) {
  throw new Error("Comparison report was not generated with --successful-only");
}
if (!Number.isInteger(expectedRepeats) || expectedRepeats < 1) {
  throw new Error("--expected-repeats must be a positive integer");
}
if (!Number.isInteger(expectedGroups) || expectedGroups < 1) {
  throw new Error("--expected-groups must be a positive integer");
}
if (!Array.isArray(report.rows) || report.rows.length === 0) {
  throw new Error("Comparison report has no rows");
}
if (report.rows.some((row) => row.success !== true || row.attempts < 1)) {
  throw new Error("Successful-only comparison contains a failed or empty row");
}

function runnerLabel(row, runner) {
  if (runner === "spark-harness") return "Spark harness";
  if (runner !== "codex-cli") throw new Error(`Unsupported runner label: ${row.runner}`);

  const version = String(row.command_version ?? "").trim();
  const match = /^codex-cli\s+(.+)$/.exec(version);
  if (!match) {
    throw new Error(`Missing Codex CLI command version for ${row.runner}/${row.scenario}`);
  }
  return `Codex CLI ${match[1]}`;
}
async function sparkTotalTokens(source) {
  const tracePaths = source
    .replace(/^averaged \d+ attempts:\s*/, "")
    .split(";")
    .map((path) => path.trim())
    .filter(Boolean);
  const totals = [];
  for (const tracePath of tracePaths) {
    const resolvedTrace = isAbsolute(tracePath) ? tracePath : resolve(tracePath);
    const entries = await readdir(resolvedTrace);
    const responseFiles = entries.filter((entry) => /^\d+-response(?:-\d+)?\.json$/.test(entry));
    let total = 0;
    for (const responseFile of responseFiles) {
      const response = JSON.parse(await readFile(join(resolvedTrace, responseFile), "utf8"));
      const tokens = Number(response.raw?.response?.usage?.total_tokens);
      if (!Number.isFinite(tokens) || tokens < 0) {
        throw new Error(`Missing API token usage in ${join(resolvedTrace, responseFile)}`);
      }
      total += tokens;
    }
    totals.push(total);
  }
  if (totals.length === 0) throw new Error(`No Spark trace paths in: ${source}`);
  return Math.round(totals.reduce((sum, value) => sum + value, 0) / totals.length);
}
const headers = [
  "runner",
  "reasoning_effort",
  "scenario",
  "runs",
  "failed_runs_excluded",
  "successful_runs",
  "average_quality",
  "average_total_tokens",
  "average_duration_seconds",
];
const rows = await Promise.all(report.rows
  .map(async (row) => {
    const [runner, reasoning] = row.runner.split("/");
    const label = runnerLabel(row, runner);
    if (!["low", "medium", "high"].includes(reasoning)) {
      throw new Error(`Unsupported runner label: ${row.runner}`);
    }
    if (row.attempts > expectedRepeats) {
      throw new Error(`${row.runner}/${row.scenario} exceeds expected repeat count`);
    }
    const totalTokens = runner === "spark-harness"
      ? await sparkTotalTokens(row.source)
      : Math.round(Number(row.input_tokens) + Number(row.output_tokens));
    return [
      label,
      reasoning,
      row.scenario,
      row.attempts,
      expectedRepeats - row.attempts,
      row.successful_attempts,
      Number(row.quality_score).toFixed(2),
      totalTokens,
      (Number(row.duration_ms) / 1000).toFixed(2),
    ];
  }));
rows
  .sort((left, right) =>
    left[0].localeCompare(right[0])
    || ["low", "medium", "high"].indexOf(left[1])
      - ["low", "medium", "high"].indexOf(right[1])
    || left[2].localeCompare(right[2]));

const csv = [
  headers.map(csvCell).join(","),
  ...rows.map((row) => row.map(csvCell).join(",")),
].join("\n");
await writeFile(outputPath, `${csv}\n`);

const excluded = rows.reduce((sum, row) => sum + row[4], 0)
  + (expectedGroups - rows.length) * expectedRepeats;
console.log(`successful_rows=${rows.length}`);
console.log(`failed_runs_excluded=${excluded}`);
console.log(`output=${outputPath}`);
