# S1API merged-PR replay benchmark

`scripts/s1api_pr_replay.ps1` turns merged S1API pull requests into local replay tasks without copying the merged production implementation into the agent workspace.

The current corpus contains 15 user-authored, merged `ifBars/S1API` PRs: #158, #156, #147, #146, #145, #144, #143, #140, #138, #137, #136, #133, #132, #130, and #129. They cover NPC lifecycle and presentation, suppliers, product APIs and visuals, item/shop UI, and save/reload behavior.

For a PR, the script checks out the merge commit's first parent into an ignored worktree and installs only tests introduced by the merge. When the user-owned S1API checkout has ignored `local.build.props`, it copies that configuration into the disposable worktree so the normal MonoMelon/Il2CppMelon command graph remains available. It records the merged production and documentation paths as a hidden oracle. It also writes a compact `.spark-s1api-replay.md` task brief: focused test evidence, an oracle-derived working scope, and the exact runtime-specific validation graph. The brief names files, not the merged implementation, so both agents begin from the same bounded evidence and must still write the behavior themselves. The scorer then reports three distinct facts:

- changed-seam coverage: which merged production/docs paths the candidate touched;
- scope score: coverage minus unrelated changed paths;
- oracle-patch overlap: a hidden, normalized added-line precision/recall diagnostic that flags broad or structurally divergent implementations when a committed focused test is too weak;
- behavioral validation: run separately with the exact MonoMelon and Il2CppMelon command graph available to the local checkout.

This is deliberately not a patch-text-similarity benchmark. Equivalent code can receive full seam credit, but passing a weak focused test by changing unrelated lifecycle code is visible as scope drift. Oracle-patch overlap is a diagnostic, not a replacement for behavioral evidence: it catches cases where a thin API test passes but the implementation shape has drifted from the merged solution. A scenario is accepted only when behavioral validation and scope review both support it.

PR #146 has no committed focused test change, so its manifest declares `oracle-scope-only`. It remains useful for measuring task orientation and minimality, but it must not be counted as a behavioral success without separately captured Mono/IL2CPP evidence.

## Commands

List the corpus:

```powershell
.\scripts\s1api_pr_replay.ps1 -List
```

Prepare a disposable PR #156 workspace from the parent commit:

```powershell
.\scripts\s1api_pr_replay.ps1 -Prepare -Pr 156
```

After an agent run, score its changed seams against the merged oracle:

```powershell
.\scripts\s1api_pr_replay.ps1 -Score -Pr 156
```

Compare completed Spark and Codex workspaces without exposing the oracle to either runner:

```powershell
.\scripts\s1api_pr_replay.ps1 -Compare -Pr 156 `
  -SparkWorkspace <spark-worktree> `
  -CodexWorkspace <codex-worktree> `
  -OutputPath .spark-profile\s1api-pr-replay\pr156-comparison.json
```

`-Compare` deliberately reports its verdict as an oracle-patch diagnostic. It does not promote a focused-contract pass to behavioral acceptance; validate each runtime separately.

The replay worktrees are local `.spark-profile/` evidence. Do not commit proprietary game assemblies, generated wrappers, smoke mods, saves, logs, or game assets to either repository.

When running Spark in a prepared worktree, load the copied workflow guidance:

```powershell
cargo run --bin spark -- chat --cwd <replay-worktree> --skill schedule-one-modding "<issue-level task>"
```

For a controlled replay, instruct the agent to read `.spark-s1api-replay.md` before starting. Its validation section is authoritative: bare `dotnet test` does not establish a MonoMelon or Il2CppMelon result for this repository.
