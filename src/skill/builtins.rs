#[derive(Debug, Clone, Copy)]
pub(super) struct BuiltInSkill {
    pub(super) name: &'static str,
    pub(super) raw: &'static str,
}

const GITHUB: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/skills/github/SKILL.md"
));

const CODE_REVIEW: &str = include_str!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/skills/code-review/SKILL.md"
));

const SKILLS: &[BuiltInSkill] = &[
    BuiltInSkill {
        name: "code-review",
        raw: CODE_REVIEW,
    },
    BuiltInSkill {
        name: "github",
        raw: GITHUB,
    },
];

pub(super) fn all() -> &'static [BuiltInSkill] {
    SKILLS
}

pub(super) fn implicit_skill_names(text: &str) -> Vec<&'static str> {
    let code_review = code_review_prompt(text);
    let supplied_review_artifacts = code_review
        && [".patch", "review.json", "review.md"]
            .iter()
            .any(|marker| text.to_ascii_lowercase().contains(marker));
    let mut names = Vec::new();
    if code_review {
        names.push("code-review");
    }
    if github_prompt(text) && !supplied_review_artifacts {
        names.push("github");
    }
    names
}

fn code_review_prompt(text: &str) -> bool {
    let normalized = text.to_ascii_lowercase();
    const PHRASES: &[&str] = &[
        "code review",
        "review the pr",
        "review this pr",
        "review the pull request",
        "review this pull request",
        "review the diff",
        "review this diff",
        "review this change",
        "review this patch",
    ];
    if PHRASES.iter().any(|phrase| normalized.contains(phrase)) {
        return true;
    }

    normalized.contains("review")
        && [
            "function",
            "implementation",
            "changed file",
            "edge case",
            "regression",
            "defect",
        ]
        .iter()
        .any(|context| normalized.contains(context))
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
    fn ordinary_code_review_activates_focused_skill_without_github() {
        assert_eq!(
            implicit_skill_names("Review this parser function for edge cases."),
            vec!["code-review"]
        );
    }

    #[test]
    fn supplied_diff_review_avoids_unrelated_github_workflow() {
        assert_eq!(
            implicit_skill_names(
                "Review the PR from diff.patch and write findings to review.json."
            ),
            vec!["code-review"]
        );
    }
}
