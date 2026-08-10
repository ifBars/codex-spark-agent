use std::cmp::Reverse;
use std::collections::HashSet;

use serde_json::{Value, json};

use crate::agent::{AgentRunner, LocalFilesystemToolBudget};
use crate::tools::{
    ToolDescriptor, ToolResult, builtin_tools, is_local_filesystem_tool, tools_for_mode,
};

const DEFAULT_TOOL_SEARCH_LIMIT: usize = 3;
const MAX_TOOL_SEARCH_LIMIT: usize = 8;

impl AgentRunner {
    pub(super) fn reset_deferred_tool_surface(&mut self) {
        self.active_deferred_tools.clear();
    }

    pub(super) fn tools_for_current_loop(&self) -> Vec<ToolDescriptor> {
        let eligible = self.eligible_tools();
        if self.local_filesystem_only {
            if self
                .local_filesystem_tool_budget
                .is_some_and(LocalFilesystemToolBudget::exhausted)
            {
                return Vec::new();
            }
            return eligible
                .into_iter()
                .filter(|tool| is_local_filesystem_tool(&tool.name))
                .collect();
        }

        let mut core = Vec::new();
        let mut deferred = Vec::new();
        for tool in eligible {
            if is_core_tool(&tool.name) {
                core.push(tool);
            } else {
                deferred.push(tool);
            }
        }
        deferred.sort_by(|left, right| left.name.cmp(&right.name));

        if !deferred.is_empty() && self.delegated_write_ownership.is_none() {
            core.push(tool_search_descriptor());
            core.extend(
                deferred
                    .into_iter()
                    .filter(|tool| self.active_deferred_tools.contains(&tool.name)),
            );
        }
        core
    }

    pub(super) fn search_and_activate_tools(&mut self, args: Value) -> ToolResult {
        let query = args
            .get("query")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .trim();
        if query.is_empty() {
            return ToolResult {
                ok: false,
                data: json!({
                    "error_kind": "invalid_tool_search",
                    "hint": "Provide a concrete capability such as `public web search`, `GitHub pull request`, `browser QA`, or an exact MCP tool name."
                }),
                error: Some("tool.search requires a non-empty query".to_string()),
            };
        }

        let limit = args
            .get("limit")
            .and_then(Value::as_u64)
            .map(|value| value as usize)
            .unwrap_or(DEFAULT_TOOL_SEARCH_LIMIT)
            .clamp(1, MAX_TOOL_SEARCH_LIMIT);
        let terms = search_terms(query);
        let query_lower = query.to_ascii_lowercase();
        let mut ranked = self
            .eligible_tools()
            .into_iter()
            .filter(|tool| !is_core_tool(&tool.name))
            .filter_map(|tool| {
                let score = tool_relevance(&tool, &query_lower, &terms);
                (score > 0).then_some((score, tool))
            })
            .collect::<Vec<_>>();
        ranked.sort_by_key(|(score, tool)| (Reverse(*score), tool.name.clone()));

        let mut activated = Vec::new();
        let mut already_active = Vec::new();
        for (_, tool) in ranked.into_iter().take(limit) {
            let summary = json!({
                "name": tool.name,
                "description": compact_description(&tool.description),
            });
            if self.active_deferred_tools.insert(tool.name) {
                activated.push(summary);
            } else {
                already_active.push(summary);
            }
        }

        ToolResult {
            ok: true,
            data: json!({
                "query": query,
                "activated": activated,
                "already_active": already_active,
                "available_next_turn": true,
                "hint": if activated.is_empty() && already_active.is_empty() {
                    "No deferred capability matched. Refine the query or finish with the core workspace tools."
                } else {
                    "Call an activated tool by its exact name on the next turn. Search again only for a different missing capability."
                },
            }),
            error: None,
        }
    }

    fn eligible_tools(&self) -> Vec<ToolDescriptor> {
        let mut tools = tools_for_mode(builtin_tools(), self.mode)
            .into_iter()
            .filter(|tool| self.subagent_depth == 0 || !tool.name.starts_with("subagent."))
            .collect::<Vec<_>>();
        if self.mode == crate::tools::AgentMode::Work
            && let Some(registry) = &self.mcp_registry
        {
            tools.extend(registry.tools());
        }
        tools
    }
}

fn is_core_tool(name: &str) -> bool {
    name.starts_with("fs.") || name == "cmd.exec"
}

fn tool_search_descriptor() -> ToolDescriptor {
    ToolDescriptor {
        name: "tool.search".to_string(),
        description: "Find and activate a deferred specialist capability for the next turn. Core workspace file and command tools are already loaded; use this only when the task genuinely needs public web search, GitHub, browser QA, subagents, or an MCP integration. Search by capability or exact tool name. Do not search when the supplied local evidence is sufficient.".to_string(),
        input_schema: json!({
            "type": "object",
            "properties": {
                "query": {
                    "type": "string",
                    "description": "Concrete missing capability, for example `public web search`, `GitHub pull request`, `browser QA`, or an exact MCP tool name."
                },
                "limit": {
                    "type": "integer",
                    "minimum": 1,
                    "maximum": MAX_TOOL_SEARCH_LIMIT,
                    "description": "Maximum matching tools to activate; defaults to 3."
                }
            },
            "required": ["query"],
            "additionalProperties": false
        }),
        hosted_type: None,
        hosted_config: None,
    }
}

fn search_terms(query: &str) -> Vec<String> {
    query
        .split(|ch: char| !ch.is_ascii_alphanumeric() && ch != '_' && ch != '.')
        .map(str::trim)
        .filter(|term| term.len() > 1)
        .map(str::to_ascii_lowercase)
        .collect::<HashSet<_>>()
        .into_iter()
        .collect()
}

fn tool_relevance(tool: &ToolDescriptor, query: &str, terms: &[String]) -> usize {
    let name = tool.name.to_ascii_lowercase();
    let description = tool.description.to_ascii_lowercase();
    let mut score = 0usize;
    if name == query {
        score += 10_000;
    } else if name.contains(query) {
        score += 1_000;
    }
    for term in terms {
        if name == *term {
            score += 500;
        } else if name.contains(term) {
            score += 100;
        }
        if description.contains(term) {
            score += 10;
        }
    }
    score
}

fn compact_description(description: &str) -> String {
    const MAX_CHARS: usize = 240;
    let mut chars = description.chars();
    let compact = chars.by_ref().take(MAX_CHARS).collect::<String>();
    if chars.next().is_some() {
        format!("{}...", compact.trim_end())
    } else {
        compact
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::AgentMode;

    fn descriptor(name: &str, description: &str) -> ToolDescriptor {
        ToolDescriptor {
            name: name.to_string(),
            description: description.to_string(),
            input_schema: json!({"type": "object"}),
            hosted_type: None,
            hosted_config: None,
        }
    }

    #[test]
    fn public_web_query_prefers_hosted_search() {
        let web = descriptor(
            "web.search",
            "Search the public web for current information.",
        );
        let browser = descriptor("browser.run", "Run browser QA against a supplied URL.");
        let terms = search_terms("public web current facts");

        assert!(
            tool_relevance(&web, "public web current facts", &terms)
                > tool_relevance(&browser, "public web current facts", &terms)
        );
    }

    #[test]
    fn tool_search_is_harness_mediated_instead_of_policy_advertised() {
        let descriptor = tool_search_descriptor();

        assert_eq!(descriptor.name, "tool.search");
        assert!(tools_for_mode([descriptor.clone()], AgentMode::Ask).is_empty());
        assert_eq!(tools_for_mode([descriptor], AgentMode::Work).len(), 1);
    }
}
