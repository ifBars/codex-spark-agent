use std::{path::Path, process::Stdio, time::Duration};

use anyhow::{Context, Result, bail};
use serde_json::{Value, json};
use tokio::process::Command;

use super::{ToolResult, command::bounded_text};

const DEFAULT_TIMEOUT_MS: u64 = 20_000;
const MAX_TIMEOUT_MS: u64 = 30_000;
const MAX_STREAM_CHARS: usize = 24_000;
const MAX_ARGS: usize = 48;
const MAX_ARG_CHARS: usize = 4_096;

pub(super) async fn gh_read(cwd: &Path, input: Value) -> Result<ToolResult> {
    let args = parse_args(&input)?;
    validate_read_only_args(&args)?;
    let timeout_ms = input
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_TIMEOUT_MS)
        .clamp(100, MAX_TIMEOUT_MS);

    let mut command = Command::new("gh");
    command
        .args(&args)
        .current_dir(cwd)
        .env("GH_PAGER", "cat")
        .env("PAGER", "cat")
        .env("GH_PROMPT_DISABLED", "1")
        .env("NO_COLOR", "1")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);

    let output =
        match tokio::time::timeout(Duration::from_millis(timeout_ms), command.output()).await {
            Ok(output) => output.context(
                "failed to launch `gh`; install GitHub CLI and ensure it is available on PATH",
            )?,
            Err(_) => {
                return Ok(ToolResult {
                    ok: false,
                    data: json!({
                        "args": args,
                        "timed_out": true,
                        "timeout_ms": timeout_ms,
                    }),
                    error: Some(format!("gh read command timed out after {timeout_ms}ms")),
                });
            }
        };

    let stdout = bounded_text(
        String::from_utf8_lossy(&output.stdout).as_ref(),
        MAX_STREAM_CHARS,
    );
    let stderr = bounded_text(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        MAX_STREAM_CHARS,
    );
    let ok = output.status.success();
    let code = output.status.code();

    Ok(ToolResult {
        ok,
        data: json!({
            "args": args,
            "code": code,
            "stdout": stdout.text,
            "stderr": stderr.text,
            "stdout_chars": stdout.original_chars,
            "stderr_chars": stderr.original_chars,
            "stdout_truncated": stdout.truncated,
            "stderr_truncated": stderr.truncated,
        }),
        error: (!ok).then(|| match code {
            Some(code) => format!("gh read command exited with code {code}"),
            None => "gh read command terminated by signal".to_string(),
        }),
    })
}

fn parse_args(input: &Value) -> Result<Vec<String>> {
    let values = input
        .get("args")
        .and_then(Value::as_array)
        .ok_or_else(|| anyhow::anyhow!("args must be a non-empty array of strings"))?;
    if values.is_empty() {
        bail!("args must be a non-empty array of strings");
    }
    if values.len() > MAX_ARGS {
        bail!("args supports at most {MAX_ARGS} entries");
    }

    values
        .iter()
        .map(|value| {
            let arg = value
                .as_str()
                .ok_or_else(|| anyhow::anyhow!("every args entry must be a string"))?;
            if arg.chars().count() > MAX_ARG_CHARS {
                bail!("one args entry exceeds {MAX_ARG_CHARS} characters");
            }
            if arg.contains('\0') {
                bail!("args entries cannot contain NUL characters");
            }
            Ok(arg.to_string())
        })
        .collect()
}

fn validate_read_only_args(args: &[String]) -> Result<()> {
    let command = args.first().map(String::as_str).unwrap_or_default();
    let subcommand = args.get(1).map(String::as_str).unwrap_or_default();
    if command == "api" {
        return validate_read_only_api_args(args);
    }
    let allowed = match command {
        "auth" => matches!(subcommand, "status"),
        "repo" => matches!(subcommand, "view" | "list"),
        "issue" => matches!(subcommand, "view" | "list" | "status"),
        "pr" => matches!(subcommand, "view" | "list" | "status" | "diff" | "checks"),
        "run" => matches!(subcommand, "view" | "list" | "watch"),
        "workflow" => matches!(subcommand, "view" | "list"),
        "release" => matches!(subcommand, "view" | "list"),
        "search" => matches!(subcommand, "code" | "commits" | "issues" | "prs" | "repos"),
        "label" => matches!(subcommand, "list"),
        _ => false,
    };
    if !allowed {
        bail!(
            "gh.read only permits read-only GitHub CLI operations; `{command} {subcommand}` is not allowed"
        );
    }
    if args
        .iter()
        .any(|arg| matches!(arg.as_str(), "--web" | "-w"))
    {
        bail!("gh.read does not open browser windows; remove --web/-w");
    }
    if command == "auth"
        && args
            .iter()
            .any(|arg| matches!(arg.as_str(), "--show-token" | "-t"))
    {
        bail!("gh.read never exposes authentication tokens");
    }
    Ok(())
}

fn validate_read_only_api_args(args: &[String]) -> Result<()> {
    let endpoint = args.get(1).map(String::as_str).unwrap_or_default();
    if endpoint.is_empty() || endpoint == "graphql" || endpoint.starts_with('-') {
        bail!("gh.read permits only explicit REST endpoints for `gh api`; GraphQL is not allowed");
    }

    const MUTATING_FLAGS: &[&str] = &[
        "-X",
        "--method",
        "-f",
        "--raw-field",
        "-F",
        "--field",
        "--input",
    ];
    if args.iter().skip(2).any(|arg| {
        MUTATING_FLAGS.contains(&arg.as_str())
            || ["--method=", "--raw-field=", "--field=", "--input="]
                .iter()
                .any(|prefix| arg.starts_with(prefix))
            || ["-X", "-f", "-F"]
                .iter()
                .any(|prefix| arg.starts_with(prefix) && arg.len() > prefix.len())
    }) {
        bail!(
            "gh.read allows only the default GET form of `gh api`; method, field, and input flags are rejected"
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).to_string()).collect()
    }

    #[test]
    fn allows_pr_review_inspection_commands() {
        validate_read_only_args(&args(&[
            "pr",
            "view",
            "42",
            "--json",
            "title,files,reviews,statusCheckRollup",
            "--repo",
            "owner/repo",
        ]))
        .expect("read-only pr view");
        validate_read_only_args(&args(&["pr", "diff", "42", "--repo", "owner/repo"]))
            .expect("read-only pr diff");
    }

    #[test]
    fn rejects_mutations_graphql_and_token_output() {
        for command in [
            args(&["pr", "review", "42", "--approve"]),
            args(&["issue", "create", "--title", "x"]),
            args(&["run", "rerun", "123"]),
            args(&["api", "graphql"]),
            args(&["api", "repos/owner/repo/issues", "-f", "title=x"]),
            args(&["api", "repos/owner/repo/issues", "-XPOST"]),
            args(&["api", "repos/owner/repo/issues", "-ftitle=x"]),
            args(&["auth", "status", "--show-token"]),
        ] {
            assert!(validate_read_only_args(&command).is_err(), "{command:?}");
        }
    }

    #[test]
    fn allows_paginated_get_only_rest_api_queries() {
        validate_read_only_args(&args(&[
            "api",
            "repos/owner/repo/pulls/42/comments",
            "--paginate",
            "--jq",
            ".[].body",
        ]))
        .expect("GET-only REST query");
    }
}
