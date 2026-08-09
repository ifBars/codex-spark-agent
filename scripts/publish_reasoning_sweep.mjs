import { readFile, readdir, writeFile } from "node:fs/promises";
import { basename, isAbsolute, relative, resolve } from "node:path";

function argument(name) {
  const index = process.argv.indexOf(name);
  if (index === -1 || !process.argv[index + 1]) {
    throw new Error(`Missing required ${name} argument`);
  }
  return process.argv[index + 1];
}

function round(value, digits = 2) {
  const factor = 10 ** digits;
  return Math.round(value * factor) / factor;
}

function mean(values) {
  return values.reduce((sum, value) => sum + value, 0) / values.length;
}

function csvCell(value) {
  if (value === null || value === undefined) return "";
  const text = String(value);
  return /[",\r\n]/.test(text) ? `"${text.replaceAll('"', '""')}"` : text;
}

function repositoryPath(path) {
  return relative(repoRoot, path).replaceAll("\\", "/");
}

function titleCase(value) {
  return value
    .split("-")
    .map((part) => part.charAt(0).toUpperCase() + part.slice(1))
    .join(" ");
}

function runnerIdentity(row) {
  const [runner, reasoning] = row.runner.split("/");
  if (runner === "spark-harness") {
    return { id: "spark", label: "Spark harness", reasoning };
  }
  if (runner === "codex-cli") {
    const version = /^codex-cli\s+(.+)$/.exec(String(row.command_version ?? ""))?.[1];
    if (!version) throw new Error(`Missing Codex CLI version for ${row.scenario}`);
    return { id: "codex", label: `Codex CLI ${version}`, reasoning };
  }
  throw new Error(`Unsupported comparison runner: ${row.runner}`);
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
      const response = JSON.parse(await readFile(resolve(resolvedTrace, responseFile), "utf8"));
      const tokens = Number(response.raw?.response?.usage?.total_tokens);
      if (!Number.isFinite(tokens) || tokens < 0) {
        throw new Error(`Missing Spark API token usage in ${responseFile}`);
      }
      total += tokens;
    }
    totals.push(total);
  }

  if (totals.length === 0) throw new Error(`No Spark trace paths in ${source}`);
  return Math.round(mean(totals));
}

const repoRoot = resolve(import.meta.dirname, "..");
const inputPath = resolve(argument("--input"));
const outputJsonPath = resolve(argument("--output-json"));
const outputCsvPath = resolve(argument("--output-csv"));
const outputSummaryPath = resolve(argument("--output-summary"));
const expectedRepeats = Number(argument("--expected-repeats"));
const expectedScenarios = Number(argument("--expected-scenarios"));
const publishedDate = argument("--date");
const report = JSON.parse(await readFile(inputPath, "utf8"));

if (report.inputs?.successful_only !== false || !report.inputs?.group_by_reasoning) {
  throw new Error("Reasoning sweep must be grouped by reasoning and retain failed task rows");
}
if (!Number.isInteger(expectedRepeats) || expectedRepeats < 1) {
  throw new Error("--expected-repeats must be a positive integer");
}
if (!Number.isInteger(expectedScenarios) || expectedScenarios < 4) {
  throw new Error("--expected-scenarios must be at least four");
}

const validity = report.aggregate?.diagnostics?.comparison_validity;
if (!validity || validity.mixed_input_warning || validity.directional_until_fresh_paired_run) {
  throw new Error("Comparison report is not a fresh paired comparison");
}
if (validity.excluded_provider_api_rows !== 0) {
  throw new Error("Comparison report contains provider/API exclusions");
}

const reasoningLevels = ["low", "medium", "high"];
const runnerIds = ["spark", "codex"];
const expectedRunnerGroups = new Set(
  ["spark-harness", "codex-cli"].flatMap((runner) =>
    reasoningLevels.map((reasoning) => `${runner}/${reasoning}`)),
);
const actualRunnerGroups = new Set(report.rows.map((row) => row.runner));
if ([...expectedRunnerGroups].some((runner) => !actualRunnerGroups.has(runner))) {
  throw new Error(`Missing reasoning groups: ${[...expectedRunnerGroups].filter((runner) => !actualRunnerGroups.has(runner)).join(", ")}`);
}

const scenarioNames = [...new Set([
  ...(report.inputs.harness_reports ?? []).flatMap((input) => input.scenarios ?? []),
  ...(report.inputs.codex_cli_reports ?? []).flatMap((input) => input.scenarios ?? []),
])].sort();
if (scenarioNames.length !== expectedScenarios) {
  throw new Error(`Expected ${expectedScenarios} scenarios, received ${scenarioNames.length}`);
}

const comparisonRows = await Promise.all(report.rows.map(async (row) => {
  if (row.attempts !== expectedRepeats || row.successful_attempts > row.attempts) {
    throw new Error(`Invalid attempt counts for ${row.runner}/${row.scenario}`);
  }
  const runner = runnerIdentity(row);
  const tokens = runner.id === "spark"
    ? await sparkTotalTokens(row.source)
    : Math.round(Number(row.input_tokens) + Number(row.output_tokens));
  return {
    runner: runner.id,
    runnerName: runner.label,
    reasoning: runner.reasoning,
    scenario: row.scenario,
    attempts: row.attempts,
    passed: row.successful_attempts,
    failed: row.attempts - row.successful_attempts,
    quality: round(Number(row.quality_score)),
    completion: round(Number(row.completion_score)),
    process: round(Number(row.process_score)),
    tokens,
    duration: round(Number(row.duration_ms) / 1000),
    toolCalls: round(Number(row.tool_or_item_calls)),
  };
}));

const runnerNames = Object.fromEntries(comparisonRows.map((row) => [row.runner, row.runnerName]));
const byGroupScenario = new Map(
  comparisonRows.map((row) => [`${row.runner}/${row.reasoning}/${row.scenario}`, row]),
);
const commonScenarioIds = scenarioNames.filter((scenario) =>
  runnerIds.every((runner) => reasoningLevels.every((reasoning) =>
    byGroupScenario.get(`${runner}/${reasoning}/${scenario}`)?.failed === 0)),
);

const rows = runnerIds.flatMap((runner) => reasoningLevels.map((reasoning) => {
  const group = comparisonRows.filter(
    (row) => row.runner === runner && row.reasoning === reasoning,
  );
  if (group.length !== expectedScenarios) {
    throw new Error(`Expected ${expectedScenarios} scenarios for ${runner}/${reasoning}, received ${group.length}`);
  }
  const common = group.filter((row) => commonScenarioIds.includes(row.scenario));
  const passed = group.reduce((sum, row) => sum + row.passed, 0);
  const attempts = group.reduce((sum, row) => sum + row.attempts, 0);
  return {
    runner,
    runnerName: runnerNames[runner],
    reasoning,
    attempts,
    passed,
    failed: attempts - passed,
    passRate: round((passed / attempts) * 100),
    scoredScenarios: group.length,
    quality: round(mean(group.map((row) => row.quality))),
    commonScenarioQuality: common.length > 0 ? round(mean(common.map((row) => row.quality))) : null,
    completion: round(mean(group.map((row) => row.completion))),
    process: round(mean(group.map((row) => row.process))),
    tokens: Math.round(mean(group.map((row) => row.tokens))),
    duration: round(mean(group.map((row) => row.duration))),
    toolCalls: round(mean(group.map((row) => row.toolCalls))),
    ceilingScenarios: group.filter((row) => row.quality >= 99.5).length,
  };
}));

const attempts = scenarioNames.flatMap((scenario) => runnerIds.flatMap((runner) =>
  reasoningLevels.map((reasoning) => {
    const measured = byGroupScenario.get(`${runner}/${reasoning}/${scenario}`);
    if (!measured) {
      throw new Error(`Missing ${runner}/${reasoning}/${scenario}`);
    }
    return {
      runner,
      runnerName: runnerNames[runner],
      reasoning,
      scenario,
      attempts: measured.attempts,
      passed: measured.passed,
      failed: measured.failed,
      passRate: round((measured.passed / measured.attempts) * 100),
      outcome: measured.failed === 0
        ? "validated"
        : measured.passed === 0 ? "validation_failed" : "partial_validation",
      quality: measured.quality,
      completion: measured.completion,
      process: measured.process,
      tokens: measured.tokens,
      duration: measured.duration,
      toolCalls: measured.toolCalls,
    };
  })));

const failedAttempts = attempts.reduce((sum, row) => sum + row.failed, 0);

const sourcePath = repositoryPath(outputCsvPath);
const summaryPath = repositoryPath(outputSummaryPath);
const dataset = {
  id: "current-reasoning-sweep",
  date: publishedDate,
  model: report.rows[0].model,
  scenarioCount: expectedScenarios,
  expectedRepeats,
  totalAttempts: attempts.length * expectedRepeats,
  validatedAttempts: attempts.reduce((sum, row) => sum + row.passed, 0),
  failedAttempts,
  providerExclusions: 0,
  commonScenarioIds,
  sourcePath,
  summaryPath,
  runnerNames,
  rows,
  attempts,
};

const csvHeaders = [
  "runner", "reasoning_effort", "scenario", "outcome", "attempts", "passed", "failed",
  "validation_pass_rate", "quality_score", "completion_score", "process_score",
  "average_total_tokens", "average_duration_seconds", "tool_or_item_calls",
];
const csvRows = attempts.map((row) => [
  row.runnerName, row.reasoning, row.scenario, row.outcome, row.attempts, row.passed, row.failed,
  row.passRate, row.quality, row.completion, row.process, row.tokens, row.duration, row.toolCalls,
]);

const runnerSummary = rows.map((row) =>
  `| ${row.runnerName} | ${titleCase(row.reasoning)} | ${row.quality.toFixed(1)} | ${row.commonScenarioQuality?.toFixed(1) ?? "—"} | ${row.process.toFixed(1)} | ${row.passRate.toFixed(1)}% | ${row.tokens.toLocaleString("en-US")} | ${row.duration.toFixed(1)}s | ${row.ceilingScenarios}/${row.scoredScenarios} |`,
).join("\n");
const summary = `# Current reasoning sweep\n\nPublished ${publishedDate}. This is a paired ${expectedScenarios}-scenario sweep of Spark harness and native Codex CLI using \`${dataset.model}\` at low, medium, and high reasoning.\n\n- ${dataset.totalAttempts} total attempts; ${dataset.validatedAttempts} passed and ${dataset.failedAttempts} failed.\n- Quality is the scenario-balanced mean of weighted validator scores across every non-infrastructure attempt, including partial scores from failed tasks. Pass rate reports full task success.\n- ${commonScenarioIds.length} scenarios fully passed for every runner and reasoning level; the common-scenario column provides a same-task control.\n- ${expectedRepeats} attempts per runner/reasoning/scenario cell provide a repeat check; conclusions remain bounded to these fixtures.\n\n| Runner | Reasoning | Outcome quality | Full-pass common-task quality | Process | Pass rate | Tokens | Duration | Near ceiling |\n| --- | --- | ---: | ---: | ---: | ---: | ---: | ---: | ---: |\n${runnerSummary}\n\nSource comparison: \`${basename(inputPath)}\` (local profiling artifact; absolute trace paths are not published).\n`;

await writeFile(outputJsonPath, `${JSON.stringify({ schemaVersion: 1, dataset }, null, 2)}\n`);
await writeFile(
  outputCsvPath,
  `${[csvHeaders, ...csvRows].map((row) => row.map(csvCell).join(",")).join("\n")}\n`,
);
await writeFile(outputSummaryPath, summary);

console.log(`input=${inputPath}`);
console.log(`json=${outputJsonPath}`);
console.log(`csv=${outputCsvPath}`);
console.log(`summary=${outputSummaryPath}`);
console.log(`attempts=${dataset.totalAttempts}`);
console.log(`validated=${dataset.validatedAttempts}`);
console.log(`failed=${dataset.failedAttempts}`);
console.log(`common_scenarios=${dataset.commonScenarioIds.length}`);
