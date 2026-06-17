use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};
use serde_json::json;
use tokio_util::sync::CancellationToken;

use crate::agent::AgentRunner;

const MAX_GOAL_OBJECTIVE_CHARS: usize = 4_000;
const MAX_GOAL_NOTE_CHARS: usize = 2_000;
const GOAL_MARKER_OPEN: &str = "<spark_goal";
const GOAL_MARKER_CLOSE: &str = "</spark_goal>";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum GoalStatus {
    Running,
    Paused,
    Complete,
    Blocked,
}

impl GoalStatus {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Paused => "paused",
            Self::Complete => "complete",
            Self::Blocked => "blocked",
        }
    }

    fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "continue" | "running" => Some(Self::Running),
            "paused" => Some(Self::Paused),
            "complete" | "completed" | "done" => Some(Self::Complete),
            "blocked" => Some(Self::Blocked),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GoalProgressEntry {
    pub(crate) checkpoint: usize,
    pub(crate) status: GoalStatus,
    pub(crate) note: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct GoalState {
    pub(crate) objective: String,
    pub(crate) status: GoalStatus,
    pub(crate) checkpoint: usize,
    #[serde(default)]
    pub(crate) progress_log: Vec<GoalProgressEntry>,
}

impl GoalState {
    pub(crate) fn new(objective: impl Into<String>) -> Self {
        Self {
            objective: objective.into(),
            status: GoalStatus::Running,
            checkpoint: 0,
            progress_log: Vec::new(),
        }
    }
}

pub(crate) struct GoalRunReport {
    pub(crate) checkpoints_run: usize,
    pub(crate) status: GoalStatus,
    pub(crate) summary: String,
}

impl AgentRunner {
    pub(crate) fn set_goal(&mut self, objective: &str) -> Result<()> {
        let objective = objective.trim();
        if objective.is_empty() {
            bail!("goal objective is required");
        }
        if objective.chars().count() > MAX_GOAL_OBJECTIVE_CHARS {
            bail!("goal objective must be at most {MAX_GOAL_OBJECTIVE_CHARS} characters");
        }
        self.goal = Some(GoalState::new(objective));
        self.record_goal_trace("set", None);
        Ok(())
    }

    pub(crate) fn goal(&self) -> Option<&GoalState> {
        self.goal.as_ref()
    }

    pub(crate) fn clear_goal(&mut self) {
        self.goal = None;
        self.record_goal_trace("clear", None);
    }

    pub(crate) fn pause_goal(&mut self) -> Result<()> {
        let Some(goal) = &mut self.goal else {
            bail!("no active goal");
        };
        goal.status = GoalStatus::Paused;
        self.record_goal_trace("pause", None);
        Ok(())
    }

    pub(crate) fn resume_goal(&mut self) -> Result<()> {
        let Some(goal) = &mut self.goal else {
            bail!("no active goal");
        };
        goal.status = GoalStatus::Running;
        self.record_goal_trace("resume", None);
        Ok(())
    }

    pub(crate) fn record_goal_progress(
        &mut self,
        status: GoalStatus,
        note: &str,
    ) -> Option<GoalProgressEntry> {
        let goal = self.goal.as_mut()?;
        goal.status = status;
        goal.checkpoint += 1;
        let entry = GoalProgressEntry {
            checkpoint: goal.checkpoint,
            status,
            note: truncate_chars(note.trim(), MAX_GOAL_NOTE_CHARS),
        };
        goal.progress_log.push(entry.clone());
        self.record_goal_trace("progress", Some(&entry));
        Some(entry)
    }

    pub(crate) fn record_goal_decision_from_assistant(
        &mut self,
        text: &str,
    ) -> Option<GoalProgressEntry> {
        let decision = parse_goal_marker(text)?;
        self.record_goal_progress(decision.status, &decision.note)
    }

    pub(crate) fn goal_status_line(&self) -> String {
        match &self.goal {
            Some(goal) => format!(
                "goal: {} checkpoint={} objective={}",
                goal.status.name(),
                goal.checkpoint,
                goal.objective
            ),
            None => "no active goal".to_string(),
        }
    }

    pub(crate) fn goal_continuation_prompt(&self) -> Option<String> {
        let goal = self.goal.as_ref()?;
        if goal.status != GoalStatus::Running {
            return None;
        }

        let recent = if goal.progress_log.is_empty() {
            "No checkpoints recorded yet.".to_string()
        } else {
            goal.progress_log
                .iter()
                .rev()
                .take(5)
                .rev()
                .map(|entry| {
                    format!(
                        "- checkpoint {} [{}]: {}",
                        entry.checkpoint,
                        entry.status.name(),
                        entry.note
                    )
                })
                .collect::<Vec<_>>()
                .join("\n")
        };

        Some(format!(
            "Continue the active Spark goal.\n\nObjective:\n{}\n\nRecent progress:\n{}\n\nWork one concrete checkpoint toward the objective. Preserve normal repository and tool discipline, validate when practical, and stop at a verifiable stopping condition for this checkpoint. If the whole objective is finished, mark it complete. If you cannot make meaningful progress without user input or an external state change, mark it blocked.\n\nEnd your response with exactly one marker on its own line:\n<spark_goal status=\"continue|complete|blocked\">short evidence-backed checkpoint note</spark_goal>",
            goal.objective, recent
        ))
    }

    pub(crate) async fn run_goal_checkpoints(
        &mut self,
        max_checkpoints: usize,
        cancellation: CancellationToken,
    ) -> Result<GoalRunReport> {
        if max_checkpoints == 0 {
            bail!("goal checkpoint count must be greater than zero");
        }
        let Some(goal) = self.goal.as_ref() else {
            bail!("no active goal");
        };
        if goal.status != GoalStatus::Running {
            return Ok(GoalRunReport {
                checkpoints_run: 0,
                status: goal.status,
                summary: self.goal_status_line(),
            });
        }

        let start_checkpoint = goal.checkpoint;
        for _ in 0..max_checkpoints {
            if cancellation.is_cancelled() {
                bail!("goal run cancelled");
            }
            let checkpoint_before = self
                .goal
                .as_ref()
                .map(|goal| goal.checkpoint)
                .unwrap_or_default();
            let Some(prompt) = self.goal_continuation_prompt() else {
                break;
            };
            self.run_with_cancel(&prompt, cancellation.child_token())
                .await?;
            if self
                .goal
                .as_ref()
                .is_some_and(|goal| goal.checkpoint == checkpoint_before)
            {
                self.record_goal_progress(
                    GoalStatus::Running,
                    "assistant response ended without a spark goal marker",
                );
            }
            if !matches!(
                self.goal.as_ref().map(|goal| goal.status),
                Some(GoalStatus::Running)
            ) {
                break;
            }
        }

        let goal = self.goal.as_ref().expect("goal exists after run");
        Ok(GoalRunReport {
            checkpoints_run: goal.checkpoint.saturating_sub(start_checkpoint),
            status: goal.status,
            summary: self.goal_status_line(),
        })
    }

    fn record_goal_trace(&mut self, event: &str, progress: Option<&GoalProgressEntry>) {
        let Some(trace) = &mut self.trace else {
            return;
        };
        let _ = trace.write(
            self.request_seq,
            "goal",
            &json!({
                "event": event,
                "goal": self.goal,
                "progress": progress,
            }),
        );
    }
}

struct GoalDecision {
    status: GoalStatus,
    note: String,
}

fn parse_goal_marker(text: &str) -> Option<GoalDecision> {
    let open_start = text.rfind(GOAL_MARKER_OPEN)?;
    let after_open_start = &text[open_start..];
    let open_end = after_open_start.find('>')?;
    let open_tag = &after_open_start[..=open_end];
    let status = parse_status_attribute(open_tag)?;
    let body_start = open_start + open_end + 1;
    let close_start = text[body_start..].find(GOAL_MARKER_CLOSE)? + body_start;
    Some(GoalDecision {
        status,
        note: text[body_start..close_start].trim().to_string(),
    })
}

fn parse_status_attribute(open_tag: &str) -> Option<GoalStatus> {
    let status_pos = open_tag.find("status=")?;
    let after = &open_tag[status_pos + "status=".len()..];
    let quote = after.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }
    let after_quote = &after[quote.len_utf8()..];
    let end = after_quote.find(quote)?;
    GoalStatus::parse(&after_quote[..end])
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    let mut chars = value.chars();
    let truncated = chars.by_ref().take(max_chars).collect::<String>();
    if chars.next().is_some() {
        format!("{truncated}...")
    } else {
        truncated
    }
}

#[cfg(test)]
mod tests {
    use super::{GoalStatus, parse_goal_marker};

    #[test]
    fn parses_goal_marker_from_trailing_response_text() {
        let marker =
            parse_goal_marker("done\n<spark_goal status='blocked'>needs credentials</spark_goal>")
                .expect("marker");

        assert_eq!(marker.status, GoalStatus::Blocked);
        assert_eq!(marker.note, "needs credentials");
    }
}
