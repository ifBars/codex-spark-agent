use std::{
    path::Path,
    time::{Duration, Instant},
};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use tokio::process::Command;

use crate::{cli::ProfileScenarioKind, profile_scenarios};

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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub(crate) browser: Option<BrowserValidationResult>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(crate) struct BrowserValidationResult {
    pub(crate) command: Vec<String>,
    pub(crate) exit_code: Option<i32>,
    pub(crate) timed_out: bool,
    pub(crate) duration_ms: u128,
    pub(crate) screenshot: String,
    pub(crate) stdout: String,
    pub(crate) stderr: String,
}

pub(crate) async fn run_and_write_scenario_validation(
    cwd: &Path,
    artifact_dir: &Path,
    scenario: ProfileScenarioKind,
) -> Result<Option<ScenarioValidationResult>> {
    let Some(spec) = profile_scenarios::profile_scenario_validation_command(scenario) else {
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
            browser: None,
        },
    };
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
    let workdir = cwd.join(".spark-scenarios/react-calculator");
    let screenshot = artifact_subdir.join("react-calculator-browser.png");
    let script = workdir.join(".spark-browser-smoke.mjs");
    std::fs::write(&script, browser_validation_script(&screenshot)).map_err(|error| {
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
            .current_dir(&workdir)
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
                    .current_dir(&workdir)
                    .output(),
            )
            .await,
        )
    } else {
        None
    };
    let command = command_parts("node", &[&script_arg]);

    let result = match output {
        Some(Ok(Ok(output))) => BrowserValidationResult {
            command,
            exit_code: output.status.code(),
            timed_out: false,
            duration_ms: started.elapsed().as_millis(),
            screenshot: child_process_path(&screenshot),
            stdout: format!(
                "playwright install stdout:\n{}\nscript stdout:\n{}",
                install_stdout,
                String::from_utf8_lossy(&output.stdout)
            ),
            stderr: format!(
                "playwright install stderr:\n{}\nscript stderr:\n{}",
                install_stderr,
                String::from_utf8_lossy(&output.stderr)
            ),
        },
        Some(Ok(Err(error))) => BrowserValidationResult {
            command,
            exit_code: None,
            timed_out: false,
            duration_ms: started.elapsed().as_millis(),
            screenshot: child_process_path(&screenshot),
            stdout: install_stdout,
            stderr: format!("failed to start browser validation command: {error}"),
        },
        Some(Err(_)) => BrowserValidationResult {
            command,
            exit_code: None,
            timed_out: true,
            duration_ms: started.elapsed().as_millis(),
            screenshot: child_process_path(&screenshot),
            stdout: install_stdout,
            stderr: "browser validation timed out".to_string(),
        },
        None => BrowserValidationResult {
            command,
            exit_code: install_exit_code,
            timed_out: false,
            duration_ms: started.elapsed().as_millis(),
            screenshot: child_process_path(&screenshot),
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

fn browser_validation_script(screenshot: &Path) -> String {
    let screenshot = child_process_path(screenshot).replace('\\', "\\\\");
    format!(
        r#"import {{ chromium }} from "playwright";
import {{ spawn }} from "node:child_process";

const screenshotPath = "{screenshot}";
const server = spawn("bun", ["x", "vite", "--host", "127.0.0.1", "--port", "4173"], {{
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
