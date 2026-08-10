---
name: github
description: Use Git and the GitHub CLI safely for repository orientation, dirty-worktree-aware changes, branches, commits, pushes, issues, pull requests, reviews, Actions CI, releases, and GitHub API queries. Use for requests involving git, GitHub, `gh`, the current branch or PR, issue work, CI failures, publishing changes, or release verification.
---

# Git and GitHub

Use local `git` as the source of truth for checkout and worktree state. Use the native `gh.read` tool, which invokes GitHub CLI read-only, for GitHub-hosted state. In Work mode, use `cmd.exec` with `gh` only for capabilities or authorized mutations that `gh.read` intentionally rejects. Keep local and GitHub contexts aligned before acting.

## When to Use

- Inspecting a Git repository, branch, remote, commit, tag, diff, or worktree
- Creating or switching branches, staging files, committing, fetching, rebasing, or pushing
- Reading, creating, updating, or closing GitHub issues and pull requests
- Reviewing PR comments, requested changes, checks, merge readiness, or branch protection
- Diagnosing GitHub Actions runs, jobs, annotations, or failed logs
- Creating, editing, uploading, or verifying GitHub releases and assets
- Querying GitHub REST or GraphQL APIs with `gh api`

## Workflow

1. Read repository instructions such as `AGENTS.md`, contributor docs, PR templates, issue templates, and release policy before deciding on a workflow.
2. Resolve local context with `git rev-parse --show-toplevel`, `git status --short --branch`, `git branch --show-current`, and `git remote -v`. Inspect `git diff`, `git diff --cached`, untracked files, and recent history before modifying or staging anything.
3. Resolve GitHub context through `gh.read` using `args: ["auth", "status", "--active"]` and `args: ["repo", "view", "--json", "nameWithOwner,defaultBranchRef,url"]`. Never show tokens. Pass `--repo OWNER/REPO` when context is ambiguous. Use `gh.read`, not web search, for identifiable GitHub state. Fall back only when GitHub CLI is unavailable or unauthenticated, or for explicitly requested broader web research; state why.
4. Classify the task as read-only orientation, local change management, issue work, review follow-up, CI diagnosis, publish/PR work, merge work, or release work. Begin with read-only commands and perform external writes only when the user requested them.
5. Gather the narrow source of truth. Read the issue body and comments before fixing it; read PR metadata, diff, reviews, and checks before review work; use `gh api graphql` when unresolved review-thread state is not exposed by `gh pr view`; read failed Actions logs before changing CI; read release workflows, tags, and recent release structure before publishing.
6. Make the smallest coherent change. Preserve unrelated work, validate locally, and stage explicit paths with `git add -- <paths>`. Review `git diff --cached --check`, `git diff --cached`, and `git status --short` before committing.
7. Before publishing, fetch the intended remote, compare the exact base with `git log <remote>/<base>..HEAD` and `git diff <remote>/<base>...HEAD`, then push the explicit branch. Create the PR with an explicit base/head and a repository template when one exists.
8. Verify every mutation by reading it back: inspect the commit and remote ref after pushing, the issue or PR after editing, checks after reruns, merge state after merging, and tag, assets, URLs, and checksums after releasing.

## Core Principles

- Treat every pre-existing modification and untracked file as user-owned. Do not discard, overwrite, hide, stage, or commit it unless it is explicitly in scope.
- Never run `git reset --hard`, `git clean`, destructive `git restore` or `git checkout --`, broad recursive deletion, or an unconditional force push unless the user clearly requests that exact destructive outcome and the targets were verified first.
- Do not use `git pull` merely to inspect remote state; use `git fetch` and compare refs. Do not rewrite published history by default. If force-pushing is explicitly authorized, prefer `--force-with-lease` and verify the expected remote ref.
- Separate authorization boundaries. A request to inspect, diagnose, review, or explain does not authorize commits, pushes, comments, issue edits, PR creation, merging, workflow reruns, releases, or other external mutations.
- Use explicit targets: pathspecs for staging, remote and branch for pushes, `--repo OWNER/REPO` for ambiguous GitHub context, PR/issue numbers for mutations, and tag names for releases.
- Prefer stable machine-readable output for automation: `git status --porcelain=v1 --branch`, `gh ... --json <fields>`, `--jq`, and `gh api --paginate`. Do not parse colorized human output when structured output exists.
- Prefer `gh.read` over `web.search` for GitHub state. A GitHub URL is a target for GitHub CLI, not a reason to search the web.
- Inspect before acting. A branch name does not prove the intended base, `origin` does not always prove the canonical GitHub repository, and a green workflow summary does not replace checking required jobs or published artifacts.
- Keep credentials private. Never print tokens, credential-helper contents, authenticated URLs, private keys, or secret values. Redact sensitive command output before reporting it.
- Follow repository conventions for branch prefixes, commit messages, PR templates, required checks, merge strategy, release tags, and generated artifacts. Do not invent policy when the repository defines it.
- Report partial completion honestly. A local commit is not a push, a push is not a PR, a green PR is not a merge, and a created tag is not a verified release.

## Useful Defaults

Use this read-only orientation set when the repository context is unclear:

```text
git rev-parse --show-toplevel
git status --short --branch
git remote -v
git branch --show-current
git log -8 --oneline --decorate
git diff --stat
git diff
git diff --cached
gh auth status --active
gh repo view --json nameWithOwner,defaultBranchRef,url
gh pr status
```

Use focused GitHub inspection commands and request only needed fields:

```text
gh issue view <number> --comments --repo OWNER/REPO
gh issue list --state all --search "terms" --repo OWNER/REPO
gh pr view <number-or-branch> --comments --json number,title,state,baseRefName,headRefName,url,reviewDecision,statusCheckRollup --repo OWNER/REPO
gh pr diff <number> --repo OWNER/REPO
gh pr checks <number> --required --repo OWNER/REPO
gh run view <run-id> --log-failed --repo OWNER/REPO
gh release view <tag> --json tagName,isDraft,isPrerelease,url,assets --repo OWNER/REPO
gh api --paginate <endpoint>
```

Pass `gh.read` arguments without the leading `gh`: `{"args":["pr","view","42","--json","title,files,reviews,statusCheckRollup","--repo","OWNER/REPO"]}`. It works in both modes and permits GET-only REST API queries. Mutations, GraphQL, browser flags, and token output are rejected.

For write operations, prefer non-interactive explicit commands and repository templates. Use `--body-file` for substantial issue, PR, review, or release text. Note that `gh pr create --dry-run` may still push changes; do not treat it as read-only. After a write, query the exact object with `--json` or `gh api` and confirm the intended state.

## Output Shape

- Lead with the actual result or current blocker.
- State the resolved repository, branch, base, and dirty-worktree condition when they affect the task.
- List files intentionally staged or committed; distinguish pre-existing unrelated changes.
- Report validation and GitHub checks by exact command or job name and outcome.
- For external mutations, include the resulting commit, branch, issue, PR, run, tag, or release URL and the read-back verification.
- State what remains, especially required checks, reviews, permissions, conflicts, or user authorization.
