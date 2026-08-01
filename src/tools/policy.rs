use serde::{Deserialize, Serialize};

use super::ToolDescriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AgentMode {
    Ask,
    Work,
}

impl AgentMode {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Ask => "ask",
            Self::Work => "work",
        }
    }

    pub(crate) fn allows_tool(self, tool_name: &str) -> bool {
        match self {
            Self::Ask => is_readonly_tool(tool_name),
            Self::Work => true,
        }
    }
}

pub(crate) fn is_readonly_tool(tool_name: &str) -> bool {
    matches!(
        tool_name,
        "fs.read"
            | "fs.list"
            | "fs.stat"
            | "fs.search"
            | "web.search"
            | "subagent.run"
            | "subagent.spawn"
            | "subagent.wait"
            | "subagent.followup"
            | "subagent.steer"
            | "subagent.cancel"
            | "subagent.list"
    )
}

pub(crate) fn is_local_filesystem_tool(tool_name: &str) -> bool {
    matches!(tool_name, "fs.read" | "fs.list" | "fs.stat" | "fs.search")
}

pub(crate) fn tools_for_mode(
    tools: impl IntoIterator<Item = ToolDescriptor>,
    mode: AgentMode,
) -> Vec<ToolDescriptor> {
    tools
        .into_iter()
        .filter(|tool| mode.allows_tool(&tool.name))
        .collect()
}
