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

use std::path::Path;

use anyhow::Result;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use command::cmd_exec;
use errors::structured_tool_error;
use fs::{fs_edit, fs_list, fs_read, fs_rename, fs_replace, fs_search, fs_stat, fs_write};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    pub ok: bool,
    pub data: Value,
    pub error: Option<String>,
}

pub async fn invoke(cwd: &Path, mode: AgentMode, tool_name: &str, args: Value) -> ToolResult {
    match invoke_inner(cwd, mode, tool_name, args.clone()).await {
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
    mode: AgentMode,
    tool_name: &str,
    args: Value,
) -> Result<ToolResult> {
    if !mode.allows_tool(tool_name) {
        anyhow::bail!("tool `{tool_name}` is blocked in {} mode", mode.name());
    }

    match tool_name {
        "fs.read" => fs_read(cwd, args),
        "fs.list" => fs_list(cwd, args),
        "fs.stat" => fs_stat(cwd, args),
        "fs.write" => fs_write(cwd, args),
        "fs.search" => fs_search(cwd, args),
        "fs.replace" => fs_replace(cwd, args),
        "fs.edit" => fs_edit(cwd, args),
        "fs.rename" => fs_rename(cwd, args),
        "cmd.exec" => cmd_exec(cwd, args).await,
        _ => anyhow::bail!("unknown tool: {tool_name}"),
    }
}

#[cfg(test)]
mod tests;
