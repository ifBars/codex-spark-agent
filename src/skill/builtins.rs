#[derive(Debug, Clone, Copy)]
pub(super) struct BuiltInSkill {
    pub(super) name: &'static str,
    pub(super) raw: &'static str,
}

const GITHUB: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/skills/github/SKILL.md"
));

const SKILLS: &[BuiltInSkill] = &[BuiltInSkill {
    name: "github",
    raw: GITHUB,
}];

pub(super) fn all() -> &'static [BuiltInSkill] {
    SKILLS
}

pub(super) fn implicit_skill_names(text: &str) -> Vec<&'static str> {
    github_prompt(text)
        .then_some("github")
        .into_iter()
        .collect()
}

fn github_prompt(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    let trimmed = normalized.trim_start();
    if trimmed.starts_with("git ") || trimmed.starts_with("gh ") {
        return true;
    }

    const PHRASES: &[&str] = &[
        "github.com/",
        "git@github.com:",
        "github",
        "gh cli",
        "`gh ",
        " gh ",
        "pull request",
        "pull-request",
        "current pr",
        "this pr",
        "pr #",
        "open a pr",
        "create a pr",
        "merge a pr",
        "merge the pr",
        "git status",
        "git diff",
        "git commit",
        "git push",
        "git pull",
        "git fetch",
        "git branch",
        "git tag",
        "git worktree",
        "commit these changes",
        "commit the changes",
        "push this branch",
        "stage these changes",
        "staged changes",
        "dirty worktree",
    ];
    if PHRASES.iter().any(|phrase| normalized.contains(phrase)) {
        return true;
    }

    contains_word(&normalized, "pr")
        && ["review", "repo", "merge", "checks", "comments"]
            .iter()
            .any(|context| normalized.contains(context))
}

fn contains_word(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(start, matched)| {
        let before = text[..start].chars().next_back();
        let after = text[start + matched.len()..].chars().next();
        before.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
            && after.is_none_or(|ch| !ch.is_ascii_alphanumeric() && ch != '_')
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn github_prompts_activate_builtin_skill() {
        for prompt in [
            "Review PR 42 on one of my repos.",
            "Review https://github.com/owner/repo/pull/42.",
            "Check the current pull request comments.",
            "git status and explain the dirty worktree",
        ] {
            assert_eq!(implicit_skill_names(prompt), vec!["github"], "{prompt}");
        }
    }

    #[test]
    fn ordinary_code_review_does_not_activate_github_skill() {
        assert!(implicit_skill_names("Review this parser function for edge cases.").is_empty());
    }
}
