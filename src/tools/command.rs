use std::path::{Path, PathBuf};
use std::process::Stdio;

use anyhow::Result;
use serde_json::{Value, json};
use tokio::process::Command;

use super::ToolResult;
use super::paths::{display_rel, required_str, resolve_under};

const MAX_COMMAND_STREAM_CHARS: usize = 24_000;
const DEFAULT_COMMAND_TIMEOUT_MS: u64 = 20_000;
const MAX_COMMAND_TIMEOUT_MS: u64 = 30_000;

pub(super) async fn cmd_exec(cwd: &Path, args: Value) -> Result<ToolResult> {
    let shell_backend = ShellBackend::from_env();
    let workdir = CommandWorkdir::resolve(
        cwd,
        args.get("workdir").and_then(Value::as_str),
        &shell_backend,
    )?;
    let timeout_ms = args
        .get("timeout_ms")
        .and_then(Value::as_u64)
        .unwrap_or(DEFAULT_COMMAND_TIMEOUT_MS)
        .clamp(100, MAX_COMMAND_TIMEOUT_MS);

    let mut command = if let Some(shell_command) = args.get("command").and_then(Value::as_str) {
        shell_command_for(cwd, &workdir, shell_command, &shell_backend)
    } else {
        let cmd = required_str(&args, "cmd")?;
        let mut command = Command::new(cmd);
        if let Some(argv) = args.get("args").and_then(Value::as_array) {
            command.args(argv.iter().filter_map(Value::as_str));
        }
        command.current_dir(&workdir.host);
        command
    };
    command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .kill_on_drop(true);
    let output = match tokio::time::timeout(
        std::time::Duration::from_millis(timeout_ms),
        command.output(),
    )
    .await
    {
        Ok(output) => output?,
        Err(_) => {
            return Ok(ToolResult {
                ok: false,
                data: json!({
                    "timed_out": true,
                    "timeout_ms": timeout_ms,
                    "workdir": workdir.display(cwd),
                }),
                error: Some(format!("command timed out after {timeout_ms}ms")),
            });
        }
    };

    let stdout = bounded_text(
        String::from_utf8_lossy(&output.stdout).as_ref(),
        MAX_COMMAND_STREAM_CHARS,
    );
    let stderr = bounded_text(
        String::from_utf8_lossy(&output.stderr).as_ref(),
        MAX_COMMAND_STREAM_CHARS,
    );

    let ok = output.status.success();
    let code = output.status.code();
    let hint = command_failure_hint(&stderr.text);
    Ok(ToolResult {
        ok,
        data: json!({
            "code": code,
            "shell": shell_backend.shell_name(),
            "stdout": stdout.text,
            "stderr": stderr.text,
            "hint": hint,
            "stdout_chars": stdout.original_chars,
            "stderr_chars": stderr.original_chars,
            "stdout_truncated": stdout.truncated,
            "stderr_truncated": stderr.truncated,
            "workdir": workdir.display(cwd),
        }),
        error: (!ok).then(|| {
            let base = match code {
                Some(code) => format!("command exited with code {code}"),
                None => "command terminated by signal".to_string(),
            };
            match hint {
                Some(hint) => format!("{base}; {hint}"),
                None => base,
            }
        }),
    })
}

pub(super) struct BoundedText {
    pub(super) text: String,
    pub(super) original_chars: usize,
    pub(super) truncated: bool,
}

pub(super) fn bounded_text(raw: &str, max_chars: usize) -> BoundedText {
    let original_chars = raw.chars().count();
    if original_chars <= max_chars {
        return BoundedText {
            text: raw.to_string(),
            original_chars,
            truncated: false,
        };
    }

    let marker = format!(
        "\n...[command stream truncated; original_chars={original_chars}, max_chars={max_chars}]...\n"
    );
    let marker_chars = marker.chars().count();
    let available = max_chars.saturating_sub(marker_chars);
    let head_len = available.saturating_mul(2) / 3;
    let tail_len = available.saturating_sub(head_len);
    let head = raw.chars().take(head_len).collect::<String>();
    let tail_vec = raw.chars().rev().take(tail_len).collect::<Vec<_>>();
    let tail = tail_vec.into_iter().rev().collect::<String>();

    BoundedText {
        text: format!("{head}{marker}{tail}"),
        original_chars,
        truncated: true,
    }
}

enum ShellBackend {
    Local,
    Docker { container: String, workdir: String },
}

struct CommandWorkdir {
    host: PathBuf,
    docker: Option<String>,
}

impl CommandWorkdir {
    fn resolve(cwd: &Path, requested: Option<&str>, backend: &ShellBackend) -> Result<Self> {
        if matches!(backend, ShellBackend::Docker { .. }) {
            if let Some(path) = requested.filter(|path| path.starts_with('/')) {
                return Ok(Self {
                    host: cwd.to_path_buf(),
                    docker: Some(path.to_string()),
                });
            }
        }

        let host = requested
            .map(|path| resolve_under(cwd, path))
            .transpose()?
            .unwrap_or_else(|| cwd.to_path_buf());
        Ok(Self { host, docker: None })
    }

    fn display(&self, cwd: &Path) -> String {
        self.docker
            .clone()
            .unwrap_or_else(|| display_rel(cwd, &self.host))
    }
}

impl ShellBackend {
    fn from_env() -> Self {
        let Some(container) = std::env::var("SPARK_CMD_EXEC_DOCKER_CONTAINER")
            .ok()
            .filter(|value| !value.trim().is_empty())
        else {
            return Self::Local;
        };
        let workdir =
            std::env::var("SPARK_CMD_EXEC_DOCKER_WORKDIR").unwrap_or_else(|_| "/app".to_string());
        Self::Docker { container, workdir }
    }

    fn shell_name(&self) -> String {
        match self {
            Self::Local => local_shell_name().to_string(),
            Self::Docker { container, .. } => format!("docker:{container}"),
        }
    }
}

fn shell_command_for(
    cwd: &Path,
    workdir: &CommandWorkdir,
    command: &str,
    backend: &ShellBackend,
) -> Command {
    match backend {
        ShellBackend::Local => local_shell_command_for(&workdir.host, command),
        ShellBackend::Docker {
            container,
            workdir: container_root,
        } => docker_shell_command_for(cwd, workdir, command, container, container_root),
    }
}

fn docker_shell_command_for(
    cwd: &Path,
    workdir: &CommandWorkdir,
    command: &str,
    container: &str,
    container_root: &str,
) -> Command {
    let container_workdir = workdir
        .docker
        .clone()
        .unwrap_or_else(|| container_workdir_for(cwd, &workdir.host, container_root));
    let mut cmd = Command::new("docker");
    cmd.args([
        "exec",
        "-i",
        "-w",
        &container_workdir,
        container,
        "sh",
        "-lc",
        command,
    ]);
    cmd
}

fn container_workdir_for(cwd: &Path, workdir: &Path, container_root: &str) -> String {
    let Ok(relative) = workdir.strip_prefix(cwd) else {
        return container_root.to_string();
    };
    if relative.as_os_str().is_empty() {
        return container_root.to_string();
    }

    let mut path = PathBuf::from(container_root);
    path.push(relative);
    path.to_string_lossy().replace('\\', "/")
}

fn local_shell_command_for(workdir: &Path, command: &str) -> Command {
    #[cfg(target_os = "windows")]
    {
        let mut cmd = Command::new("powershell");
        cmd.args(["-NoProfile", "-Command", command])
            .current_dir(workdir);
        cmd
    }
    #[cfg(not(target_os = "windows"))]
    {
        let mut cmd = Command::new("sh");
        cmd.args(["-lc", command]).current_dir(workdir);
        cmd
    }
}

fn local_shell_name() -> &'static str {
    #[cfg(target_os = "windows")]
    {
        "powershell"
    }
    #[cfg(not(target_os = "windows"))]
    {
        "sh"
    }
}

fn command_failure_hint(stderr: &str) -> Option<&'static str> {
    #[cfg(target_os = "windows")]
    {
        if stderr.contains("The token '&&' is not a valid statement separator")
            || stderr.contains("InvalidEndOfLine")
        {
            return Some(
                "Windows cmd.exec runs through powershell -NoProfile -Command; PowerShell 5.1 does not support &&. Run commands in separate cmd.exec calls or use PowerShell-compatible separators.",
            );
        }
    }
    let _ = stderr;
    None
}
