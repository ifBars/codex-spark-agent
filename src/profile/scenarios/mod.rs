mod expectations;
mod fixtures;
mod prompts;
mod validation_commands;

pub(crate) use expectations::{
    profile_scenario_expected_skills, profile_scenario_expected_tool_calls,
    profile_scenario_expected_tool_groups, profile_scenario_optional_tool_calls,
};
pub(crate) use fixtures::prepare_profile_scenario;
#[cfg_attr(not(test), allow(unused_imports))]
pub(crate) use prompts::benchmark_task_prompt;
pub(crate) use prompts::{
    benchmark_profile_prompts, codex_cli_benchmark_prompt, profile_scenario_prompts,
};
pub(crate) use validation_commands::profile_scenario_validation_command;

use anyhow::Result;

use crate::MAX_SCENARIO_REPEAT;

#[derive(Debug, Clone, Copy)]
pub(crate) struct ProfileScenarioValidationCommand {
    pub(crate) workdir: &'static str,
    pub(crate) program: &'static str,
    pub(crate) args: &'static [&'static str],
}

pub(crate) fn validate_scenario_repeat(repeat: usize) -> Result<()> {
    if repeat == 0 {
        anyhow::bail!("--repeat must be greater than 0");
    }
    if repeat > MAX_SCENARIO_REPEAT {
        anyhow::bail!("--repeat must be <= {MAX_SCENARIO_REPEAT}");
    }
    Ok(())
}
