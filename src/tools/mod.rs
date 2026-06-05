mod command;
mod descriptors;
mod errors;
mod fs;
mod paths;
mod policy;

pub use descriptors::{ToolDescriptor, builtin_tools};
pub use policy::AgentMode;
pub(crate) use policy::is_readonly_tool;
pub(crate) use policy::tools_for_mode;

use std::path::{Path, PathBuf};

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use command::cmd_exec;
use errors::structured_tool_error;
use fs::{
    fs_edit, fs_list_with_read_roots, fs_read_with_read_roots, fs_rename, fs_replace,
    fs_search_with_read_roots, fs_stat_with_read_roots, fs_write,
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

#[cfg(test)]
pub async fn invoke(cwd: &Path, mode: AgentMode, tool_name: &str, args: Value) -> ToolResult {
    invoke_with_read_roots(cwd, &[], mode, tool_name, args).await
}

pub async fn invoke_with_read_roots(
    cwd: &Path,
    read_roots: &[PathBuf],
    mode: AgentMode,
    tool_name: &str,
    args: Value,
) -> ToolResult {
    let read_roots = canonical_read_roots(read_roots);
    match invoke_inner(cwd, &read_roots, mode, tool_name, args.clone()).await {
        Ok(result) => result,
        Err(error) => ToolResult {
            ok: false,
            data: structured_tool_error(tool_name, &args, &error.to_string()),
            error: Some(error.to_string()),
        },
    }
}

async fn invoke_inner(
    cwd: &Path,
    read_roots: &[PathBuf],
    mode: AgentMode,
    tool_name: &str,
    args: Value,
) -> Result<ToolResult> {
    if !mode.allows_tool(tool_name) {
        anyhow::bail!("tool `{tool_name}` is blocked in {} mode", mode.name());
    }

    match tool_name {
        "fs.read" => fs_read_with_read_roots(cwd, read_roots, args),
        "fs.list" => fs_list_with_read_roots(cwd, read_roots, args),
        "fs.stat" => fs_stat_with_read_roots(cwd, read_roots, args),
        "fs.write" => fs_write(cwd, args),
        "fs.search" => fs_search_with_read_roots(cwd, read_roots, args),
        "fs.replace" => fs_replace(cwd, args),
        "fs.edit" => fs_edit(cwd, args),
        "fs.rename" => fs_rename(cwd, args),
        "cmd.exec" => cmd_exec(cwd, args).await,
        "web.search" => anyhow::bail!(
            "tool `web.search` is a hosted Responses tool and is executed by the model provider, not the local harness"
        ),
        _ => anyhow::bail!("unknown tool: {tool_name}"),
    }
}

fn canonical_read_roots(read_roots: &[PathBuf]) -> Vec<PathBuf> {
    read_roots
        .iter()
        .filter_map(|root| std::fs::canonicalize(root).ok())
        .collect()
}

#[cfg(test)]
mod tests;
