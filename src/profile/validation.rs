use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::{cli::ProfileScenarioKind, profile::scenarios};

const VALIDATION_TIMEOUT: Duration = Duration::from_secs(180);
const VALIDATION_ARTIFACT: &str = "scenario-validation.json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScenarioValidationResult {
    pub(crate) scenario: String,
    pub(crate) workdir: String,
    pub(crate) command: Vec<String>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) duration_ms: u128,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) checks: Vec<ScenarioValidationCheckResult>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) source_footprint: Option<SourceFootprint>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) browser: Option<BrowserValidationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct ScenarioValidationCheckResult {
    pub(crate) name: String,
    pub(crate) weight: u32,
    pub(crate) passed: bool,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) duration_ms: u128,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

impl ScenarioValidationResult {
    pub(crate) fn granular_score(&self) -> Option<f64> {
        let total_weight = self
            .checks
            .iter()
            .map(|check| check.weight as u64)
            .sum::<u64>();
        if total_weight == 0 {
            return None;
        }
        let passed_weight = self
            .checks
            .iter()
            .filter(|check| check.passed)
            .map(|check| check.weight as u64)
            .sum::<u64>();
        Some(passed_weight as f64 / total_weight as f64 * 100.0)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct SourceFootprint {
    pub(crate) files: u64,
    pub(crate) bytes: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BrowserValidationResult {
    pub(crate) command: Vec<String>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) duration_ms: u128,
    pub(crate) screenshot: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) smoke: Option<BrowserSmokeResult>,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BrowserSmokeResult {
    pub(crate) buttons: u64,
    pub(crate) result: String,
}

pub(crate) async fn run_and_write_scenario_validation(
    cwd: &Path,
    artifact_dir: &Path,
    scenario: ProfileScenarioKind,
) -> Result<Option<ScenarioValidationResult>> {
    let Some(spec) = scenarios::profile_scenario_validation_command(scenario) else {
        return Ok(None);
    };

    let started = Instant::now();
    let output = tokio::time::timeout(
        VALIDATION_TIMEOUT,
        Command::new(spec.program)
            .args(spec.args)
            .current_dir(cwd.join(spec.workdir))
            .output(),
    )
    .await;

    let mut result = match output {
        Ok(Ok(output)) => ScenarioValidationResult {
            scenario: scenario.name().to_string(),
            workdir: spec.workdir.to_string(),
            command: command_parts(spec.program, spec.args),
            exit_code: output.status.code(),
            timed_out: false,
            duration_ms: started.elapsed().as_millis(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            checks: Vec::new(),
            source_footprint: source_footprint(&cwd.join(spec.workdir)).ok(),
            browser: None,
        },
        Ok(Err(error)) => ScenarioValidationResult {
            scenario: scenario.name().to_string(),
            workdir: spec.workdir.to_string(),
            command: command_parts(spec.program, spec.args),
            exit_code: None,
            timed_out: false,
            duration_ms: started.elapsed().as_millis(),
            stdout: String::new(),
            stderr: format!("failed to start validation command: {error}"),
            checks: Vec::new(),
            source_footprint: source_footprint(&cwd.join(spec.workdir)).ok(),
            browser: None,
        },
        Err(_) => ScenarioValidationResult {
            scenario: scenario.name().to_string(),
            workdir: spec.workdir.to_string(),
            command: command_parts(spec.program, spec.args),
            exit_code: None,
            timed_out: true,
            duration_ms: started.elapsed().as_millis(),
            stdout: String::new(),
            stderr: "validation timed out".to_string(),
            checks: Vec::new(),
            source_footprint: source_footprint(&cwd.join(spec.workdir)).ok(),
            browser: None,
        },
    };
    result.checks = run_validation_checks(cwd, spec.workdir, scenario).await;
    result.browser = run_browser_validation(cwd, artifact_dir, scenario).await?;

    std::fs::write(
        artifact_dir.join(VALIDATION_ARTIFACT),
        serde_json::to_string_pretty(&result)?,
    )
    .map_err(|error| {
        anyhow::anyhow!(
            "failed to write validation artifact in {}: {error}",
            artifact_dir.display()
        )
    })?;

    Ok(Some(result))
}

async fn run_validation_checks(
    cwd: &Path,
    workdir: &str,
    scenario: ProfileScenarioKind,
) -> Vec<ScenarioValidationCheckResult> {
    let mut results = Vec::new();
    for check in scenarios::profile_scenario_validation_checks(scenario) {
        let started = Instant::now();
        let output = tokio::time::timeout(
            VALIDATION_TIMEOUT,
            Command::new(check.program)
                .args(check.args)
                .current_dir(cwd.join(workdir))
                .output(),
        )
        .await;
        let result = match output {
            Ok(Ok(output)) => ScenarioValidationCheckResult {
                name: check.name.to_string(),
                weight: check.weight,
                passed: output.status.success(),
                exit_code: output.status.code(),
                timed_out: false,
                duration_ms: started.elapsed().as_millis(),
                stdout: String::from_utf8_lossy(&output.stdout).to_string(),
                stderr: String::from_utf8_lossy(&output.stderr).to_string(),
            },
            Ok(Err(error)) => ScenarioValidationCheckResult {
                name: check.name.to_string(),
                weight: check.weight,
                passed: false,
                exit_code: None,
                timed_out: false,
                duration_ms: started.elapsed().as_millis(),
                stdout: String::new(),
                stderr: format!("failed to start validation check: {error}"),
            },
            Err(_) => ScenarioValidationCheckResult {
                name: check.name.to_string(),
                weight: check.weight,
                passed: false,
                exit_code: None,
                timed_out: true,
                duration_ms: started.elapsed().as_millis(),
                stdout: String::new(),
                stderr: "validation check timed out".to_string(),
            },
        };
        results.push(result);
    }
    results
}

async fn run_browser_validation(
    cwd: &Path,
    artifact_dir: &Path,
    scenario: ProfileScenarioKind,
) -> Result<Option<BrowserValidationResult>> {
    if scenario != ProfileScenarioKind::ReactCalculatorScaffold {
        return Ok(None);
    }

    let artifact_subdir = artifact_dir.join("browser-artifacts");
    std::fs::create_dir_all(&artifact_subdir).map_err(|error| {
        anyhow::anyhow!(
            "failed to create browser artifact directory {}: {error}",
            artifact_subdir.display()
        )
    })?;
    let workdir = cwd.to_path_buf();
    let screenshot = artifact_subdir.join("react-calculator-browser.png");
    let script = artifact_subdir.join("react-calculator-browser-smoke.mjs");
    std::fs::write(&script, browser_validation_script(&screenshot, &workdir)).map_err(|error| {
        anyhow::anyhow!(
            "failed to write browser validation script {}: {error}",
            script.display()
        )
    })?;

    let started = Instant::now();
    let install_output = tokio::time::timeout(
        VALIDATION_TIMEOUT,
        Command::new("bun")
            .args(["add", "--dev", "--no-save", "playwright"])
            .current_dir(&artifact_subdir)
            .output(),
    )
    .await;
    let (install_exit_code, install_stdout, install_stderr) = command_output_parts(install_output);
    let script_arg = child_process_path(&script);
    let output = if install_exit_code == Some(0) {
        Some(
            tokio::time::timeout(
                VALIDATION_TIMEOUT,
                Command::new("node")
                    .arg(&script_arg)
                    .current_dir(&artifact_subdir)
                    .output(),
            )
            .await,
        )
    } else {
        None
    };
    let command = command_parts("node", &[&script_arg]);

    let result = match output {
        Some(Ok(Ok(output))) => {
            let script_stdout = String::from_utf8_lossy(&output.stdout).to_string();
            BrowserValidationResult {
                command,
                exit_code: output.status.code(),
                timed_out: false,
                duration_ms: started.elapsed().as_millis(),
                screenshot: child_process_path(&screenshot),
                smoke: parse_browser_smoke_result(&script_stdout),
                stdout: format!(
                    "playwright install stdout:\n{}\nscript stdout:\n{}",
                    install_stdout, script_stdout
                ),
                stderr: format!(
                    "playwright install stderr:\n{}\nscript stderr:\n{}",
                    install_stderr,
                    String::from_utf8_lossy(&output.stderr)
                ),
            }
        }
        Some(Ok(Err(error))) => BrowserValidationResult {
            command,
            exit_code: None,
            timed_out: false,
            duration_ms: started.elapsed().as_millis(),
            screenshot: child_process_path(&screenshot),
            smoke: None,
            stdout: install_stdout,
            stderr: format!("failed to start browser validation command: {error}"),
        },
        Some(Err(_)) => BrowserValidationResult {
            command,
            exit_code: None,
            timed_out: true,
            duration_ms: started.elapsed().as_millis(),
            screenshot: child_process_path(&screenshot),
            smoke: None,
            stdout: install_stdout,
            stderr: "browser validation timed out".to_string(),
        },
        None => BrowserValidationResult {
            command,
            exit_code: install_exit_code,
            timed_out: false,
            duration_ms: started.elapsed().as_millis(),
            screenshot: child_process_path(&screenshot),
            smoke: None,
            stdout: install_stdout,
            stderr: format!("failed to install playwright with bun:\n{install_stderr}"),
        },
    };

    Ok(Some(result))
}

fn command_output_parts(
    output: Result<std::io::Result<std::process::Output>, tokio::time::error::Elapsed>,
) -> (Option<i32>, String, String) {
    match output {
        Ok(Ok(output)) => (
            output.status.code(),
            String::from_utf8_lossy(&output.stdout).to_string(),
            String::from_utf8_lossy(&output.stderr).to_string(),
        ),
        Ok(Err(error)) => (
            None,
            String::new(),
            format!("failed to start dependency install command: {error}"),
        ),
        Err(_) => (
            None,
            String::new(),
            "dependency install timed out".to_string(),
        ),
    }
}

fn browser_validation_script(screenshot: &Path, app_dir: &Path) -> String {
    let screenshot = child_process_path(screenshot).replace('\\', "\\\\");
    let app_dir = child_process_path(app_dir).replace('\\', "\\\\");
    format!(
        r#"import {{ chromium }} from "playwright";
import {{ spawn }} from "node:child_process";

const screenshotPath = "{screenshot}";
const appDir = "{app_dir}";
const server = spawn("bun", ["x", "vite", "--host", "127.0.0.1", "--port", "4173"], {{
  cwd: appDir,
  shell: false,
  stdio: ["ignore", "pipe", "pipe"],
}});
let serverOutput = "";
server.stdout.on("data", (chunk) => serverOutput += chunk.toString());
server.stderr.on("data", (chunk) => serverOutput += chunk.toString());

const delay = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
const withTimeout = (promise, ms, label) => Promise.race([
  promise,
  new Promise((_, reject) => setTimeout(() => reject(new Error(label)), ms)),
]);
let browser;
let page;
const browserEvents = [];
let exitCode = 0;
try {{
  const deadline = Date.now() + 30000;
  let ready = false;
  while (Date.now() < deadline) {{
    try {{
      const response = await fetch("http://127.0.0.1:4173/");
      if (response.ok) {{
        ready = true;
        break;
      }}
    }} catch {{}}
    await delay(500);
  }}
  if (!ready) throw new Error(`vite server did not become ready. Output:\n${{serverOutput}}`);

  browser = await withTimeout(chromium.launch({{ headless: true }}), 60000, "chromium launch timed out");
  page = await withTimeout(browser.newPage({{ viewport: {{ width: 1280, height: 900 }} }}), 10000, "browser page creation timed out");
  page.on("console", (message) => browserEvents.push(`console ${{message.type()}} ${{message.text()}}`));
  page.on("pageerror", (error) => browserEvents.push(`pageerror ${{error.message}}`));
  page.setDefaultTimeout(5000);
  page.setDefaultNavigationTimeout(10000);
  await page.goto("http://127.0.0.1:4173/", {{ waitUntil: "domcontentloaded", timeout: 10000 }});
  await page.waitForTimeout(500);

  const buttons = await page.locator("button").count();
  const display = page.locator('[data-testid="display"], [aria-label="display"], [role="status"], .calculator-display, .display').first();
  const displayText = await display.innerText({{ timeout: 5000 }});
  if (buttons < 10) throw new Error(`expected at least 10 calculator buttons, found ${{buttons}}`);
  if (!displayText.trim()) throw new Error("calculator display is empty");

  const clickText = async (text) => page.locator("button").filter({{ hasText: text }}).first().click();
  await clickText("1");
  await clickText("+");
  await clickText("2");
  await clickText("=");
  await page.waitForTimeout(250);
  const resultText = (await display.innerText({{ timeout: 5000 }})).trim();
  if (resultText !== "3" && !/^3(?:\.0+)?$/.test(resultText)) {{
    throw new Error(`expected display result 3 after 1 + 2 =, got "${{resultText}}"`);
  }}
  await page.screenshot({{ path: screenshotPath, fullPage: true }});
  console.log(JSON.stringify({{ ok: true, buttons, result: resultText, screenshot: screenshotPath }}));
}} catch (error) {{
  if (page) {{
    await page.screenshot({{ path: screenshotPath, fullPage: true }}).catch(() => {{}});
  }}
  if (browserEvents.length) console.error(`browser events:\n${{browserEvents.join("\n")}}`);
  console.error(error?.stack ?? String(error));
  exitCode = 1;
}} finally {{
  if (browser) {{
    try {{
      await withTimeout(browser.close(), 2000, "browser close timed out");
    }} catch {{}}
  }}
  server.kill();
  process.exit(exitCode);
}}
"#
    )
}

fn child_process_path(path: &Path) -> String {
    let path = path.display().to_string();
    path.strip_prefix(r"\\?\").unwrap_or(&path).to_string()
}

fn parse_browser_smoke_result(stdout: &str) -> Option<BrowserSmokeResult> {
    stdout.lines().rev().find_map(|line| {
        let value = serde_json::from_str::<serde_json::Value>(line).ok()?;
        (value.get("ok").and_then(serde_json::Value::as_bool) == Some(true)).then(|| {
            BrowserSmokeResult {
                buttons: value
                    .get("buttons")
                    .and_then(serde_json::Value::as_u64)
                    .unwrap_or(0),
                result: value
                    .get("result")
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default()
                    .to_string(),
            }
        })
    })
}

fn source_footprint(workdir: &Path) -> Result<SourceFootprint> {
    let mut files = 0u64;
    let mut bytes = 0u64;
    visit_source_files(workdir, &mut |path| {
        if source_file_counts(path) {
            files += 1;
            bytes += path.metadata().map(|metadata| metadata.len()).unwrap_or(0);
        }
    })?;
    Ok(SourceFootprint { files, bytes })
}

fn visit_source_files(dir: &Path, visit: &mut impl FnMut(&Path)) -> Result<()> {
    if !dir.exists() {
        return Ok(());
    }
    for entry in std::fs::read_dir(dir)
        .map_err(|error| anyhow::anyhow!("failed to read {}: {error}", dir.display()))?
    {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if path.is_dir() {
            // Spark artifact directory names are reserved at every depth, matching the
            // benchmark read-root deny-list and trace-mirroring exclusions.
            if matches!(
                name.as_ref(),
                "node_modules"
                    | "target"
                    | ".git"
                    | ".vite"
                    | ".spark-runs"
                    | ".spark-profile"
                    | ".spark-scenarios"
            ) {
                continue;
            }
            visit_source_files(&path, visit)?;
        } else {
            visit(&path);
        }
    }
    Ok(())
}

fn source_file_counts(path: &Path) -> bool {
    let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
        return false;
    };
    if matches!(
        name,
        "brief.md" | "sample.log" | "bun.lock" | "Cargo.lock" | ".spark-browser-smoke.mjs"
    ) {
        return false;
    }
    matches!(
        path.extension().and_then(|extension| extension.to_str()),
        Some("rs" | "toml" | "tsx" | "ts" | "css" | "html" | "json")
    )
}

pub(crate) fn read_scenario_validation(run: &Path) -> Option<ScenarioValidationResult> {
    let raw = std::fs::read_to_string(run.join(VALIDATION_ARTIFACT)).ok()?;
    serde_json::from_str(&raw).ok()
}

fn command_parts(program: &str, args: &[&str]) -> Vec<String> {
    std::iter::once(program)
        .chain(args.iter().copied())
        .map(str::to_string)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::{
        ScenarioValidationCheckResult, ScenarioValidationResult, browser_validation_script,
        source_footprint,
    };

    #[test]
    fn browser_validation_script_runs_vite_from_app_directory() {
        let screenshot = std::path::Path::new("artifacts/react-calculator-browser.png");
        let app_dir = std::path::Path::new(".");

        let script = browser_validation_script(screenshot, app_dir);

        assert!(script.contains("const appDir = \".\";"));
        assert!(script.contains("cwd: appDir"));
        assert!(script.contains("from \"playwright\""));
    }

    #[test]
    fn granular_score_uses_check_weights() {
        let check = |name: &str, weight: u32, passed: bool| ScenarioValidationCheckResult {
            name: name.to_string(),
            weight,
            passed,
            exit_code: Some(i32::from(!passed)),
            timed_out: false,
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
        };
        let result = ScenarioValidationResult {
            scenario: "fixture".to_string(),
            workdir: ".".to_string(),
            command: Vec::new(),
            exit_code: Some(1),
            timed_out: false,
            duration_ms: 1,
            stdout: String::new(),
            stderr: String::new(),
            checks: vec![check("core", 70, true), check("edge", 30, false)],
            source_footprint: None,
            browser: None,
        };

        assert_eq!(result.granular_score(), Some(70.0));
    }

    #[test]
    fn source_footprint_excludes_harness_owned_artifacts() {
        let workspace = tempfile::tempdir().expect("workspace");
        std::fs::create_dir_all(workspace.path().join("src")).expect("create source directory");
        std::fs::write(
            workspace.path().join("src/app.ts"),
            "export const ready = true;\n",
        )
        .expect("write source");

        for directory in [".spark-runs", ".spark-profile", ".spark-scenarios"] {
            let artifact_dir = workspace.path().join(directory);
            std::fs::create_dir_all(&artifact_dir).expect("create artifact directory");
            std::fs::write(
                artifact_dir.join("large-response.json"),
                "x".repeat(100_000),
            )
            .expect("write artifact");
        }
        let nested_artifact_dir = workspace.path().join("nested/.spark-runs");
        std::fs::create_dir_all(&nested_artifact_dir).expect("create nested artifact directory");
        std::fs::write(
            nested_artifact_dir.join("nested-response.json"),
            "x".repeat(100_000),
        )
        .expect("write nested artifact");

        let footprint = source_footprint(workspace.path()).expect("measure source footprint");

        assert_eq!(footprint.files, 1);
        assert_eq!(footprint.bytes, 27);
    }
}
