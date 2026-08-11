use serde::{Deserialize, Serialize};

use super::ToolDescriptor;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct ToolAccessPolicy {
    pub(crate) workspace_writes: bool,
    pub(crate) command_execution: bool,
    pub(crate) github_cli: bool,
    pub(crate) hosted_web_search: bool,
    pub(crate) browser: bool,
    pub(crate) subagents: bool,
    pub(crate) mcp: bool,
}

impl ToolAccessPolicy {
    pub(crate) const fn unrestricted() -> Self {
        Self {
            workspace_writes: true,
            command_execution: true,
            github_cli: true,
            hosted_web_search: true,
            browser: true,
            subagents: true,
            mcp: true,
        }
    }

    pub(crate) fn allows(self, tool_name: &str) -> bool {
        match tool_name {
            "fs.read" | "fs.list" | "fs.stat" | "fs.search" => true,
            "fs.write" | "fs.replace" | "fs.edit" | "fs.rename" => self.workspace_writes,
            "cmd.exec" => self.command_execution,
            "gh.read" => self.github_cli,
            "web.search" => self.hosted_web_search,
            "browser.run" => self.browser,
            "tool.search" => {
                self.github_cli
                    || self.hosted_web_search
                    || self.browser
                    || self.subagents
                    || self.mcp
            }
            name if name.starts_with("subagent.") => self.subagents,
            name if name.starts_with("mcp__") => self.mcp,
            _ => false,
        }
    }
}

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
            | "gh.read"
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

#[cfg(test)]
mod tests {
    use super::ToolAccessPolicy;

    #[test]
    fn restricted_policy_keeps_workspace_reads_and_explicit_mcp_only() {
        let policy = ToolAccessPolicy {
            workspace_writes: false,
            command_execution: false,
            github_cli: false,
            hosted_web_search: false,
            browser: false,
            subagents: false,
            mcp: true,
        };

        assert!(policy.allows("fs.read"));
        assert!(policy.allows("mcp__diffuin_github__read_file"));
        assert!(policy.allows("tool.search"));
        assert!(!policy.allows("fs.write"));
        assert!(!policy.allows("cmd.exec"));
        assert!(!policy.allows("web.search"));
        assert!(!policy.allows("subagent.run"));
    }
}
