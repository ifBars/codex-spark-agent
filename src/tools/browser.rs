use std::{
    path::{Path, PathBuf},
    process::Stdio,
    time::{Duration, Instant},
};

use anyhow::{Context, Result};
use serde_json::{Value, json};
use tokio::process::Command;

use super::ToolResult;
use super::command::bounded_text;
use super::paths::{display_rel, required_str, resolve_under_for_write};

const MAX_BROWSER_STREAM_CHARS: usize = 24_000;
const MAX_BROWSER_TEXT_CHARS: usize = 16_000;
const DEFAULT_TIMEOUT_MS: u64 = 45_000;

pub(super) async fn browser_run(cwd: &Path, args: Value) -> Result<ToolResult> {
    let url = required_str(&args, "url")?;
    let timeout_ms = args
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(1_000, 120_000);
    let text_limit = args
        .get("text_limit")
        .and_then(Value::as_u64)
        .unwrap_or(MAX_BROWSER_TEXT_CHARS as u64)
        .clamp(0, 40_000) as usize;

    let tool_dir = cwd.join(".spark").join("browser-tools");
    std::fs::create_dir_all(&tool_dir)
        .with_context(|| format!("failed to create {}", tool_dir.display()))?;
    ensure_playwright_dependency(&tool_dir, timeout_ms).await?;

    let screenshot_path = screenshot_path(cwd, &tool_dir, &args)?;
    let config_path = tool_dir.join("browser-run-config.json");
    let script_path = tool_dir.join("browser-run.mjs");
    let config = browser_config(&args, &screenshot_path, text_limit);
    std::fs::write(&config_path, serde_json::to_vec_pretty(&config)?)
        .with_context(|| format!("failed to write {}", config_path.display()))?;
    std::fs::write(&script_path, browser_script())
        .with_context(|| format!("failed to write {}", script_path.display()))?;

    let started = Instant::now();
    let mut command = Command::new("node");
    command
        .arg(child_process_path(&script_path))
        .arg(child_process_path(&config_path))
        .current_dir(&tool_dir)
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output =
        match tokio::time::timeout(Duration::from_millis(timeout_ms), command.output()).await {
            Ok(output) => output?,
            Err(_) => {
                return Ok(ToolResult {
                    ok: false,
                    data: json!({
                        "timed_out": true,
                        "timeout_ms": timeout_ms,
                        "url": url,
                    }),
                    error: Some(format!("browser run timed out after {timeout_ms}ms")),
                });
            }
        };

    let stdout_raw = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr_raw = String::from_utf8_lossy(&output.stderr).to_string();
    let stdout = bounded_text(&stdout_raw, MAX_BROWSER_STREAM_CHARS);
    let stderr = bounded_text(&stderr_raw, MAX_BROWSER_STREAM_CHARS);
    let parsed = parse_browser_result(&stdout_raw);
    let ok = output.status.success()
        && parsed
            .as_ref()
            .is_some_and(|value| value.get("ok").and_then(Value::as_bool) == Some(true));
    let mut data = parsed.unwrap_or_else(|| json!({}));
    if let Some(object) = data.as_object_mut() {
        object.insert("code".to_string(), json!(output.status.code()));
        object.insert(
            "duration_ms".to_string(),
            json!(started.elapsed().as_millis()),
        );
        object.insert("stdout".to_string(), json!(stdout.text));
        object.insert("stderr".to_string(), json!(stderr.text));
        object.insert("stdout_chars".to_string(), json!(stdout.original_chars));
        object.insert("stderr_chars".to_string(), json!(stderr.original_chars));
        object.insert("stdout_truncated".to_string(), json!(stdout.truncated));
        object.insert("stderr_truncated".to_string(), json!(stderr.truncated));
        if let Some(path) = screenshot_path.as_ref().filter(|path| path.exists()) {
            object.insert("screenshot_path".to_string(), json!(display_rel(cwd, path)));
        }
        object.insert(
            "dependency_dir".to_string(),
            json!(display_rel(cwd, &tool_dir)),
        );
    }

    Ok(ToolResult {
        ok,
        data,
        error: (!ok).then(|| {
            browser_error_message(&stderr_raw)
                .or_else(|| {
                    output
                        .status
                        .code()
                        .map(|code| format!("browser run exited with code {code}"))
                })
                .unwrap_or_else(|| "browser run failed".to_string())
        }),
    })
}

async fn ensure_playwright_dependency(tool_dir: &Path, timeout_ms: u64) -> Result<()> {
    if tool_dir.join("node_modules").join("playwright").exists() {
        return Ok(());
    }

    let output = tokio::time::timeout(
        Duration::from_millis(timeout_ms),
        Command::new("bun")
            .args(["add", "--dev", "--no-save", "playwright"])
            .current_dir(tool_dir)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true)
            .output(),
    )
    .await
    .context("playwright dependency install timed out")?
    .context("failed to start bun to install playwright")?;

    if !output.status.success() {
        anyhow::bail!(
            "failed to install playwright with bun:\n{}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    Ok(())
}

fn screenshot_path(cwd: &Path, tool_dir: &Path, args: &Value) -> Result<Option<PathBuf>> {
    if let Some(path) = args.get("screenshot_path").and_then(Value::as_str) {
        let path = resolve_under_for_write(cwd, path)?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("failed to create {}", parent.display()))?;
        }
        return Ok(Some(path));
    }

    if args
        .get("capture_screenshot")
        .and_then(Value::as_bool)
        .unwrap_or(false)
    {
        return Ok(Some(tool_dir.join("browser-run.png")));
    }

    Ok(None)
}

fn browser_config(args: &Value, screenshot_path: &Option<PathBuf>, text_limit: usize) -> Value {
    json!({
        "url": args.get("url").cloned().unwrap_or(Value::Null),
        "actions": args.get("actions").cloned().unwrap_or_else(|| json!([])),
        "viewport": {
            "width": args.get("viewport_width").and_then(Value::as_u64).unwrap_or(1280),
            "height": args.get("viewport_height").and_then(Value::as_u64).unwrap_or(900),
        },
        "headless": args.get("headless").and_then(Value::as_bool).unwrap_or(true),
        "waitUntil": args.get("wait_until").and_then(Value::as_str).unwrap_or("domcontentloaded"),
        "textLimit": text_limit,
        "screenshotPath": screenshot_path.as_ref().map(|path| child_process_path(path)),
    })
}

fn parse_browser_result(stdout: &str) -> Option<Value> {
    stdout
        .lines()
        .rev()
        .find_map(|line| serde_json::from_str::<Value>(line).ok())
}

fn browser_error_message(stderr: &str) -> Option<String> {
    stderr
        .lines()
        .rev()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .map(str::to_string)
}

fn child_process_path(path: &Path) -> String {
    let path = path.display().to_string();
    path.strip_prefix(r"\\?\").unwrap_or(&path).to_string()
}

fn browser_script() -> &'static str {
    r#"import { chromium } from "playwright";
import { readFile } from "node:fs/promises";

const configPath = process.argv[2];
const config = JSON.parse(await readFile(configPath, "utf8"));
const consoleEvents = [];
const pageErrors = [];
let browser;
let page;
let exitCode = 0;

const withTimeout = (promise, ms, label) => Promise.race([
  promise,
  new Promise((_, reject) => setTimeout(() => reject(new Error(label)), ms)),
]);

const bounded = (value, limit) => {
  const text = String(value ?? "");
  if (limit <= 0) return "";
  return text.length > limit ? `${text.slice(0, limit)}\n...[browser text truncated]...` : text;
};

const locatorFor = (selector) => page.locator(selector).first();

const runAction = async (action) => {
  const type = action.type;
  if (type === "click") {
    await locatorFor(action.selector).click({ timeout: action.timeout_ms ?? 5000 });
  } else if (type === "fill") {
    await locatorFor(action.selector).fill(String(action.value ?? ""), { timeout: action.timeout_ms ?? 5000 });
  } else if (type === "select") {
    await locatorFor(action.selector).selectOption(action.value, { timeout: action.timeout_ms ?? 5000 });
  } else if (type === "press") {
    await locatorFor(action.selector ?? "body").press(action.key, { timeout: action.timeout_ms ?? 5000 });
  } else if (type === "wait") {
    if (action.selector) {
      await page.waitForSelector(action.selector, { timeout: action.timeout_ms ?? 5000 });
    } else {
      await page.waitForTimeout(action.ms ?? 500);
    }
  } else {
    throw new Error(`unknown browser action type: ${type}`);
  }
};

try {
  browser = await withTimeout(chromium.launch({ headless: config.headless !== false }), 60000, "chromium launch timed out");
  page = await browser.newPage({ viewport: config.viewport });
  page.on("console", (message) => consoleEvents.push({ type: message.type(), text: message.text() }));
  page.on("pageerror", (error) => pageErrors.push(error.message));
  page.setDefaultTimeout(5000);
  page.setDefaultNavigationTimeout(15000);

  const response = await page.goto(config.url, { waitUntil: config.waitUntil ?? "domcontentloaded", timeout: 30000 });
  for (const action of config.actions ?? []) {
    await runAction(action);
  }

  let aria = "";
  try {
    aria = await page.locator("body").ariaSnapshot({ timeout: 5000 });
  } catch (error) {
    aria = `aria snapshot unavailable: ${error.message}`;
  }

  if (config.screenshotPath) {
    await page.screenshot({ path: config.screenshotPath, fullPage: true });
  }

  const bodyText = await page.locator("body").innerText({ timeout: 5000 }).catch(() => "");
  console.log(JSON.stringify({
    ok: true,
    final_url: page.url(),
    title: await page.title(),
    status: response?.status() ?? null,
    text: bounded(bodyText, config.textLimit ?? 16000),
    text_chars: bodyText.length,
    text_truncated: bodyText.length > (config.textLimit ?? 16000),
    aria: bounded(aria, config.textLimit ?? 16000),
    console: consoleEvents.slice(-50),
    page_errors: pageErrors.slice(-20),
  }));
} catch (error) {
  if (config.screenshotPath && page) {
    await page.screenshot({ path: config.screenshotPath, fullPage: true }).catch(() => {});
  }
  console.error(error?.stack ?? String(error));
  console.log(JSON.stringify({
    ok: false,
    final_url: page?.url?.() ?? config.url,
    title: page ? await page.title().catch(() => "") : "",
    console: consoleEvents.slice(-50),
    page_errors: pageErrors.slice(-20),
    error: error?.message ?? String(error),
  }));
  exitCode = 1;
} finally {
  if (browser) {
    await browser.close().catch(() => {});
  }
  process.exit(exitCode);
}
"#
}
